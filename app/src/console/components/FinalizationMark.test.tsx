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
 *  explorer jump (and hash addressing) needs. Bare renders elsewhere in this
 *  file stay provider-less on purpose: the mark must degrade to a stats-only
 *  indicator there. */
const renderWithStore = (
  mark: React.ReactElement,
  ops: Record<string, OpRecord> = {},
) => {
  const openExplorerAt = vi.fn();
  const actions = { openExplorerAt } as unknown as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state: { ...createInitialState(), ops }, actions }}>
      {mark}
    </ConsoleContext.Provider>,
  );
  return { openExplorerAt };
};

describe("FinalizationMark", () => {
  it("renders nothing without a ledger record", () => {
    const { container } = render(<FinalizationMark op={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a single check while the op is in flight (sent + preconfirmed)", () => {
    render(<FinalizationMark op={record({ phase: "pending" })} />);
    expect(
      screen.getByLabelText("sent — awaiting confirmation"),
    ).toBeInTheDocument();
  });

  it("shows a double check once confirmed", () => {
    render(<FinalizationMark op={record({ phase: "finalized", height: 42 })} />);
    expect(screen.getByLabelText("confirmed at height 42")).toBeInTheDocument();
  });

  it("hovering shows the short status, not the stats", () => {
    render(<FinalizationMark op={record({ phase: "finalized", height: 42 })} />);
    const mark = screen.getByLabelText("confirmed at height 42");
    fireEvent.mouseEnter(mark);
    expect(screen.getByRole("tooltip")).toHaveTextContent("confirmed at height 42");
    expect(screen.getByRole("tooltip")).toHaveTextContent("click for details");
    fireEvent.mouseLeave(mark);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("clicking opens the stats popover: times, latency, height, op hash", () => {
    const opHash = "ab".repeat(32);
    render(
      <FinalizationMark
        op={record({
          phase: "finalized",
          startedAt: 1_000,
          settledAt: 1_420,
          height: 42,
          opHash,
        })}
      />,
    );
    fireEvent.click(screen.getByLabelText("confirmed at height 42"));
    const pop = screen.getByRole("dialog");
    expect(pop).toHaveTextContent("confirmed at height 42");
    expect(pop).toHaveTextContent("sent");
    expect(pop).toHaveTextContent("(+420 ms)");
    expect(pop).toHaveTextContent("height 42");
    expect(pop).toHaveTextContent(`op ${opHash}`);
  });

  it("formats second-scale latency in seconds", () => {
    render(
      <FinalizationMark
        op={record({ phase: "finalized", startedAt: 0, settledAt: 2_300, height: 1 })}
      />,
    );
    fireEvent.click(screen.getByLabelText("confirmed at height 1"));
    expect(screen.getByRole("dialog")).toHaveTextContent("(+2.3 s)");
  });

  it("clicking again (or pressing Escape) closes the popover", () => {
    render(<FinalizationMark op={record({ phase: "pending" })} />);
    const mark = screen.getByLabelText("sent — awaiting confirmation");
    fireEvent.click(mark);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(mark);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(mark);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("a click outside the mark and popover dismisses it", () => {
    render(<FinalizationMark op={record({ phase: "pending" })} />);
    fireEvent.click(screen.getByLabelText("sent — awaiting confirmation"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows the rejection time and error on a failed op", () => {
    render(
      <FinalizationMark
        op={record({
          phase: "failed",
          startedAt: 500,
          settledAt: 800,
          error: "chat: not author",
        })}
      />,
    );
    fireEvent.click(screen.getByLabelText("rejected"));
    const pop = screen.getByRole("dialog");
    expect(pop).toHaveTextContent("rejected");
    expect(pop).toHaveTextContent("(+300 ms)");
    expect(pop).toHaveTextContent("chat: not author");
  });

  it("the popover's explorer button jumps to the inclusion height", () => {
    const { openExplorerAt } = renderWithStore(
      <FinalizationMark op={record({ phase: "finalized", height: 42 })} />,
    );
    fireEvent.click(screen.getByLabelText("confirmed at height 42"));
    fireEvent.click(screen.getByRole("button", { name: "view in explorer" }));
    expect(openExplorerAt).toHaveBeenCalledWith(42);
    // the jump navigates away — the popover must not linger.
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("omits the explorer button without a height or without a store", () => {
    // finalized but heightless (an old node's receipt): stats only.
    const { openExplorerAt } = renderWithStore(
      <FinalizationMark op={record({ phase: "finalized" })} />,
    );
    fireEvent.click(screen.getByLabelText("confirmed"));
    expect(
      screen.queryByRole("button", { name: "view in explorer" }),
    ).not.toBeInTheDocument();
    expect(openExplorerAt).not.toHaveBeenCalled();
  });

  it("renders provider-less with a height as stats-only, not a throw", () => {
    render(<FinalizationMark op={record({ phase: "finalized", height: 42 })} />);
    fireEvent.click(screen.getByLabelText("confirmed at height 42"));
    expect(screen.getByRole("dialog")).toHaveTextContent("height 42");
    expect(
      screen.queryByRole("button", { name: "view in explorer" }),
    ).not.toBeInTheDocument();
  });

  it("resolves a content address against the session ledger", () => {
    const opHash = "cd".repeat(32);
    renderWithStore(<FinalizationMark hash={opHash} />, {
      "file/t1": record({ phase: "finalized", height: 9, opHash }),
    });
    fireEvent.click(screen.getByLabelText("confirmed at height 9"));
    expect(screen.getByRole("dialog")).toHaveTextContent(`op ${opHash}`);
  });

  it("renders nothing for an address the ledger never saw, or without a store", () => {
    const { openExplorerAt } = renderWithStore(
      <FinalizationMark hash={"ee".repeat(32)} />,
    );
    expect(openExplorerAt).not.toHaveBeenCalled();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    const { container } = render(<FinalizationMark hash={"ee".repeat(32)} />);
    expect(container).toBeEmptyDOMElement();
  });
});
