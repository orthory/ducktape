// The Touch ID status row on the Devices card. Everything else on this card
// (the link ceremony, phone enrollment, key removal) is covered by the Home
// view test; here we only pin the Touch ID enable/disable affordance, with the
// touchid-client mocked (its native invokes never run in vitest).

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

const touchid = vi.hoisted(() => ({
  touchidAvailable: vi.fn().mockResolvedValue(true),
  touchidEnrolled: vi.fn().mockResolvedValue(false),
  touchidEnroll: vi.fn().mockResolvedValue(undefined),
  touchidDisable: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../../../domain/touchid-client", () => touchid);

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { DevicesCard } from "./DevicesCard";

const ACCOUNT_ID = "a1b2".repeat(16);
const DEVICE_KEY = "cd34".repeat(16);

const bytesOf = (hex: string): number[] => {
  const out: number[] = [];
  for (let i = 0; i < hex.length; i += 2) out.push(parseInt(hex.slice(i, i + 2), 16));
  return out;
};

const identity: IdentityStateReport = {
  state: "unlocked",
  pubkey: DEVICE_KEY,
  mnemonicConfirmed: true,
};

const renderCard = () => {
  const initialState = {
    ...createInitialState(),
    accountKeys: {
      [ACCOUNT_ID]: [{ pubkey: bytesOf(DEVICE_KEY), kind: "ed25519", label: null, added_at: 1 }],
    },
  } as ConsoleState;
  const spies: Record<string, Mock<(...args: unknown[]) => unknown>> = {};

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      { get: (_t, key: string) => (spies[key] ??= vi.fn()) },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <DevicesCard accountId={ACCOUNT_ID} identity={identity} />
      </ConsoleContext.Provider>
    );
  }
  render(<Harness />);
  return { spies };
};

afterEach(() => vi.clearAllMocks());

describe("DevicesCard — Touch ID", () => {
  it("offers Enable when available and not yet enrolled, and enrolls with the confirmed password", async () => {
    renderCard();

    const enable = await screen.findByRole("button", { name: /enable touch id/i });
    fireEvent.click(enable);

    const password = await screen.findByPlaceholderText("Password");
    fireEvent.change(password, { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /^enable touch id$/i }));

    await waitFor(() =>
      expect(touchid.touchidEnroll).toHaveBeenCalledWith("correct horse battery"),
    );
  });

  it("shows Disable when already enrolled and disables behind a confirm", async () => {
    touchid.touchidEnrolled.mockResolvedValueOnce(true);
    renderCard();

    const disable = await screen.findByRole("button", { name: /disable touch id/i });
    fireEvent.click(disable);
    const dialog = screen.getByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: /disable/i }));

    await waitFor(() => expect(touchid.touchidDisable).toHaveBeenCalled());
  });

  it("renders nothing Touch-ID when unavailable", async () => {
    touchid.touchidAvailable.mockResolvedValueOnce(false);
    renderCard();
    // Give the availability probe a tick to resolve.
    await screen.findByText("DEVICES & KEYS");
    expect(screen.queryByRole("button", { name: /touch id/i })).not.toBeInTheDocument();
  });
});
