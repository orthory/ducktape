import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { SettingsView } from "./SettingsView";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const renderSettings = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    author: "Rae",
    workspace,
    managed: true,
    connected: true,
    ...patch,
  };
  const spies: Record<string, Mock<(...args: unknown[]) => void>> = {};
  const noop: Mock<(...args: unknown[]) => void> = vi.fn();

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn();
          if (key === "setAccent") {
            return (accent: string) => {
              spies[key]?.(accent);
              setState((prev) => ({ ...prev, accent }));
            };
          }
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <SettingsView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("SettingsView", () => {
  it("renders the thinned surface: account link row, prefs, workspace facts", () => {
    const { spies } = renderSettings();

    const content = document.querySelector('[data-settings-content="full-width"]') as HTMLElement;
    expect(content).toHaveStyle({ width: "100%" });
    expect(content.style.maxWidth).toBe("");

    // Workspace facts that still live here.
    expect(screen.getByText("WORKSPACE")).toBeInTheDocument();
    expect(screen.getByText("Acme Research")).toBeInTheDocument();
    expect(screen.getByText("acme#abcd1234")).toBeInTheDocument();

    // The person moved to the Account view — only a link row remains.
    expect(screen.getByText("ACCOUNT")).toBeInTheDocument();
    expect(screen.queryByText("YOUR IDENTITY")).not.toBeInTheDocument();
    expect(screen.queryByText("DEVICES")).not.toBeInTheDocument();
    expect(screen.queryByText("User key")).not.toBeInTheDocument();
    expect(screen.queryByText("Password lock")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /reveal recovery phrase/i }),
    ).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("Rae")).not.toBeInTheDocument();

    // Everything a module view owns is gone from Settings: ops facts belong
    // to the Node view, invite/admit to Members.
    expect(screen.queryByText("NETWORK")).not.toBeInTheDocument();
    expect(screen.queryByText(/~\/\.ducktape\/workspaces/)).not.toBeInTheDocument();
    expect(screen.queryByText(/quorum threshold/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /reveal invite/i }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open account/i }));
    expect(spies.goHome).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /set accent #3d63b8/i }));
    expect(spies.setAccent).toHaveBeenCalledWith("#3d63b8");

    fireEvent.click(screen.getByRole("button", { name: /workspaces/i }));
    expect(spies.newWorkspace).toHaveBeenCalled();
  });

  it("never invokes any identity command — custody lives on the Account view", async () => {
    renderSettings();
    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("links into the module views that own membership and the daemon", () => {
    const { spies } = renderSettings();

    fireEvent.click(screen.getByRole("button", { name: /open members/i }));
    expect(spies.setScreen).toHaveBeenCalledWith("members");

    fireEvent.click(screen.getByRole("button", { name: /open node/i }));
    expect(spies.setScreen).toHaveBeenCalledWith("status");
  });

  it("shows only read-only node links for a remote client", () => {
    const { spies } = renderSettings({
      workspace: null,
      nodeUrl: "https://node.example",
      managed: false,
    });

    expect(screen.queryByRole("button", { name: /open members/i })).not.toBeInTheDocument();
    expect(screen.queryByText("DANGER ZONE")).not.toBeInTheDocument();
    expect(screen.queryByText("Governance")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open node/i }));
    fireEvent.click(screen.getByRole("button", { name: /open metrics/i }));
    expect(spies.setScreen).toHaveBeenNthCalledWith(1, "status");
    expect(spies.setScreen).toHaveBeenNthCalledWith(2, "metrics");
  });

  it("requests an on-chain leave through an in-app dialog, without tearing down", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    // A member of a set of two -> Request leave is enabled (a real majority is
    // needed, so the node must stay up through its own pending removal).
    const { spies } = renderSettings({
      members: [workspace.pubkey, "beefbeef".repeat(8)],
    });

    // The copy is honest: an on-chain self-removal, node keeps running.
    expect(screen.getByText(/on-chain self-removal/i)).toBeInTheDocument();
    expect(screen.getByText(/keeps running until they approve/i)).toBeInTheDocument();

    try {
      fireEvent.click(screen.getByRole("button", { name: /request leave/i }));
      const dialog = screen.getByRole("dialog", { name: /request to leave/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /request leave/i }));
      expect(spies.requestLeaveWorkspace).toHaveBeenCalled();
      // Requesting a leave never tears down: no node stop, no forget.
      expect(spies.forgetWorkspace?.mock.calls ?? []).toHaveLength(0);
      expect(spies.stopNode?.mock.calls ?? []).toHaveLength(0);
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("aborts the request-leave when the dialog is cancelled", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { spies } = renderSettings({
      members: [workspace.pubkey, "beefbeef".repeat(8)],
    });

    try {
      fireEvent.click(screen.getByRole("button", { name: /request leave/i }));
      const dialog = screen.getByRole("dialog", { name: /request to leave/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /cancel/i }));
      expect(spies.requestLeaveWorkspace?.mock.calls ?? []).toHaveLength(0);
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("forgets the workspace from an in-app dialog (guarded in the backend)", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderSettings();

    try {
      fireEvent.click(screen.getByRole("button", { name: /forget workspace/i }));
      const dialog = screen.getByRole("dialog", { name: /forget Acme Research/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /forget workspace/i }));
      expect(spies.forgetWorkspace).toHaveBeenCalled();
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("hides force-forget until a guarded forget reveals it", () => {
    renderSettings();
    expect(
      screen.queryByRole("button", { name: /force forget workspace/i }),
    ).not.toBeInTheDocument();
  });

  it("force-forgets when the guarded attempt couldn't confirm the node left", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderSettings({ forgetNeedsForce: true });

    try {
      const force = screen.getByRole("button", {
        name: /force forget workspace/i,
      });
      expect(force).toBeEnabled();
      fireEvent.click(force);
      const dialog = screen.getByRole("dialog", { name: /force-forget Acme Research/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /force forget/i }));
      expect(spies.forgetWorkspace).toHaveBeenCalledWith(true);
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("does not lock a validator out of leaving during the cold-start window", () => {
    renderSettings({ members: [] });

    expect(
      screen.getByRole("button", { name: /request leave/i }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", { name: /forget workspace/i }),
    ).toBeEnabled();
    expect(
      screen.queryByText(/can’t remove the last validator/i),
    ).not.toBeInTheDocument();
  });

  it("keeps request-leave disabled for a non-member remote node", () => {
    renderSettings({
      workspace: { ...workspace, member: false },
      members: [],
    });

    expect(
      screen.getByRole("button", { name: /request leave/i }),
    ).toBeDisabled();
  });

  it("disables Request leave for a solo validator (can't remove the last one)", () => {
    renderSettings({ members: [workspace.pubkey] });

    const requestLeave = screen.getByRole("button", { name: /request leave/i });
    expect(requestLeave).toBeDisabled();
    expect(screen.getByRole("button", { name: /forget workspace/i })).toBeEnabled();
  });
});
