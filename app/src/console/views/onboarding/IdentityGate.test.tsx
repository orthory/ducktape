// The identity gate's state machine: absent → create|restore, plaintext → a
// dismissable "secure your identity" interstitial, locked → unlock (with
// skip), unlocked → no gate. Non-desktop never gates. Drives the gate over a
// mocked `user-identity-client` (not Tauri `invoke` directly — the gate talks
// to that module's typed surface, never IPC itself).

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BIP39_ENGLISH_WORDLIST } from "../../../domain/bip39-wordlist";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { IdentityGate } from "./IdentityGate";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const identityStateMock = vi.hoisted(() => vi.fn());
const createIdentityMock = vi.hoisted(() => vi.fn());
const restoreIdentityMock = vi.hoisted(() => vi.fn());
const unlockIdentityMock = vi.hoisted(() => vi.fn());
const revealMnemonicMock = vi.hoisted(() => vi.fn());
const encryptLegacyMock = vi.hoisted(() => vi.fn());
const confirmMnemonicMock = vi.hoisted(() => vi.fn());

vi.mock("../../../domain/user-identity-client", () => ({
  identityState: identityStateMock,
  createIdentity: createIdentityMock,
  restoreIdentity: restoreIdentityMock,
  unlockIdentity: unlockIdentityMock,
  revealMnemonic: revealMnemonicMock,
  encryptLegacy: encryptLegacyMock,
  confirmMnemonic: confirmMnemonicMock,
  lockIdentity: vi.fn(),
}));

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

const TEST_MNEMONIC = BIP39_ENGLISH_WORDLIST.slice(0, 24).join(" ");
const TEST_WORDS = TEST_MNEMONIC.split(" ");

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  localStorage.removeItem("ducktape.pendingDisplayName");
  localStorage.removeItem("ducktape.accountLinkPending");
  vi.clearAllMocks();
});

function Child() {
  return <div data-testid="console">the console</div>;
}

/** Reads the 3 confirm inputs the component rendered ("Word #N" labels,
 *  1-indexed) and types the correct word from `words` into each — the exact
 *  indices are chosen at random by the component, so tests discover them from
 *  the DOM rather than predicting them. */
async function fillConfirmWords(words: string[], corrupt?: number) {
  const inputs = await screen.findAllByLabelText(/^Word #\d+$/);
  expect(inputs).toHaveLength(3);
  for (const input of inputs) {
    const label = input.getAttribute("aria-label")!;
    const oneIndexed = Number(label.replace("Word #", ""));
    const word = words[oneIndexed - 1];
    const value = oneIndexed === corrupt ? `${word}-wrong` : word;
    fireEvent.change(input, { target: { value } });
  }
}

describe("identity gate — platform gating", () => {
  it("never renders on non-desktop, even before any fetch could resolve", () => {
    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );
    expect(screen.getByTestId("console")).toBeInTheDocument();
    expect(identityStateMock).not.toHaveBeenCalled();
  });
});

describe("identity gate — state machine", () => {
  it("absent renders the create/restore/link chooser under the first-run step rail", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "absent",
      mnemonicConfirmed: true,
    } satisfies IdentityStateReport);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    expect(await screen.findByText("Create your account")).toBeInTheDocument();
    expect(screen.getByText("Restore")).toBeInTheDocument();
    expect(screen.getByText("Link device")).toBeInTheDocument();
    // a true first run shows the 3-step rail (this is step 1 of 3).
    expect(screen.getByText("Workspace")).toBeInTheDocument();
    expect(screen.getByText("Connect")).toBeInTheDocument();
    expect(screen.queryByTestId("console")).toBeNull();
  });

  it("plaintext renders the dismissable secure-your-identity interstitial", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "plaintext",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    } satisfies IdentityStateReport);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    expect(await screen.findByText("Secure your account")).toBeInTheDocument();
  });

  it("locked (confirmed) renders the unlock form without the first-run rail", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "locked",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    } satisfies IdentityStateReport);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    expect(await screen.findByText("Unlock your account")).toBeInTheDocument();
    // the skip consequence is stated, and a returning user gets no stepper.
    expect(screen.getByText(/stay unlinked to your account/)).toBeInTheDocument();
    expect(screen.queryByText("Workspace")).toBeNull();
  });

  it("unlocked (confirmed) renders the console with no gate", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "unlocked",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    } satisfies IdentityStateReport);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await waitFor(() => expect(identityStateMock).toHaveBeenCalled());
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });
});

