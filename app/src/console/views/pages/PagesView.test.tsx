import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi, type Mock } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { color } from "../../theme/tokens";
import { MAX_PASTE_BLOCKS } from "./page-paste";
import { EDIT_BOUNDARY_MS, PagesView } from "./PagesView";

const makeActions = () => {
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        // the block-write actions resolve true once the op commits (false on a
        // surfaced failure); the editor's split/merge await that to compensate.
        // Void actions resolving a promise nobody reads is harmless.
        spies[key] ??= vi.fn(() => Promise.resolve(true)) as unknown as (
          ...args: unknown[]
        ) => void;
        return spies[key];
      },
    },
  ) as ConsoleActions;
  return { actions, spies };
};

const blockOf = (patch: Partial<PageBlock> & { id: string }): PageBlock => ({
  parent: "p1",
  page: "p1",
  kind: "paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

const PAGE: PageBlock[] = [
  blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["a", "b"] }),
  blockOf({ id: "a", text: "First draft" }),
  blockOf({ id: "b", kind: "todo", text: "Ship it" }),
];

const renderPagesView = (patch: Partial<ConsoleState> = {}) => {
  const stateOf = (p: Partial<ConsoleState>) => ({
    ...createInitialState(),
    pages: [
      { id: "p1", title: "Launch plan", parent: null },
      { id: "p2", title: "Retro", parent: null },
    ],
    activePage: "p1",
    activePageBlocks: PAGE,
    ...p,
  });
  const { actions, spies } = makeActions();
  const view = (p: Partial<ConsoleState>) => (
    <ConsoleContext.Provider value={{ state: stateOf(p), actions }}>
      <PagesView />
    </ConsoleContext.Provider>
  );
  const { rerender, unmount } = render(view(patch));
  // the Proxy mints a spy on first access, so `spies.x` is undefined until the
  // view calls `actions.x`. Asserting "never called" needs the spy to exist:
  // touch it through `actions` first.
  const materialize = (...names: (keyof ConsoleActions)[]) => {
    for (const name of names) void actions[name];
  };
  // Make an op NOT LAND — the action resolves false, exactly as submitTracked
  // does for a write the node rejected or never answered. The editor's whole
  // safety net (the compensating split, the merge that refuses to remove a block
  // whose children did not make it across) hangs off that boolean, so this is the
  // ONLY way to exercise it. Without it the net was never once tested.
  const fails = (...names: (keyof ConsoleActions)[]) => {
    materialize(...names);
    for (const name of names) {
      (spies[name as string] as unknown as Mock).mockReturnValue(Promise.resolve(false));
    }
  };
  return {
    spies,
    materialize,
    fails,
    unmount,
    rerender: (p: Partial<ConsoleState>) => rerender(view(p)),
  };
};

/** Drain the microtask queue (the compensations and the merge's adoption chain
 *  hang off promise `.then`s, several deep) and let React commit what they
 *  patched. A bare `await` only runs one tick of the chain. */
const settle = () =>
  act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

/** The order calls were made in, across DIFFERENT spies. */
const callOrder = (spy: (...args: unknown[]) => void): number[] =>
  (spy as unknown as Mock).mock.invocationCallOrder;

describe("PagesView", () => {
  it("enumerates pages on mount and via the refresh control", () => {
    const { spies } = renderPagesView();
    expect(spies.listPages).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Refresh pages" }));
    expect(spies.listPages).toHaveBeenCalledTimes(2);
  });

  it("creates an untitled page from the New page button and opens one from the tree", () => {
    const { spies } = renderPagesView();

    // the "New page title" form is gone — a single button creates instantly.
    expect(screen.queryByLabelText("New page title")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "New page" }));
    expect(spies.createChildPage).toHaveBeenCalledWith(null);

    fireEvent.click(screen.getByRole("button", { name: "Open Retro" }));
    expect(spies.openPage).toHaveBeenCalledWith("p2");
  });

  it("filters the page tree while keeping matching ancestors visible", () => {
    renderPagesView({
      pages: [
        { id: "p1", title: "Launch plan", parent: null },
        { id: "p2", title: "SMS fallback", parent: "p1" },
        { id: "p3", title: "Retro", parent: null },
      ],
    });

    fireEvent.change(screen.getByRole("searchbox", { name: "Search pages" }), {
      target: { value: "sms" },
    });

    screen.getByRole("button", { name: "Open Launch plan" });
    screen.getByRole("button", { name: "Open SMS fallback" });
    expect(screen.queryByRole("button", { name: "Open Retro" })).toBeNull();
  });

  it("deletes a page through an in-app dialog", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderPagesView();

    try {
      const retro = screen.getByRole("button", { name: "Open Retro" });
      const row = retro.closest('[role="treeitem"]');
      expect(row).not.toBeNull();
      fireEvent.mouseEnter(row!);
      fireEvent.click(screen.getByRole("button", { name: /more actions for Retro/i }));
      fireEvent.click(screen.getByRole("menuitem", { name: /^delete$/i }));

      const dialog = screen.getByRole("dialog", { name: /delete Retro/i });
      expect(nativeConfirm).not.toHaveBeenCalled();
      fireEvent.click(within(dialog).getByRole("button", { name: /delete page/i }));

      expect(spies.deletePage).toHaveBeenCalledWith("p2");
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("drops the block-id/count clutter — no permanent hash chip, no block counter", () => {
    renderPagesView();
    // the copy-link affordance only appears on hover, never as steady chrome.
    expect(screen.queryByRole("button", { name: /copy link to block/i })).toBeNull();
    // the header is a breadcrumb, not an "N blocks" counter.
    expect(screen.queryByText(/^\d+ blocks?$/)).toBeNull();
  });

  it("shows the placeholder only on the focused empty block", () => {
    renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["e"] }),
        blockOf({ id: "e", text: "" }),
      ],
    });
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    expect(area.placeholder).toBe("");
    fireEvent.focus(area);
    expect(area.placeholder).toBe("Write, or press '/' for commands");
  });

  it("renders the tree as labelled editors with the title on the root", () => {
    renderPagesView();
    expect(screen.getByLabelText("Page title")).toHaveValue("Launch plan");
    expect(screen.getByLabelText("Edit paragraph block 1")).toHaveValue("First draft");
    expect(screen.getByLabelText("Edit todo block 2")).toHaveValue("Ship it");
  });

  it("splits on Enter: a fresh sibling after the current block", () => {
    const { spies } = renderPagesView();
    fireEvent.keyDown(screen.getByLabelText("Edit paragraph block 1"), {
      key: "Enter",
    });
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ parent: "p1", after: "a", kind: "paragraph" }),
    );
  });

  it("moves a right-side exact comment with text split into a new block", async () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["a"] }),
        blockOf({ id: "a", text: "left right" }),
      ],
      pageThreads: [{
        target: "a",
        threads: [{
          thread: {
            id: "t1", target: "a", opener: { user: [1] }, created_at: 1,
            anchor: { start: 5, end: 10 }, resolved: false, resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [],
        }],
      }],
    });
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    area.setSelectionRange(5, 5);
    fireEvent.keyDown(area, { key: "Enter" });
    await settle();

    const inserted = (spies.insertPageBlock as unknown as Mock).mock.calls[0][0];
    expect(spies.moveCommentThread).toHaveBeenCalledWith({
      threadId: "t1",
      target: inserted.blockId,
      anchor: { start: 0, end: 5 },
    });
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "left " });
  });

  it("keeps a crossing comment intact when its split insert fails", async () => {
    const { spies, fails, materialize } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["a"] }),
        blockOf({ id: "a", text: "left right" }),
      ],
      pageThreads: [{
        target: "a",
        threads: [{
          thread: {
            id: "t1", target: "a", opener: { user: [1] }, created_at: 1,
            anchor: { start: 3, end: 8 }, resolved: false, resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [],
        }],
      }],
    });
    materialize("updatePageBlockText", "moveCommentThread");
    fails("insertPageBlock");
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    area.setSelectionRange(5, 5);
    fireEvent.keyDown(area, { key: "Enter" });
    await settle();

    expect(spies.updatePageBlockText).not.toHaveBeenCalled();
    expect(spies.moveCommentThread).not.toHaveBeenCalled();
  });

  it("converts a paragraph via a typed markdown prefix", () => {
    const { spies } = renderPagesView();
    fireEvent.change(screen.getByLabelText("Edit paragraph block 1"), {
      target: { value: "# First draft" },
    });
    expect(spies.setPageBlockKind).toHaveBeenCalledWith({
      blockId: "a",
      kind: "heading1",
    });
  });

  it("checks a to-do through its gutter checkbox", () => {
    const { spies } = renderPagesView();
    fireEvent.click(screen.getByRole("button", { name: "Check to-do block 2" }));
    expect(spies.setPageBlockChecked).toHaveBeenCalledWith({
      blockId: "b",
      checked: true,
    });
  });

  it("indents with Tab using the previous sibling as the new parent", () => {
    const { spies } = renderPagesView();
    fireEvent.keyDown(screen.getByLabelText("Edit todo block 2"), {
      key: "Tab",
    });
    expect(spies.movePageBlock).toHaveBeenCalledWith({
      blockId: "b",
      parent: "a",
      after: null,
    });
  });

  it("types Tab inside code instead of moving the block", () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["code"] }),
        blockOf({ id: "code", kind: "code", text: "let x = 1" }),
      ],
    });
    const area = screen.getByLabelText("Edit code block 1") as HTMLTextAreaElement;
    area.setSelectionRange(3, 3);
    fireEvent.keyDown(area, { key: "Tab" });
    expect(area).toHaveValue("let\t x = 1");
    expect(area.selectionStart).toBe(4);
    expect(spies.movePageBlock).toBeUndefined();
  });

  it("removes an empty block on Backspace", () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["a", "empty"] }),
        blockOf({ id: "a", text: "keep" }),
        blockOf({ id: "empty", text: "" }),
      ],
    });
    fireEvent.keyDown(screen.getByLabelText("Edit paragraph block 2"), {
      key: "Backspace",
    });
    expect(spies.removePageBlock).toHaveBeenCalledWith("empty");
  });
});

