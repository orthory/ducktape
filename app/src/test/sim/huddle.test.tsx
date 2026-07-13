// Huddle MEMBERSHIP consensus scenarios against the deterministic sim node —
// the roster ops (join/leave/sweep), NOT media/voice. The store gates
// joinHuddle on status.publicKey (a node with no voice identity can't join) and
// carries that node key into the join op; leave/sweep prune the roster. The sim
// fabricates the node key via --node-key so these ops have a real key to name.
//
// Two boundaries the provider-only lane forces, and how each is handled:
//   1. joinHuddle also needs a resolved node URL (for the media socket), which
//      an injected-transport boot leaves null. `connectRemote` dials the SAME
//      sim with the url set, unlocking the real join/leave actions.
//   2. joinHuddle opens a media session (getUserMedia); jsdom has none, and a
//      failed acquire would fire a consensus LEAVE. We hang getUserMedia so the
//      session stays 'connecting' forever — isolating the consensus roster ops
//      from media, exactly the scope here (NO voice).
//
// The sim keeps the client submit origin verbatim (a real node stamps its own
// key), so a committed huddle member's `user` is the submit author while its
// `node` is the key the join carried — the self-match keys on `node`, as the
// app's buildParticipants does.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { keyHex } from "../../domain/chat-client";
import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";
import { opKey } from "../../console/store/finalization";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.huddle.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

/** K — the fabricated node key --node-key seeds; status.publicKey serves it. */
const nodeKeyBytes = Array.from({ length: 32 }, () => 0x33);
const nodeKeyHex = keyHex(nodeKeyBytes);

const hasNode = (huddle: { node: number[] }[] | undefined, hex: string): boolean =>
  (huddle ?? []).some((m) => keyHex(m.node) === hex);

describe.skipIf(!bin)("huddle membership scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  // jsdom has no mediaDevices; hang getUserMedia so joinHuddle's media session
  // never fails (a failed acquire would fire a consensus leave). Consensus-only.
  beforeEach(() => {
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getUserMedia: () => new Promise(() => {}) },
    });
  });

  const openChannel = async (name: string): Promise<string> => {
    act(() => actions().createChannel(name, "open"));
    await waitFor(() => expect(state().activeChannel).not.toBeNull());
    return state().activeChannel!;
  };

  it(
    "joinHuddle is gated on status.publicKey: no voice identity, no join op",
    { timeout: 30_000 },
    async () => {
      // boot WITHOUT --node-key: the sim reports an empty publicKey. Dial the
      // node url in (connectRemote) so publicKey is the ONLY thing missing —
      // isolating the gate the task names.
      const { base } = await boot({ auto: true });
      act(() => actions().connectRemote(base));
      await waitFor(() => expect(state().connected).toBe(true));
      expect(state().status?.publicKey).toBe("");

      const channelId = await openChannel("General");
      act(() => actions().joinHuddle(channelId));

      // no publicKey → the action bailed: no session, and no huddle op minted.
      expect(state().voice.channelId).toBeNull();
      expect(state().ops[opKey.huddle(channelId)]).toBeUndefined();
    },
  );

  it(
    "join adds this node to the committed roster; leave removes it",
    { timeout: 30_000 },
    async () => {
      const { base } = await boot({ nodeKey: nodeKeyHex, auto: true });
      // set the node url so joinHuddle's media leg can dial (same sim).
      act(() => actions().connectRemote(base));
      await waitFor(() => expect(state().status?.publicKey).toBe(nodeKeyHex));

      const channelId = await openChannel("Standup");
      act(() => actions().joinHuddle(channelId));

      // the join landed: the committed roster carries node K, and the op
      // finalized (media stays 'connecting' behind the hung getUserMedia).
      await waitFor(() =>
        expect(
          hasNode(state().channels.find((c) => c.id === channelId)?.huddle, nodeKeyHex),
        ).toBe(true),
      );
      await waitFor(() =>
        expect(state().ops[opKey.huddle(channelId)]?.phase).toBe("finalized"),
      );
      expect(state().voice.channelId).toBe(channelId);

      act(() => actions().leaveHuddle());
      // the leave op prunes us from committed truth.
      await waitFor(() =>
        expect(
          hasNode(state().channels.find((c) => c.id === channelId)?.huddle, nodeKeyHex),
        ).toBe(false),
      );
      expect(state().voice.channelId).toBeNull();
    },
  );

  it(
    "sweepHuddle evicts a stale rival while leaving this node in the roster",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ nodeKey: nodeKeyHex, auto: true });
      const channelId = await openChannel("War Room");

      // seed the roster over the wire: US (authored AS K via the hex: escape, so
      // user==node==K) and a RIVAL (authored "rival", its own mesh node).
      const rivalNode = Array.from({ length: 32 }, () => 0x44);
      const rivalNodeHex = keyHex(rivalNode);
      await sim.peerBlock(
        "chat",
        { join_huddle: { channel_id: channelId, node: nodeKeyBytes } },
        "hex:" + nodeKeyHex,
      );
      await sim.peerBlock(
        "chat",
        { join_huddle: { channel_id: channelId, node: rivalNode } },
        "rival",
      );
      await waitFor(() => {
        const huddle = state().channels.find((c) => c.id === channelId)?.huddle;
        expect(hasNode(huddle, nodeKeyHex)).toBe(true);
        expect(hasNode(huddle, rivalNodeHex)).toBe(true);
      });

      // the store sweeps the rival by its submitter identity ("rival" bytes).
      act(() =>
        actions().sweepHuddle(
          channelId,
          Array.from(new TextEncoder().encode("rival")),
        ),
      );
      await waitFor(() =>
        expect(state().ops[opKey.huddle(channelId)]?.phase).toBe("finalized"),
      );
      // committed truth: the rival is gone, this node remains.
      const huddle = state().channels.find((c) => c.id === channelId)?.huddle;
      expect(hasNode(huddle, rivalNodeHex)).toBe(false);
      expect(hasNode(huddle, nodeKeyHex)).toBe(true);
    },
  );
});
