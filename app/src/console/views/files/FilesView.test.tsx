import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Manifest } from "../../../domain/files-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { FilesView } from "./FilesView";

const files: Manifest[] = [
  {
    file_id: "file-report-123456",
    name: "quarterly-report.pdf",
    mime: "application/pdf",
    size: 348_160,
    chunk_size: 262_144,
    chunks: ["aa".repeat(32), "bb".repeat(32)],
    digest: "cc".repeat(32),
    owner: "0123456789abcdef",
    created_at_height: 42,
  },
];

const renderFiles = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "files", root: "bb".repeat(32) }],
    },
    files,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const noop = vi.fn() as (...args: unknown[]) => void;

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn() as (...args: unknown[]) => void;
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <FilesView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("FilesView", () => {
  it("lists a manifest and deletes it through a two-step confirm", () => {
    const { spies } = renderFiles();

    expect(screen.getByText("quarterly-report.pdf")).toBeInTheDocument();

    const deleteButton = screen.getByRole("button", { name: /^delete quarterly-report\.pdf$/i });
    fireEvent.click(deleteButton);
    expect(screen.queryByText("quarterly-report.pdf")).toBeInTheDocument();

    const confirmButton = screen.getByRole("button", {
      name: /confirm delete quarterly-report\.pdf/i,
    });
    fireEvent.click(confirmButton);

    expect(spies.removeFile).toHaveBeenCalledWith("file-report-123456");
  });

  it("is honest when the files module is not backed by the node", () => {
    renderFiles({
      files: [],
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(screen.getByText(/files module is not available/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /upload/i })).toBeDisabled();
  });
});
