import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { OpRecord } from "../store/finalization";
import { FinalizationMark } from "./FinalizationMark";

const record = (patch: Partial<OpRecord>): OpRecord => ({
  seq: 1,
  phase: "pending",
  startedAt: 0,
  ...patch,
});

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
});
