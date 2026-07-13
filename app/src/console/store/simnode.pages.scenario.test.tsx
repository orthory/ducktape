// Pages-module scenarios against the deterministic sim node: the editor's
// anchor-chained wire discipline, the concurrent-writer races the fleet's
// live-QA can only hit by accident, and the module semantics (#457's
// data-loss traps) pinned over the REAL provider + transport. See
// sim-scenario.tsx for the harness contract.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { simnodeBinary } from "../../test/simnode-harness";
import { useSimScenario } from "../../test/sim-scenario";
import { opKey } from "./finalization";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.pages.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

describe.skipIf(!bin)("pages scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  /** Create a page and wait until it is open with its committed root loaded.
   *  Steps the held queue when the sim is not in auto mode. */
  const openFreshPage = async (
    sim: Awaited<ReturnType<typeof boot>>["sim"],
    title: string,
  ): Promise<string> => {
    act(() => actions().createPage(title));
    if (!(await sim.state()).auto) {
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
    }
    await waitFor(() => expect(state().activePage).not.toBeNull());
    const pageId = state().activePage!;
    // the committed tree loaded: the root block names itself.
    await waitFor(() =>
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
    );
    return pageId;
  };

  it(
    "anchor-chained inserts paint in one tick but ride the wire one at a time, committing in issue order",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();
      const pageId = await openFreshPage(sim, "Spec");

      // a paste: each block anchors `after` the one before it. Fired in one
      // tick — the wire MUST serialize them (transport.submit has no ordering
      // of its own; an early-arriving anchor is a rejected op).
      const [b1, b2, b3] = [
        crypto.randomUUID(),
        crypto.randomUUID(),
        crypto.randomUUID(),
      ];
      act(() => {
        void actions().insertPageBlock({
          blockId: b1,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "one",
        });
        void actions().insertPageBlock({
          blockId: b2,
          parent: pageId,
          after: b1,
          kind: "paragraph",
          text: "two",
        });
        void actions().insertPageBlock({
          blockId: b3,
          parent: pageId,
          after: b2,
          kind: "paragraph",
          text: "three",
        });
      });

      // all three painted optimistically, in document order, before any commit…
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([
        pageId,
        b1,
        b2,
        b3,
      ]);

      // …while the node holds exactly ONE parked op: the second and third wait
      // on the first's receipt (inPageOrder), so each step commits the next
      // insert in issue order.
      const heights: number[] = [];
      for (let i = 0; i < 3; i += 1) {
        await waitFor(async () => expect((await sim.state()).held).toBe(1));
        const report = await sim.step();
        expect(report.committed?.target).toBe("pages");
        heights.push(report.committed!.height);
      }
      expect(heights).toEqual([2, 3, 4]);

      await waitFor(() => {
        for (const id of [b1, b2, b3]) {
          expect(state().ops[opKey.pageBlock(id)]?.phase).toBe("finalized");
        }
      });
      expect(
        [b1, b2, b3].map((id) => state().ops[opKey.pageBlock(id)]?.height),
      ).toEqual([2, 3, 4]);

      // committed truth converged on exactly what was painted.
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.text)).toEqual([
          "Spec",
          "one",
          "two",
          "three",
        ]),
      );
    },
  );

  it(
    "a concurrent writer destroys the anchor: the parked insert is rejected, never invented",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage(sim, "Doc");

      const anchor = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId: anchor,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "anchor",
        });
      });
      await waitFor(() =>
        expect(state().ops[opKey.pageBlock(anchor)]?.phase).toBe("finalized"),
      );
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([
          pageId,
          anchor,
        ]),
      );

      // script the race: our next insert parks…
      await sim.setAuto(false);
      const late = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId: late,
          parent: pageId,
          after: anchor,
          kind: "paragraph",
          text: "after anchor",
        });
      });
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([
        pageId,
        anchor,
        late,
      ]);
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      // …and the rival removes its anchor, committed immediately. The stream
      // delivered (lastBlock advanced), but our fresh pending holds the pages
      // slices — the editor keeps its painted view rather than unmounting the
      // block mid-edit.
      const peer = await sim.peerBlock(
        "pages",
        { remove_block: { block_id: anchor } },
        "rival",
      );
      await waitFor(() => expect(state().lastBlock).toBe(peer.height));
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([
        pageId,
        anchor,
        late,
      ]);

      // release our insert: the REAL module says no (no synthetic-rejection
      // knob exists), and no block is minted for it.
      const report = await sim.step();
      expect(report.committed).toBeNull();
      await waitFor(() =>
        expect(state().ops[opKey.pageBlock(late)]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.pageBlock(late)]?.error).toMatch(
        /after-anchor not found/,
      );

      // the failure refresh is the rollback: committed truth has neither the
      // rival-removed anchor nor our never-committed insert.
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
      );
    },
  );

  it(
    "removeBlock takes the whole subtree — optimistically and on commit",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage(sim, "Tree");

      const trunk = crypto.randomUUID();
      const branch = crypto.randomUUID();
      const leaf = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId: trunk,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "trunk",
        });
        void actions().insertPageBlock({
          blockId: branch,
          parent: trunk,
          after: null,
          kind: "bulleted",
          text: "branch",
        });
        void actions().insertPageBlock({
          blockId: leaf,
          parent: branch,
          after: null,
          kind: "bulleted",
          text: "leaf",
        });
      });
      await waitFor(() => {
        for (const id of [trunk, branch, leaf]) {
          expect(state().ops[opKey.pageBlock(id)]?.phase).toBe("finalized");
        }
      });
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([
          pageId,
          trunk,
          branch,
          leaf,
        ]),
      );

      await sim.setAuto(false);
      act(() => {
        void actions().removePageBlock(trunk);
      });
      // one op, three blocks gone from the painted tree in one tick.
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]);

      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const report = await sim.step();
      expect(report.committed?.target).toBe("pages");
      await waitFor(() =>
        expect(state().ops[opKey.pageBlock(trunk)]?.phase).toBe("finalized"),
      );
      // committed truth agrees: the SUBTREE went with the block.
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
      );
    },
  );

  it(
    "deleting a parent page promotes its child page and drops the dead tab",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const parentId = await openFreshPage(sim, "Parent");

      act(() => actions().createChildPage(parentId));
      await waitFor(() => {
        expect(state().activePage).not.toBeNull();
        expect(state().activePage).not.toBe(parentId);
      });
      const childId = state().activePage!;
      await waitFor(() =>
        expect(state().pages.find((p) => p.id === childId)?.parent).toBe(
          parentId,
        ),
      );
      expect(state().openTabs).toEqual([parentId, childId]);

      act(() => actions().deletePage(parentId));
      // the tab closes optimistically…
      expect(state().openTabs).toEqual([childId]);
      // …and the committed enumeration promotes the child to top level
      // (child PAGES survive a parent's delete; only its blocks go).
      await waitFor(() => {
        expect(state().pages.map((p) => p.id)).toEqual([childId]);
        expect(state().pages[0]!.parent).toBeNull();
      });
      expect(state().activePage).toBe(childId);
    },
  );

  it(
    "renaming the page root renames the rail enumeration (title == root text)",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });
      const pageId = await openFreshPage(sim, "Draft");

      act(() => {
        void actions().updatePageBlockText({ blockId: pageId, text: "Final" });
      });
      // the rail renames in the same tick as the editor…
      expect(state().pages.find((p) => p.id === pageId)?.title).toBe("Final");

      await waitFor(() =>
        expect(state().ops[opKey.pageBlock(pageId)]?.phase).toBe("finalized"),
      );
      // …and committed truth confirms both sides of the title == root-text law.
      await waitFor(() => {
        expect(state().pages.find((p) => p.id === pageId)?.title).toBe("Final");
        expect(state().activePageBlocks[0]?.text).toBe("Final");
      });
    },
  );
});