describe("edit boundaries & draft protection", () => {
  it("a typing pause commits one update op without leaving the block", () => {
    vi.useFakeTimers();
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1");
    fireEvent.focus(area);
    fireEvent.change(area, { target: { value: "First draft, extended" } });

    // no boundary yet: mid-typing must not flow an op per keystroke. (spies
    // materialize lazily on first action call, so absence == never called.)
    if (spies.updatePageBlockText) {
      expect(spies.updatePageBlockText).not.toHaveBeenCalled();
    }
    act(() => {
      vi.advanceTimersByTime(EDIT_BOUNDARY_MS);
    });
    expect(spies.updatePageBlockText).toHaveBeenCalledTimes(1);
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({
      blockId: "a",
      text: "First draft, extended",
    });
    vi.useRealTimers();
  });

  it("an open slash menu is a command in progress — no boundary commit", () => {
    vi.useFakeTimers();
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1");
    fireEvent.focus(area);
    fireEvent.change(area, { target: { value: "/head" } });
    act(() => {
      vi.advanceTimersByTime(EDIT_BOUNDARY_MS * 2);
    });
    // lazily-materialized spy: absent means the action was never reached.
    if (spies.updatePageBlockText) {
      expect(spies.updatePageBlockText).not.toHaveBeenCalled();
    }
    vi.useRealTimers();
  });

  it("a snapshot landing mid-edit never clobbers the focused draft", () => {
    const { spies, rerender } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1");
    fireEvent.focus(area);
    fireEvent.change(area, { target: { value: "my live draft" } });

    // an earlier op's completion refresh lands a snapshot that predates the
    // edit — the focused block keeps its draft.
    rerender({
      activePageBlocks: PAGE.map((b) =>
        b.id === "a" ? { ...b, text: "stale committed" } : b,
      ),
    });
    expect(area).toHaveValue("my live draft");

    // blur commits the draft as usual…
    fireEvent.blur(area);
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({
      blockId: "a",
      text: "my live draft",
    });

    // …and once unfocused, committed truth is adopted again.
    rerender({
      activePageBlocks: PAGE.map((b) =>
        b.id === "a" ? { ...b, text: "peer edit" } : b,
      ),
    });
    expect(area).toHaveValue("peer edit");
  });

  it("the title shares the contract: focused draft survives, page switch resets", () => {
    const { rerender } = renderPagesView();
    const title = screen.getByLabelText("Page title");
    fireEvent.focus(title);
    fireEvent.change(title, { target: { value: "Launch plan v2" } });

    rerender({
      activePageBlocks: PAGE.map((b) =>
        b.id === "p1" ? { ...b, text: "Launch plan" } : b,
      ),
    });
    expect(title).toHaveValue("Launch plan v2");

    // switching pages resets the draft even while the input is focused — a
    // draft never crosses pages.
    rerender({
      activePage: "p2",
      activePageBlocks: [
        blockOf({ id: "p2", parent: null, page: "p2", kind: "page", text: "Retro", children: [] }),
      ],
    });
    expect(screen.getByLabelText("Page title")).toHaveValue("Retro");
  });
});

describe("Pages keyboard shortcuts & tab strip", () => {
  const withTabs = (patch: Partial<ConsoleState> = {}) =>
    renderPagesView({ openTabs: ["p1", "p2", "p3"], activePage: "p1", ...patch });

  it("cycles to the next tab on ⌘⇧]", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "BracketRight", metaKey: true, shiftKey: true });
    expect(spies.openPage).toHaveBeenLastCalledWith("p2");
  });

  it("wraps from the last tab back to the first on ⌘⇧]", () => {
    const { spies } = withTabs({ activePage: "p3" });
    fireEvent.keyDown(document, { code: "BracketRight", metaKey: true, shiftKey: true });
    expect(spies.openPage).toHaveBeenLastCalledWith("p1");
  });

  it("cycles to the previous tab on ⌘⇧[ (wrapping past the first to the last)", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "BracketLeft", metaKey: true, shiftKey: true });
    expect(spies.openPage).toHaveBeenLastCalledWith("p3");
  });

  it("accepts Ctrl as well as ⌘ for tab cycling", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "BracketRight", ctrlKey: true, shiftKey: true });
    expect(spies.openPage).toHaveBeenLastCalledWith("p2");
  });

  it("creates a new top-level page on ⌘T and ⌘N", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "KeyT", metaKey: true });
    fireEvent.keyDown(document, { code: "KeyN", metaKey: true });
    expect(spies.createChildPage).toHaveBeenCalledTimes(2);
    expect(spies.createChildPage).toHaveBeenNthCalledWith(1, null);
    expect(spies.createChildPage).toHaveBeenNthCalledWith(2, null);
  });

  it("closes the active doc tab on ⌘W and Ctrl+W", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "KeyW", metaKey: true });
    expect(spies.closeTab).toHaveBeenLastCalledWith("p1");
    fireEvent.keyDown(document, { code: "KeyW", ctrlKey: true });
    expect(spies.closeTab).toHaveBeenCalledTimes(2);
    expect(spies.openPage).not.toHaveBeenCalled();
  });

  it("⌘W with no open doc falls through to the window untouched", () => {
    const { spies } = renderPagesView({
      activePage: null,
      activePageBlocks: [],
      openTabs: [],
    });
    fireEvent.keyDown(document, { code: "KeyW", metaKey: true });
    // closeTab is spied lazily on first access; ⌘W must never reach it.
    if (spies.closeTab) expect(spies.closeTab).not.toHaveBeenCalled();
  });

  it("keeps the tab strip scrollable but hides the scrollbar chrome", () => {
    withTabs();
    const strip = screen.getByRole("tablist", { name: "Open pages" });
    // scroll is retained (overflow-x auto) …
    expect(strip).toHaveStyle({ overflowX: "auto" });
    // … while the .no-scrollbar utility suppresses the global 10px bar.
    expect(strip.className).toContain("no-scrollbar");
  });

});

