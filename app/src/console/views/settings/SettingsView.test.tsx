import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { shortKey } from "../../../domain/names";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { SettingsView } from "./SettingsView";

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
          if (key === "setAuthor") {
            return (author: string) => {
              spies[key]?.(author);
              setState((prev) => ({ ...prev, author }));
            };
          }
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
  it("renders the workspace settings and preserves the existing actions", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { spies } = renderSettings();

    expect(screen.getByText("NETWORK")).toBeInTheDocument();
    expect(screen.getByText("Acme Research")).toBeInTheDocument();
    expect(screen.getByText("~/.ducktape/workspaces/acme-research")).toBeInTheDocument();
    expect(screen.getByText(/p2p 7420/i)).toBeInTheDocument();
    expect(screen.getByText(/member validator/i)).toBeInTheDocument();

    expect(screen.getByText("YOUR IDENTITY")).toBeInTheDocument();
    expect(screen.getByText(/abcdef012345/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /copy key/i }));
    expect(writeText).toHaveBeenCalledWith(workspace.pubkey);

    const name = screen.getByDisplayValue("Rae");
    fireEvent.change(name, { target: { value: "Ari" } });
    expect(spies.setAuthor).toHaveBeenCalledWith("Ari");
    fireEvent.blur(name);
    expect(spies.setDisplayName).toHaveBeenCalledWith("Ari");

    fireEvent.click(screen.getByRole("button", { name: /set accent #3d63b8/i }));
    expect(spies.setAccent).toHaveBeenCalledWith("#3d63b8");

    fireEvent.click(screen.getByRole("button", { name: /workspaces/i }));
    expect(spies.newWorkspace).toHaveBeenCalled();
  });

  it("requests an on-chain leave that keeps the node running, without tearing down", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    // A member of a set of two -> Request leave is enabled (a real majority is
    // needed, so the node must stay up through its own pending removal).
    const { spies } = renderSettings({
      members: [workspace.pubkey, "beefbeef".repeat(8)],
    });

    // The copy is honest: an on-chain self-removal, node keeps running.
    expect(screen.getByText(/on-chain self-removal/i)).toBeInTheDocument();
    expect(screen.getByText(/keeps running until they approve/i)).toBeInTheDocument();
    expect(
      screen.queryByText(/full workspace deletion is not wired/i),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /request leave/i }));
    expect(confirm).toHaveBeenCalledOnce();
    expect(spies.requestLeaveWorkspace).toHaveBeenCalled();
    // Requesting a leave never tears down: no node stop, no forget.
    expect(spies.forgetWorkspace?.mock.calls ?? []).toHaveLength(0);
    expect(spies.stopNode?.mock.calls ?? []).toHaveLength(0);

    confirm.mockRestore();
  });

  it("aborts the request-leave when the confirm is dismissed", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { spies } = renderSettings({
      members: [workspace.pubkey, "beefbeef".repeat(8)],
    });

    fireEvent.click(screen.getByRole("button", { name: /request leave/i }));
    expect(confirm).toHaveBeenCalledOnce();
    // Dismissed confirm -> requestLeaveWorkspace is never referenced, so the
    // proxy never even created a spy for it.
    expect(spies.requestLeaveWorkspace?.mock.calls ?? []).toHaveLength(0);

    confirm.mockRestore();
  });

  it("forgets the workspace on a confirmed forget click (guarded in the backend)", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderSettings();

    fireEvent.click(screen.getByRole("button", { name: /forget workspace/i }));
    expect(confirm).toHaveBeenCalledOnce();
    expect(spies.forgetWorkspace).toHaveBeenCalled();

    confirm.mockRestore();
  });

  it("hides force-forget until a guarded forget reveals it", () => {
    // Default: the guarded forget hasn't failed, so no force override is offered.
    renderSettings();
    expect(
      screen.queryByRole("button", { name: /force forget workspace/i }),
    ).not.toBeInTheDocument();
  });

  it("force-forgets when the guarded attempt couldn't confirm the node left", () => {
    // forgetNeedsForce is set by the store when a guarded forget can't reach the
    // node (bricked / won't start). The override then appears and forces past the
    // liveness guard.
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderSettings({ forgetNeedsForce: true });

    const force = screen.getByRole("button", {
      name: /force forget workspace/i,
    });
    expect(force).toBeEnabled();
    fireEvent.click(force);
    expect(confirm).toHaveBeenCalledOnce();
    // Forces past the guard: forgetWorkspace(true).
    expect(spies.forgetWorkspace).toHaveBeenCalledWith(true);

    confirm.mockRestore();
  });

  it("does not lock a validator out of leaving during the cold-start window", () => {
    // Before the first roster query hydrates state.members it is []. A real
    // member must NOT be locked out of request-leave (or forget) just because
    // the roster hasn't arrived yet — we fall back to workspace.member.
    renderSettings({ members: [] });

    expect(
      screen.getByRole("button", { name: /request leave/i }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", { name: /forget workspace/i }),
    ).toBeEnabled();
    // No confirmed-solo hint before the roster proves the set size.
    expect(
      screen.queryByText(/can’t remove the last validator/i),
    ).not.toBeInTheDocument();
  });

  it("keeps request-leave disabled for a non-member remote node", () => {
    // A remote (non-managed) node, or one whose workspace.member is false, is
    // not a validator: leaving is not its to request.
    renderSettings({
      workspace: { ...workspace, member: false },
      members: [],
    });

    expect(
      screen.getByRole("button", { name: /request leave/i }),
    ).toBeDisabled();
  });

  it("shows the linked device state and this user's other bound nodes", () => {
    const otherNodeKey = "beadbead".repeat(8);
    renderSettings({
      nodeUsers: {
        [workspace.pubkey]: { userKey: "user-1", name: "Rae" },
        [otherNodeKey]: { userKey: "user-1", name: "Rae" },
      },
    });

    expect(screen.getByText("DEVICES")).toBeInTheDocument();
    expect(screen.getByText(shortKey(workspace.pubkey))).toBeInTheDocument();
    expect(screen.getByText("Linked to Rae")).toBeInTheDocument();
    expect(screen.getByText(shortKey(otherNodeKey))).toBeInTheDocument();
  });

  it("shows Not linked when this node has no bound user", () => {
    renderSettings();

    expect(screen.getByText("DEVICES")).toBeInTheDocument();
    expect(screen.getByText("Not linked")).toBeInTheDocument();
  });

  it("omits the Devices section entirely when there is no workspace (web build)", () => {
    renderSettings({ workspace: null });

    expect(screen.queryByText("DEVICES")).not.toBeInTheDocument();
  });

  it("renders the machine user key once user_identity_status resolves (desktop)", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_status")
        return Promise.resolve({ pubkey: "cd34".repeat(16) });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderSettings();

    expect(await screen.findByText("User key")).toBeInTheDocument();
    expect(
      screen.getByText(shortKey("cd34".repeat(16))),
    ).toBeInTheDocument();
  });

  it("surfaces the user_identity_status error string on failure (corrupt user.key)", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_status")
        return Promise.reject("user.key exists but is corrupt (bad hex)");
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderSettings();

    expect(
      await screen.findByText("user.key exists but is corrupt (bad hex)"),
    ).toBeInTheDocument();
  });

  it("never calls user_identity_status on the web build (no tauri shell)", async () => {
    renderSettings();

    // Give any stray microtask a chance to run before asserting the negative.
    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.queryByText("User key")).not.toBeInTheDocument();
  });

  it("disables Request leave for a solo validator (can't remove the last one)", () => {
    // Only this node in the set -> leaving on-chain would empty it, which is
    // refused; the button is disabled and the user forgets instead.
    renderSettings({ members: [workspace.pubkey] });

    const requestLeave = screen.getByRole("button", { name: /request leave/i });
    expect(requestLeave).toBeDisabled();
    // Forget is still available for a solo network.
    expect(screen.getByRole("button", { name: /forget workspace/i })).toBeEnabled();
  });
});
