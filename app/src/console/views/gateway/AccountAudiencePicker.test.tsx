// The explicit-audience cap is security-relevant: accountsAudience() slices at
// MAX_AUDIENCE_ACCOUNTS, so anything the picker lets an operator select past it
// would be dropped SILENTLY from a policy they then sign. The picker must make
// the cap unreachable rather than lean on that backstop.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MAX_AUDIENCE_ACCOUNTS } from "../../../domain/gateway-client";
import { AccountAudiencePicker } from "./AccountAudiencePicker";

const id = (n: number) => n.toString(16).padStart(2, "0").repeat(32);

const renderPicker = (selectedCount: number, rosterCount = selectedCount + 1) => {
  const onChange = vi.fn();
  const roster = Array.from({ length: rosterCount }, (_, i) => id(i));
  render(
    <AccountAudiencePicker
      roster={roster}
      label={(hex) => `account ${hex.slice(0, 2)}`}
      selected={roster.slice(0, selectedCount)}
      onChange={onChange}
      ownerAccountId={id(0xff)}
    />,
  );
  return { onChange, roster };
};

describe("AccountAudiencePicker", () => {
  it("blocks every add path at the cap", () => {
    const { onChange, roster } = renderPicker(MAX_AUDIENCE_ACCOUNTS);

    expect(screen.getByRole("status")).toHaveTextContent(
      `maximum of ${MAX_AUDIENCE_ACCOUNTS} accounts`,
    );

    // the one unselected roster row cannot be checked...
    const spare = screen.getByRole("checkbox", {
      name: `Include account ${roster[MAX_AUDIENCE_ACCOUNTS].slice(0, 2)}`,
    });
    expect(spare).toBeDisabled();
    fireEvent.click(spare);

    // ...nor can the hex escape hatch or the owner shortcut add one.
    expect(screen.getByRole("button", { name: "Add account id" })).toBeDisabled();
    expect(screen.getByLabelText("Account id hex")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Include my account" })).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("still adds below the cap", () => {
    const { onChange, roster } = renderPicker(MAX_AUDIENCE_ACCOUNTS - 1);

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: `Include account ${roster[MAX_AUDIENCE_ACCOUNTS - 1].slice(0, 2)}`,
      }),
    );

    expect(onChange).toHaveBeenCalledWith([
      ...roster.slice(0, MAX_AUDIENCE_ACCOUNTS - 1),
      roster[MAX_AUDIENCE_ACCOUNTS - 1],
    ]);
  });
});