describe("floating comment card", () => {
  const threadsOn = (target: string) => [
    {
      target,
      threads: [
        {
          thread: {
            id: "t1",
            target,
            opener: { user: [1] },
            created_at: 1,
            resolved: false,
            resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [
            {
              id: "c1",
              thread_id: "t1",
              author: { user: [1] },
              text: "a note",
              created_at: 1,
              edited_at: null,
              deleted: false,
            },
          ],
        },
      ],
    },
  ] as ConsoleState["pageThreads"];

  it("opens a floating card on the block comment button — never the panel", () => {
    renderPagesView({ pageThreads: threadsOn("a") });
    fireEvent.click(screen.getByRole("button", { name: "Comment on block 1" }));
    screen.getByRole("dialog", { name: "Comments on this block" });
    expect(screen.queryByRole("complementary", { name: "Comments" })).toBeNull();
  });

  it("writes a page comment through the header card; Escape dismisses it", () => {
    const { spies } = renderPagesView();

    fireEvent.click(screen.getByRole("button", { name: "Comment on page" }));
    const dialog = screen.getByRole("dialog", { name: "Comments on this page" });
    expect(screen.queryByRole("complementary", { name: "Comments" })).toBeNull();

    // an uncommented page opens straight into the composer.
    fireEvent.change(within(dialog).getByLabelText("New comment text"), {
      target: { value: "ship checklist looks thin" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add comment" }));
    expect(spies.addComment).toHaveBeenCalledWith({
      target: "p1",
      text: "ship checklist looks thin",
    });

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Comments on this page" })).toBeNull();
  });

  it("does not duplicate target threads in an all-comments list", () => {
    renderPagesView();
    expect(screen.queryByRole("button", { name: "Show comments" })).toBeNull();
    expect(screen.queryByRole("complementary", { name: "Comments" })).toBeNull();
  });

  it("hover reveals the comment affordance but no copy-block-link", () => {
    renderPagesView();
    fireEvent.mouseOver(screen.getByLabelText("Edit paragraph block 1"));
    screen.getByRole("button", { name: "Comment on block 1" });
    expect(screen.queryByRole("button", { name: /copy link to block/i })).toBeNull();
  });

  it("persists inline marks and exact comment anchors for selected text", () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 5);
    fireEvent.select(area);

    screen.getByRole("toolbar", { name: "Selection actions" });
    fireEvent.click(screen.getByRole("button", { name: "Bold" }));
    expect(spies.setPageBlockSpanMark).toHaveBeenCalledWith({
      blockId: "a",
      start: 0,
      end: 5,
      kind: "bold",
      active: true,
    });

    fireEvent.click(screen.getByRole("button", { name: "Comment on selected text" }));
    const dialog = screen.getByRole("dialog", { name: "Comments on selected text" });
    fireEvent.change(within(dialog).getByLabelText("New comment text"), {
      target: { value: "tighten this" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add comment" }));
    expect(spies.addComment).toHaveBeenCalledWith({
      target: "a",
      text: "tighten this",
      anchor: { start: 0, end: 5 },
    });
  });

  it("renders persisted marks and opens an anchored thread from its highlight", () => {
    const marked = PAGE.map((block) =>
      block.id === "a"
        ? { ...block, marks: [{ start: 0, end: 5, kind: "bold" as const }] }
        : block,
    );
    renderPagesView({
      activePageBlocks: marked,
      pageThreads: [{
        target: "a",
        threads: [{
          thread: {
            id: "t1", target: "a", opener: { user: [1] }, created_at: 1,
            anchor: { start: 0, end: 5 }, resolved: false, resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [{
            id: "c1", thread_id: "t1", author: { user: [1] }, text: "note",
            created_at: 1, edited_at: null, deleted: false,
          }],
        }],
      }],
    });
    const mirror = document.querySelector("[data-inline-text]");
    expect(mirror?.textContent).toBe("First draft");
    expect(mirror?.querySelector("span")?.style.fontWeight).toBe("750");

    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    area.setSelectionRange(2, 2);
    fireEvent.click(area);
    screen.getByRole("dialog", { name: "Comments on selected text" });
    expect(screen.getByLabelText("Commented text").textContent).toBe("First");
  });

  const selectFirstBlock = (start = 0, end = 5) => {
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(start, end);
    fireEvent.select(area);
    screen.getByRole("toolbar", { name: "Selection actions" });
    return area;
  };

  it("waits for the pointer release before showing the guide menu", () => {
    renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    fireEvent.focus(area);
    fireEvent.mouseDown(area);
    area.setSelectionRange(0, 5);
    fireEvent.select(area);
    expect(screen.queryByRole("toolbar", { name: "Selection actions" })).toBeNull();
    // release over the row (bubbles to the one-shot document listener); firing
    // straight on `document` would bypass the React root and wedge React's
    // select-event plugin's module-level mouse state for every later test.
    fireEvent.mouseUp(area);
    screen.getByRole("toolbar", { name: "Selection actions" });
  });

  it("turns the block into a heading from the guide menu", () => {
    const { spies } = renderPagesView();
    selectFirstBlock();
    fireEvent.click(screen.getByRole("button", { name: "Heading 2" }));
    expect(spies.setPageBlockKind).toHaveBeenCalledWith({ blockId: "a", kind: "heading2" });
  });

  it("dismisses the guide menu on scroll and on focus loss", () => {
    renderPagesView();
    selectFirstBlock();
    fireEvent.scroll(document);
    expect(screen.queryByRole("toolbar", { name: "Selection actions" })).toBeNull();

    const area = selectFirstBlock();
    fireEvent.blur(area);
    expect(screen.queryByRole("toolbar", { name: "Selection actions" })).toBeNull();
  });

  it("⌘/ comments on the live selection", () => {
    renderPagesView();
    const area = selectFirstBlock(0, 5);
    fireEvent.keyDown(area, { key: "/", metaKey: true });
    screen.getByRole("dialog", { name: "Comments on selected text" });
    expect(screen.queryByRole("toolbar", { name: "Selection actions" })).toBeNull();
  });

  it("keeps the fresh anchor visible and quoted while composing", () => {
    renderPagesView();
    const area = selectFirstBlock(0, 5);
    fireEvent.keyDown(area, { key: "/", metaKey: true });
    const dialog = screen.getByRole("dialog", { name: "Comments on selected text" });
    // no thread exists yet, but the range still paints behind the textarea…
    expect(document.querySelector("[data-inline-text]")?.textContent).toBe("First draft");
    // …and the composer echoes the selected text.
    expect(within(dialog).getByLabelText("Commented text").textContent).toBe("First");
  });

  const twoRangeThreads = [
    {
      target: "a",
      threads: [
        {
          thread: {
            id: "t1", target: "a", opener: { user: [1] }, created_at: 1,
            anchor: { start: 0, end: 5 }, resolved: false, resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [{
            id: "c1", thread_id: "t1", author: { user: [1] }, text: "about First",
            created_at: 1, edited_at: null, deleted: false,
          }],
        },
        {
          thread: {
            id: "t2", target: "a", opener: { user: [1] }, created_at: 2,
            anchor: { start: 6, end: 11 }, resolved: false, resolved_by: null,
            comment_ids: ["c2"],
          },
          comments: [{
            id: "c2", thread_id: "t2", author: { user: [1] }, text: "about draft",
            created_at: 2, edited_at: null, deleted: false,
          }],
        },
      ],
    },
  ] as ConsoleState["pageThreads"];

  it("scopes the card to the clicked range's thread, not the whole block", () => {
    renderPagesView({ pageThreads: twoRangeThreads });
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    area.setSelectionRange(2, 2); // inside "First" (t1), outside "draft" (t2)
    fireEvent.click(area);
    const dialog = screen.getByRole("dialog", { name: "Comments on selected text" });
    within(dialog).getByText("about First");
    expect(within(dialog).queryByText("about draft")).toBeNull();
  });

  it("the block affordance still shows every thread on the block", () => {
    renderPagesView({ pageThreads: twoRangeThreads });
    fireEvent.click(screen.getByRole("button", { name: "Comment on block 1" }));
    const dialog = screen.getByRole("dialog", { name: "Comments on this block" });
    within(dialog).getByText("about First");
    within(dialog).getByText("about draft");
  });

  it("badges open discussions; resolved-only history keeps a quiet badge", () => {
    const resolved = (threads: ConsoleState["pageThreads"]) =>
      threads.map((group) => ({
        ...group,
        threads: group.threads.map((view) => ({
          ...view,
          thread: { ...view.thread, resolved: true, resolved_by: { user: [1] } },
        })),
      }));
    const { rerender } = renderPagesView({ pageThreads: twoRangeThreads });
    const badge = () => screen.getByRole("button", { name: "Comment on block 1" });
    expect(badge().textContent).toBe("2");
    expect(badge().title).toBe("2 open of 2 discussions");
    // all resolved: the badge stays (the history is still discoverable) but
    // reads quiet — total count, muted tone.
    rerender({ pageThreads: resolved(twoRangeThreads) });
    expect(badge().textContent).toBe("2");
    expect(badge().title).toBe("0 open of 2 discussions");
  });

  it("docks the card in the side rail on a wide viewport; scroll keeps it", () => {
    const wide = window.innerWidth;
    window.innerWidth = 1600;
    try {
      renderPagesView({ pageThreads: threadsOn("a") });
      fireEvent.click(screen.getByRole("button", { name: "Comment on block 1" }));
      const dialog = screen.getByRole("dialog", { name: "Comments on this block" });
      const rail = document.querySelector("[data-comment-rail]");
      expect(rail?.contains(dialog)).toBe(true);
      expect(getComputedStyle(dialog).position).toBe("absolute");
      fireEvent.scroll(document);
      expect(screen.queryByRole("dialog", { name: "Comments on this block" })).not.toBeNull();
    } finally {
      window.innerWidth = wide;
    }
  });

  it("keeps the floating popover on a narrow viewport; scroll dismisses it", () => {
    const wide = window.innerWidth;
    window.innerWidth = 1000;
    try {
      renderPagesView({ pageThreads: threadsOn("a") });
      fireEvent.click(screen.getByRole("button", { name: "Comment on block 1" }));
      const dialog = screen.getByRole("dialog", { name: "Comments on this block" });
      expect(getComputedStyle(dialog).position).toBe("fixed");
      fireEvent.scroll(document);
      expect(screen.queryByRole("dialog", { name: "Comments on this block" })).toBeNull();
    } finally {
      window.innerWidth = wide;
    }
  });

  it("Escape spends itself on the guide menu — an open card survives", () => {
    renderPagesView({ pageThreads: threadsOn("a") });
    fireEvent.click(screen.getByRole("button", { name: "Comment on block 1" }));
    screen.getByRole("dialog", { name: "Comments on this block" });
    const area = selectFirstBlock(0, 5);
    fireEvent.keyDown(area, { key: "Escape" });
    expect(screen.queryByRole("toolbar", { name: "Selection actions" })).toBeNull();
    screen.getByRole("dialog", { name: "Comments on this block" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Comments on this block" })).toBeNull();
  });

  it("a tier flip repositions the card without dropping the composer draft", () => {
    const wide = window.innerWidth;
    window.innerWidth = 1000;
    try {
      renderPagesView();
      fireEvent.click(screen.getByRole("button", { name: "Comment on page" }));
      const draft = screen.getByLabelText("New comment text") as HTMLTextAreaElement;
      fireEvent.change(draft, { target: { value: "half-typed thought" } });
      window.innerWidth = 1600;
      fireEvent(window, new Event("resize"));
      const after = screen.getByLabelText("New comment text") as HTMLTextAreaElement;
      expect(after.value).toBe("half-typed thought");
      expect(
        getComputedStyle(screen.getByRole("dialog", { name: "Comments on this page" })).position,
      ).toBe("absolute");
    } finally {
      window.innerWidth = wide;
    }
  });
});

describe("subpages", () => {
  const withChild = {
    pages: [
      { id: "p1", title: "Launch plan", parent: null },
      { id: "p2", title: "Retro", parent: null },
      { id: "p3", title: "Child", parent: "p1" },
    ],
  };

  it("lists child pages in a Subpages section and opens them", () => {
    const { spies } = renderPagesView(withChild);
    fireEvent.click(screen.getByRole("button", { name: "Open subpage Child" }));
    expect(spies.openPage).toHaveBeenCalledWith("p3");
  });

  it("renders no Subpages section when the page has no children", () => {
    renderPagesView();
    // by LABEL, not by text: child pages now render as inline page blocks in
    // the document flow, so there is no "SUBPAGES" heading left to look for and
    // a text query would pass vacuously.
    expect(screen.queryByLabelText("Subpages")).toBeNull();
  });

  it("renders child pages as inline page blocks, not a boxed-off section", () => {
    renderPagesView(withChild);
    const section = screen.getByLabelText("Subpages");
    // no uppercase mono section header, no underlined title.
    expect(section.textContent).toBe("Child");
    const row = within(section).getByRole("button", { name: "Open subpage Child" });
    expect(row.innerHTML).not.toContain("border-bottom");
  });

  it("creates a subpage from /page instead of converting the block", () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1");
    fireEvent.focus(area);
    fireEvent.change(area, { target: { value: "/page" } });
    fireEvent.mouseDown(screen.getByRole("option", { name: /new subpage/i }));
    expect(spies.createChildPage).toHaveBeenCalledWith("p1");
    // the block itself must NOT be converted to a "page" kind.
    if (spies.setPageBlockKind) expect(spies.setPageBlockKind).not.toHaveBeenCalled();
  });
});

describe("endless canvas", () => {
  const appended = expect.objectContaining({
    parent: "p1",
    after: "b",
    kind: "paragraph",
    text: "",
  });

  it("appends a block on mousedown of Add a block — before blur can commit", () => {
    const { spies } = renderPagesView();
    fireEvent.mouseDown(screen.getByRole("button", { name: "Add a block" }));
    expect(spies.insertPageBlock).toHaveBeenCalledWith(appended);
  });

  it("appends on keyboard activation (detail-0 click) but not twice per pointer press", () => {
    const { spies } = renderPagesView();
    const btn = screen.getByRole("button", { name: "Add a block" });
    // Enter/Space on the focused button synthesizes a click with detail 0
    // and no preceding mousedown — the button must still work.
    fireEvent.click(btn, { detail: 0 });
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(1);
    // a real pointer press fires mousedown AND a trailing detail-1 click;
    // that must append exactly once.
    fireEvent.mouseDown(btn);
    fireEvent.click(btn, { detail: 1 });
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(2);
  });

  it("appends a block when the canvas below the content is pressed", () => {
    const { spies } = renderPagesView();
    fireEvent.mouseDown(screen.getByTestId("page-canvas-filler"));
    expect(spies.insertPageBlock).toHaveBeenCalledWith(appended);
  });

  it("has no canvas filler without an open page", () => {
    renderPagesView({ activePage: null, activePageBlocks: [], openTabs: [] });
    expect(screen.queryByTestId("page-canvas-filler")).toBeNull();
  });

  it("drops the bordered page card — the scroll surface itself is paper", () => {
    renderPagesView();
    expect(screen.getByTestId("doc-scroll")).toHaveStyle({ background: color.paper });
  });
});

// The block used to be an atomic text cell: Enter appended an empty sibling and
// left your text behind, Backspace never joined two blocks, and Cmd+Enter on a
// to-do made a block instead of checking it. jsdom does not place a caret for
// us, so every test here sets the selection before dispatching the key.
describe("text moves across block boundaries", () => {
  const caretAt = (area: HTMLTextAreaElement, at: number) => {
    fireEvent.focus(area);
    area.setSelectionRange(at, at);
  };

  it("splits at the caret: this block keeps the left half, the sibling takes the right", () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    caretAt(area, 5); // "First| draft"
    fireEvent.keyDown(area, { key: "Enter" });

    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ parent: "p1", after: "a", kind: "paragraph", text: " draft" }),
    );
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "First" });
  });

  it("continues a list on Enter, carrying the right half into the new item", () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 4); // "Ship| it"
    fireEvent.keyDown(area, { key: "Enter" });

    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "todo", text: " it" }),
    );
  });

  it("merges into the previous block on Backspace at offset 0, caret at the seam", () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 0);
    fireEvent.keyDown(area, { key: "Backspace" });

    expect(spies.updatePageBlockText).toHaveBeenCalledWith({
      blockId: "a",
      text: "First draftShip it",
    });
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
  });

  it("moves exact comments before removing a merged source block", async () => {
    const { spies } = renderPagesView({
      pageThreads: [{
        target: "b",
        threads: [{
          thread: {
            id: "t1", target: "b", opener: { user: [1] }, created_at: 1,
            anchor: { start: 0, end: 4 }, resolved: false, resolved_by: null,
            comment_ids: ["c1"],
          },
          comments: [],
        }],
      }],
    });
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 0);
    fireEvent.keyDown(area, { key: "Backspace" });
    await settle();

    expect(spies.moveCommentThread).toHaveBeenCalledWith({
      threadId: "t1",
      target: "a",
      anchor: { start: 11, end: 15 },
    });
    expect(callOrder(spies.moveCommentThread)[0]).toBeLessThan(callOrder(spies.removePageBlock)[0]);
  });

  it("leaves Backspace alone in the middle of a block", () => {
    const { spies, materialize } = renderPagesView();
    materialize("removePageBlock", "updatePageBlockText");
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 3);
    fireEvent.keyDown(area, { key: "Backspace" });

    expect(spies.removePageBlock).not.toHaveBeenCalled();
    expect(spies.updatePageBlockText).not.toHaveBeenCalled();
  });

  it("checks a to-do with Cmd+Enter, and makes no block", () => {
    const { spies, materialize } = renderPagesView();
    materialize("insertPageBlock");
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 2);
    fireEvent.keyDown(area, { key: "Enter", metaKey: true });

    expect(spies.setPageBlockChecked).toHaveBeenCalledWith({ blockId: "b", checked: true });
    expect(spies.insertPageBlock).not.toHaveBeenCalled();
  });
});

