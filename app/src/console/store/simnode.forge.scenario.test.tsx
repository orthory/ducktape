// Forge-tracker scenarios (issues / PRs over the `forge` module) against the
// deterministic sim node: the imperative loader contract (items never ride
// the per-block refresh), the module's authorship law, and the born-branch
// invariant for PRs — every rejection the REAL module's. See sim-scenario.tsx
// for the harness contract.
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
    "[simnode.forge.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

const REPO = "demo";

describe.skipIf(!bin)("forge tracker scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  it(
    "an issue's open → edit → close lifecycle, stepped through the held queue",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      act(() => {
        void actions().openForgeIssue({
          repo: REPO,
          title: "Bug: sim",
          body: "repro inside",
        });
      });
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const opened = await sim.step();
      expect(opened.committed?.target).toBe("forge");

      // landing reloads the item list, stamped with the repo.
      await waitFor(() => expect(state().forgeItems).toHaveLength(1));
      expect(state().forgeRepo).toBe(REPO);
      expect(state().forgeItems[0]).toMatchObject({
        number: 1,
        kind: "issue",
        title: "Bug: sim",
        state: "open",
      });

      act(() => {
        void actions().editForgeItem({
          repo: REPO,
          number: 1,
          title: "Bug: sim node",
          body: null,
        });
      });
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() =>
        expect(state().forgeItems[0]?.title).toBe("Bug: sim node"),
      );

      act(() => {
        void actions().setForgeItemState({ repo: REPO, number: 1, open: false });
      });
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      await sim.step();
      await waitFor(() => expect(state().forgeItems[0]?.state).toBe("closed"));
      expect(state().ops[opKey.forgeItem(REPO, 1)]?.phase).toBe("finalized");
    },
  );

  it(
    "a rival's item is edit-locked: the module refuses a non-author edit",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      // the rival authors issue #1 — a peer block commits immediately.
      await sim.peerBlock(
        "forge",
        { open_issue: { repo: REPO, title: "rival report", body: "" } },
        "rival",
      );
      act(() => {
        void actions().loadForgeItems(REPO);
      });
      await waitFor(() => expect(state().forgeItems).toHaveLength(1));
      expect(state().forgeItems[0]?.title).toBe("rival report");

      act(() => {
        void actions().editForgeItem({
          repo: REPO,
          number: 1,
          title: "hijacked",
          body: null,
        });
      });
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      // authorship comes from the submit origin — the REAL module says no,
      // and the rejected op never becomes a block.
      const report = await sim.step();
      expect(report.committed).toBeNull();

      await waitFor(() =>
        expect(state().ops[opKey.forgeItem(REPO, 1)]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.forgeItem(REPO, 1)]?.error).toMatch(
        /only the item author may edit/,
      );
      // items have no optimistic projection — the list still holds committed
      // truth (the reload is gated on landing, so nothing even refetched).
      expect(state().forgeItems[0]?.title).toBe("rival report");
    },
  );

  it(
    "a PR from a branch nobody pushed is refused — branches must be born in committed state",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot();

      act(() => {
        void actions().openForgePr({
          repo: REPO,
          title: "phantom pr",
          body: "",
          sourceBranch: "feature",
          targetBranch: "",
        });
      });
      await waitFor(async () => expect((await sim.state()).held).toBe(1));
      const report = await sim.step();
      expect(report.committed).toBeNull();

      await waitFor(() =>
        expect(state().ops[opKey.forgeItemOpen(REPO)]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.forgeItemOpen(REPO)]?.error).toMatch(/no repo/);
    },
  );
});
