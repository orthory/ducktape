import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { saveNetworkDevices } from "../../store/state";
import { DevicePanel } from "./DevicePanel";

const accountUnbindNode = vi.fn().mockResolvedValue(undefined);
const accountSetNodeLabel = vi.fn().mockResolvedValue(undefined);

// A connected local network ("duck-1") with two of the account's nodes bound,
// one of them this device. "aa11" has a validator seat and a label; "bb22" is
// unlabeled with no seat.
const state = {
  workspace: { id: "w1", pubkey: "aa11", name: "Orthory", chainId: "duck-1" },
  members: ["aa11"],
  residents: [] as string[],
  workspaces: [
    { id: "w1", chainId: "duck-1", name: "Orthory" },
    { id: "w2", chainId: "duck-2", name: "Side Net" },
  ],
  nodeUsers: {
    aa11: { accountId: "acct-1", name: "Kim", label: "Kim's laptop" },
    bb22: { accountId: "acct-1", name: "Kim", label: null },
    cc33: { accountId: "other", name: null, label: null },
  },
};

vi.mock("../../store/use-ducktape", () => ({
  useDucktape: () => ({ state, actions: { accountUnbindNode, accountSetNodeLabel } }),
}));

describe("DevicePanel", () => {
  beforeEach(() => {
    localStorage.clear();
    accountUnbindNode.mockClear();
    accountSetNodeLabel.mockClear();
  });
  afterEach(() => localStorage.clear());

  it("renders nothing with no account and an empty cache", () => {
    const { container } = render(<DevicePanel accountId={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists the connected network's account nodes (excluding other accounts)", () => {
    render(<DevicePanel accountId="acct-1" />);
    // two nodes of acct-1, not the "other" account's cc33.
    expect(screen.getAllByRole("button", { name: /Unbind node/ })).toHaveLength(2);
    expect(screen.getByText("Kim's laptop")).toBeTruthy();
    expect(screen.getByText("VALIDATOR")).toBeTruthy();
  });

  it("Unbind routes a lost node through the confirm dialog to accountUnbindNode", () => {
    render(<DevicePanel accountId="acct-1" />);
    fireEvent.click(screen.getAllByRole("button", { name: /Unbind node/ })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Unbind device" }));
    expect(accountUnbindNode).toHaveBeenCalledOnce();
  });

  it("editing a label submits accountSetNodeLabel, and clearing sends null", () => {
    render(<DevicePanel accountId="acct-1" />);
    // bb22 is unlabeled → its trigger reads "Label".
    fireEvent.click(screen.getByRole("button", { name: /Rename node bb22/ }));
    const input = screen.getByLabelText(/Label for node bb22/);
    fireEvent.change(input, { target: { value: "Kim's phone" } });
    fireEvent.blur(input);
    expect(accountSetNodeLabel).toHaveBeenCalledWith("bb22", "Kim's phone");

    // renaming aa11 to blank clears it (null).
    fireEvent.click(screen.getByRole("button", { name: /Rename node aa11/ }));
    const input2 = screen.getByLabelText(/Label for node aa11/);
    fireEvent.change(input2, { target: { value: "   " } });
    fireEvent.blur(input2);
    expect(accountSetNodeLabel).toHaveBeenCalledWith("aa11", null);
  });

  it("shows other networks' cached devices read-only with a switch hint", () => {
    // Seed a last-known snapshot for a DIFFERENT (not connected) network.
    saveNetworkDevices("acct-1", "duck-2", {
      name: "Side Net",
      at: Date.now(),
      rows: [{ nodeHex: "dd44", label: "Work desktop", standing: "Resident", isThisDevice: false }],
    });
    render(<DevicePanel accountId="acct-1" />);

    expect(screen.getByText(/last seen/)).toBeTruthy();
    expect(screen.getByText("Work desktop")).toBeTruthy();
    expect(screen.getByText(/Switch to Side Net to rename or unbind/)).toBeTruthy();
    // the cached group is read-only: only the CONNECTED group's nodes get
    // Unbind buttons (2), none for the cached network.
    expect(screen.getAllByRole("button", { name: /Unbind node/ })).toHaveLength(2);
  });

  it("refuses a label over 64 bytes without submitting (multibyte-safe)", () => {
    render(<DevicePanel accountId="acct-1" />);
    fireEvent.click(screen.getByRole("button", { name: /Rename node bb22/ }));
    const input = screen.getByLabelText(/Label for node bb22/);
    // 33 x '한' = 33 UTF-16 units (passes maxLength) but 99 UTF-8 bytes.
    fireEvent.change(input, { target: { value: "한".repeat(33) } });
    fireEvent.blur(input);
    expect(accountSetNodeLabel).not.toHaveBeenCalled();
    expect(screen.getByText(/64 bytes/)).toBeTruthy();
  });

  it("surfaces the recovery pointer to the recovery phrase", () => {
    render(<DevicePanel accountId="acct-1" />);
    expect(screen.getByText(/reveal your recovery phrase/)).toBeTruthy();
  });

  it("scopes the cache to the account: another identity's rows never render", () => {
    // A PREVIOUS account's snapshot (forget + re-onboard scenario): the new
    // account must never see it — the account key makes it unreachable.
    saveNetworkDevices("acct-old", "duck-2", {
      name: "Side Net",
      at: Date.now(),
      rows: [{ nodeHex: "dd44", label: null, standing: "No seat", isThisDevice: false }],
    });
    render(<DevicePanel accountId="acct-1" />);
    expect(screen.queryByText("Side Net")).toBeNull();
  });
});
