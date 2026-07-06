// Near-app scenarios: the REAL DucktapeProvider over the REAL remoteTransport
// (real http + ws) against a spawned deterministic sim node (bin/simnode).
// The unit suites fake the transport; live-daemon.e2e drives the domain layer
// only. This suite is the missing middle — the provider's refresh loop,
// optimistic ledger, and block-stream gating against actual wire traffic,
// with block production under test control so every race is a script, not a
// timing accident.
//
// Determinism levers: sim holds submits until step(); peerBlock() is the
// concurrent writer; personas flip the wire between the local daemon and the
// networked validator shapes; Date (only Date — network and timers stay
// real) is faked to age pendings past OP_STALE_MS.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { WebSocket as WsWebSocket } from "ws";

import { blocksText } from "../../domain/chat-client";
import { remoteTransport } from "../../domain/transport";
import type { SubmitReceipt } from "../../domain/transport";
import {
  simnodeBinary,
  spawnSimnode,
  type SimNode,
  type SimSpawnOptions,
} from "../../test/simnode-harness";
import { ExplorerView } from "../views/explorer/ExplorerView";
import { DucktapeProvider } from "./DucktapeProvider";
import { OP_STALE_MS, opKey } from "./finalization";
import { useDucktape } from "./use-ducktape";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

// ── Probe: capture live store state/actions per render ──

let capturedState: ReturnType<typeof useDucktape>["state"] | null = null;
let capturedActions: ReturnType<typeof useDucktape>["actions"] | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedState = state;
  capturedActions = actions;
  return null;
}

const peerChannel = (id: string, name: string) => ({
  CreateChannel: { channel_id: id, name, post_policy: "Open" },
});