describe("the caret lands on the adjacent line", () => {
  it("ArrowDown from the end of a block lands at the START of the next", () => {
    renderPagesView();
    const first = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    const second = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    fireEvent.focus(first);
    first.setSelectionRange(first.value.length, first.value.length);
    fireEvent.keyDown(first, { key: "ArrowDown" });

    expect(document.activeElement).toBe(second);
    expect(second.selectionStart).toBe(0);
  });

  it("ArrowUp from the start of a block lands at the END of the previous", () => {
    renderPagesView();
    const first = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    const second = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    fireEvent.focus(second);
    second.setSelectionRange(0, 0);
    fireEvent.keyDown(second, { key: "ArrowUp" });

    expect(document.activeElement).toBe(first);
    expect(first.selectionStart).toBe(first.value.length);
  });

  it("ArrowUp from the first block reaches the title", () => {
    renderPagesView();
    const first = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    fireEvent.focus(first);
    first.setSelectionRange(0, 0);
    fireEvent.keyDown(first, { key: "ArrowUp" });

    expect(document.activeElement).toBe(screen.getByLabelText("Page title"));
  });
});

describe("the title descends into the body", () => {
  it("focuses the first block at its start, inserting nothing", () => {
    const { spies, materialize } = renderPagesView();
    materialize("insertPageBlock");
    const title = screen.getByLabelText("Page title");
    fireEvent.keyDown(title, { key: "Enter" });

    const first = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    expect(document.activeElement).toBe(first);
    expect(first.selectionStart).toBe(0);
    expect(spies.insertPageBlock).not.toHaveBeenCalled();
  });

  // the reported papercut: on a page with no body there was nothing to focus,
  // and focusRow(undefined) means "focus the title", so Enter did nothing.
  it("creates the first block when the page has no body yet", () => {
    const empty: PageBlock[] = [
      blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: [] }),
    ];
    const { spies } = renderPagesView({ activePageBlocks: empty });
    fireEvent.keyDown(screen.getByLabelText("Page title"), { key: "Enter" });

    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ parent: "p1", kind: "paragraph", text: "" }),
    );
  });
});