describe("identity gate — create flow", () => {
  it("walks password → grid → confirm → done, then re-fetches state", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "absent", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({
      pubkey: "ab12",
      mnemonic: TEST_MNEMONIC,
    });
    confirmMnemonicMock.mockResolvedValue(undefined);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create account"));
    });

    expect(createIdentityMock).toHaveBeenCalledWith("correct horse battery");
    expect(await screen.findByText("Save your recovery phrase")).toBeInTheDocument();
    // every word from the mnemonic is rendered once, numbered.
    expect(screen.getByText("abandon")).toBeInTheDocument();

    fireEvent.click(screen.getByText("I've saved it — continue"));
    expect(await screen.findByText("Confirm your recovery phrase")).toBeInTheDocument();

    await fillConfirmWords(TEST_WORDS);
    await act(async () => {
      fireEvent.click(screen.getByText("Confirm"));
    });

    expect(confirmMnemonicMock).toHaveBeenCalledTimes(1);
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });

  it("rejects a wrong confirm word and allows retry", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "ab12", mnemonic: TEST_MNEMONIC });
    confirmMnemonicMock.mockResolvedValue(undefined);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create account"));
    });
    fireEvent.click(await screen.findByText("I've saved it — continue"));
    await screen.findByText("Confirm your recovery phrase");

    const inputs = await screen.findAllByLabelText(/^Word #\d+$/);
    const firstIndex = Number(inputs[0].getAttribute("aria-label")!.replace("Word #", ""));
    await fillConfirmWords(TEST_WORDS, firstIndex);
    fireEvent.click(screen.getByText("Confirm"));

    expect(confirmMnemonicMock).not.toHaveBeenCalled();
    expect(screen.getByText(/doesn't match/i)).toBeInTheDocument();

    await fillConfirmWords(TEST_WORDS); // now all correct
    await act(async () => {
      fireEvent.click(screen.getByText("Confirm"));
    });
    expect(confirmMnemonicMock).toHaveBeenCalledTimes(1);
  });

  it("surfaces a confirmMnemonic failure inline and allows retry", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "absent", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "ab12", mnemonic: TEST_MNEMONIC });
    confirmMnemonicMock
      .mockRejectedValueOnce(new Error("registry write failed"))
      .mockResolvedValue(undefined);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create account"));
    });
    fireEvent.click(await screen.findByText("I've saved it — continue"));
    await screen.findByText("Confirm your recovery phrase");

    await fillConfirmWords(TEST_WORDS);
    await act(async () => {
      fireEvent.click(screen.getByText("Confirm"));
    });

    // the failure surfaces inline; the gate stays on the confirm step (the
    // console never renders — onDone/refetch never fired on the failure).
    expect(await screen.findByText("registry write failed")).toBeInTheDocument();
    expect(screen.queryByTestId("console")).toBeNull();
    expect(identityStateMock).toHaveBeenCalledTimes(1);

    // retry with the same (already correct) words succeeds.
    await act(async () => {
      fireEvent.click(screen.getByText("Confirm"));
    });
    expect(confirmMnemonicMock).toHaveBeenCalledTimes(2);
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });

  it("mismatched create password shows an inline error without calling the client", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "longenoughpassword" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "somethingelse" },
    });
    fireEvent.click(screen.getByText("Create account"));

    expect(screen.getByText(/do not match/i)).toBeInTheDocument();
    expect(createIdentityMock).not.toHaveBeenCalled();
  });
});