describe.skipIf(!bin)("provider scenarios against the sim node", () => {
  let live: SimNode | null = null;

  // vitest's jsdom env leaves undici's WebSocket as the global while Event is
  // jsdom's — undici's own dispatchEvent then brand-fails across realms
  // (uncaught "must be an instance of Event"). The `ws` client is
  // callback-based (no EventTarget dispatch), so it is realm-safe here, and
  // remoteTransport only assigns onmessage/onclose/onerror, which ws supports.
  beforeAll(() => {
    vi.stubGlobal("WebSocket", WsWebSocket as unknown as typeof WebSocket);
  });
  afterAll(() => {
    vi.unstubAllGlobals();
  });

  const boot = async (options: SimSpawnOptions = {}, ui: ReactNode = null) => {
    live = await spawnSimnode(options);
    const transport = remoteTransport(live.base);
    render(
      <DucktapeProvider transport={transport}>
        <Probe />
        {ui}
      </DucktapeProvider>,
    );
    await waitFor(() => expect(capturedState?.connected).toBe(true), {
      timeout: 15_000,
    });
    return { sim: live.sim, transport };
  };

  afterEach(async () => {
    cleanup();
    vi.useRealTimers();
    capturedState = null;
    capturedActions = null;
    await live?.stop();
    live = null;
  });

  it(
    "a held write renders optimistically and finalizes on step with the receipt facts",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      act(() => capturedActions!.createChannel("General", "Open"));

      // the optimistic projection landed before any block exists...
      await waitFor(() =>
        expect(
          capturedState!.channels.some((c) => c.name === "General"),
        ).toBe(true),
      );
      const channelId = capturedState!.channels.find(
        (c) => c.name === "General",
      )!.id;
      const key = opKey.channel(channelId);
      expect(capturedState!.ops[key]?.phase).toBe("pending");

      // ...and the node really is holding it, uncommitted.
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      await sim.step();

      // the released receipt finalizes the op with the inclusion facts
      // (local persona: the content address rides the receipt).
      await waitFor(() =>
        expect(capturedState!.ops[key]?.phase).toBe("finalized"),
      );
      const op = capturedState!.ops[key]!;
      expect(op.height).toBe(1);
      expect(op.opHash).toMatch(/^[0-9a-f]{64}$/);

      // the completion refresh converged the store on committed state.
      await waitFor(() => expect(capturedState!.status?.height).toBe(1));
      expect(
        capturedState!.channels.some((c) => c.id === channelId),
      ).toBe(true);
    },
  );

  it(
    "a fresh pending gates block-stream refreshes; a stale one stops gating (the accepted race)",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      // park our write — it stays pending for the whole scenario.
      act(() => capturedActions!.createChannel("Mine", "Open"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      // a concurrent writer's block lands. telemetry frames are ungated, so
      // their arrival proves the ws delivered — then the gate must have held:
      // no refresh, so the rival's committed channel stays invisible.
      await sim.peerBlock("chat", peerChannel("rival-1", "Rival 1"));
      await waitFor(() =>
        expect(capturedState!.telemetry.some((f) => f.height === 1)).toBe(true),
      );
      expect(capturedState!.channels.some((c) => c.id === "rival-1")).toBe(false);
      expect(capturedState!.channels.some((c) => c.name === "Mine")).toBe(true);

      // age the pending past OP_STALE_MS — Date only; network and timers
      // stay real — and land another rival block.
      vi.useFakeTimers({ toFake: ["Date"] });
      vi.setSystemTime(Date.now() + OP_STALE_MS + 1_000);
      await sim.peerBlock("chat", peerChannel("rival-2", "Rival 2"));

      // stale → the gate is open → the refresh pulls committed state, which
      // includes both rivals and CLOBBERS our unconfirmed optimistic row.
      // this is the documented stale-pending trade-off, pinned as behavior.
      await waitFor(() =>
        expect(capturedState!.channels.some((c) => c.id === "rival-2")).toBe(true),
      );
      expect(capturedState!.channels.some((c) => c.id === "rival-1")).toBe(true);
      expect(capturedState!.channels.some((c) => c.name === "Mine")).toBe(false);
      // the parked op itself is untouched — still pending, still held.
      expect((await sim.state()).held).toBe(1);
    },
  );

  it(
    "networked persona: height-only receipts, ring reaches the app, op hash dereferences",
    { timeout: 30_000 },
    async () => {
      const { sim, transport } = await boot({ persona: "networked" });

      act(() => capturedActions!.createChannel("General", "Open"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();

      // the validator wire: inclusion height, no content address on the receipt.
      await waitFor(() => {
        const finalized = Object.values(capturedState!.ops).find(
          (op) => op.phase === "finalized",
        );
        expect(finalized?.height).toBe(1);
        expect(finalized?.opHash).toBeUndefined();
      });

      // the explorer ring is where the content address lives instead —
      // pulled by the completion refresh into provider state.
      await waitFor(() => expect(capturedState!.blocks.length).toBe(1));
      const record = capturedState!.blocks[0];
      expect(record.height).toBe(1);
      expect(record.target).toBe("chat");
      expect(record.opHash).toMatch(/^[0-9a-f]{64}$/);

      // and it is a REAL content address: the blob lane serves the committed
      // payload bytes back through the same transport the app uses.
      const bytes = await transport.getBlob(record.opHash!);
      const payload = JSON.parse(new TextDecoder().decode(bytes)) as {
        CreateChannel: { channel_id: string };
      };
      expect(payload.CreateChannel.channel_id).toBe(
        capturedState!.channels[0]!.id,
      );

      // the finalization-mark cross-link hand-off works over live data.
      act(() => capturedActions!.openExplorerAt(1));
      expect(capturedState!.screen).toBe("explorer");
      expect(capturedState!.explorerFocus).toBe(1);
    },
  );

  it(
    "a genuinely rejected op fails its record with the module error and rolls back",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      // a rival authors the channel and a message — peer blocks commit
      // immediately, and with nothing pending the block-stream refresh is
      // ungated, so committed truth reaches the app on its own.
      await sim.peerBlock("chat", peerChannel("general", "General"), "rival");
      await sim.peerBlock(
        "chat",
        {
          PostMessage: {
            channel_id: "general",
            message_id: "m-rival",
            blocks: [{ Paragraph: [{ text: "rival wrote this", marks: [] }] }],
            thread: null,
            as_agent: null,
          },
        },
        "rival",
      );
      await waitFor(() =>
        expect(capturedState!.channels.some((c) => c.id === "general")).toBe(true),
      );
      act(() => capturedActions!.selectChannel("general"));
      await waitFor(() => expect(capturedState!.messages.length).toBe(1));
      const seq = capturedState!.messages[0]!.seq;

      // editing someone else's message: the optimistic projection applies
      // first — chat's authorship check only runs at commit.
      act(() => capturedActions!.editMessage(seq, "hijacked"));
      await waitFor(() =>
        expect(blocksText(capturedState!.messages[0]!.head.blocks)).toBe("hijacked"),
      );
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      // no synthetic rejection knob exists BY DESIGN — this is the real
      // module saying no. the step commits nothing.
      const report = await sim.step();
      expect(report.committed).toBeNull();

      const key = opKey.messageSeq("general", seq);
      await waitFor(() => expect(capturedState!.ops[key]?.phase).toBe("failed"));
      expect(capturedState!.ops[key]?.error).toMatch(/only the author may edit/);

      // the failure refresh IS the rollback: committed truth replaces the
      // optimistic edit...
      await waitFor(() =>
        expect(blocksText(capturedState!.messages[0]!.head.blocks)).toBe(
          "rival wrote this",
        ),
      );
      // ...and no block was minted — a rejected op never becomes a block in
      // the sim (Host::submit_at aborts pre-commit).
      expect((await sim.state()).height).toBe(2);
    },
  );

  it(
    "echo-oracle follow-ups queue behind the mention and drain one per step, over ws only",
    { timeout: 30_000 },
    async () => {
      const { sim, transport } = await boot({ echoOracle: true });

      // the daemon_e2e mention recipe, stepped through the held queue: each
      // submit parks, a step releases it and returns its receipt.
      const receipts: SubmitReceipt[] = [];
      const submitStepped = async (target: string, payload: unknown, origin: string) => {
        const pending = transport.submit(target, payload, origin);
        await waitFor(async () => expect((await sim.state()).held).toBe(1));
        await sim.step();
        receipts.push(await pending);
      };

      await submitStepped(
        "chat",
        { CreateChannel: { channel_id: "general", name: "General", post_policy: "Open" } },
        "owner",
      );
      await submitStepped(
        "agent",
        {
          RegisterAgent: {
            agent_id: "quackbot",
            display_name: "Quackbot",
            capability: "echo",
            prompt_hash: Array(32).fill(7),
            prompt_doc: null,
            allowed_actions: ["chat.post"],
          },
        },
        "owner",
      );
      await submitStepped(
        "runs",
        { WatchChannel: { channel_id: "general", policy: "Mention" } },
        "owner",
      );
      await submitStepped(
        "chat",
        {
          PostMessage: {
            channel_id: "general",
            message_id: "m1",
            blocks: [
              {
                Paragraph: [
                  { text: "hey ", marks: [] },
                  {
                    text: "@quackbot",
                    marks: [{ Mention: { Agent: { module: "runs", agent_id: "quackbot" } } }],
                  },
                  { text: " can you handle this?", marks: [] },
                ],
              },
            ],
            thread: null,
            as_agent: null,
          },
        },
        "eddy",
      );

      // the mention's commit enqueued the echo worker's follow-up — parked
      // as oracle work, not committed with the post.
      expect(receipts.map((r) => r.height)).toEqual([1, 2, 3, 4]);
      const parked = await sim.state();
      expect(parked.oracleQueued).toBe(1);
      expect(parked.height).toBe(4);

      // a step drains exactly one follow-up as its own block.
      const report = await sim.step();
      expect(report.committed?.kind).toBe("oracle");
      expect(report.committed?.height).toBe(5);

      // the follow-up's height reaches the app over the ws stream ONLY —
      // no submit receipt ever carries it, and no op record claims it.
      await waitFor(() =>
        expect(capturedState!.telemetry.some((f) => f.height === 5)).toBe(true),
      );
      await waitFor(() => expect(capturedState!.status?.height).toBe(5));
      expect(receipts.some((r) => r.height === 5)).toBe(false);
      expect(
        Object.values(capturedState!.ops).some((op) => op.height === 5),
      ).toBe(false);
    },
  );

  it(
    "explorer renders live ring data and consumes the cross-link focus",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ persona: "networked" }, <ExplorerView />);

      act(() => capturedActions!.createChannel("General", "Open"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() => expect(capturedState!.blocks.length).toBe(1));
      const record = capturedState!.blocks[0]!;
      expect(record.opHash).toMatch(/^[0-9a-f]{64}$/);

      // the finalization-mark hand-off against the REAL view: focus lands,
      // the detail opens on the block…
      act(() => capturedActions!.openExplorerAt(1));
      expect(capturedState!.screen).toBe("explorer");
      await waitFor(() => expect(screen.getByText("OP HASH")).toBeInTheDocument());

      // …the OP HASH digest line shows the record's full content address…
      expect(screen.getByText(record.opHash!)).toBeInTheDocument();

      // …and the focus was consumed, so re-entering won't replay the jump.
      await waitFor(() => expect(capturedState!.explorerFocus).toBeNull());
    },
  );
});
