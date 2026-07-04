import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { DocumentView } from "./DocumentView";

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

const renderDocumentView = (patch: Partial<ConsoleState> = {}) => {
  const state = {
    ...createInitialState(),
    docIds: ["plan", "retro"],
    activeDoc: "plan",
    activeDocBlocks: [
      { id: "heading-1", kind: "Heading" as const, text: "Launch plan" },
      { id: "body-1", kind: "Paragraph" as const, text: "First draft" },
    ],
    ...patch,
  };
  const { actions, spies } = makeActions();
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <DocumentView />
    </ConsoleContext.Provider>,
  );
  return { spies };
};

describe("DocumentView", () => {
  it("enumerates the document index on mount and via the refresh control", () => {
    const { spies } = renderDocumentView();
    // Fired once on mount so the tree reflects every enumerated doc.
    expect(spies.listDocs).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Refresh documents" }));
    expect(spies.listDocs).toHaveBeenCalledTimes(2);
  });

  it("exposes labelled create/open flows and opens a doc from the tree", () => {
    const { spies } = renderDocumentView();

    fireEvent.change(screen.getByLabelText("Create document id"), {
      target: { value: "Architecture Notes" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create document" }));
    expect(spies.createDoc).toHaveBeenCalledWith("Architecture Notes");

    fireEvent.change(screen.getByLabelText("Open document id"), {
      target: { value: "retro" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open document" }));
    expect(spies.openDoc).toHaveBeenCalledWith("retro");

    // A top-level doc renders as a leaf in the tree; clicking it opens it.
    fireEvent.click(screen.getByRole("button", { name: "Open retro" }));
    expect(spies.openDoc).toHaveBeenCalledWith("retro");
  });

  it("derives a collapsible folder tree from slash-delimited path ids", () => {
    const { spies } = renderDocumentView({
      docIds: ["projects/launch-plan", "projects/retro", "notes"],
      activeDoc: null,
      activeDocBlocks: [],
    });

    // The shared "projects" prefix becomes a folder; children hang under it.
    const collapse = screen.getByRole("button", { name: "Collapse projects" });
    expect(screen.getByRole("button", { name: "Open projects/launch-plan" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open projects/retro" })).toBeTruthy();
    // A single-segment id is a top-level document leaf, not a folder.
    expect(screen.getByRole("button", { name: "Open notes" })).toBeTruthy();

    // Collapsing the folder hides its children.
    fireEvent.click(collapse);
    expect(screen.queryByRole("button", { name: "Open projects/launch-plan" })).toBeNull();
    expect(screen.getByRole("button", { name: "Expand projects" })).toBeTruthy();

    // Leaves still open through the same enterDoc path (full path id).
    fireEvent.click(screen.getByRole("button", { name: "Open notes" }));
    expect(spies.openDoc).toHaveBeenCalledWith("notes");
  });

  it("wires block edit, insert, remove, and move controls to the store actions", () => {
    const { spies } = renderDocumentView();

    const body = screen.getByLabelText("Edit paragraph block 2");
    fireEvent.change(body, { target: { value: "Revised draft" } });
    fireEvent.blur(body);
    expect(spies.updateBlock).toHaveBeenCalledWith({
      blockId: "body-1",
      text: "Revised draft",
    });

    fireEvent.click(screen.getByRole("button", { name: "Insert Code block" }));
    fireEvent.change(screen.getByLabelText("New block text"), {
      target: { value: "bunx tsc --noEmit" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Insert block" }));
    expect(spies.insertBlock).toHaveBeenCalledWith({
      after: "body-1",
      kind: "Code",
      text: "bunx tsc --noEmit",
    });

    fireEvent.click(screen.getByRole("button", { name: "Move block 2 up" }));
    expect(spies.moveBlock).toHaveBeenCalledWith({
      blockId: "body-1",
      after: null,
    });

    fireEvent.click(screen.getByRole("button", { name: "Remove block 2" }));
    expect(spies.removeBlock).toHaveBeenCalledWith("body-1");
  });
});