describe("a divider is reachable from the keyboard", () => {
  it("Backspace at the start of the block below removes the divider above", () => {
    const withDivider: PageBlock[] = [
      blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["d", "a"] }),
      blockOf({ id: "d", kind: "divider" }),
      blockOf({ id: "a", text: "First draft" }),
    ];
    const { spies, materialize } = renderPagesView({ activePageBlocks: withDivider });
    materialize("updatePageBlockText");
    const area = screen.getByLabelText("Edit paragraph block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 0);
    fireEvent.keyDown(area, { key: "Backspace" });

    expect(spies.removePageBlock).toHaveBeenCalledWith("d");
    // the text is not merged into a divider, and this block survives.
    expect(spies.updatePageBlockText).not.toHaveBeenCalled();
  });

  // THE BLOCKER. `indentTarget` had no kind check and MoveBlock validates only
  // page-match + cycles, so Tab really could make a DIVIDER the parent of the
  // block you were typing in. That block's row then sits directly under the rule,
  // so Backspace at offset 0 reads as "remove the divider above" — and the old
  // removeDividerAbove called removePageBlock bare, with no children check and no
  // confirm. RemoveBlock runs delete_subtree: it took the divider, the block
  // holding the caret, and everything under it. One keystroke, no dialog, no undo.
  //
  // The suite passed because it only ever tested a CHILDLESS divider (above).
  it("CONFIRMS instead of deleting a divider that adopted the caret's own block", () => {
    const adopted: PageBlock[] = [
      blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["d"] }),
      blockOf({ id: "d", kind: "divider", children: ["x"] }),
      blockOf({ id: "x", parent: "d", text: "the text I am typing" }),
    ];
    const { spies, materialize } = renderPagesView({ activePageBlocks: adopted });
    materialize("removePageBlock");
    const area = screen.getByLabelText("Edit paragraph block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 0);
    fireEvent.keyDown(area, { key: "Backspace" });

    // it must NOT go straight to the wire — that op destroys "x" too.
    expect(spies.removePageBlock).not.toHaveBeenCalled();
    within(screen.getByRole("dialog", { name: /delete this block/i })).getByText(/1 nested block/);
  });
});

describe("the document has one left edge", () => {
  it("hangs a list marker in the margin instead of indenting the text column", () => {
    renderPagesView();
    const checkbox = screen.getByRole("button", { name: "Check to-do block 2" });
    const gutter = checkbox.parentElement as HTMLElement;

    // out of flow, so the text column beside it starts at offset 0.
    expect(gutter.style.position).toBe("absolute");
    expect(gutter.style.left).toBe("-28px");
  });

  it("gives prose no marker box to pay for", () => {
    renderPagesView();
    const rowOf = (label: string) =>
      screen.getByLabelText(label).closest('[style*="margin-left"]') as HTMLElement;

    // a paragraph renders no marker element at all. It used to render an empty
    // 20px box + an 8px gap, which is what pushed every line of body text 28px
    // right of the title. A to-do still renders its checkbox — hanging, now.
    const prose = rowOf("Edit paragraph block 1");
    const todo = rowOf("Edit todo block 2");
    expect(prose.children.length).toBe(todo.children.length - 1);
    expect(prose.querySelector('[style*="-28px"]')).toBeNull();
    expect(todo.querySelector('[style*="-28px"]')).not.toBeNull();
  });

  it("keeps the title flush with the text column", () => {
    renderPagesView();
    const title = screen.getByLabelText("Page title") as HTMLInputElement;
    expect(title.style.padding).toBe("0px");
  });

  it("stops padding the add-block button around a gutter that no longer exists", () => {
    renderPagesView();
    const add = screen.getByRole("button", { name: /add a block|start writing/i });
    expect(add.style.padding).toBe("8px 0px");
  });
});

// ── The left gutter: the affordances that replaced a one-click subtree bomb ──

describe("the left hover gutter", () => {
  const NESTED: PageBlock[] = [
    blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["a", "b"] }),
    blockOf({ id: "a", kind: "toggle", text: "Parent", children: ["a1"] }),
    blockOf({ id: "a1", parent: "a", text: "Child" }),
    blockOf({ id: "b", text: "Leaf" }),
  ];

  const openMenu = (n: number) =>
    fireEvent.click(screen.getByRole("button", { name: `Block ${n} actions` }));

  it("inserts a paragraph below from the + affordance", () => {
    const { spies } = renderPagesView();
    fireEvent.mouseDown(screen.getByRole("button", { name: "Insert block below block 1" }));
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ parent: "p1", after: "a", kind: "paragraph", text: "" }),
    );
  });

  it("turns a block into another kind from the handle menu — the slash catalogue, reused", () => {
    const { spies } = renderPagesView();
    openMenu(1);
    fireEvent.click(screen.getByRole("menuitem", { name: /Heading 1/ }));
    expect(spies.setPageBlockKind).toHaveBeenCalledWith({ blockId: "a", kind: "heading1" });
  });

  it("never offers Page as a conversion (it would create, not convert)", () => {
    renderPagesView();
    openMenu(1);
    const menu = screen.getByRole("menu", { name: "Block 1 actions" });
    expect(within(menu).queryByRole("menuitem", { name: /new subpage/i })).toBeNull();
  });

  it("duplicates a block and its whole subtree", () => {
    const { spies } = renderPagesView({ activePageBlocks: NESTED });
    openMenu(1); // the toggle, which holds a child
    fireEvent.click(screen.getByRole("menuitem", { name: "Duplicate" }));
    // one insert for the toggle, one for the nested child.
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(2);
    expect(spies.insertPageBlock).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ parent: "p1", after: "a", kind: "toggle", text: "Parent" }),
    );
    expect(spies.insertPageBlock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ after: null, kind: "paragraph", text: "Child" }),
    );
  });

  it("deletes a childless block outright", () => {
    const { spies } = renderPagesView({ activePageBlocks: NESTED });
    openMenu(3); // the leaf
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
  });

  // this is the dangerous one: the old bare `X` took the entire subtree with a
  // single click, no confirm, no undo.
  it("CONFIRMS before deleting a block that has children", () => {
    const { spies, materialize } = renderPagesView({ activePageBlocks: NESTED });
    materialize("removePageBlock");
    openMenu(1); // the toggle, which holds a child
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));

    const dialog = screen.getByRole("dialog", { name: /delete this block/i });
    expect(spies.removePageBlock).not.toHaveBeenCalled();
    within(dialog).getByText(/1 nested block/);

    fireEvent.click(within(dialog).getByRole("button", { name: /delete block/i }));
    expect(spies.removePageBlock).toHaveBeenCalledWith("a");
  });

  it("reserves no column width on the right — the finalization tray hangs out of the row", () => {
    renderPagesView();
    const row = screen
      .getByLabelText("Edit paragraph block 1")
      .closest('[style*="margin-left"]') as HTMLElement;
    // the old tray was an in-flow flex item with `min-width: 44px` on EVERY row.
    expect(row.innerHTML).not.toContain("min-width: 44px");
    expect(row.querySelector('[style*="left: 100%"]')).not.toBeNull();
  });
});

