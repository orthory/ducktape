import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { ForgeView } from "./ForgeView";

const forgeGit = vi.hoisted(() => ({
  isForgeGitAvailable: vi.fn(() => true),
  forgeListRepos: vi.fn(),
  forgeHead: vi.fn(),
  forgeListBranches: vi.fn(),
  forgeLog: vi.fn(),
  forgeTree: vi.fn(),
  forgeReadFile: vi.fn(),
  forgeReadFilePage: vi.fn(),
  forgeCompare: vi.fn(),
  forgeBuildMerge: vi.fn(),
  forgeDiff: vi.fn(),
}));

vi.mock("../../../domain/forge-git-client", () => forgeGit);

const HEAD = "a".repeat(40);
const COMMIT_PAGE_REQUEST = 51;
const FILE_PAGE_BYTES = 64 * 1024;

const oid = (n: number) => n.toString(16).padStart(40, "0");
const makeCommit = (n: number) => ({
  id: oid(n),
  summary: `commit ${n}`,
  message: `commit ${n}`,
  parentIds: n > 0 ? [oid(n - 1)] : [],
  author: "operator",
  time: 1_800_000_000 - n,
});

const COMMITS = [
  {
    id: HEAD,
    summary: "initial import",
    message: "initial import\n\nBootstraps the forge repository.",
    parentIds: ["b".repeat(40)],
    author: "operator",
    time: 1_800_000_000,
  },
  {
    id: "b".repeat(40),
    summary: "older cleanup",
    message: "older cleanup",
    parentIds: [],
    author: "operator",
    time: 1_799_999_000,
  },
];

function renderForge(stateOverrides: Partial<ConsoleState> = {}) {
  const commitForge = vi.fn();
  const noop = vi.fn();
  const actions = new Proxy(
    { commitForge },
    {
      get: (target, key: string) =>
        key in target ? target[key as keyof typeof target] : noop,
    },
  ) as unknown as ConsoleActions;

  render(
    <ConsoleContext.Provider
      value={{
        state: {
          ...createInitialState(),
          forgeHead: HEAD,
          connected: true,
          ...stateOverrides,
        },
        actions,
      }}
    >
      <ForgeView />
    </ConsoleContext.Provider>,
  );

  return { commitForge };
}

