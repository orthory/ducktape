// What makes a long list feel slow is not the list — it is that every store
// patch used to re-render and reconcile all N rows. Building a list is a burst
// of back-to-back Enters, and each Enter flows two ops whose finalize + refresh
// each dispatch another patch, with no cheap keystrokes in between to space
// them out.
//
// This file measures that directly. `headingTopSpace` is called exactly once per
// BlockRow render, so mocking it gives an honest per-row render counter.
//
// The assertions deliberately avoid pinning exact render counts — a single
// changed row legitimately renders twice (its own render, then the draft-sync
// effect adopting the new store text), and React may add a bail-out pass. What
// must hold is that the cost of a patch does not GROW WITH N. So the same
// mutation is measured on a short page and a long one, and the two must agree.
//
// It also guards the two conditions the memo depends on, either of which a
// future edit could silently break:
//   1. the comparator reads FIELDS, not object identity (a refresh deserializes
//      the snapshot, so every block is a fresh object even when unchanged);
//   2. `handlers` stays referentially stable, or the fresh prop defeats the memo
//      on its own.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { PagesView } from "./PagesView";

const headingTopSpace = vi.fn(() => 0);
// only headingTopSpace is a spy; everything else is the REAL module, so adding
// a constant there can never silently break this counter again.
vi.mock("./pages-style", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./pages-style")>()),
  headingTopSpace: () => headingTopSpace(),
}));

const blockOf = (patch: Partial<PageBlock> & { id: string }): PageBlock => ({
  parent: "p1",
  page: "p1",
  kind: "paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

/** A root plus `n` paragraph children — long enough for O(N) to bite. */
const makePage = (n: number, textOf: (i: number) => string = (i) => `line ${i}`): PageBlock[] => [
  blockOf({
    id: "p1",
    parent: null,
    kind: "page",
    text: "Long doc",
    children: Array.from({ length: n }, (_, i) => `b${i}`),
  }),
  ...Array.from({ length: n }, (_, i) => blockOf({ id: `b${i}`, text: textOf(i) })),
];

const makeActions = () => {
  const spies: Record<string, (...args: unknown[]) => void> = {};
  return new Proxy(
    {},
    {
      get: (_t, key: string) => {
        spies[key] ??= vi.fn(() => Promise.resolve(true)) as unknown as (
          ...args: unknown[]
        ) => void;
        return spies[key];
      },
    },
  ) as ConsoleActions;
};

const mount = (blocks: PageBlock[]) => {
  const actions = makeActions();
  const stateOf = (bs: PageBlock[]): ConsoleState => ({
    ...createInitialState(),
    pages: [{ id: "p1", title: "Long doc", parent: null }],
    activePage: "p1",
    activePageBlocks: bs,
  });
  const view = (bs: PageBlock[]) => (
    <ConsoleContext.Provider value={{ state: stateOf(bs), actions }}>
      <PagesView />
    </ConsoleContext.Provider>
  );
  const { rerender, unmount } = render(view(blocks));
  return { patch: (bs: PageBlock[]) => rerender(view(bs)), unmount };
};

const renders = () => headingTopSpace.mock.calls.length;

/** Row-renders caused by `mutate`, on a page of `n` rows. */
const costOf = (n: number, mutate: (m: ReturnType<typeof mount>) => void): number => {
  headingTopSpace.mockClear();
  const m = mount(makePage(n));
  const before = renders();
  mutate(m);
  const cost = renders() - before;
  m.unmount();
  return cost;
};

describe("render cost of a store patch", () => {
  beforeEach(() => headingTopSpace.mockClear());

  it("renders each row exactly once on mount", () => {
    mount(makePage(50));
    expect(renders()).toBe(50);
  });

  // This is the whole fix. A refresh hands the store a freshly deserialized
  // snapshot: same values, all-new objects. Nothing visible changed, so nothing
  // should re-render — and before the memo, all N rows did.
  it("re-renders NO row when a patch changes nothing but object identity", () => {
    const identityOnlyPatch = (m: ReturnType<typeof mount>) =>
      m.patch(makePage(50).map((b) => ({ ...b, children: [...b.children] })));

    expect(costOf(50, identityOnlyPatch)).toBe(0);
  });

  it("costs the same to edit one row on a long page as on a short one", () => {
    const editRow7 = (m: ReturnType<typeof mount>) =>
      m.patch(makePage(80, (i) => (i === 7 ? "line 7 edited" : `line ${i}`)));
    const editRow7Short = (m: ReturnType<typeof mount>) =>
      m.patch(makePage(20, (i) => (i === 7 ? "line 7 edited" : `line ${i}`)));

    const long = costOf(80, editRow7);
    const short = costOf(20, editRow7Short);

    expect(long).toBe(short);
    expect(long).toBeLessThan(20); // O(1), nowhere near N
  });

  it("costs the same to type into a row on a long page as on a short one", () => {
    const type = (m: ReturnType<typeof mount>) => {
      void m;
      const area = screen.getByLabelText("Edit paragraph block 1");
      fireEvent.focus(area);
      fireEvent.change(area, { target: { value: "line 0 and more" } });
    };

    expect(costOf(80, type)).toBe(costOf(20, type));
  });

  it("costs the same to check a to-do on a long page as on a short one", () => {
    const checkFirst = (n: number) => (m: ReturnType<typeof mount>) => {
      const page = makePage(n);
      page[1] = blockOf({ id: "b0", kind: "todo", text: "line 0", checked: true });
      m.patch(page);
    };

    expect(costOf(80, checkFirst(80))).toBe(costOf(20, checkFirst(20)));
  });

  it("still re-renders the row that actually changed", () => {
    const { patch } = mount(makePage(50));
    patch(makePage(50, (i) => (i === 7 ? "line 7 edited" : `line ${i}`)));
    expect(screen.getByLabelText("Edit paragraph block 8")).toHaveValue("line 7 edited");
  });
});