describe("breadcrumbs", () => {
  const NESTED_PAGES = {
    pages: [
      { id: "top", title: "Handbook", parent: null },
      { id: "mid", title: "Engineering", parent: "top" },
      { id: "p1", title: "Launch plan", parent: "mid" },
    ],
  };

  it("renders the TRUE ancestry, not a hardcoded 'Pages / <title>'", () => {
    renderPagesView(NESTED_PAGES);
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(
      within(crumbs)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Handbook", "Engineering", "Launch plan"]);
  });

  it("opens an ancestor from its segment; the current page is not a link", () => {
    const { spies } = renderPagesView(NESTED_PAGES);
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    fireEvent.click(within(crumbs).getByRole("button", { name: "Engineering" }));
    expect(spies.openPage).toHaveBeenCalledWith("mid");
    expect(within(crumbs).getByRole("button", { name: "Launch plan" })).toBeDisabled();
  });
});

describe("pasting a document", () => {
  const pasteInto = (label: string, text: string, selection?: [number, number]) => {
    const area = screen.getByLabelText(label) as HTMLTextAreaElement;
    fireEvent.focus(area);
    if (selection) area.setSelectionRange(selection[0], selection[1]);
    fireEvent.paste(area, { clipboardData: { getData: () => text } });
    return area;
  };

  it("splits a markdown paste into blocks instead of dumping literal newlines", () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["e"] }),
        blockOf({ id: "e", text: "" }),
      ],
    });
    pasteInto("Edit paragraph block 1", "# Title\n\nintro\n- one\n- two");

    // the empty paragraph adopts the first line — kind and text.
    expect(spies.setPageBlockKind).toHaveBeenCalledWith({ blockId: "e", kind: "heading1" });
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "e", text: "Title" });
    // the rest become their own blocks, in order, each with its own kind.
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(3);
    expect(spies.insertPageBlock).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ kind: "paragraph", text: "intro" }),
    );
    expect(spies.insertPageBlock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ kind: "bulleted", text: "one" }),
    );
    expect(spies.insertPageBlock).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({ kind: "bulleted", text: "two" }),
    );
  });

  it("leaves a single-line paste to the browser", () => {
    const { spies, materialize } = renderPagesView();
    materialize("insertPageBlock");
    pasteInto("Edit paragraph block 1", "just some words");
    expect(spies.insertPageBlock).not.toHaveBeenCalled();
  });

  it("keeps marks with the original text around a multi-block paste", async () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["e"] }),
        blockOf({
          id: "e",
          text: "AA middle ZZ",
          marks: [
            { start: 0, end: 2, kind: "bold" },
            { start: 10, end: 12, kind: "italic" },
          ],
        }),
      ],
      pageThreads: [{
        target: "e",
        threads: [{
          thread: {
            id: "tail-thread", target: "e", opener: { user: [1] }, created_at: 1,
            anchor: { start: 10, end: 12 }, resolved: false, resolved_by: null,
            comment_ids: ["tail-comment"],
          },
          comments: [],
        }],
      }],
    });
    pasteInto("Edit paragraph block 1", "one\ntwo", [3, 9]);
    await settle();

    expect(spies.updatePageBlockText).toHaveBeenCalledWith({
      blockId: "e",
      text: "AA one",
      marks: [{ start: 0, end: 2, kind: "bold" }],
    });
    expect(spies.insertPageBlock).toHaveBeenCalledWith(expect.objectContaining({
      text: "two ZZ",
      marks: [{ start: 4, end: 6, kind: "italic" }],
    }));
    const inserted = (spies.insertPageBlock as unknown as Mock).mock.calls[0][0];
    expect(spies.moveCommentThread).toHaveBeenCalledWith({
      threadId: "tail-thread",
      target: inserted.blockId,
      anchor: { start: 4, end: 6 },
    });
  });

  it("keeps an anchored tail in the source when a pasted block fails", async () => {
    const { spies, fails, materialize } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["e"] }),
        blockOf({ id: "e", text: "head tail" }),
      ],
      pageThreads: [{
        target: "e",
        threads: [{
          thread: {
            id: "tail-thread", target: "e", opener: { user: [1] }, created_at: 1,
            anchor: { start: 5, end: 9 }, resolved: false, resolved_by: null,
            comment_ids: ["tail-comment"],
          },
          comments: [],
        }],
      }],
    });
    materialize("updatePageBlockText", "moveCommentThread");
    fails("insertPageBlock");
    pasteInto("Edit paragraph block 1", "one\ntwo", [5, 5]);
    await settle();

    expect(spies.updatePageBlockText).not.toHaveBeenCalled();
    expect(spies.moveCommentThread).not.toHaveBeenCalled();
  });

  it("caps the burst and says so — every block is one consensus write", () => {
    const { spies } = renderPagesView();
    pasteInto(
      "Edit paragraph block 1",
      Array.from({ length: MAX_PASTE_BLOCKS + 5 }, (_, i) => `line ${i}`).join("\n"),
    );
    // first line lands in the row itself; the other 59 are inserts.
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(MAX_PASTE_BLOCKS - 1);
    screen.getByRole("status");
    expect(screen.getByRole("status").textContent).toMatch(/5 more lines were dropped/);
  });
});

describe("toggle collapse", () => {
  const TOGGLES: PageBlock[] = [
    blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["t", "e"] }),
    blockOf({ id: "t", kind: "toggle", text: "Parent", children: ["c"] }),
    blockOf({ id: "c", parent: "t", text: "Child" }),
    blockOf({ id: "e", kind: "toggle", text: "Childless" }),
  ];

  it("shows a chevron only on a toggle that has something to hide", () => {
    renderPagesView({ activePageBlocks: TOGGLES });
    screen.getByRole("button", { name: /collapse toggle block 1/i });
    expect(screen.queryByRole("button", { name: /toggle block 3/i })).toBeNull();
  });

  it("persists the collapsed set per page across a remount", () => {
    localStorage.clear();
    const first = renderPagesView({ activePageBlocks: TOGGLES });
    fireEvent.click(screen.getByRole("button", { name: /collapse toggle block 1/i }));
    expect(screen.queryByLabelText("Edit paragraph block 2")).toBeNull();
    first.unmount();

    // a remount used to re-expand every toggle in the document.
    renderPagesView({ activePageBlocks: TOGGLES });
    expect(screen.queryByLabelText("Edit paragraph block 2")).toBeNull();
    screen.getByRole("button", { name: /expand toggle block 1/i });
  });
});