describe("identity gate — restore flow", () => {
  it("rejects a word-count mismatch before calling the client", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Restore"));
    await screen.findByText("Restore your account");

    fireEvent.change(
      screen.getByPlaceholderText("24-word recovery phrase, separated by spaces"),
      { target: { value: BIP39_ENGLISH_WORDLIST.slice(0, 5).join(" ") } },
    );
    fireEvent.change(screen.getByPlaceholderText("New password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm new password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.click(screen.getByText("Restore account"));

    expect(screen.getByText(/got 5/i)).toBeInTheDocument();
    expect(restoreIdentityMock).not.toHaveBeenCalled();
  });

  it("rejects an unknown word before calling the client", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Restore"));
    await screen.findByText("Restore your account");

    const words = BIP39_ENGLISH_WORDLIST.slice(200, 224);
    words[10] = "zzznotarealword";
    fireEvent.change(
      screen.getByPlaceholderText("24-word recovery phrase, separated by spaces"),
      { target: { value: words.join(" ") } },
    );
    fireEvent.change(screen.getByPlaceholderText("New password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm new password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.click(screen.getByText("Restore account"));

    expect(screen.getByText('"zzznotarealword" is not a recovery-phrase word')).toBeInTheDocument();
    expect(restoreIdentityMock).not.toHaveBeenCalled();
  });

  it("surfaces the server's checksum rejection inline for well-formed words", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });
    restoreIdentityMock.mockRejectedValue(new Error("invalid mnemonic checksum"));

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Restore"));
    await screen.findByText("Restore your account");

    const words = BIP39_ENGLISH_WORDLIST.slice(300, 324);
    fireEvent.change(
      screen.getByPlaceholderText("24-word recovery phrase, separated by spaces"),
      { target: { value: words.join(" ") } },
    );
    fireEvent.change(screen.getByPlaceholderText("New password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm new password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Restore account"));
    });

    expect(restoreIdentityMock).toHaveBeenCalledWith(words.join(" "), "correct horse battery");
    expect(await screen.findByText(/invalid mnemonic checksum/)).toBeInTheDocument();
  });

  it("restoring successfully re-fetches state and renders the console", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "absent", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", mnemonicConfirmed: true });
    restoreIdentityMock.mockResolvedValue({ pubkey: "cd34" });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Restore"));
    await screen.findByText("Restore your account");

    const words = BIP39_ENGLISH_WORDLIST.slice(400, 424);
    fireEvent.change(
      screen.getByPlaceholderText("24-word recovery phrase, separated by spaces"),
      { target: { value: words.join(" ") } },
    );
    fireEvent.change(screen.getByPlaceholderText("New password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm new password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Restore account"));
    });

    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });
});

describe("identity gate — unlock flow", () => {
  it("shows a wrong-password error inline without leaving the gate", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "locked",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    });
    unlockIdentityMock.mockRejectedValue(new Error("wrong password"));

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Unlock your account");
    fireEvent.change(screen.getByPlaceholderText("Password"), {
      target: { value: "nope" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Unlock"));
    });

    expect(await screen.findByText("wrong password")).toBeInTheDocument();
    expect(screen.queryByTestId("console")).toBeNull();
  });

  it("unlocking successfully re-fetches state and renders the console", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "locked", pubkey: "ab12", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", pubkey: "ab12", mnemonicConfirmed: true });
    unlockIdentityMock.mockResolvedValue({ pubkey: "ab12" });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Unlock your account");
    fireEvent.change(screen.getByPlaceholderText("Password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Unlock"));
    });

    expect(unlockIdentityMock).toHaveBeenCalledWith("correct horse battery");
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });

  it("skip for now proceeds straight to the console without unlocking", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "locked",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Unlock your account");
    fireEvent.click(screen.getByText("Skip for now"));

    expect(await screen.findByTestId("console")).toBeInTheDocument();
    expect(unlockIdentityMock).not.toHaveBeenCalled();
  });
});

