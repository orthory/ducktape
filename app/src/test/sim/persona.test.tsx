// D4 — networked-persona pass. The `--persona networked` sim strips `opHash`
// from every /v1/submit receipt (a response-layer strip, see bin/simnode), so
// receipts are HEIGHT-ONLY — the ordered validator's shape before its
// convergence. The store's optimistic-projection lifecycle must still settle:
// `finalizeOp` keys settlement on the receipt's `height` (opHash is optional in
// the SubmitReceipt type by design), and committed truth lands via the
// completion refresh, not by matching a receipt opHash. One happy path per
// surface (pages + forge) proves the persona's receipt shape doesn't break it.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";
import { opKey } from "../../console/store/finalization";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.persona.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

const REPO = "demo";

describe.skipIf(!bin)("networked-persona settlement against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  it(
    "pages: an insert settles on a height-only receipt (no opHash) and committed truth converges",
    { timeout: 30_000 },
    async () => {
      await boot({ persona: "networked", auto: true });

      act(() => actions().createPage("Networked"));
      await waitFor(() => expect(state().activePage).not.toBeNull());
      const pageId = state().activePage!;
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.id)).toEqual([pageId]),
      );

      const blockId = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "height only",
        });
      });

      // the op settles from the receipt: finalized with an inclusion HEIGHT…
      await waitFor(() =>
        expect(state().ops[opKey.pageBlock(blockId)]?.phase).toBe("finalized"),
      );
      const record = state().ops[opKey.pageBlock(blockId)]!;
      expect(record.height).toBeGreaterThan(0);
      // …and NO opHash — the networked receipt carries none, and settlement did
      // not need one (this is the shape a validator's receipt has).
      expect(record.opHash).toBeUndefined();

      // committed truth converged on the optimistic projection.
      await waitFor(() =>
        expect(state().activePageBlocks.map((b) => b.text)).toEqual([
          "Networked",
          "height only",
        ]),
      );
    },
  );

  it(
    "forge: an open → edit lifecycle settles on height-only receipts",
    { timeout: 30_000 },
    async () => {
      await boot({ persona: "networked", auto: true });

      act(() => {
        void actions().openForgeIssue({
          repo: REPO,
          title: "networked issue",
          body: "no opHash here",
        });
      });
      await waitFor(() => expect(state().forgeItems).toHaveLength(1));
      expect(state().forgeItems[0]).toMatchObject({
        number: 1,
        kind: "issue",
        title: "networked issue",
        state: "open",
      });

      act(() => {
        void actions().editForgeItem({
          repo: REPO,
          number: 1,
          title: "networked issue (edited)",
          body: null,
        });
      });
      await waitFor(() =>
        expect(state().forgeItems[0]?.title).toBe("networked issue (edited)"),
      );

      // the edit op finalized off a height-only receipt — settlement used the
      // inclusion height, and the record carries no opHash.
      const record = state().ops[opKey.forgeItem(REPO, 1)]!;
      expect(record.phase).toBe("finalized");
      expect(record.height).toBeGreaterThan(0);
      expect(record.opHash).toBeUndefined();
    },
  );
});
