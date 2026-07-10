import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import type { ForgeItemSummary } from "../../../../domain/forge-client";
import type { ConsoleActions } from "../../../store/actions";
import { ConsoleContext } from "../../../store/context";
import { createInitialState, type ConsoleState } from "../../../store/state";
import { IssuesTab } from "./IssuesTab";
import { PullsTab } from "./PullsTab";

// The detail panel / merge box reach the desktop git bridge — mock the whole
// module exactly like ForgeView.test.tsx does.
const forgeGit = vi.hoisted(() => ({
  isForgeGitAvailable: vi.fn(() => true),
  forgeListRepos: vi.fn(),
  forgeHead: vi.fn(),
  forgeListBranches: vi.fn(() => Promise.resolve([])),
  forgeLog: vi.fn(),
  forgeTree: vi.fn(),
  forgeReadFile: vi.fn(),
  forgeCompare: vi.fn(),
  forgeBuildMerge: vi.fn(),
  forgeDiff: vi.fn(),
}));

vi.mock("../../../../domain/forge-git-client", () => forgeGit);

const HEAD = "a".repeat(40);
const FEATURE_HEAD = "b".repeat(40);

const operator = { user: Array.from(new TextEncoder().encode("operator")) };

const issue = (over: Partial<ForgeItemSummary>): ForgeItemSummary => ({
  number: 1,
  kind: "issue",
  title: "an issue",
  state: "open",
  author: operator,
  created_at: 1_800_000_000,
  updated_at: 1_800_000_000,
  ...over,
});

function renderInConsole(
  node: ReactNode,
  stateOverrides: Partial<ConsoleState>,
  actionOverrides: Partial<Record<keyof ConsoleActions, unknown>> = {},
) {
  const noop = vi.fn();
  const actions = new Proxy(actionOverrides, {
    get: (target, key: string) =>
      key in target ? target[key as keyof typeof target] : noop,
  }) as unknown as ConsoleActions;

  render(
    <ConsoleContext.Provider
      value={{ state: { ...createInitialState(), connected: true, ...stateOverrides }, actions }}
    >
      {node}
    </ConsoleContext.Provider>,
  );

  return { actions };
}