describe("the page icon", () => {
  it("shows the title's leading emoji as an icon, and edits the title without it", () => {
    renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "🦆 Launch plan", children: [] }),
      ],
    });
    expect(screen.getByLabelText("Page title")).toHaveValue("Launch plan");
    expect(screen.getByRole("button", { name: "Change page icon" }).textContent).toBe("🦆");
  });

  it("composes the icon back onto the title when the rename commits", () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "🦆 Launch plan", children: [] }),
      ],
    });
    const title = screen.getByLabelText("Page title");
    fireEvent.focus(title);
    fireEvent.change(title, { target: { value: "Launch plan v2" } });
    fireEvent.blur(title);
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({
      blockId: "p1",
      text: "🦆 Launch plan v2",
    });
  });

  it("takes the icon off again — the input alone could never reach it", () => {
    const { spies } = renderPagesView({
      activePageBlocks: [
        blockOf({ id: "p1", parent: null, kind: "page", text: "🦆 Launch plan", children: [] }),
      ],
    });
    // the affordance shows on hover, like Notion's.
    fireEvent.mouseOver(screen.getByLabelText("Page title"));
    fireEvent.click(screen.getByRole("button", { name: "Remove page icon" }));
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "p1", text: "Launch plan" });
  });

  const pageTitled = (text: string) => ({
    activePageBlocks: [blockOf({ id: "p1", parent: null, kind: "page", text, children: [] })],
  });

  // The icon is derived from the STORE every render; the draft is a local copy of
  // an older store. Type an emoji and it commits verbatim — and now it is BOTH
  // the store's leading emoji (the icon) and still in the focused draft, which the
  // draft-protection guard refuses to resync. commit() then composed one onto the
  // other, and did it again at every boundary: "🚀 plan" -> "🚀 🚀 plan" -> …
  it("does not double an emoji typed into the title", () => {
    vi.useFakeTimers();
    try {
      const { spies, rerender } = renderPagesView(pageTitled("Launch plan"));
      const title = screen.getByLabelText("Page title");
      fireEvent.focus(title);
      fireEvent.change(title, { target: { value: "🚀 Launch plan" } });
      act(() => {
        vi.advanceTimersByTime(EDIT_BOUNDARY_MS);
      });
      expect(spies.updatePageBlockText).toHaveBeenCalledWith({
        blockId: "p1",
        text: "🚀 Launch plan",
      });

      // the commit lands: the store now reads the emoji as the page's ICON, while
      // the still-focused input keeps it in the draft. This is the exact state the
      // old commit doubled from.
      rerender(pageTitled("🚀 Launch plan"));
      act(() => {
        vi.advanceTimersByTime(EDIT_BOUNDARY_MS * 3);
      });
      expect(spies.updatePageBlockText).toHaveBeenCalledTimes(1);
      expect(spies.updatePageBlockText).not.toHaveBeenCalledWith(
        expect.objectContaining({ text: "🚀 🚀 Launch plan" }),
      );

      // and leaving the field moves it out of the input for good — the icon
      // button shows it, the input holds the title.
      fireEvent.blur(title);
      expect(title).toHaveValue("Launch plan");
      expect(spies.updatePageBlockText).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  // splitTitleEmoji eats the whitespace after the emoji and composeTitle re-emits
  // exactly one space, so the round-trip is NOT the identity for "🦆Launch plan".
  // The boundary condition was `composeTitle(icon, draft) !== raw`, which is
  // therefore true on MOUNT — merely opening the page renamed it, with no
  // keystroke anywhere near the input.
  it("never rewrites a title on open, however it is spaced", () => {
    vi.useFakeTimers();
    try {
      const { spies, materialize } = renderPagesView(pageTitled("🦆Launch plan"));
      materialize("updatePageBlockText");
      act(() => {
        vi.advanceTimersByTime(EDIT_BOUNDARY_MS * 3);
      });
      expect(spies.updatePageBlockText).not.toHaveBeenCalled();

      // a focus and a blur are not an edit either.
      const title = screen.getByLabelText("Page title");
      fireEvent.focus(title);
      fireEvent.blur(title);
      expect(spies.updatePageBlockText).not.toHaveBeenCalled();
      // it still READS as an icon + title, it is only not rewritten.
      expect(screen.getByRole("button", { name: "Change page icon" }).textContent).toBe("🦆");
      expect(title).toHaveValue("Launch plan");
    } finally {
      vi.useRealTimers();
    }
  });
});

// RemoveBlock takes the whole subtree with it. Every path that reaches it —
// the menu, Backspace on an empty block, the merge — has to reckon with that,
// or a keystroke silently destroys nested work.
describe("no delete path quietly eats a subtree", () => {
  const PARENTED: PageBlock[] = [
    blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["a", "b"] }),
    blockOf({ id: "a", text: "First" }),
    blockOf({ id: "b", kind: "toggle", text: "", children: ["b1"] }),
    blockOf({ id: "b1", parent: "b", text: "buried note" }),
  ];

  it("CONFIRMS a Backspace that would delete an empty block holding children", () => {
    const { spies, materialize } = renderPagesView({ activePageBlocks: PARENTED });
    materialize("removePageBlock");
    const area = screen.getByLabelText("Edit toggle block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    fireEvent.keyDown(area, { key: "Backspace" });

    expect(spies.removePageBlock).not.toHaveBeenCalled();
    const dialog = screen.getByRole("dialog", { name: /delete this block/i });
    fireEvent.click(within(dialog).getByRole("button", { name: /delete block/i }));
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
  });

  it("hands the children over before merging a block away", async () => {
    const withText: PageBlock[] = [
      ...PARENTED.slice(0, 2),
      blockOf({ id: "b", text: "second", children: ["b1"] }),
      blockOf({ id: "b1", parent: "b", text: "buried note" }),
    ];
    const { spies } = renderPagesView({ activePageBlocks: withText });
    const area = screen.getByLabelText("Edit paragraph block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 0);
    fireEvent.keyDown(area, { key: "Backspace" });
    await settle();

    // the text merges up…
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "Firstsecond" });
    // …and the child follows it, instead of dying with its old parent.
    expect(spies.movePageBlock).toHaveBeenCalledWith({
      blockId: "b1",
      parent: "a",
      after: null,
    });
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
  });

  // Reported by a live-QA pass: merging a parent with TWO children lost the
  // second one. Both children ride an anchor chain (child 2 lands `after` child
  // 1) and the remove has to follow both — and the wire orders NOTHING, so the
  // three ops raced. RemoveBlock reaching the node before child 2's move took
  // child 2 down with the subtree.
  const TWO_CHILDREN: PageBlock[] = [
    blockOf({ id: "p1", parent: null, kind: "page", text: "T", children: ["a", "b"] }),
    blockOf({ id: "a", text: "First" }),
    blockOf({ id: "b", text: "second", children: ["b1", "b2"] }),
    blockOf({ id: "b1", parent: "b", text: "child one" }),
    blockOf({ id: "b2", parent: "b", text: "child two" }),
  ];

  it("adopts BOTH children of a merged parent, in order, and removes it only after", async () => {
    const { spies, materialize } = renderPagesView({ activePageBlocks: TWO_CHILDREN });
    materialize("movePageBlock", "removePageBlock");
    const area = screen.getByLabelText("Edit paragraph block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 0);
    fireEvent.keyDown(area, { key: "Backspace" });

    // NOTHING destructive goes out in this tick. The ops used to be fired off
    // together and left to race — which is the bug: each one's anchor is the op
    // before it, and the wire orders nothing. Every one of these waits for the op
    // it depends on to actually LAND.
    expect(spies.movePageBlock).not.toHaveBeenCalled();
    expect(spies.removePageBlock).not.toHaveBeenCalled();
    await settle();

    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "Firstsecond" });
    // CHILD2 is the one that went missing. Its anchor is CHILD1, so it can only
    // be issued once CHILD1 has actually landed.
    expect(spies.movePageBlock).toHaveBeenNthCalledWith(1, {
      blockId: "b1",
      parent: "a",
      after: null,
    });
    expect(spies.movePageBlock).toHaveBeenNthCalledWith(2, {
      blockId: "b2",
      parent: "a",
      after: "b1",
    });
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
    // and the remove is issued LAST — it deletes a subtree, so it can never
    // precede the moves that empty it.
    expect(Math.max(...callOrder(spies.movePageBlock))).toBeLessThan(
      callOrder(spies.removePageBlock)[0],
    );
  });

  it("keeps the merged block when a child's adoption never lands", async () => {
    const { spies, fails, materialize } = renderPagesView({ activePageBlocks: TWO_CHILDREN });
    materialize("removePageBlock");
    fails("movePageBlock");
    const area = screen.getByLabelText("Edit paragraph block 2") as HTMLTextAreaElement;
    fireEvent.focus(area);
    area.setSelectionRange(0, 0);
    fireEvent.keyDown(area, { key: "Backspace" });
    await settle();

    // the chain stops at the first move that did not land: child 2's anchor was
    // never created, so issuing its move would only add a second rejection.
    expect(spies.movePageBlock).toHaveBeenCalledTimes(1);
    // and the block is NOT removed — its children are still under it, and
    // RemoveBlock takes the whole subtree. A duplicate row is recoverable.
    expect(spies.removePageBlock).not.toHaveBeenCalled();
  });

});

