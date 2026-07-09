// Near-app scenarios: the REAL DucktapeProvider over the REAL remoteTransport
// (real http + ws) against a spawned deterministic sim node (bin/simnode).
// The unit suites fake the transport; live-daemon.e2e drives the domain layer
// only. This suite is the missing middle — the provider's refresh loop,
// optimistic ledger, and module event-stream gating against actual wire traffic,
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
  create_channel: { channel_id: id, name, post_policy: "open" },
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

      act(() => capturedActions!.createChannel("General", "open"));

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
    "a fresh pending gates module-event refreshes; a stale one stops gating (the accepted race)",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      // park our write — it stays pending for the whole scenario.
      act(() => capturedActions!.createChannel("Mine", "open"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      // a concurrent writer's chat event lands. the ws chain tip is ungated, so
      // its advance proves the stream delivered — then the gate must have held:
      // no refresh, so the rival's committed channel stays invisible.
      await sim.peerBlock("chat", peerChannel("rival-1", "Rival 1"));
      await waitFor(() => expect(capturedState!.lastBlock).toBe(1));
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

      act(() => capturedActions!.createChannel("General", "open"));
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
      const rootOp = record.ops[0]!;
      expect(rootOp.target).toBe("chat");
      expect(rootOp.opHash).toMatch(/^[0-9a-f]{64}$/);

      // and it is a REAL content address: the blob lane serves the committed
      // payload bytes back through the same transport the app uses.
      const bytes = await transport.getBlob(rootOp.opHash);
      const payload = JSON.parse(new TextDecoder().decode(bytes)) as {
        create_channel: { channel_id: string };
      };
      expect(payload.create_channel.channel_id).toBe(
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
      // immediately, and with nothing pending the module-event refresh is
      // ungated, so committed truth reaches the app on its own.
      await sim.peerBlock("chat", peerChannel("general", "General"), "rival");
      await sim.peerBlock(
        "chat",
        {
          post_message: {
            channel_id: "general",
            message_id: "m-rival",
            blocks: [{ paragraph: [{ text: "rival wrote this", marks: [] }] }],
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
        { create_channel: { channel_id: "general", name: "General", post_policy: "open" } },
        "owner",
      );
      await submitStepped(
        "agent",
        {
          register_agent: {
            agent_id: "quackbot",
            display_name: "Quackbot",
            capability: "echo",
            prompt_hash: Array(32).fill(7),
            allowed_actions: ["chat.post"],
          },
        },
        "owner",
      );
      await submitStepped(
        "runs",
        { watch_channel: { channel_id: "general", policy: "mention" } },
        "owner",
      );
      await submitStepped(
        "chat",
        {
          post_message: {
            channel_id: "general",
            message_id: "m1",
            blocks: [
              {
                paragraph: [
                  { text: "hey ", marks: [] },
                  {
                    text: "@quackbot",
                    marks: [{ mention: { agent: { module: "runs", agent_id: "quackbot" } } }],
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

      // the follow-up's height reaches the app over the module event stream ONLY —
      // no submit receipt ever carries it, and no op record claims it.
      await waitFor(() => expect(capturedState!.lastBlock).toBe(5));
      await waitFor(() => expect(capturedState!.status?.height).toBe(5));
      expect(receipts.some((r) => r.height === 5)).toBe(false);
      expect(
        Object.values(capturedState!.ops).some((op) => op.height === 5),
      ).toBe(false);
    },
  );

  it(
    "a composer @mention auto-watches the channel and the agent's reply lands",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ echoOracle: true });

      // channel + agent registration land as peer blocks — committed
      // immediately and (nothing pending) refreshed straight into provider
      // state, so the roster feeds the composer's mention resolver.
      await sim.peerBlock("chat", peerChannel("general", "General"));
      await sim.peerBlock("agent", {
        register_agent: {
          agent_id: "quackbot",
          display_name: "Quackbot",
          capability: "echo",
          prompt_hash: Array(32).fill(7),
          allowed_actions: ["chat.post"],
        },
      });
      await waitFor(() =>
        expect(capturedState!.agents.some((a) => a.agent_id === "quackbot")).toBe(true),
      );
      act(() => capturedActions!.selectChannel("general"));
      await waitFor(() => expect(capturedState!.activeChannel).toBe("general"));
      expect(capturedState!.watches).toEqual([]);

      // the REAL composer send path: parseMessageInput with the resolver
      // marks the mention, the unwatched channel gets its "mention" watch
      // FIRST, and only after that ack does the post submit.
      act(() => capturedActions!.sendMessage("hey @quackbot can you handle this?"));

      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const watchCommit = await sim.step();
      expect(watchCommit.committed?.target).toBe("runs");

      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const postCommit = await sim.step();
      expect(postCommit.committed?.target).toBe("chat");

      await waitFor(() =>
        expect(capturedState!.watches).toEqual([
          { channel_id: "general", policy: "mention" },
        ]),
      );

      // the mention engaged the echo worker — its follow-up commits the
      // result into the dispatch mailbox as its own block…
      expect((await sim.state()).oracleQueued).toBe(1);
      const oracle = await sim.step();
      expect(oracle.committed?.kind).toBe("oracle");

      // …and the never-pop-stack tail means the reply posts when the NEXT
      // block flushes the mailbox. A second mention send in the now-watched
      // channel is that block — and it also proves the existing watch is
      // respected: only the post submits, no second watch op.
      act(() => capturedActions!.sendMessage("thanks @quackbot"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const second = await sim.step();
      expect(second.committed?.target).toBe("chat");

      // the agent-authored threaded reply reaches the app over the refresh.
      await waitFor(() =>
        expect(
          capturedState!.messages.some((m) => {
            const author = m.head.author;
            return (
              typeof author === "object" &&
              "agent" in author &&
              author.agent.agent_id === "quackbot"
            );
          }),
        ).toBe(true),
      );
      expect(capturedState!.watches).toEqual([
        { channel_id: "general", policy: "mention" },
      ]);
    },
  );

  it(
    "explorer renders live ring data and consumes the cross-link focus",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ persona: "networked" }, <ExplorerView />);

      act(() => capturedActions!.createChannel("General", "open"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() => expect(capturedState!.blocks.length).toBe(1));
      const record = capturedState!.blocks[0]!;
      const rootOp = record.ops[0]!;
      expect(rootOp.opHash).toMatch(/^[0-9a-f]{64}$/);

      // the finalization-mark hand-off against the REAL view: focus lands,
      // the detail opens on the block…
      act(() => capturedActions!.openExplorerAt(1));
      expect(capturedState!.screen).toBe("explorer");
      await waitFor(() => expect(screen.getByText("OP HASH")).toBeInTheDocument());

      // …the OP HASH digest line shows the record's full content address…
      expect(screen.getByText(rootOp.opHash)).toBeInTheDocument();

      // …and the focus was consumed, so re-entering won't replay the jump.
      await waitFor(() => expect(capturedState!.explorerFocus).toBeNull());
    },
  );
});
