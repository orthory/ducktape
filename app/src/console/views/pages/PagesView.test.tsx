import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
  return {
    spies,
    materialize,
    unmount,
    rerender: (p: Partial<ConsoleState>) => rerender(view(p)),
  };
};

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

  it("still opens the aside panel from the header Comments toggle", () => {
    renderPagesView();
    fireEvent.click(screen.getByRole("button", { name: "Show comments" }));
    screen.getByRole("complementary", { name: "Comments" });
    expect(screen.queryByRole("dialog", { name: /comments on/i })).toBeNull();
  });

  it("hover reveals the comment affordance but no copy-block-link", () => {
    renderPagesView();
    fireEvent.mouseOver(screen.getByLabelText("Edit paragraph block 1"));
    screen.getByRole("button", { name: "Comment on block 1" });
    expect(screen.queryByRole("button", { name: /copy link to block/i })).toBeNull();
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

  it("hands the children over before merging a block away", () => {
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
