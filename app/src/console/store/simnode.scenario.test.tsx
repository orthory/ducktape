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

import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { WebSocket as WsWebSocket } from "ws";

import { remoteTransport } from "../../domain/transport";
import {
  simnodeBinary,
  spawnSimnode,
  type SimNode,
  type SimSpawnOptions,
} from "../../test/simnode-harness";
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

  const boot = async (options: SimSpawnOptions = {}) => {
    live = await spawnSimnode(options);
    const transport = remoteTransport(live.base);
    render(
      <DucktapeProvider transport={transport}>
        <Probe />
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
});
