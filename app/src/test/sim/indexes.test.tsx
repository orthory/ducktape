// Derived-index scenarios against the sim node: the materialized-view lane
// (/v1/index/{module}/view) that the tag catalog, tag filter, and the ⌘K
// cross-module search read — served by the sim's REAL per-module indexers
// (ChatIndex, PagesIndex), fed block by block exactly like the daemons'. See
// sim-scenario.tsx for the harness contract.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.index.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

describe.skipIf(!bin)("derived-index scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  /** Auto-mode chat bootstrap: a committed channel, entered. */
  const openChannel = async (name: string): Promise<string> => {
    act(() => actions().createChannel(name, "open"));
    await waitFor(() => expect(state().activeChannel).not.toBeNull());
    return state().activeChannel!;
  };

  const send = (body: string): void => {
    act(() => actions().sendMessage(body));
  };

  /** The index is fed at COMMIT — an optimistic paint proves nothing. Gate
   *  view queries on the node's committed height instead. */
  const committedHeight = (height: number): Promise<void> =>
    waitFor(() => expect(state().status?.height).toBe(height));

  it(
    "#tags reach the catalog and the tag filter serves index hits",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });
      await openChannel("General");

      send("ship the #roadmap tonight");
      send("boring untagged message");
      // channel=1, then one block per post.
      await committedHeight(3);

      act(() => actions().loadChannelTags());
      await waitFor(() => expect(state().channelTags).toHaveLength(1));
      expect(state().channelTags[0]).toMatchObject({ tag: "roadmap", count: 1 });

      act(() => actions().setTagFilter("#roadmap"));
      await waitFor(() => {
        expect(state().tagHitsPending).toBe(false);
        expect(state().tagHits).toHaveLength(1);
      });
      expect(state().tagHits[0]!.text).toContain("#roadmap");
      expect(state().tagHits[0]!.tags).toEqual(["roadmap"]);

      act(() => actions().clearTagFilter());
      await waitFor(() => expect(state().tagFilter).toBeNull());
      expect(state().tagHits).toHaveLength(0);
    },
  );

  it(
    "the ⌘K search fans out over the chat AND pages views in one query",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });
      await openChannel("General");
      send("the flux capacitor hums");

      act(() => actions().createPage("Design"));
      await waitFor(() => expect(state().activePage).not.toBeNull());
      const pageId = state().activePage!;
      const blockId = crypto.randomUUID();
      act(() => {
        void actions().insertPageBlock({
          blockId,
          parent: pageId,
          after: null,
          kind: "paragraph",
          text: "flux capacitor drawings",
        });
      });
      // channel=1, post=2, page=3, block=4 — all fed to the index at commit.
      await committedHeight(4);

      act(() => actions().runSearch("capacitor"));
      await waitFor(() => {
        expect(state().searchPending).toBe(false);
        expect(state().search?.query).toBe("capacitor");
        expect(state().search?.chat).toHaveLength(1);
        expect(state().search?.docs).toHaveLength(1);
      });
      expect(state().search!.chat[0]!.text).toContain("flux capacitor");
      expect(state().search!.docs[0]!).toMatchObject({
        blockId,
        pageId,
        text: "flux capacitor drawings",
      });
    },
  );
});
