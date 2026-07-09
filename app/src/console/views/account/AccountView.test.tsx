// The Account console: profile + custody (ported whole from the old Settings
// DevicesSection — the always-re-prompt reveal regressions ride along),
// member keys with the link ceremony, and the account's nodes. Store comes in
// through a ConsoleContext harness with Proxy-spied actions; Tauri custody
// verbs through a mocked invoke.

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { BIP39_ENGLISH_WORDLIST } from "../../../domain/bip39-wordlist";
import { shortKey } from "../../../domain/names";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { AccountView } from "./AccountView";
import { encodeLinkResponse, type LinkChallenge } from "./link-device";

const TEST_MNEMONIC = BIP39_ENGLISH_WORDLIST.slice(0, 24).join(" ");

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const ACCOUNT_ID = "a1b2".repeat(16);
const OTHER_NODE = "beadbead".repeat(8);
const DEVICE_KEY = "cd34".repeat(16);
const SECOND_KEY = "ef56".repeat(16);

const bytesOf = (hex: string): number[] => {
  const out: number[] = [];
  for (let i = 0; i < hex.length; i += 2) out.push(parseInt(hex.slice(i, i + 2), 16));
  return out;
};

/** A linked two-key, two-node account as the store projects it. */
const linkedState = (): Partial<ConsoleState> => ({
  nodeUsers: {
    [workspace.pubkey]: { accountId: ACCOUNT_ID, name: "Rae" },
    [OTHER_NODE]: { accountId: ACCOUNT_ID, name: "Rae" },
  },
  accountKeys: {
    [ACCOUNT_ID]: [
      { pubkey: bytesOf(DEVICE_KEY), kind: "ed25519", label: null, added_at: 1 },
      { pubkey: bytesOf(SECOND_KEY), kind: "ed25519", label: "work laptop", added_at: 2 },
    ],
  },
  members: [workspace.pubkey],
  residents: [OTHER_NODE],
  workspaces: [workspace],
});

type ActionImpls = Partial<Record<string, (...args: unknown[]) => unknown>>;

const renderAccount = (patch: Partial<ConsoleState> = {}, impls: ActionImpls = {}) => {
  const initialState = {
    ...createInitialState(),
    author: "Rae",
    workspace,
    managed: true,
    connected: true,
    ...patch,
  };
  const spies: Record<string, Mock<(...args: unknown[]) => unknown>> = {};

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn(impls[key]);
          if (key === "setAuthor") {
            return (author: string) => {
              spies[key]?.(author);
              setState((prev) => ({ ...prev, author }));
            };
          }
          return spies[key];
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <AccountView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

const mockIdentity = (state: "absent" | "plaintext" | "locked" | "unlocked") => {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "user_identity_state")
      return Promise.resolve({ state, pubkey: DEVICE_KEY, mnemonicConfirmed: true });
    throw new Error(`unexpected invoke ${cmd}`);
  });
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("AccountView — profile", () => {
  it("shows the name editor and the account id, with copy", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { spies } = renderAccount(linkedState());

    expect(screen.getByText(/account id/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /copy account id/i }));
    expect(writeText).toHaveBeenCalledWith(ACCOUNT_ID);

    const name = screen.getByDisplayValue("Rae");
    fireEvent.change(name, { target: { value: "Ari" } });
    expect(spies.setAuthor).toHaveBeenCalledWith("Ari");
    fireEvent.blur(name);
    expect(spies.setDisplayName).toHaveBeenCalledWith("Ari");

    // The node key and workspace role are the Node page's facts, not ours.
    expect(screen.queryByText(/key on this device/)).not.toBeInTheDocument();
  });

  it("says so when this node resolves to no account", () => {
    renderAccount();
    expect(screen.getByText("not linked to an account yet")).toBeInTheDocument();
  });

  it("shows the honest chain-scope banner when nothing is connected", () => {
    renderAccount({ workspace: null, nodeUrl: null, connected: false });
    expect(screen.getByText(/Account data lives on each network/)).toBeInTheDocument();
  });
});

