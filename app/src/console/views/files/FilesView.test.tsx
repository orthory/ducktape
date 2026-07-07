import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { FileEntry, FilePage } from "../../../domain/files-client";
import type { NodeTransport } from "../../../domain/transport";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { FilesView } from "./FilesView";

const HEAD_ID = "88".repeat(32);
const SNAP_ID = "77".repeat(32);

const fileEntry = (path: string, size = 4): FileEntry => ({
  path,
  kind: "file",
  size,
  exec: false,
  object: "ab".repeat(32),
  meta: { mime: "text/plain" },
});
const dirEntry = (path: string): FileEntry => ({
  path,
  kind: "dir",
  size: 0,
  exec: false,
  object: "cd".repeat(32),
  meta: {},
});

// The browser opens at /shared; /shared/docs is a nested folder.
const lsByPath: Record<string, FilePage> = {
  "/shared": { entries: [dirEntry("/shared/docs"), fileEntry("/shared/readme.md")], next: null },
  "/shared/docs": { entries: [fileEntry("/shared/docs/plan.md")], next: null },
};

const makeTransport = (overrides: Partial<NodeTransport> = {}): NodeTransport => ({
  submit: vi.fn(),
  query: vi.fn().mockResolvedValue({ refs: { head: HEAD_ID, pins: {}, window_len: 1 } }),
  view: vi.fn(),
  putBlob: vi.fn(),
  getBlob: vi.fn(),
  filesStage: vi.fn().mockResolvedValue({ digest: "ab".repeat(32) }),
  filesCommit: vi.fn().mockResolvedValue({ height: 2, appHash: "aa".repeat(32) }),
  filesStat: vi.fn().mockResolvedValue(null),
  filesLs: vi.fn(({ path }: { path: string }) =>
    Promise.resolve(lsByPath[path] ?? { entries: [], next: null }),
  ),
  filesRead: vi.fn().mockResolvedValue({ b64: btoa("readme body"), eof: true }),
  filesHistory: vi.fn().mockResolvedValue([
    {
      id: SNAP_ID,
      parent: null,
      root_tree: "aa".repeat(32),
      author: "me",
      height: 1,
      consensus_time: 1_700_000_000,
      message: "initial commit",
    },
  ]),
  status: vi.fn(),
  metrics: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
  ...overrides,
});

const backedStatus = {
  version: "0.1.0",
  appHash: "aa".repeat(32),
  height: 8,
  modules: [{ id: "files", root: "bb".repeat(32) }],
};

const renderView = (transport: NodeTransport | null, patch: Partial<ConsoleState> = {}) => {
  const state: ConsoleState = {
    ...createInitialState(),
    connected: true,
    status: backedStatus,
    ...patch,
  };
  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  const actions = new Proxy(
    {},
    { get: (_t, key: string) => (spies[key] ??= vi.fn()) },
  ) as unknown as ConsoleActions;
  return render(
    <ConsoleContext.Provider value={{ state, actions, transport }}>
      <FilesView />
    </ConsoleContext.Provider>,
  );
};

describe("FilesView", () => {
  it("lists the current directory's folders and files", async () => {
    renderView(makeTransport());
    expect(await screen.findByText("docs")).toBeInTheDocument();
    expect(screen.getByText("readme.md")).toBeInTheDocument();
  });

  it("navigates into a folder and lists its entries", async () => {
    const transport = makeTransport();
    renderView(transport);
    fireEvent.click(await screen.findByRole("button", { name: /open folder docs/i }));
    expect(await screen.findByText("plan.md")).toBeInTheDocument();
    expect(transport.filesLs).toHaveBeenCalledWith(
      expect.objectContaining({ path: "/shared/docs" }),
    );
  });

  it("opens a file panel with a text preview and a download control", async () => {
    const transport = makeTransport({
      filesRead: vi.fn().mockResolvedValue({ b64: btoa("hello duckfs"), eof: true }),
    });
    renderView(transport);
    fireEvent.click(await screen.findByRole("button", { name: /open file readme\.md/i }));
    expect(await screen.findByText("hello duckfs")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /download readme\.md/i })).toBeInTheDocument();
  });

  it("uploads a chosen file into the current directory, then reloads", async () => {
    const transport = makeTransport();
    const { container } = renderView(transport);
    await screen.findByText("readme.md");

    const file = new File(["hi"], "notes.txt", { type: "text/plain" });
    // jsdom's File has no arrayBuffer(); stub it for the reader.
    Object.defineProperty(file, "arrayBuffer", {
      value: () => Promise.resolve(new TextEncoder().encode("hi").buffer),
    });
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.changes[0].put.path).toBe("/shared/notes.txt");
    // a small file is inlined, never staged.
    expect(transport.filesStage).not.toHaveBeenCalled();
  });

  it("creates new folders under /shared even before /shared exists", async () => {
    const transport = makeTransport({
      filesLs: vi.fn(({ path }: { path: string }) =>
        path === "/shared"
          ? Promise.reject(new Error("missing"))
          : Promise.resolve({ entries: [], next: null }),
      ),
    });
    const promptSpy = vi.spyOn(window, "prompt").mockReturnValue("docs");
    try {
      renderView(transport);
      await screen.findByText("Empty directory");

      fireEvent.click(screen.getByRole("button", { name: /new folder/i }));

      await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
      const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
      expect(body.changes).toEqual([{ mkdir: { path: "/shared/docs" } }]);
    } finally {
      promptSpy.mockRestore();
    }
  });

  it("deletes the open file after a two-step confirm", async () => {
    const transport = makeTransport();
    renderView(transport);
    fireEvent.click(await screen.findByRole("button", { name: /open file readme\.md/i }));

    fireEvent.click(await screen.findByRole("button", { name: /^delete readme\.md$/i }));
    fireEvent.click(screen.getByRole("button", { name: /confirm delete readme\.md/i }));

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.changes).toEqual([{ rm: { path: "/shared/readme.md" } }]);
  });

  it("shows history and switches the browse to a selected snapshot", async () => {
    const transport = makeTransport();
    renderView(transport);
    await screen.findByText("readme.md");

    fireEvent.click(screen.getByRole("button", { name: /^history$/i }));
    expect(await screen.findByText("initial commit")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /browse snapshot/i }));
    expect(await screen.findByText("snapshot")).toBeInTheDocument();
    await waitFor(() =>
      expect(transport.filesLs).toHaveBeenCalledWith(
        expect.objectContaining({ snapshot: SNAP_ID }),
      ),
    );
  });

  it("is honest when the files module is not backed by the node", () => {
    renderView(makeTransport(), {
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });
    expect(screen.getByText(/files module is not available/i)).toBeInTheDocument();
  });

  it("prompts to connect when no node is resolved", () => {
    renderView(null);
    expect(screen.getByText(/no node connected/i)).toBeInTheDocument();
  });
});
