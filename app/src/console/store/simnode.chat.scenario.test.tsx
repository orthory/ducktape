// Chat-module gate scenarios against the deterministic sim node: the consensus
// authorization the fleet's live-QA can only reach by luck — members-only
// posting, the archived-channel refusals, the owner-gated rename — plus the
// reaction roundtrip and the thread-panel resync trap (a rival-deleted message
// must not linger as a ghost). Every rejection here is the REAL chat module
// refusing over noded's exact wire. See sim-scenario.tsx for the harness.
//
// Media/voice paths are deliberately out of scope: the sim reports no node
// public key, so huddle-join's node-key requirement is unreachable here — only
// the consensus-op gates are exercised (huddle join's ARCHIVED refusal shares
// the same check_post_policy guard, already covered by the post/reaction case).
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { blocksText } from "../../domain/chat-client";
import { simnodeBinary } from "../../test/simnode-harness";
import { useSimScenario } from "../../test/sim-scenario";
import { opKey } from "./finalization";
import { selfAuthorBytes } from "./state";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.chat.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

const peerCreate = (id: string, name: string, policy = "open") => ({
  create_channel: { channel_id: id, name, post_policy: policy },
});

const peerPost = (
  channelId: string,
  messageId: string,
  text: string,
  thread: number | null = null,
) => ({
  post_message: {
    channel_id: channelId,
    message_id: messageId,
    blocks: [{ paragraph: [{ text, marks: [] }] }],
    thread,
    as_agent: null,
  },
});

