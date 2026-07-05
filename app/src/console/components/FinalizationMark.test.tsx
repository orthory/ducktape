import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../store/actions";
import { ConsoleContext } from "../store/context";
import type { OpRecord } from "../store/finalization";
import { createInitialState } from "../store/state";
import { FinalizationMark } from "./FinalizationMark";

const record = (patch: Partial<OpRecord>): OpRecord => ({
  seq: 1,
  phase: "pending",
  startedAt: 0,
  ...patch,
});

/** Wrap the mark in a live console store with spied actions — the shape the
 *  cross-link needs. Bare renders elsewhere in this file stay provider-less
 *  on purpose: the mark must degrade to a passive indicator there. */
const renderWithStore = (op: OpRecord) => {
  const openExplorerAt = vi.fn();
  const actions = { openExplorerAt } as unknown as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state: createInitialState(), actions }}>
      <FinalizationMark op={op} />
    </ConsoleContext.Provider>,
  );
  return { openExplorerAt };
};

describe("FinalizationMark", () => {
  it("renders nothing without a ledger record", () => {
    const { container } = render(<FinalizationMark op={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the pending dot while the op is in flight", () => {
    render(<FinalizationMark op={record({ phase: "pending" })} />);
    expect(screen.getByLabelText("awaiting inclusion")).toBeInTheDocument();
  });

  it("shows a checkmark once finalized, with height + op hash on hover", () => {
    const opHash = "ab".repeat(32);
    render(
      <FinalizationMark
        op={record({ phase: "finalized", height: 42, opHash })}
      />,
    );
    const mark = screen.getByLabelText("included at height 42");
    fireEvent.mouseEnter(mark);
    expect(screen.getByRole("tooltip")).toHaveTextContent("included at height 42");
    expect(screen.getByRole("tooltip")).toHaveTextContent(`op ${opHash}`);
    fireEvent.mouseLeave(mark);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("omits the hash line when the node returned none", () => {
    render(<FinalizationMark op={record({ phase: "finalized", height: 7 })} />);
    fireEvent.mouseEnter(screen.getByLabelText("included at height 7"));
    expect(screen.getByRole("tooltip")).not.toHaveTextContent("op ");
  });

  it("shows the rejection on a failed op", () => {
    render(
      <FinalizationMark op={record({ phase: "failed", error: "chat: not author" })} />,
    );
    fireEvent.mouseEnter(screen.getByLabelText("rejected"));
    expect(screen.getByRole("tooltip")).toHaveTextContent("chat: not author");
  });

  it("clicking a settled mark jumps to the explorer at the inclusion height", () => {
    const { openExplorerAt } = renderWithStore(
      record({ phase: "finalized", height: 42 }),
    );
    const mark = screen.getByRole("button", { name: "included at height 42" });
    fireEvent.mouseEnter(mark);
    expect(screen.getByRole("tooltip")).toHaveTextContent("view in explorer");
    fireEvent.click(mark);
    expect(openExplorerAt).toHaveBeenCalledWith(42);
  });

  it("stays a passive indicator without a height or without a store", () => {
    // finalized but heightless (an old node's receipt): no jump affordance.
    const { openExplorerAt } = renderWithStore(record({ phase: "finalized" }));
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("included"));
    expect(openExplorerAt).not.toHaveBeenCalled();
  });

  it("renders provider-less with a height as a plain mark, not a link", () => {
    render(<FinalizationMark op={record({ phase: "finalized", height: 42 })} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    // clicking must be a no-op, not a missing-provider throw.
    fireEvent.click(screen.getByLabelText("included at height 42"));
  });
});