// The headline of this whole change: an op that does not land must never cost
// the user text. Every additive op is compensated, and the compensation had
// never once been exercised — `submitTracked` resolving false is the seam, and
// the suite could not reach it until `fails()` existed.
describe("a write that never lands does not eat your text", () => {
  const caretAt = (area: HTMLTextAreaElement, at: number) => {
    fireEvent.focus(area);
    area.setSelectionRange(at, at);
  };

  it("restores the whole line when the split's insert never lands", async () => {
    const { spies, fails } = renderPagesView();
    fails("insertPageBlock");
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    caretAt(area, 5); // "First| draft"
    fireEvent.keyDown(area, { key: "Enter" });

    // the truncation commits immediately — the right half now lives ONLY in the
    // insert that is about to fail.
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "First" });
    await settle();
    // …so when it fails, the whole line goes back. A failed op is never rolled
    // back — it is erased by the next authoritative refresh — so without this the
    // right half is gone for good.
    expect(spies.updatePageBlockText).toHaveBeenLastCalledWith({
      blockId: "a",
      text: "First draft",
    });
  });

  it("does not compensate a split that DID land", async () => {
    const { spies } = renderPagesView();
    const area = screen.getByLabelText("Edit paragraph block 1") as HTMLTextAreaElement;
    caretAt(area, 5);
    fireEvent.keyDown(area, { key: "Enter" });
    await settle();

    // one truncation, and no "restore" behind it — a compensation that always
    // fires would resurrect the right half as a duplicate on every Enter.
    expect(spies.updatePageBlockText).toHaveBeenCalledTimes(1);
    expect(spies.updatePageBlockText).toHaveBeenCalledWith({ blockId: "a", text: "First" });
  });

  it("puts a merged-away block back when the merge never lands", async () => {
    const { spies, fails } = renderPagesView();
    fails("updatePageBlockText");
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 0);
    fireEvent.keyDown(area, { key: "Backspace" });

    // the block is removed on the spot: the row has to vanish under the caret.
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
    await settle();
    // its text went nowhere, so the block comes back — kind, text and all. (The
    // wire is a FIFO, so this re-insert reaches the node behind the remove and
    // the id is free.)
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ blockId: "b", parent: "p1", after: "a", kind: "todo", text: "Ship it" }),
    );
  });

  it("does not resurrect a merged-away block when the merge DID land", async () => {
    const { spies, materialize } = renderPagesView();
    materialize("insertPageBlock");
    const area = screen.getByLabelText("Edit todo block 2") as HTMLTextAreaElement;
    caretAt(area, 0);
    fireEvent.keyDown(area, { key: "Backspace" });
    await settle();

    expect(spies.removePageBlock).toHaveBeenCalledWith("b");
    expect(spies.insertPageBlock).not.toHaveBeenCalled();
  });
});

describe("drag to reorder", () => {
  // jsdom has NO layout engine: getBoundingClientRect is all zeros, so the
  // before/after edge cannot be exercised here (page-drag.test.ts covers that
  // geometry as a pure function). What this proves is the WIRING — that the
  // handle's drag reaches MoveBlock at all, and as exactly ONE op.
  const dataTransfer = () => {
    const store = new Map<string, string>();
    return {
      effectAllowed: "",
      dropEffect: "",
      types: ["application/x-ducktape-block"],
      setData: (type: string, value: string) => store.set(type, value),
      getData: (type: string) => store.get(type) ?? "",
    };
  };

  it("moves the dragged block with a single MoveBlock op", () => {
    const { spies } = renderPagesView();
    const dt = dataTransfer();
    const rowOf = (label: string) =>
      screen.getByLabelText(label).closest('[style*="margin-left"]') as HTMLElement;

    // drag block 1 ("First draft") down onto block 2 ("Ship it").
    fireEvent.dragStart(screen.getByRole("button", { name: "Block 1 actions" }), {
      dataTransfer: dt,
    });
    fireEvent.dragOver(rowOf("Edit todo block 2"), { dataTransfer: dt });
    fireEvent.drop(rowOf("Edit todo block 2"), { dataTransfer: dt });

    // ONE op for the whole gesture — never one per dragover.
    expect(spies.movePageBlock).toHaveBeenCalledTimes(1);
    expect(spies.movePageBlock).toHaveBeenCalledWith({
      blockId: "a",
      parent: "p1",
      after: "b",
    });
  });

  it("does not submit an op for a drop that changes nothing", () => {
    const { spies, materialize } = renderPagesView();
    materialize("movePageBlock");
    const dt = dataTransfer();
    const row = screen
      .getByLabelText("Edit paragraph block 1")
      .closest('[style*="margin-left"]') as HTMLElement;
    // block 2 dropped below block 1 — it is already there.
    fireEvent.dragStart(screen.getByRole("button", { name: "Block 2 actions" }), {
      dataTransfer: dt,
    });
    fireEvent.dragOver(row, { dataTransfer: dt });
    fireEvent.drop(row, { dataTransfer: dt });
    expect(spies.movePageBlock).not.toHaveBeenCalled();
  });

  it("ignores a drag that is not one of our blocks", () => {
    const { spies, materialize } = renderPagesView();
    materialize("movePageBlock");
    const foreign = { ...dataTransfer(), types: ["Files"] };
    const row = screen
      .getByLabelText("Edit paragraph block 1")
      .closest('[style*="margin-left"]') as HTMLElement;
    fireEvent.drop(row, { dataTransfer: foreign });
    expect(spies.movePageBlock).not.toHaveBeenCalled();
  });
});

// RemoveBlock takes the whole subtree and consensus has no undo of its own, so
// the view keeps a snapshot and replays it as inserts. Three ways that goes
// wrong, all of them found in review, none of them corrupting state:
describe("the delete-undo toast", () => {
  const openMenu = (n: number) =>
    fireEvent.click(screen.getByRole("button", { name: `Block ${n} actions` }));
  const deleteBlock = (n: number) => {
    openMenu(n);
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
  };
  const undo = () => fireEvent.click(screen.getByRole("button", { name: "Undo" }));

  it("replays the subtree with its ids, kind and position", async () => {
    const { spies } = renderPagesView();
    deleteBlock(2); // the childless to-do
    expect(spies.removePageBlock).toHaveBeenCalledWith("b");

    undo();
    await settle();
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ blockId: "b", parent: "p1", after: "a", kind: "todo", text: "Ship it" }),
    );
  });

  // SetKind leaves `checked` set in the module, so a to-do that was ticked and
  // then converted to text still carries the bit. Replaying it is a SetChecked
  // on a paragraph — NotTodo, an error toast on a restore that WORKED.
  it("does not replay `checked` on a block that is no longer a to-do", async () => {
    const converted: PageBlock[] = [
      blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["a"] }),
      blockOf({ id: "a", kind: "paragraph", text: "was a to-do", checked: true }),
    ];
    const { spies, materialize } = renderPagesView({ activePageBlocks: converted });
    materialize("setPageBlockChecked");
    deleteBlock(1);

    undo();
    await settle();
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ blockId: "a", kind: "paragraph" }),
    );
    expect(spies.setPageBlockChecked).not.toHaveBeenCalled();
  });

  // this view survives a doc switch, and InsertBlock is page-agnostic: the undo
  // used to restore the subtree into the other document, off-screen, so the
  // click read as a no-op.
  it("goes back to the page the block was deleted from before restoring", async () => {
    const { spies, rerender } = renderPagesView();
    deleteBlock(2);
    rerender({ activePage: "p2", activePageBlocks: [] });

    undo();
    await settle();
    expect(spies.openPage).toHaveBeenCalledWith("p1");
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ blockId: "b", parent: "p1" }),
    );
  });

  // the anchor or the parent can be gone by the time Undo is clicked (someone
  // else deleted it). Every op in the batch then rejects, and each rejection
  // used to raise its own toast while nothing came back.
  it("reports a restore that cannot land ONCE, instead of one toast per block", async () => {
    const nested: PageBlock[] = [
      blockOf({ id: "p1", parent: null, kind: "page", text: "Launch plan", children: ["a"] }),
      blockOf({ id: "a", kind: "toggle", text: "Parent", children: ["a1"] }),
      blockOf({ id: "a1", parent: "a", text: "Child" }),
    ];
    const { spies, fails } = renderPagesView({ activePageBlocks: nested });
    fails("insertPageBlock");
    deleteBlock(1); // has a child, so it asks first
    fireEvent.click(
      within(screen.getByRole("dialog", { name: /delete this block/i })).getByRole("button", {
        name: /delete block/i,
      }),
    );

    undo();
    await settle();
    // the root op did not land, so the two ops chained behind it never went out
    // (they would each have been rejected, and each have raised its own toast).
    expect(spies.insertPageBlock).toHaveBeenCalledTimes(1);
    expect(spies.insertPageBlock).toHaveBeenCalledWith(expect.objectContaining({ quiet: true }));
    // …and the failure is reported, once, in the reader's words.
    within(screen.getByRole("alert")).getByText(/couldn't restore the block/i);
  });
});