describe.skipIf(!bin)("chat gate scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  const bytesEqual = (a: number[], b: number[]) =>
    a.length === b.length && a.every((x, i) => x === b[i]);

  const failedOpMatching = (pattern: RegExp) =>
    Object.values(state().ops).find(
      (op) => op.phase === "failed" && pattern.test(op.error ?? ""),
    );

  const openChannelSendable = async (name: string, policy = "open") => {
    act(() => actions().createChannel(name, policy as "open" | "members_only"));
    await waitFor(() => expect(state().activeChannel).not.toBeNull());
    return state().activeChannel!;
  };

  const sendAndSettle = async (text: string): Promise<number> => {
    act(() => actions().sendMessage(text));
    await waitFor(() =>
      expect(
        state().messages.some((m) => blocksText(m.head.blocks) === text),
      ).toBe(true),
    );
    return state().messages.find((m) => blocksText(m.head.blocks) === text)!.seq;
  };

  it(
    "members-only: the creator is auto-enrolled and can post; a non-member is refused",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });

      // (a) the creator is auto-added on create — CreateChannel seeds no members,
      // so the store's follow-up SetMembership is what stops the creator locking
      // itself out. The member set carries the creator's self bytes…
      const teamId = await openChannelSendable("Team", "members_only");
      const self = selfAuthorBytes(state().status, state().author);
      await waitFor(() =>
        expect(state().channelMembers.some((m) => bytesEqual(m, self))).toBe(true),
      );
      // …and the creator's own post lands, with no op ever failing.
      await sendAndSettle("hello team");
      expect(failedOpMatching(/.*/)).toBeUndefined();
      expect(teamId).toBeTruthy();

      // (b) a rival owns a members-only channel; the store's author is not a
      // member of it, so its post is refused by the module.
      await sim.peerBlock(
        "chat",
        peerCreate("rival-team", "Rival Team", "members_only"),
        "rival",
      );
      await waitFor(() =>
        expect(state().channels.some((c) => c.id === "rival-team")).toBe(true),
      );
      act(() => actions().selectChannel("rival-team"));
      await waitFor(() => expect(state().activeChannel).toBe("rival-team"));

      act(() => actions().sendMessage("intruding"));
      await waitFor(() =>
        expect(
          failedOpMatching(/members-only and the author is not a member/),
        ).toBeDefined(),
      );
    },
  );

  it(
    "an archived channel refuses posts and reactions; edits/deletes still pass",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });
      const channelId = await openChannelSendable("General");
      const seq = await sendAndSettle("before archive");

      // the creator owns the channel, so archiving lands.
      act(() => actions().setChannelArchived(channelId, true));
      await waitFor(() =>
        expect(state().channels.find((c) => c.id === channelId)?.archived).toBe(
          true,
        ),
      );

      // a post into the archived channel is refused (one guard turns away every
      // posting-class op).
      act(() => actions().sendMessage("after archive"));
      await waitFor(() =>
        expect(failedOpMatching(/is archived/)).toBeDefined(),
      );

      // a reaction is refused too — reactions route through the same guard.
      act(() => actions().toggleReaction(seq, "👍"));
      await waitFor(() =>
        expect(
          state().ops[opKey.reaction(channelId, seq, "👍")]?.phase,
        ).toBe("failed"),
      );
      expect(state().ops[opKey.reaction(channelId, seq, "👍")]?.error).toMatch(
        /is archived/,
      );

      // but a DELETE of one's own message still passes (redaction stays possible
      // in a closed channel — delete does not call the post-policy guard).
      act(() => actions().deleteMessage(seq));
      await waitFor(() =>
        expect(
          state().messages.find((m) => m.seq === seq)?.head.deleted,
        ).toBe(true),
      );
    },
  );

  it(
    "rename is owner-gated: a non-owner's rename is refused and rolled back",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });

      // a rival owns the channel (peer block author == owner).
      await sim.peerBlock("chat", peerCreate("ops", "Ops"), "rival");
      await waitFor(() =>
        expect(state().channels.some((c) => c.id === "ops")).toBe(true),
      );
      act(() => actions().selectChannel("ops"));

      // the store's author is not the owner — the module refuses the rename.
      act(() => actions().renameChannel("ops", "Renamed"));
      await waitFor(() =>
        expect(state().ops[opKey.channel("ops")]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.channel("ops")]?.error).toMatch(
        /only the owner may administer/,
      );
      // the optimistic rename rolled back to the rival's committed name.
      await waitFor(() =>
        expect(state().channels.find((c) => c.id === "ops")?.name).toBe("Ops"),
      );
    },
  );

  it(
    "toggleReaction adds then removes the reaction — a clean roundtrip",
    { timeout: 30_000 },
    async () => {
      const channelId = await boot({ auto: true }).then(() =>
        openChannelSendable("General"),
      );
      const seq = await sendAndSettle("react to me");

      // add
      act(() => actions().toggleReaction(seq, "🎉"));
      await waitFor(() =>
        expect(state().ops[opKey.reaction(channelId, seq, "🎉")]?.phase).toBe(
          "finalized",
        ),
      );
      await waitFor(() => {
        const r = state()
          .messages.find((m) => m.seq === seq)!
          .reactions.find((x) => x.emoji === "🎉");
        expect(r?.reactors.length).toBe(1);
      });

      // toggle the SAME emoji off → the reaction is gone
      act(() => actions().toggleReaction(seq, "🎉"));
      await waitFor(() =>
        expect(
          state()
            .messages.find((m) => m.seq === seq)!
            .reactions.find((x) => x.emoji === "🎉"),
        ).toBeUndefined(),
      );
    },
  );

  it(
    "the thread panel resyncs on a failed action — a rival-deleted reply leaves no ghost",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });

      // a rival builds the thread: a channel, a root, and a reply under it —
      // all rival-authored, so the rival can later delete its own reply.
      await sim.peerBlock("chat", peerCreate("general", "General"), "rival");
      await sim.peerBlock("chat", peerPost("general", "root", "root msg"), "rival");
      await waitFor(() =>
        expect(state().channels.some((c) => c.id === "general")).toBe(true),
      );
      act(() => actions().selectChannel("general"));
      await waitFor(() => expect(state().messages.length).toBe(1));
      const rootSeq = state().messages[0]!.seq;

      await sim.peerBlock(
        "chat",
        peerPost("general", "reply", "reply msg", rootSeq),
        "rival",
      );
      await waitFor(() => expect(state().messages.length).toBe(2));
      const replySeq = state().messages.find(
        (m) => blocksText(m.head.blocks) === "reply msg",
      )!.seq;

      // open the thread panel — it snapshots the live reply.
      act(() => actions().openThread(rootSeq));
      await waitFor(() =>
        expect(state().activeThread?.replies.length).toBe(1),
      );
      expect(
        blocksText(state().activeThread!.replies[0]!.head.blocks),
      ).toBe("reply msg");

      // the rival deletes its reply — the generic refresh tombstones it in the
      // flat list, but the thread panel keeps its own (now stale) snapshot: the
      // reply is a ghost, still shown live in the panel.
      await sim.peerBlock(
        "chat",
        { delete_message: { channel_id: "general", seq: replySeq } },
        "rival",
      );
      await waitFor(() =>
        expect(
          state().messages.find((m) => m.seq === replySeq)?.head.deleted,
        ).toBe(true),
      );
      expect(
        state().activeThread?.replies.some(
          (r) => r.seq === replySeq && !r.head.deleted,
        ),
      ).toBe(true);

      // acting on the ghost resyncs the panel (resyncOpenThread runs whether the
      // op lands or not): the deleted reply is no longer shown live.
      act(() => actions().toggleReaction(replySeq, "👍"));
      await waitFor(() =>
        expect(
          state().activeThread?.replies.some(
            (r) => r.seq === replySeq && !r.head.deleted,
          ),
        ).toBe(false),
      );
    },
  );
});