describe("identity gate — plaintext (legacy) flow", () => {
  it("dismiss proceeds to the console for this launch", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "plaintext",
      pubkey: "ab12",
      mnemonicConfirmed: true,
    });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Secure your account");
    fireEvent.click(screen.getByText("Not now"));

    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });

  it("secure flow sets a password (encryptLegacy) then offers to reveal the phrase", async () => {
    markTauri();
    // mnemonicConfirmed: true post-encrypt is real backend behavior, not just
    // a convenient mock: `user_identity_encrypt` now sets the registry flag
    // itself (mirroring `user_identity_restore`) — a legacy key predates the
    // shown-once mnemonic ceremony, so there is no confirm step to force here.
    // Before that fix this mock was aspirational (the real command left the
    // flag false, which would have routed this user into ResumeScreen next).
    identityStateMock
      .mockResolvedValueOnce({ state: "plaintext", pubkey: "ab12", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", pubkey: "ab12", mnemonicConfirmed: true });
    encryptLegacyMock.mockResolvedValue({ pubkey: "ab12" });
    revealMnemonicMock.mockResolvedValue({ mnemonic: TEST_MNEMONIC });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Secure your account");
    fireEvent.click(screen.getByText("Set a password"));

    await screen.findByText("Set a password", { selector: "span" });
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Secure account"));
    });

    expect(encryptLegacyMock).toHaveBeenCalledWith("correct horse battery");
    const revealButton = await screen.findByText("View recovery phrase");
    await act(async () => {
      fireEvent.click(revealButton);
    });

    // reveal re-verifies the just-set password fresh, never the session cache.
    expect(revealMnemonicMock).toHaveBeenCalledWith("correct horse battery");
    expect(await screen.findByText("abandon")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Done"));
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });
});

describe("identity gate — create-flow resume", () => {
  it("locked + unconfirmed resumes at password → mnemonic → confirm", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "locked", pubkey: "ab12", mnemonicConfirmed: false })
      .mockResolvedValue({ state: "locked", pubkey: "ab12", mnemonicConfirmed: true });
    revealMnemonicMock.mockResolvedValue({ mnemonic: TEST_MNEMONIC });
    confirmMnemonicMock.mockResolvedValue(undefined);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    // resumes directly at the confirm-your-phrase step, asking for the
    // password first (the mnemonic isn't in component state on a fresh boot).
    await screen.findByText("Confirm your recovery phrase");
    fireEvent.change(screen.getByPlaceholderText("Password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Continue"));
    });

    expect(revealMnemonicMock).toHaveBeenCalledWith("correct horse battery");
    fireEvent.click(await screen.findByText("Continue", { selector: "button" }));

    await fillConfirmWords(TEST_WORDS);
    await act(async () => {
      fireEvent.click(screen.getByText("Confirm"));
    });

    expect(confirmMnemonicMock).toHaveBeenCalledTimes(1);
    // mnemonic now confirmed but the encrypted key is still locked — the
    // normal unlock screen takes over rather than the console.
    expect(await screen.findByText("Unlock your account")).toBeInTheDocument();
  });

  it("skip for now proceeds straight to the console without confirming", async () => {
    // Same escape hatch as the locked screen: this resume step still demands
    // a password (possibly forgotten) before the console renders, so it must
    // not be a hard trap — skipping just means the gate re-offers next
    // launch, same as any other unconfirmed mnemonic.
    markTauri();
    identityStateMock.mockResolvedValue({
      state: "locked",
      pubkey: "ab12",
      mnemonicConfirmed: false,
    });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Confirm your recovery phrase");
    fireEvent.click(screen.getByText("Skip for now"));

    expect(await screen.findByTestId("console")).toBeInTheDocument();
    expect(confirmMnemonicMock).not.toHaveBeenCalled();
    expect(revealMnemonicMock).not.toHaveBeenCalled();
  });
});

describe("identity gate — pending display name", () => {
  it("parks the chosen name for the first connect to apply on-chain", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "ab12", mnemonic: TEST_MNEMONIC });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByLabelText("Display name"), {
      target: { value: "  Eddy Hong  " },
    });
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create account"));
    });

    expect(localStorage.getItem("ducktape.pendingDisplayName")).toBe("Eddy Hong");
  });

  it("parks nothing when the name is left blank", async () => {
    markTauri();
    identityStateMock.mockResolvedValue({ state: "absent", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "ab12", mnemonic: TEST_MNEMONIC });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create account"));
    });

    expect(localStorage.getItem("ducktape.pendingDisplayName")).toBeNull();
  });
});

