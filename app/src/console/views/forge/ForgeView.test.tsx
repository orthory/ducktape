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
  forgeCompare: vi.fn(),
  forgeBuildMerge: vi.fn(),
}));

vi.mock("../../../domain/forge-git-client", () => forgeGit);

const HEAD = "a".repeat(40);
const COMMITS = [
  {
    id: HEAD,
    summary: "initial import",
    author: "operator",
    time: 1_800_000_000,
  },
  {
    id: "b".repeat(40),
    summary: "older cleanup",
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