describe("AccountView — devices & keys", () => {
  it("lists the account's member keys with scheme labels and a this-device marker", async () => {
    markTauri();
    mockIdentity("unlocked");
    renderAccount(linkedState());

    expect(await screen.findByText(new RegExp(`${shortKey(DEVICE_KEY)}.*this device`)))
      .toBeInTheDocument();
    expect(screen.getByText("work laptop")).toBeInTheDocument();
    expect(screen.getByText(new RegExp(shortKey(SECOND_KEY)))).toBeInTheDocument();
  });

  it("removes a key behind a confirm dialog", async () => {
    markTauri();
    mockIdentity("unlocked");
    const { spies } = renderAccount(linkedState(), {
      accountRemoveMember: () => Promise.resolve(),
    });

    const removeButtons = await screen.findAllByRole("button", { name: /remove key/i });
    fireEvent.click(removeButtons[1]);
    const dialog = screen.getByRole("dialog", { name: /remove this key/i });
    fireEvent.click(within(dialog).getByRole("button", { name: /remove key/i }));

    await waitFor(() =>
      expect(spies.accountRemoveMember).toHaveBeenCalledWith(SECOND_KEY),
    );
  });

  it("walks the inviter side of the link ceremony: mint → paste reply → approve", async () => {
    markTauri();
    mockIdentity("unlocked");
    const challenge: LinkChallenge = {
      chainId: workspace.chainId,
      accountId: ACCOUNT_ID,
      nonce: 5,
      name: "Rae",
    };
    const { spies } = renderAccount(linkedState(), {
      accountLinkChallenge: () => Promise.resolve(challenge),
      accountAddMember: () => Promise.resolve(),
    });

    fireEvent.click(await screen.findByRole("button", { name: /^start$/i }));

    // The freshly-minted challenge renders as a copyable code.
    const challengeBox = (await screen.findByLabelText(
      "Link challenge code",
    )) as HTMLTextAreaElement;
    expect(challengeBox.value.startsWith("ducktape-link-challenge-v1:")).toBe(true);
    expect(spies.accountLinkChallenge).toHaveBeenCalled();

    const reply = encodeLinkResponse({
      pubkey: "aa11".repeat(16),
      kind: "ed25519",
      possession: '{"signature":{"sig":[1]}}',
      label: null,
    });
    fireEvent.change(screen.getByLabelText("Link response code"), {
      target: { value: reply },
    });
    fireEvent.click(screen.getByRole("button", { name: /approve link/i }));

    await waitFor(() =>
      expect(spies.accountAddMember).toHaveBeenCalledWith(challenge, reply),
    );
  });

  it("offers the new-device link wizard when this node is unlinked", async () => {
    markTauri();
    mockIdentity("unlocked");
    renderAccount(); // no nodeUsers entry → unlinked

    expect(await screen.findByText("Link this device")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^link$/i }));
    expect(await screen.findByLabelText("Link challenge code")).toBeInTheDocument();
  });
});

describe("AccountView — nodes", () => {
  it("lists the account's nodes with standing chips and machine workspaces", async () => {
    markTauri();
    mockIdentity("unlocked");
    renderAccount(linkedState());

    expect(await screen.findByText("NODES ON THIS NETWORK")).toBeInTheDocument();
    expect(screen.getByText("This device's node")).toBeInTheDocument();
    expect(screen.getByText("VALIDATOR")).toBeInTheDocument();
    expect(screen.getByText("RESIDENT")).toBeInTheDocument();

    expect(screen.getByText("WORKSPACES ON THIS MACHINE")).toBeInTheDocument();
    expect(screen.getByText("ACTIVE")).toBeInTheDocument();
  });

  it("unbinds a lost node behind a confirm dialog", async () => {
    markTauri();
    mockIdentity("unlocked");
    const { spies } = renderAccount(linkedState(), {
      accountUnbindNode: () => Promise.resolve(),
    });

    fireEvent.click(
      await screen.findByRole("button", {
        name: new RegExp(`unbind node ${shortKey(OTHER_NODE)}`, "i"),
      }),
    );
    const dialog = screen.getByRole("dialog", { name: /unbind node/i });
    fireEvent.click(within(dialog).getByRole("button", { name: /^unbind node$/i }));

    await waitFor(() =>
      expect(spies.accountUnbindNode).toHaveBeenCalledWith(OTHER_NODE),
    );
  });

  it("surfaces an unbind failure inline", async () => {
    markTauri();
    mockIdentity("unlocked");
    renderAccount(linkedState(), {
      accountUnbindNode: () =>
        Promise.reject(new Error("your account is locked on this device — unlock it first, then retry")),
    });

    fireEvent.click(
      await screen.findByRole("button", {
        name: new RegExp(`unbind node ${shortKey(OTHER_NODE)}`, "i"),
      }),
    );
    const dialog = screen.getByRole("dialog", { name: /unbind node/i });
    fireEvent.click(within(dialog).getByRole("button", { name: /^unbind node$/i }));

    expect(await screen.findByText(/locked on this device/)).toBeInTheDocument();
  });
});

