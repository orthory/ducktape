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
  kind: "Paragraph",
  text: "",
  checked: false,
  children: [],
  ...patch,
});

const PAGE: PageBlock[] = [
  blockOf({ id: "p1", parent: null, kind: "Page", text: "Launch plan", children: ["a", "b"] }),
  blockOf({ id: "a", text: "First draft" }),
  blockOf({ id: "b", kind: "Todo", text: "Ship it" }),
];

const renderPagesView = (patch: Partial<ConsoleState> = {}) => {
  const state = {
    ...createInitialState(),
    pages: [
      { id: "p1", title: "Launch plan" },
      { id: "p2", title: "Retro" },
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

  it("creates a page from the rail form and opens one from the list", () => {
    const { spies } = renderPagesView();

    fireEvent.change(screen.getByLabelText("New page title"), {
      target: { value: "Architecture" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create page" }));
    expect(spies.createPage).toHaveBeenCalledWith("Architecture");

    fireEvent.click(screen.getByRole("button", { name: "Open Retro" }));
    expect(spies.openPage).toHaveBeenCalledWith("p2");
  });

  it("renders the tree as labelled editors with the title on the root", () => {
    renderPagesView();
    expect(screen.getByLabelText("Page title")).toHaveValue("Launch plan");
    expect(screen.getByLabelText("Edit Paragraph block 1")).toHaveValue("First draft");
    expect(screen.getByLabelText("Edit Todo block 2")).toHaveValue("Ship it");
  });

  it("splits on Enter: a fresh sibling after the current block", () => {
    const { spies } = renderPagesView();
    fireEvent.keyDown(screen.getByLabelText("Edit Paragraph block 1"), {
      key: "Enter",
    });
    expect(spies.insertPageBlock).toHaveBeenCalledWith(
      expect.objectContaining({ parent: "p1", after: "a", kind: "Paragraph" }),
    );
  });

  it("converts a paragraph via a typed markdown prefix", () => {
    const { spies } = renderPagesView();
    fireEvent.change(screen.getByLabelText("Edit Paragraph block 1"), {
      target: { value: "# First draft" },
    });
    expect(spies.setPageBlockKind).toHaveBeenCalledWith({
      blockId: "a",
      kind: "Heading1",
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
    fireEvent.keyDown(screen.getByLabelText("Edit Todo block 2"), {
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
        blockOf({ id: "p1", parent: null, kind: "Page", text: "T", children: ["a", "empty"] }),
        blockOf({ id: "a", text: "keep" }),
        blockOf({ id: "empty", text: "" }),
      ],
    });
    fireEvent.keyDown(screen.getByLabelText("Edit Paragraph block 2"), {
      key: "Backspace",
    });
    expect(spies.removePageBlock).toHaveBeenCalledWith("empty");
  });
});
