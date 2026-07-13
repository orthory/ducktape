// Scoped-hydration scenarios against the deterministic sim node: the console's
// refreshScoped path refetches ONLY the slice groups whose module roots moved
// (changedModules → scopeFor). The unit suite (hydration.test.ts) pins the map;
// this proves the user-observable end of it over the real wire — a pages block
// keeps the pages slice fresh, and a block for a module with NO console slice
// (jobs) still advances the height without disturbing a held projection. See
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
    "[simnode.hydration.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

describe.skipIf(!bin)("scoped-hydration scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  it(
    "a pages block refreshes the pages slice; a jobs block advances the height but scopes nothing",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      // committed pages state: one open page.
      act(() => actions().createPage("Doc"));
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() => expect(state().activePage).not.toBeNull());
      const pageId = state().activePage!;
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
      );

      // (A) nothing pending → a rival PAGES block changes the pages root, so the
      // scoped refresh refetches the pages group: the rival page is not stale.
      const pagesBefore = state().pages.length;
      await sim.peerBlock(
        "pages",
        { create_page: { page_id: "rival-page", title: "Rival", parent: null } },
        "rival",
      );
      await waitFor(() =>
        expect(state().pages.some((p) => p.id === "rival-page")).toBe(true),
      );
      expect(state().pages.length).toBe(pagesBefore + 1);

      // (B) park a page-block insert so a live optimistic projection is on
      // screen, then land a JOBS block — a module in NO slice group.
      const held = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId: held,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "held",
        });
      });
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId, held]);
      await waitFor(async () => expect((await sim.state()).held).toBe(1));

      const heightBefore = state().status?.height ?? 0;
      const jobs = await sim.peerBlock(
        "jobs",
        { submit: { job_id: "j1", kind: "build", spec: "{}" } },
        "poster",
      );

      // the ws block advanced the chain tip and the height — a no-console module
      // still moves the height, so the app never wedges waiting on a slice.
      await waitFor(() => expect(state().lastBlock).toBe(jobs.height));
      await waitFor(() => expect(state().status?.height).toBe(jobs.height));
      expect(state().status!.height).toBeGreaterThan(heightBefore);

      // jobs maps to no slice group → scopeFor is empty → no group was refetched:
      // the held optimistic projection is untouched, and the rival page loaded in
      // (A) is still present (nothing clobbered it).
      expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId, held]);
      expect(state().ops[opKey.pageBlock(held)]?.phase).toBe("pending");
      expect(state().pages.some((p) => p.id === "rival-page")).toBe(true);
      expect((await sim.state()).held).toBe(1);
    },
  );
});
