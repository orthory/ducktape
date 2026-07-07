import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { EDIT_BOUNDARY_MS, PagesView } from "./PagesView";

const makeActions = () => {
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        spies[key] ??= vi.fn() as (...args: unknown[]) => void;
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
  const { rerender } = render(view(patch));
  return { spies, rerender: (p: Partial<ConsoleState>) => rerender(view(p)) };
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

  it("leaves ⌘W to the window — it never closes a doc tab or creates a page", () => {
    const { spies } = withTabs();
    fireEvent.keyDown(document, { code: "KeyW", metaKey: true });
    expect(spies.closeTab).not.toHaveBeenCalled();
    expect(spies.openPage).not.toHaveBeenCalled();
    // createChildPage is spied lazily on first access; ⌘W must never reach it,
    // so the spy stays undefined (and if present, uncalled).
    if (spies.createChildPage) expect(spies.createChildPage).not.toHaveBeenCalled();
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
    expect(screen.getByTestId("doc-scroll")).toHaveStyle({ background: "#ffffff" });
  });
});
