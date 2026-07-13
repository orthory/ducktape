// Comment-thread scenarios (the pages module's comment plane, #490) against
// the deterministic sim node: the optimistic projections, the pull-only
// refresh contract, and the module's authorship law — every rejection here is
// the REAL module refusing, never a synthetic knob. See sim-scenario.tsx for
// the harness contract.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ThreadView } from "../../domain/pages-client";
import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";
import { opKey } from "../../console/store/finalization";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.comments.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

describe.skipIf(!bin)("comment scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  const threads = (): ThreadView[] =>
    state().pageThreads.flatMap((group) => group.threads);

  /** Auto-mode page bootstrap: create, open, committed root loaded. */
  const openFreshPage = async (title: string): Promise<string> => {
    act(() => actions().createPage(title));
    await waitFor(() => expect(state().activePage).not.toBeNull());
    const pageId = state().activePage!;
    await waitFor(() =>
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
    );
    return pageId;
  };

  it(
    "a held comment renders its thread optimistically and survives the committed reload",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage("Notes");
      await sim.setAuto(false);

      act(() => actions().addComment({ target: pageId, text: "first!" }));

      // the thread painted before any block exists…
      await waitFor(() => expect(threads()).toHaveLength(1));
      expect(threads()[0]!.comments[0]!.text).toBe("first!");
      const threadId = threads()[0]!.thread.id;
      expect(state().ops[opKey.commentThread(threadId)]?.phase).toBe("pending");

      // …and the node really is holding it, uncommitted.
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const report = await sim.step();
      expect(report.committed?.target).toBe("pages");

      await waitFor(() =>
        expect(state().ops[opKey.commentThread(threadId)]?.phase).toBe(
          "finalized",
        ),
      );
      // the settle reload replaced the projection with committed truth —
      // same thread, same text, no flicker to zero.
      await waitFor(() => {
        expect(threads()).toHaveLength(1);
        expect(threads()[0]!.thread.id).toBe(threadId);
        expect(threads()[0]!.comments[0]!.text).toBe("first!");
      });
    },
  );

  it(
    "a rival's comment does NOT ride the block refresh — only the panel's explicit reload lands it",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage("Notes");

      const peer = await sim.peerBlock(
        "pages",
        {
          add_comment: {
            thread_id: "t-rival",
            comment_id: "c-rival",
            target: pageId,
            text: "drive-by",
            mentions: [],
          },
        },
        "rival",
      );
      // the stream delivered the block, but refresh() never writes
      // pageThreads: comments are pull-only outside comment ops (this is the
      // documented seam — the panel reloads on open and after own ops).
      await waitFor(() => expect(state().lastBlock).toBe(peer.height));
      expect(threads()).toHaveLength(0);

      act(() => actions().loadPageThreads());
      await waitFor(() => expect(threads()).toHaveLength(1));
      expect(threads()[0]!.comments[0]!.text).toBe("drive-by");
    },
  );

  it(
    "editing a rival's comment is refused by the module and the reload rolls the projection back",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage("Notes");
      await sim.peerBlock(
        "pages",
        {
          add_comment: {
            thread_id: "t-rival",
            comment_id: "c-rival",
            target: pageId,
            text: "drive-by",
            mentions: [],
          },
        },
        "rival",
      );
      act(() => actions().loadPageThreads());
      await waitFor(() => expect(threads()).toHaveLength(1));

      await sim.setAuto(false);
      // the projection applies first — authorship is only checked at commit.
      act(() =>
        actions().editComment({ commentId: "c-rival", text: "hijacked" }),
      );
      await waitFor(() =>
        expect(threads()[0]!.comments[0]!.text).toBe("hijacked"),
      );

      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const report = await sim.step();
      expect(report.committed).toBeNull();

      await waitFor(() =>
        expect(state().ops[opKey.comment("c-rival")]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.comment("c-rival")]?.error).toMatch(
        /not the comment author/,
      );
      // comment ops reload threads UNCONDITIONALLY after settling — that
      // reload IS the rollback (the generic refresh never covers this slice).
      await waitFor(() =>
        expect(threads()[0]!.comments[0]!.text).toBe("drive-by"),
      );
    },
  );

  it(
    "deleting the last live comment removes the whole thread",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });
      const pageId = await openFreshPage("Notes");

      act(() => actions().addComment({ target: pageId, text: "obsolete" }));
      await waitFor(() => {
        expect(threads()).toHaveLength(1);
        const tid = threads()[0]!.thread.id;
        expect(state().ops[opKey.commentThread(tid)]?.phase).toBe("finalized");
      });
      const commentId = threads()[0]!.comments[0]!.id;

      act(() => actions().deleteComment(commentId));
      // the tombstone takes its thread with it (no live comment remains), and
      // committed truth agrees after the settle reload — no flash back.
      await waitFor(() => expect(threads()).toHaveLength(0));
      await waitFor(() =>
        expect(state().ops[opKey.comment(commentId)]?.phase).toBe("finalized"),
      );
      expect(threads()).toHaveLength(0);
    },
  );

  it(
    "resolving a thread stamps resolved_by; unresolving clears it",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });
      const pageId = await openFreshPage("Notes");

      act(() => actions().addComment({ target: pageId, text: "fix the title" }));
      await waitFor(() => expect(threads()).toHaveLength(1));
      const threadId = threads()[0]!.thread.id;
      const opRecord = () => state().ops[opKey.commentThread(threadId)];
      await waitFor(() => expect(opRecord()?.phase).toBe("finalized"));
      const addHeight = opRecord()!.height!;

      // the projection paints resolved/resolved_by synchronously — a height
      // advance on the (re-begun) op record is what proves the COMMIT, and
      // the reload that follows serves the module's own stamp.
      act(() => actions().resolveThread({ threadId, resolved: true }));
      await waitFor(() => {
        expect(opRecord()?.phase).toBe("finalized");
        expect(opRecord()!.height!).toBeGreaterThan(addHeight);
      });
      const resolveHeight = opRecord()!.height!;
      act(() => actions().loadPageThreads());
      await waitFor(() => {
        expect(threads()[0]!.thread.resolved).toBe(true);
        expect(threads()[0]!.thread.resolved_by).not.toBeNull();
      });

      act(() => actions().resolveThread({ threadId, resolved: false }));
      await waitFor(() => {
        expect(opRecord()?.phase).toBe("finalized");
        expect(opRecord()!.height!).toBeGreaterThan(resolveHeight);
      });
      act(() => actions().loadPageThreads());
      await waitFor(() => {
        expect(threads()[0]!.thread.resolved).toBe(false);
        expect(threads()[0]!.thread.resolved_by).toBeNull();
      });
    },
  );

  // ── D2: comment @mention → agent engagement (and the edit asymmetry) ──

  /** Register the echo agent (peer block, committed) and open a fresh page in
   *  NON-auto mode so the oracle follow-up can be observed parked. */
  const registerAgentAndOpenPage = async (
    sim: Awaited<ReturnType<typeof boot>>["sim"],
    agentId: string,
  ): Promise<string> => {
    await sim.peerBlock("agent", {
      register_agent: {
        agent_id: agentId,
        display_name: "Quackbot",
        capability: "echo",
        allowed_actions: ["pages.comment"],
      },
    });
    await waitFor(() =>
      expect(state().agents.some((a) => a.agent_id === agentId)).toBe(true),
    );
    act(() => actions().createPage("Notes"));
    await waitFor(async () => expect((await sim.state()).held).toBe(1));
    await sim.step();
    await waitFor(() => expect(state().activePage).not.toBeNull());
    const pageId = state().activePage!;
    await waitFor(() =>
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
    );
    return pageId;
  };

  it(
    "a comment @mention of an agent engages it — the mention rides the tagging plane",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ echoOracle: true });
      const pageId = await registerAgentAndOpenPage(sim, "quackbot");

      // addComment parses mentions with the live resolver, so "@quackbot"
      // resolves to the registered agent and rides AddComment.mentions.
      act(() =>
        actions().addComment({ target: pageId, text: "please review @quackbot" }),
      );
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const added = await sim.step();
      expect(added.committed?.target).toBe("pages");

      // the comment mention reached the tagging plane (module_impl emits a Tag
      // event for AddComment) and engaged the echo worker — unlike chat, no
      // channel watch is needed: an entity mention reaches the agent owner
      // directly. Its follow-up parked as oracle work.
      await waitFor(async () => expect((await sim.state()).oracleQueued).toBe(1));
      const oracle = await sim.step();
      expect(oracle.committed?.kind).toBe("oracle");
    },
  );

  it(
    "editing a comment to add an @mention engages the newly mentioned agent",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ echoOracle: true });
      const pageId = await registerAgentAndOpenPage(sim, "quackbot");

      // a mention-FREE comment first — nothing to engage.
      act(() => actions().addComment({ target: pageId, text: "initial thoughts" }));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() => expect(threads()).toHaveLength(1));
      const commentId = threads()[0]!.comments[0]!.id;
      expect((await sim.state()).oracleQueued).toBe(0);

      // The edit path diffs structured refs against the previous comment, so
      // this newly introduced mention rides EditComment.mentions.
      act(() =>
        actions().editComment({
          commentId,
          text: "actually @quackbot please look",
        }),
      );
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const edited = await sim.step();
      expect(edited.committed?.target).toBe("pages");
      await waitFor(() =>
        expect(threads()[0]!.comments[0]!.text).toBe(
          "actually @quackbot please look",
        ),
      );

      await waitFor(async () => expect((await sim.state()).oracleQueued).toBe(1));
      const oracle = await sim.step();
      expect(oracle.committed?.kind).toBe("oracle");
    },
  );
});