describe("ForgeView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    forgeGit.isForgeGitAvailable.mockReturnValue(true);
    forgeGit.forgeListRepos.mockResolvedValue([
      {
        id: "ducktape",
        name: "ducktape",
        branch: "main",
        defaultBranch: "main",
        head: HEAD,
        browsable: true,
      },
    ]);
    forgeGit.forgeHead.mockResolvedValue(HEAD);
    forgeGit.forgeLog.mockResolvedValue(COMMITS);
    forgeGit.forgeTree.mockImplementation((_repo, path = "") =>
      Promise.resolve(
        path === ""
          ? [
              { name: "src", kind: "dir" },
              { name: "README.md", kind: "file" },
            ]
          : [{ name: "index.ts", kind: "file" }],
      ),
    );
    forgeGit.forgeReadFile.mockResolvedValue("# Ducktape\n");
    forgeGit.forgeReadFilePage.mockResolvedValue({
      text: "# Ducktape\n",
      offset: 0,
      nextOffset: null,
      totalBytes: 11,
    });
    forgeGit.forgeListBranches.mockResolvedValue([{ name: "main", head: HEAD }]);
    forgeGit.forgeCompare.mockResolvedValue({
      mergeBase: HEAD,
      files: [],
      totalAdditions: 0,
      totalDeletions: 0,
      commits: [],
    });
    forgeGit.forgeBuildMerge.mockResolvedValue({
      mergeOid: null,
      packHex: null,
      conflicts: [],
    });
    forgeGit.forgeDiff.mockResolvedValue([
      {
        path: "README.md",
        status: "modified",
        hunks: [
          {
            header: "@@ -1,1 +1,2 @@",
            lines: [
              { origin: " ", content: "# Ducktape" },
              { origin: "+", content: "Forge-ready repository." },
            ],
          },
        ],
      },
    ]);
  });

  it("starts at a repositories overview and does not render commit controls", async () => {
    const { commitForge } = renderForge();

    await waitFor(() => {
      expect(screen.getByText("ducktape/ducktape")).toBeInTheDocument();
    });

    expect(screen.getByText("REPOSITORIES")).toBeInTheDocument();
    expect(screen.queryByText("NEW COMMIT")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^commit$/i })).not.toBeInTheDocument();
    expect(commitForge).not.toHaveBeenCalled();
  });

  it("separates code browsing from the full commit history", async () => {
    renderForge();

    fireEvent.click(await screen.findByText("ducktape/ducktape"));

    await waitFor(() => {
      expect(screen.getByText("FILES")).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: /^Code$/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Commits$/ })).toBeInTheDocument();
    expect(screen.queryByText("older cleanup")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Commits$/ }));

    expect(await screen.findByText("older cleanup")).toBeInTheDocument();
    expect(screen.queryByText("FILES")).not.toBeInTheDocument();
    expect(screen.queryByText("NEW COMMIT")).not.toBeInTheDocument();
  });

  it("loads commit history in cursor pages instead of walking the full log", async () => {
    const firstPage = Array.from({ length: COMMIT_PAGE_REQUEST }, (_, index) => makeCommit(index));
    const secondPage = [makeCommit(50), makeCommit(51)];
    forgeGit.forgeLog
      .mockResolvedValueOnce(firstPage)
      .mockResolvedValueOnce(secondPage);

    renderForge();

    fireEvent.click(await screen.findByText("ducktape/ducktape"));

    await waitFor(() => {
      expect(forgeGit.forgeLog).toHaveBeenCalledWith(
        "ducktape",
        COMMIT_PAGE_REQUEST,
        undefined,
        null,
      );
    });

    fireEvent.click(await screen.findByRole("button", { name: /^Commits$/ }));

    expect(await screen.findByText("commit 49")).toBeInTheDocument();
    expect(screen.queryByText("commit 50")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Load more commits" }));

    await waitFor(() => {
      expect(forgeGit.forgeLog).toHaveBeenLastCalledWith(
        "ducktape",
        COMMIT_PAGE_REQUEST,
        undefined,
        oid(49),
      );
    });
    expect(await screen.findByText("commit 50")).toBeInTheDocument();
    expect(await screen.findByText("commit 51")).toBeInTheDocument();
  });

  it("loads file content in byte pages and appends more on demand", async () => {
    forgeGit.forgeReadFilePage
      .mockResolvedValueOnce({
        text: "# Part 1\n",
        offset: 0,
        nextOffset: 9,
        totalBytes: 17,
      })
      .mockResolvedValueOnce({
        text: "# Part 2\n",
        offset: 9,
        nextOffset: null,
        totalBytes: 17,
      });

    renderForge();

    fireEvent.click(await screen.findByText("ducktape/ducktape"));

    expect(await screen.findByText("# Part 1")).toBeInTheDocument();
    expect(forgeGit.forgeReadFilePage).toHaveBeenCalledWith("ducktape", "README.md", {
      reference: undefined,
      offset: 0,
      limit: FILE_PAGE_BYTES,
    });
    expect(screen.queryByText("# Part 2")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Load more file" }));

    expect(await screen.findByText("# Part 2")).toBeInTheDocument();
    expect(forgeGit.forgeReadFilePage).toHaveBeenLastCalledWith("ducktape", "README.md", {
      reference: undefined,
      offset: 9,
      limit: FILE_PAGE_BYTES,
    });
  });

  it("opens commit details from history with description and diff", async () => {
    renderForge();

    fireEvent.click(await screen.findByText("ducktape/ducktape"));
    fireEvent.click(await screen.findByRole("button", { name: /^Commits$/ }));
    fireEvent.click(await screen.findByRole("button", { name: /initial import/ }));

    expect(await screen.findByText("Bootstraps the forge repository.")).toBeInTheDocument();
    expect(await screen.findByText("README.md")).toBeInTheDocument();
    expect(screen.getByText("+Forge-ready repository.")).toBeInTheDocument();
    expect(forgeGit.forgeDiff).toHaveBeenCalledWith("ducktape", {
      from: "b".repeat(40),
      to: HEAD,
    });
  });

  it("adds Issues and Pull requests tabs that render the tracker lists", async () => {
    renderForge({
      forgeRepo: "ducktape",
      forgeItems: [
        {
          number: 1,
          kind: "issue",
          title: "Login button broken",
          state: "open",
          author: { user: Array.from(new TextEncoder().encode("operator")) },
          created_at: 1_800_000_000,
          updated_at: 1_800_000_000,
        },
      ],
    });

    fireEvent.click(await screen.findByText("ducktape/ducktape"));

    fireEvent.click(await screen.findByRole("button", { name: "Issues" }));
    expect(await screen.findByText("Login button broken")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Pull requests" }));
    expect(await screen.findByText("No pull requests yet")).toBeInTheDocument();
  });
});
