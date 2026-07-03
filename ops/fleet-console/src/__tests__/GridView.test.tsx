import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { GridView } from "../components/GridView";
import type { FleetNode } from "../types";

// The RFB core touches WebSocket/canvas that jsdom lacks — mock it so tiles
// render. The lazy import in useRfb resolves to this fake.
vi.mock("@novnc/novnc", () => ({
  default: class {
    viewOnly = false;
    scaleViewport = false;
    background = "";
    addEventListener() {}
    disconnect() {}
  },
}));

const node = (over: Partial<FleetNode>): FleetNode => ({
  id: "feat-qa-multiwindow",
  branch: "feat/qa-multiwindow",
  path: ".claude/worktrees/feat+qa-multiwindow",
  head: { sha: "2617e0a", subject: "add multiwindow" },
  parent: "dev",
  ahead: 3,
  behind: 1,
  status: "up",
  token: "feat-qa-multiwindow",
  vncPort: 5911,
  ...over,
});

beforeEach(() => cleanup());

describe("GridView", () => {
  it("renders one tile per worktree with branch, sha and ahead/behind", () => {
    render(
      <GridView
        nodes={[
          node({}),
          node({
            id: "dev",
            branch: "dev",
            ahead: 0,
            behind: 0,
            head: { sha: "d5b04d5", subject: "dev tip" },
          }),
        ]}
        onOpen={() => {}}
      />,
    );
    expect(screen.getByText("feat/qa-multiwindow")).toBeInTheDocument();
    expect(screen.getByText("dev")).toBeInTheDocument();
    expect(screen.getByText("2617e0a")).toBeInTheDocument();
    expect(screen.getByText("↑3")).toBeInTheDocument();
    expect(screen.getByText("↓1")).toBeInTheDocument();
  });

  it("shows a not-running placeholder for down worktrees and no connect role", () => {
    render(
      <GridView
        nodes={[node({ status: "down", token: undefined })]}
        onOpen={() => {}}
      />,
    );
    expect(screen.getByText("not running")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders the empty state with no worktrees", () => {
    render(<GridView nodes={[]} onOpen={() => {}} />);
    expect(screen.getByText(/No worktrees in the fleet yet/)).toBeInTheDocument();
  });
});
