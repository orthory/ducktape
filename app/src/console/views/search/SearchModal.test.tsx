import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ChatSearchHit } from "../../../domain/chat-client";
import type { Manifest } from "../../../domain/files-client";
import type { PageSearchHit } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { SearchModal } from "./SearchModal";

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

const fileOf = (name: string, id: string): Manifest => ({
  file_id: id,
  name,
  mime: "text/plain",
  size: 1,
  chunk_size: 1,
  chunks: [],
  digest: "",
  owner: "",
  created_at_height: 0,
});

const renderModal = (patch: Partial<ConsoleState> = {}) => {
  const state: ConsoleState = {
    ...createInitialState(),
    members: ["aa".repeat(32), "bb".repeat(32)],
    authorNames: { ["aa".repeat(32)]: "Alice", ["bb".repeat(32)]: "Bob" },
    files: [fileOf("roadmap.md", "f1"), fileOf("budget.csv", "f2")],
    searchOpen: true,
    ...patch,
  };
  const { actions, spies } = makeActions();
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <SearchModal />
    </ConsoleContext.Provider>,
  );
  return { spies };
};

describe("SearchModal", () => {
  it("prompts before any input", () => {
    renderModal();
    expect(screen.getByText(/Type to search/i)).toBeInTheDocument();
  });

  it("filters members client-side by name", () => {
    renderModal();
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "ali" } });
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByText("Bob")).not.toBeInTheDocument();
  });

  it("filters files client-side by filename and navigates on click", () => {
    const { spies } = renderModal();
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "roadmap" } });
    const hit = screen.getByText("roadmap.md");
    expect(hit).toBeInTheDocument();
    expect(screen.queryByText("budget.csv")).not.toBeInTheDocument();
    fireEvent.click(hit);
    expect(spies.setScreen).toHaveBeenCalledWith("files");
    expect(spies.closeSearch).toHaveBeenCalled();
  });

  it("renders node-index chat and docs groups from state.search", () => {
    renderModal({
      search: {
        query: "ship",
        chat: [
          { channelId: "general", seq: 3, author: "Alice", edited: false, text: "ship it" } as ChatSearchHit,
        ],
        docs: [
          { pageId: "p1", blockId: "b1", kind: "paragraph", text: "shipping plan" } as PageSearchHit,
        ],
      },
    });
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "ship" } });
    expect(screen.getByText("ship it")).toBeInTheDocument();
    expect(screen.getByText("shipping plan")).toBeInTheDocument();
  });

  it("hides node-index results whose query does not match the current input", () => {
    renderModal({
      search: {
        query: "old",
        chat: [
          { channelId: "general", seq: 1, author: "Alice", edited: false, text: "stale hit" } as ChatSearchHit,
        ],
        docs: [],
      },
    });
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "new" } });
    // the seeded "old" results must not leak into the "new" query
    expect(screen.queryByText("stale hit")).not.toBeInTheDocument();
    expect(screen.getByText(/Searching/i)).toBeInTheDocument();
    expect(screen.queryByText(/Nothing matches/i)).not.toBeInTheDocument();
  });

  it("shows 'Searching…' — not a false empty state — while the query is unresolved", () => {
    renderModal();
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "zzz" } });
    expect(screen.getByText(/Searching/i)).toBeInTheDocument();
    expect(screen.queryByText(/Nothing matches/i)).not.toBeInTheDocument();
  });

  it("closes on Escape", () => {
    const { spies } = renderModal();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(spies.closeSearch).toHaveBeenCalled();
  });
});