describe("AccountView — custody (ported from Settings)", () => {
  it("locked: shows the Locked row plus Unlock and Reveal, no Lock/Set password", async () => {
    markTauri();
    mockIdentity("locked");
    renderAccount();

    expect(await screen.findByText("Locked")).toBeInTheDocument();
    expect(screen.getByText("Account key (this device)")).toBeInTheDocument();
    expect(screen.getByText(shortKey(DEVICE_KEY))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^unlock$/i })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /reveal recovery phrase/i }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^lock$/i })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^set password$/i }),
    ).not.toBeInTheDocument();
  });

  it("absent: renders no custody rows at all", async () => {
    markTauri();
    mockIdentity("absent");
    renderAccount();

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_identity_state"),
    );
    expect(screen.queryByText("Password lock")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^unlock$/i })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /reveal recovery phrase/i }),
    ).not.toBeInTheDocument();
  });

  it("unlocks via the inline password form and re-fetches state", async () => {
    markTauri();
    let stateCalls = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state") {
        stateCalls += 1;
        return Promise.resolve(
          stateCalls === 1
            ? { state: "locked", pubkey: DEVICE_KEY, mnemonicConfirmed: true }
            : { state: "unlocked", pubkey: DEVICE_KEY, mnemonicConfirmed: true },
        );
      }
      if (cmd === "user_identity_unlock") return Promise.resolve({ pubkey: DEVICE_KEY });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    await screen.findByText("Locked");
    fireEvent.click(screen.getByRole("button", { name: /^unlock$/i }));

    const passwordInput = await screen.findByPlaceholderText("Password");
    fireEvent.change(passwordInput, { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /^unlock$/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_identity_unlock", {
        password: "correct horse battery",
      }),
    );
    expect(await screen.findByText("Unlocked")).toBeInTheDocument();
  });

  it("reveal always re-prompts for a password even when already unlocked, and hides on Done", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "unlocked", pubkey: DEVICE_KEY, mnemonicConfirmed: true });
      if (cmd === "user_identity_reveal")
        return Promise.resolve({ mnemonic: TEST_MNEMONIC });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    await screen.findByText("Unlocked");
    expect(screen.queryByPlaceholderText("Password")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /reveal recovery phrase/i }));

    const passwordInput = await screen.findByPlaceholderText("Password");
    expect(invokeMock).not.toHaveBeenCalledWith("user_identity_reveal", expect.anything());
    fireEvent.change(passwordInput, { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: /^reveal$/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_identity_reveal", {
        password: "correct horse battery",
      }),
    );
    expect(await screen.findByText("abandon")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^done$/i }));
    expect(screen.queryByText("abandon")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /reveal recovery phrase/i }),
    ).toBeInTheDocument();
  });

  it("re-opening reveal after switching panels re-prompts — never a stale grid", async () => {
    // Regression ported from Settings: every panel transition drops the
    // revealed mnemonic, so re-opening reveal always lands on the password
    // form — a stale grid would bypass the always-re-prompt rule.
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "locked", pubkey: DEVICE_KEY, mnemonicConfirmed: true });
      if (cmd === "user_identity_reveal")
        return Promise.resolve({ mnemonic: TEST_MNEMONIC });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    await screen.findByText("Locked");

    fireEvent.click(screen.getByRole("button", { name: /reveal recovery phrase/i }));
    fireEvent.change(await screen.findByPlaceholderText("Password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^reveal$/i }));
    expect(await screen.findByText("abandon")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^unlock$/i }));
    expect(screen.queryByText("abandon")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /reveal recovery phrase/i }));
    expect(await screen.findByPlaceholderText("Password")).toBeInTheDocument();
    expect(screen.queryByText("abandon")).not.toBeInTheDocument();
    expect(
      invokeMock.mock.calls.filter(([cmd]) => cmd === "user_identity_reveal"),
    ).toHaveLength(1);
  });

  it("reveal on a plaintext account key skips the password prompt entirely", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "plaintext", pubkey: DEVICE_KEY, mnemonicConfirmed: true });
      if (cmd === "user_identity_reveal")
        return Promise.resolve({ mnemonic: TEST_MNEMONIC });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    await screen.findByText("Not password-protected");
    fireEvent.click(screen.getByRole("button", { name: /reveal recovery phrase/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_identity_reveal", { password: "" }),
    );
    expect(screen.queryByPlaceholderText("Password")).not.toBeInTheDocument();
    expect(await screen.findByText("abandon")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^done$/i }));
    expect(screen.queryByText("abandon")).not.toBeInTheDocument();
  });

  it("sets a password on a plaintext key via encryptLegacy, then re-fetches to Unlocked", async () => {
    markTauri();
    let stateCalls = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state") {
        stateCalls += 1;
        return Promise.resolve(
          stateCalls === 1
            ? { state: "plaintext", pubkey: DEVICE_KEY, mnemonicConfirmed: true }
            : { state: "unlocked", pubkey: DEVICE_KEY, mnemonicConfirmed: true },
        );
      }
      if (cmd === "user_identity_encrypt") return Promise.resolve({ pubkey: DEVICE_KEY });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    await screen.findByText("Not password-protected");
    fireEvent.click(screen.getByRole("button", { name: /^set password$/i }));

    fireEvent.change(await screen.findByPlaceholderText("Password (min 8 characters)"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.change(screen.getByPlaceholderText("Confirm password"), {
      target: { value: "correct horse battery" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^set password$/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_identity_encrypt", {
        password: "correct horse battery",
      }),
    );
    expect(await screen.findByText("Unlocked")).toBeInTheDocument();
  });

  it("surfaces the identityState() error string on failure (corrupt key file)", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.reject("user.key exists but is corrupt (bad hex)");
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderAccount();

    expect(
      await screen.findByText(/user.key exists but is corrupt/),
    ).toBeInTheDocument();
  });

  it("never invokes any identity command on the web build (no tauri shell)", async () => {
    renderAccount();

    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.queryByText("Account key (this device)")).not.toBeInTheDocument();
    expect(screen.queryByText("Password lock")).not.toBeInTheDocument();
  });
});
