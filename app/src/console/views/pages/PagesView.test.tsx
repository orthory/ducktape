import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { PageBlock } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { PagesView } from "./PagesView";

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
  const state = {
    ...createInitialState(),
    pages: [
      { id: "p1", title: "Launch plan", parent: null },
      { id: "p2", title: "Retro", parent: null },
    ],
    activePage: "p1",
    activePageBlocks: PAGE,
    ...patch,
  };
  const { actions, spies } = makeActions();
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <PagesView />
    </ConsoleContext.Provider>,
  );
  return { spies };
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

  it("writes a page comment through the panel composer (no window.prompt)", () => {
    const { spies } = renderPagesView();

    fireEvent.click(screen.getByRole("button", { name: "Comment on page" }));
    // the panel opens with the composer aimed at the page.
    screen.getByRole("form", { name: "New comment on this page" });

    fireEvent.change(screen.getByLabelText("New comment text"), {
      target: { value: "ship checklist looks thin" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add comment" }));

    expect(spies.addComment).toHaveBeenCalledWith({
      target: "p1",
      text: "ship checklist looks thin",
    });
    // submit dismisses the composer; the panel itself stays open.
    expect(screen.queryByRole("form", { name: "New comment on this page" })).toBeNull();
    screen.getByRole("complementary", { name: "Comments" });
  });
});
