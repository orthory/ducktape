import { readFileSync } from "node:fs";
import { join } from "node:path";

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { FileEntry, FilePage } from "../../../domain/files-client";
import type { NodeTransport, TopicHandlers } from "../../../domain/transport";
import { makeTransportStub } from "../../../test/transport-stub";
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
  ...makeTransportStub({
    query: vi.fn().mockResolvedValue({ refs: { head: HEAD_ID, pins: {}, window_len: 1 } }),
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
    ...overrides,
  }),
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

describe("Files desktop drag and drop wiring", () => {
  it("lets the webview receive HTML5 drag/drop events", () => {
    const config = JSON.parse(
      readFileSync(join(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(config.app.windows[0].dragDropEnabled).toBe(false);
  });
});

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
    expect(
      vi.mocked(transport.filesLs).mock.calls.filter(([params]) => params.path === "/shared"),
    ).toHaveLength(1);
  });

  it("reloads only the directory touched by a live file change", async () => {
    let handlers: TopicHandlers | null = null;
    let docsReads = 0;
    let finishReload!: (page: FilePage) => void;
    const transport = makeTransport({
      filesLs: vi.fn(({ path }: { path: string }) => {
        if (path === "/shared/docs" && ++docsReads === 2) {
          return new Promise<FilePage>((resolve) => {
            finishReload = resolve;
          });
        }
        return Promise.resolve(lsByPath[path] ?? { entries: [], next: null });
      }),
      subscribe: vi.fn((_topics, next) => {
        handlers = next;
        return () => {};
      }),
    });
    renderView(transport);
    fireEvent.click(await screen.findByRole("button", { name: /open folder docs/i }));
    await screen.findByText("plan.md");

    await act(async () => {
      handlers?.onTail?.({
        type: "tail",
        topic: "files:watch",
        cursor: "op/2/0",
        item: {
          height: 2,
          time: 2,
          message: "update plan",
          baseSnapshot: HEAD_ID,
          paths: ["/shared/docs/plan.md"],
        },
      });
      await new Promise((resolve) => setTimeout(resolve, 120));
    });

    expect(screen.getByText("Refreshing…")).toBeInTheDocument();
    expect(screen.getByText("plan.md")).toBeInTheDocument();
    expect(
      vi.mocked(transport.filesLs).mock.calls.filter(([params]) => params.path === "/shared"),
    ).toHaveLength(1);
    expect(
      vi.mocked(transport.filesLs).mock.calls.filter(([params]) => params.path === "/shared/docs"),
    ).toHaveLength(2);

    await act(async () => finishReload(lsByPath["/shared/docs"]));
    expect(screen.queryByText("Refreshing…")).not.toBeInTheDocument();

    await act(async () => {
      handlers?.onTail?.({
        type: "tail",
        topic: "files:watch",
        cursor: "op/3/0",
        item: {
          height: 3,
          time: 3,
          message: "update readme",
          baseSnapshot: HEAD_ID,
          paths: ["/shared/readme.md"],
        },
      });
      await new Promise((resolve) => setTimeout(resolve, 120));
    });
    expect(
      vi.mocked(transport.filesLs).mock.calls.filter(([params]) => params.path === "/shared"),
    ).toHaveLength(2);
    expect(
      vi.mocked(transport.filesLs).mock.calls.filter(([params]) => params.path === "/shared/docs"),
    ).toHaveLength(2);
  });

  it("opens on a filesFocus hand-off — the agent form pointing at a skill doc", async () => {
    const transport = makeTransport();
    renderView(transport, { filesFocus: "/shared/docs" });

    // The browser lands on the handed-off directory (with its parent column
    // still there), not on the default /shared.
    expect(
      await screen.findByRole("region", { name: /column \/shared\/docs/i }),
    ).toBeInTheDocument();
    expect(await screen.findByText("plan.md")).toBeInTheDocument();
  });

  it("opens folders as adjacent browser columns without replacing the parent column", async () => {
    const transport = makeTransport();
    renderView(transport);

    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });
    expect(await within(sharedColumn).findByText("docs")).toBeInTheDocument();
    expect(await within(sharedColumn).findByText("readme.md")).toBeInTheDocument();

    fireEvent.click(within(sharedColumn).getByRole("button", { name: /open folder docs/i }));

    const docsColumn = await screen.findByRole("region", { name: /column \/shared\/docs/i });
    expect(await within(docsColumn).findByText("plan.md")).toBeInTheDocument();
    expect(within(sharedColumn).getByText("readme.md")).toBeInTheDocument();
    expect(transport.filesLs).toHaveBeenCalledWith(expect.objectContaining({ path: "/shared/docs" }));
  });

  it("previews a file range and downloads through the direct object URL", async () => {
    const anchorClick = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    const transport = makeTransport({
      filesRead: vi.fn().mockResolvedValue({ b64: btoa("hello duckfs"), eof: true }),
      filesObjectUrl: vi.fn(() => "http://node/v1/files/object/shared/readme.md"),
    });
    try {
      renderView(transport);
      fireEvent.click(await screen.findByRole("button", { name: /open file readme\.md/i }));
      expect(await screen.findByText("hello duckfs")).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: /download readme\.md/i }));

      expect(anchorClick).toHaveBeenCalledTimes(1);
      expect(transport.filesObjectUrl).toHaveBeenCalledWith({
        path: "/shared/readme.md",
        snapshot: undefined,
        size: 4,
      });
      expect(transport.filesRead).toHaveBeenCalledTimes(1);
    } finally {
      anchorClick.mockRestore();
    }
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

  it("uploads files dropped anywhere in the browser into the active directory", async () => {
    const transport = makeTransport();
    renderView(transport);
    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });

    const file = new File(["drop me"], "drop.txt", { type: "text/plain" });
    Object.defineProperty(file, "arrayBuffer", {
      value: () => Promise.resolve(new TextEncoder().encode("drop me").buffer),
    });

    // Drop lands on a column but is handled at the browser-card level (it
    // bubbles), so it always targets the active directory.
    fireEvent.drop(sharedColumn, {
      dataTransfer: { files: [file], types: ["Files"], dropEffect: "copy" },
    });

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.changes[0].put.path).toBe("/shared/drop.txt");
  });

  it("uploads a chosen folder with relative paths under the current directory", async () => {
    const transport = makeTransport({
      filesStage: vi
        .fn()
        .mockResolvedValueOnce({ digest: "11".repeat(32) })
        .mockResolvedValueOnce({ digest: "22".repeat(32) }),
    });
    const { container } = renderView(transport);
    await screen.findByText("readme.md");

    const readme = new File(["hello"], "readme.txt", { type: "text/plain" });
    Object.defineProperty(readme, "webkitRelativePath", {
      value: "Project/readme.txt",
    });
    Object.defineProperty(readme, "arrayBuffer", {
      value: () => Promise.resolve(new TextEncoder().encode("hello").buffer),
    });
    const plan = new File(["plan"], "plan.md", { type: "text/markdown" });
    Object.defineProperty(plan, "webkitRelativePath", {
      value: "Project/docs/plan.md",
    });
    Object.defineProperty(plan, "arrayBuffer", {
      value: () => Promise.resolve(new TextEncoder().encode("plan").buffer),
    });

    fireEvent.click(screen.getByRole("button", { name: /^folder$/i }));
    const input = container.querySelector('input[data-upload-kind="folder"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [readme, plan] } });

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.message).toBe("upload folder Project");
    expect(body.changes.map((change: { put?: { path: string } }) => change.put?.path)).toEqual([
      "/shared/Project/readme.txt",
      "/shared/Project/docs/plan.md",
    ]);
  });

  it("uploads a dropped folder entry with its directory prefix", async () => {
    const transport = makeTransport({
      filesStage: vi.fn().mockResolvedValue({ digest: "33".repeat(32) }),
    });
    renderView(transport);
    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });

    const file = new File(["from folder"], "readme.txt", { type: "text/plain" });
    Object.defineProperty(file, "arrayBuffer", {
      value: () => Promise.resolve(new TextEncoder().encode("from folder").buffer),
    });
    const fileEntry = {
      isFile: true,
      isDirectory: false,
      name: "readme.txt",
      fullPath: "/Project/readme.txt",
      file: (resolve: (file: File) => void) => resolve(file),
    };
    let reads = 0;
    const folderEntry = {
      isFile: false,
      isDirectory: true,
      name: "Project",
      fullPath: "/Project",
      createReader: () => ({
        readEntries: (resolve: (entries: typeof fileEntry[]) => void) => {
          reads += 1;
          resolve(reads === 1 ? [fileEntry] : []);
        },
      }),
    };

    fireEvent.drop(sharedColumn, {
      dataTransfer: {
        files: [],
        types: ["Files"],
        dropEffect: "copy",
        items: [{ webkitGetAsEntry: () => folderEntry }],
      },
    });

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.message).toBe("upload folder Project");
    expect(body.changes[0].put.path).toBe("/shared/Project/readme.txt");
  });

  it("shows the upload drop-zone overlay while a file is dragged over the browser", async () => {
    renderView(makeTransport());
    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });

    fireEvent.dragOver(sharedColumn, {
      dataTransfer: { files: [], types: ["Files"], dropEffect: "none" },
    });

    const overlay = screen.getByRole("dialog", { name: /drop files to upload/i });
    expect(within(overlay).getByText(/drop files to upload/i)).toBeInTheDocument();
    expect(within(overlay).getByText(/\/shared/)).toBeInTheDocument();
  });

  it("hides the drop-zone overlay once the drag leaves the browser", async () => {
    renderView(makeTransport());
    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });
    const dataTransfer = { files: [], types: ["Files"], dropEffect: "none" };

    fireEvent.dragEnter(sharedColumn, { dataTransfer });
    expect(screen.getByRole("dialog", { name: /drop files to upload/i })).toBeInTheDocument();

    fireEvent.dragLeave(sharedColumn, { dataTransfer });
    expect(screen.queryByRole("dialog", { name: /drop files to upload/i })).not.toBeInTheDocument();
  });

  it("ignores an internal file-download drag (no upload overlay)", async () => {
    renderView(makeTransport());
    const sharedColumn = await screen.findByRole("region", { name: /column \/shared/i });

    // Dragging a file OUT carries our own path type alongside "Files".
    fireEvent.dragOver(sharedColumn, {
      dataTransfer: {
        files: [],
        types: ["Files", "application/x-ducktape-file-path"],
        dropEffect: "none",
      },
    });

    expect(screen.queryByRole("dialog", { name: /drop files to upload/i })).not.toBeInTheDocument();
  });

  it("uses a direct download URL for drag-out without reading the file", async () => {
    const transport = makeTransport({
      filesRead: vi.fn().mockResolvedValue({ b64: btoa("readme body"), eof: true }),
      filesObjectUrl: vi.fn(() => "http://node/v1/files/object/shared/readme.md"),
    });
    renderView(transport);
    const fileRow = await screen.findByRole("button", { name: /open file readme\.md/i });

    fireEvent.mouseEnter(fileRow);
    expect(transport.filesRead).not.toHaveBeenCalled();

    const dataTransfer = { setData: vi.fn(), effectAllowed: "none" };
    fireEvent.dragStart(fileRow, { dataTransfer });

    expect(dataTransfer.effectAllowed).toBe("copy");
    expect(dataTransfer.setData).toHaveBeenCalledWith(
      "application/x-ducktape-file-path",
      "/shared/readme.md",
    );
    expect(dataTransfer.setData).toHaveBeenCalledWith("text/plain", "/shared/readme.md");
    expect(dataTransfer.setData).toHaveBeenCalledWith(
      "DownloadURL",
      "text/plain:readme.md:http://node/v1/files/object/shared/readme.md",
    );
    expect(transport.filesRead).not.toHaveBeenCalled();
  });

  it("does not fetch refs until history is opened", async () => {
    const transport = makeTransport();
    renderView(transport);
    await screen.findByText("readme.md");
    expect(transport.query).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /^history$/i }));
    await waitFor(() => expect(transport.query).toHaveBeenCalledWith("files", { refs: {} }));
  });

  it("creates new folders through an in-app dialog", async () => {
    const transport = makeTransport();
    const promptSpy = vi.spyOn(window, "prompt").mockImplementation(() => {
      throw new Error("New folder must not use native prompt");
    });
    try {
      renderView(transport);
      await screen.findByText("readme.md");

      fireEvent.click(screen.getByRole("button", { name: /new folder/i }));

      expect(await screen.findByRole("dialog", { name: /new folder/i })).toBeInTheDocument();
      fireEvent.change(screen.getByLabelText(/folder name/i), { target: { value: "notes" } });
      fireEvent.click(screen.getByRole("button", { name: /create folder/i }));

      await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
      const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
      expect(body.changes).toEqual([{ mkdir: { path: "/shared/notes" } }]);
      expect(promptSpy).not.toHaveBeenCalled();
    } finally {
      promptSpy.mockRestore();
    }
  });

  it("creates new folders under /shared even before /shared exists", async () => {
    const transport = makeTransport({
      filesLs: vi.fn(({ path }: { path: string }) =>
        path === "/shared"
          ? Promise.reject(new Error("missing"))
          : Promise.resolve({ entries: [], next: null }),
      ),
    });
    renderView(transport);
    await screen.findByText("Empty directory");

    fireEvent.click(screen.getByRole("button", { name: /new folder/i }));
    fireEvent.change(await screen.findByLabelText(/folder name/i), { target: { value: "docs" } });
    fireEvent.click(screen.getByRole("button", { name: /create folder/i }));

    await waitFor(() => expect(transport.filesCommit).toHaveBeenCalled());
    const body = (transport.filesCommit as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(body.changes).toEqual([{ mkdir: { path: "/shared/docs" } }]);
  });

  it("opens file actions from the right-click menu", async () => {
    const transport = makeTransport();
    renderView(transport);
    const fileRow = await screen.findByRole("button", { name: /open file readme\.md/i });

    fireEvent.contextMenu(fileRow, { clientX: 80, clientY: 120 });

    expect(await screen.findByRole("menu")).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /^open$/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /^new folder$/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /^upload$/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /^delete$/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("menuitem", { name: /^open$/i }));
    expect(await screen.findByText("readme body")).toBeInTheDocument();
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