describe("IssuesTab", () => {
  it("lists open issues with number, author and state filter", () => {
    renderInConsole(<IssuesTab repo="ducktape" />, {
      forgeRepo: "ducktape",
      forgeItems: [
        issue({ number: 4, title: "Login button broken" }),
        issue({ number: 2, title: "Old bug", state: "closed" }),
        {
          number: 3,
          kind: "pr",
          title: "a pull request that must not appear here",
          state: "open",
          author: operator,
          created_at: 1_800_000_000,
          updated_at: 1_800_000_000,
        },
      ],
    });

    expect(screen.getByText("Login button broken")).toBeInTheDocument();
    expect(screen.getByText("#4")).toBeInTheDocument();
    expect(screen.getByText(/operator/)).toBeInTheDocument();
    expect(screen.queryByText("Old bug")).not.toBeInTheDocument();
    expect(screen.queryByText(/a pull request/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Closed 1" }));
    expect(screen.getByText("Old bug")).toBeInTheDocument();
    expect(screen.queryByText("Login button broken")).not.toBeInTheDocument();
  });

  it("shows the friendly empty state when the repo has no issues", () => {
    renderInConsole(<IssuesTab repo="ducktape" />, { forgeRepo: "ducktape", forgeItems: [] });
    expect(screen.getByText("No issues yet")).toBeInTheDocument();
  });

  it("opens an issue through the inline form", async () => {
    const openForgeIssue = vi.fn(() => Promise.resolve());
    renderInConsole(
      <IssuesTab repo="ducktape" />,
      { forgeRepo: "ducktape", forgeItems: [] },
      { openForgeIssue },
    );

    fireEvent.click(screen.getByRole("button", { name: "New issue" }));
    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "Crash on start" },
    });
    fireEvent.change(screen.getByPlaceholderText("Description (markdown)"), {
      target: { value: "It crashes." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open issue" }));

    await waitFor(() => {
      expect(openForgeIssue).toHaveBeenCalledWith({
        repo: "ducktape",
        title: "Crash on start",
        body: "It crashes.",
      });
    });
  });
});

describe("PullsTab", () => {
  it("opens a PR from the form with a selectable target branch", async () => {
    const openForgePr = vi.fn(() => Promise.resolve());
    renderInConsole(
      <PullsTab repo="ducktape" />,
      {
        forgeRepo: "ducktape",
        forgeItems: [],
        forgeBranches: [
          { name: "main", head: HEAD },
          { name: "release", head: "c".repeat(40) },
          { name: "feature", head: FEATURE_HEAD },
        ],
      },
      { openForgePr },
    );

    fireEvent.click(screen.getByRole("button", { name: "New pull request" }));
    fireEvent.change(screen.getByLabelText("Merge"), { target: { value: "feature" } });
    fireEvent.change(screen.getByLabelText("Target"), { target: { value: "release" } });
    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "Add the feature" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open pull request" }));

    await waitFor(() => {
      expect(openForgePr).toHaveBeenCalledWith({
        repo: "ducktape",
        title: "Add the feature",
        body: "",
        sourceBranch: "feature",
        targetBranch: "release",
      });
    });
  });

  it("opens PR commit details with the full message and first-parent diff", async () => {
    const getForgeItem = vi.fn(() =>
      Promise.resolve({
        number: 7,
        kind: "pr",
        title: "Add review tools",
        state: "open",
        author: operator,
        created_at: 1_800_000_000,
        updated_at: 1_800_000_000,
        body: "Reviewable changes.",
        channel_id: "forge:ducktape:7",
        source_branch: "feature",
        target_branch: "main",
        merge_oid: null,
        reviews: [],
      }),
    );
    forgeGit.forgeCompare.mockResolvedValue({
      mergeBase: HEAD,
      files: [],
      totalAdditions: 0,
      totalDeletions: 0,
      commits: [
        {
          id: FEATURE_HEAD,
          summary: "Add review tools",
          message: "Add review tools\n\nShows commit metadata and file diffs.",
          parentIds: [HEAD],
          author: "operator",
          time: 1_800_000_100,
        },
      ],
    });
    forgeGit.forgeDiff.mockResolvedValue([
      {
        path: "src/review.ts",
        status: "modified",
        hunks: [
          {
            header: "@@ -1,1 +1,2 @@",
            lines: [
              { origin: " ", content: "export const oldValue = true;" },
              { origin: "+", content: "export const reviewable = true;" },
            ],
          },
        ],
      },
    ]);
    renderInConsole(
      <PullsTab repo="ducktape" />,
      {
        forgeRepo: "ducktape",
        forgeItems: [
          {
            number: 7,
            kind: "pr",
            title: "Add review tools",
            state: "open",
            author: operator,
            created_at: 1_800_000_000,
            updated_at: 1_800_000_000,
          },
        ],
        forgeBranches: [
          { name: "main", head: HEAD },
          { name: "feature", head: FEATURE_HEAD },
        ],
      },
      { getForgeItem },
    );

    fireEvent.click(screen.getByText("Add review tools"));
    fireEvent.click(await screen.findByRole("button", { name: "Commits" }));
    fireEvent.click(await screen.findByRole("button", { name: /Add review tools/ }));

    expect(await screen.findByText("Shows commit metadata and file diffs.")).toBeInTheDocument();
    expect(await screen.findByText("src/review.ts")).toBeInTheDocument();
    expect(screen.getByText("+export const reviewable = true;")).toBeInTheDocument();
    expect(forgeGit.forgeDiff).toHaveBeenCalledWith("ducktape", {
      from: HEAD,
      to: FEATURE_HEAD,
    });
  });

  it("hints at pushing a branch when only main exists", () => {
    renderInConsole(<PullsTab repo="ducktape" />, {
      forgeRepo: "ducktape",
      forgeItems: [],
      forgeBranches: [{ name: "main", head: HEAD }],
    });

    fireEvent.click(screen.getByRole("button", { name: "New pull request" }));
    expect(
      screen.getByText(/push a branch besides main to open a pull request/i),
    ).toBeInTheDocument();
  });
});
