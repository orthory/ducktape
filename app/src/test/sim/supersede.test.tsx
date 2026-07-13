// D6 — delayed-transport supersede. The transport gives NO ordering guarantee
// (each request is an independent POST), so a token-guarded refetch must ignore
// a stale response that arrives AFTER a newer one. `runSearch` stamps every
// fan-out with `++searchToken` and only applies a result when its token still
// equals `searchToken` (actions.ts). This test deterministically REORDERS the
// wire: it holds the first search's `/v1/index/*/view` responses, lets the
// second search complete, and only then releases the first — proving the stale
// (older-token) response arriving LAST cannot clobber the newer search state.
//
// The seam is monkey-patched on the transport object the provider holds
// (getNode() === the prop we pass), so the store's own view() calls route
// through the reorder. The sibling seam `pageThreadsToken` (loadPageThreads,
// actions.ts) is the SAME `token === pageThreadsToken` guard reached via
// comment-op reloads; one honestly-pinned token seam covers the invariant.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { NodeTransport } from "../../domain/transport";
import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.supersede.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

const searchText = (request: unknown): string | undefined =>
  (request as { search?: { text?: string } })?.search?.text;

const flushMicrotasks = async (): Promise<void> => {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
};

describe.skipIf(!bin)("supersede races against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  it(
    "an older search's response arriving LAST does not clobber the newer search (searchToken)",
    { timeout: 30_000 },
    async () => {
      const { transport } = await boot({ auto: true });

      // Reorder the wire: HOLD every view whose search text is "stale" (the
      // first search's two fan-out calls, chat + pages); everything else — the
      // "fresh" search included — passes straight through to the real node.
      const realView = transport.view.bind(transport) as NodeTransport["view"];
      const releases: Array<() => void> = [];
      transport.view = ((module: string, request: unknown) => {
        if (searchText(request) === "stale") {
          return new Promise((resolve) => {
            releases.push(() => resolve({ hits: [] }));
          });
        }
        return realView(module, request);
      }) as NodeTransport["view"];

      // fire the STALE search — both its view calls park unresolved.
      act(() => actions().runSearch("stale"));
      await waitFor(() => expect(releases.length).toBe(2));
      expect(state().searchPending).toBe(true);

      // fire the FRESH search — it supersedes the token and completes first.
      act(() => actions().runSearch("fresh"));
      await waitFor(() => expect(state().search?.query).toBe("fresh"));
      expect(state().searchPending).toBe(false);

      // NOW release the stale responses — they arrive last, but their token is
      // superseded, so the guard drops them: the fresh search survives intact.
      releases.forEach((release) => release());
      await flushMicrotasks();

      expect(state().search?.query).toBe("fresh");
      expect(state().searchPending).toBe(false);
    },
  );
});