describe("identity gate — link-device flow", () => {
  // A challenge blob exactly as the other device's Account view mints it.
  const CHALLENGE_JSON = {
    chainId: "team#abcd",
    accountId: "ab01".repeat(16),
    nonce: 4,
    name: "Eddy",
  };
  const CHALLENGE = `ducktape-link-challenge-v1:${btoa(JSON.stringify(CHALLENGE_JSON))}`;

  it("creates the key, marks link-pending, signs possession, and shows the response code", async () => {
    markTauri();
    identityStateMock
      // gate boot: no key yet
      .mockResolvedValueOnce({ state: "absent", mnemonicConfirmed: true })
      // the wizard's own fetch after the key exists (and any later refresh)
      .mockResolvedValue({ state: "unlocked", pubkey: "cd34", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "cd34", mnemonic: TEST_MNEMONIC });
    confirmMnemonicMock.mockResolvedValue(undefined);
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_sign_possession")
        return Promise.resolve('{"signature":{"sig":[7,7]}}');
      throw new Error(`unexpected invoke ${cmd}`);
    });

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Link device"));
    await screen.findByText("Link this device");

    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create this device's key"));
    });

    // the phrase ceremony is skipped for a linked device (UX-only flag) and
    // auto-bind is armed to wait for the other device's AddMemberKey.
    expect(confirmMnemonicMock).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem("ducktape.accountLinkPending")).toBe("1");

    await screen.findByText("Approve from your other device");
    fireEvent.change(screen.getByLabelText("Link challenge code"), {
      target: { value: `  ${CHALLENGE}\n` },
    });
    fireEvent.change(screen.getByPlaceholderText("Device label (optional, e.g. work laptop)"), {
      target: { value: "work laptop" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Generate link code"));
    });

    expect(invokeMock).toHaveBeenCalledWith("user_sign_possession", {
      chainId: "team#abcd",
      accountId: "ab01".repeat(16),
      nonce: 4,
    });
    const response = (await screen.findByLabelText("Link response code")) as HTMLTextAreaElement;
    expect(response.value.startsWith("ducktape-link-response-v1:")).toBe(true);
    const decoded = JSON.parse(atob(response.value.replace("ducktape-link-response-v1:", "")));
    expect(decoded).toEqual({
      pubkey: "cd34",
      kind: "ed25519",
      possession: '{"signature":{"sig":[7,7]}}',
      label: "work laptop",
    });

    // Continue proceeds (the gate re-fetches; the key is now unlocked+confirmed).
    await act(async () => {
      fireEvent.click(screen.getByText("Continue"));
    });
    expect(await screen.findByTestId("console")).toBeInTheDocument();
  });

  it("rejects a malformed challenge inline without signing", async () => {
    markTauri();
    identityStateMock
      .mockResolvedValueOnce({ state: "absent", mnemonicConfirmed: true })
      .mockResolvedValue({ state: "unlocked", pubkey: "cd34", mnemonicConfirmed: true });
    createIdentityMock.mockResolvedValue({ pubkey: "cd34", mnemonic: TEST_MNEMONIC });
    confirmMnemonicMock.mockResolvedValue(undefined);

    render(
      <IdentityGate>
        <Child />
      </IdentityGate>,
    );

    await screen.findByText("Create your account");
    fireEvent.click(screen.getByText("Link device"));
    await screen.findByText("Link this device");
    fireEvent.change(screen.getByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Create this device's key"));
    });

    await screen.findByText("Approve from your other device");
    fireEvent.change(screen.getByLabelText("Link challenge code"), {
      target: { value: "not a link code" },
    });
    fireEvent.click(screen.getByText("Generate link code"));

    expect(
      screen.getByText(/doesn't look like a link code/i),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
