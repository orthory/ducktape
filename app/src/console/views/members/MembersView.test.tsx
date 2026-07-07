import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { shortKey } from "../../../domain/names";
import type { Workspace } from "../../../domain/workspace-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { MembersView } from "./MembersView";

const localKey = "a".repeat(64);
const peerKey = "b".repeat(64);
const joinerKey = "c".repeat(64);
const residentKey = "d".repeat(64);

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: localKey,
  founder: true,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const renderMembers = (patch: Partial<ConsoleState> = {}) => {
  const state = {
    ...createInitialState(),
    workspace,
    members: [localKey, peerKey],
    authorNames: {
      [localKey]: "Founder Rae",
      [peerKey]: "Ben Validator",
    },
    inviteBlob: "ducktape-invite-blob",
    ...patch,
  };
  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  const noop = vi.fn();
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        spies[key] ??= vi.fn();
        return spies[key] ?? noop;
      },
    },
  ) as ConsoleActions;

  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <MembersView />
    </ConsoleContext.Provider>,
  );

  return { spies };
};

describe("MembersView", () => {
  it("opens a detail pane with the selected validator's real profile and key data", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    renderMembers();

    expect(screen.getByText("Founder Rae")).toBeInTheDocument();
    expect(screen.getByText("Ben Validator")).toBeInTheDocument();
    expect(screen.getAllByText("Validator")).not.toHaveLength(0);

    fireEvent.click(screen.getByRole("button", { name: /open member Founder Rae/i }));

    expect(screen.getByRole("heading", { name: "Founder Rae" })).toBeInTheDocument();
    expect(screen.getByText(localKey)).toBeInTheDocument();
    expect(screen.getByText("genesis validator")).toBeInTheDocument();
    expect(screen.getByText("validator key")).toBeInTheDocument();
    expect(screen.getByText("not exposed by this node")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /copy public key/i }));
    expect(writeText).toHaveBeenCalledWith(localKey);
  });

  it("surfaces invite and admit controls only for an admitted workspace", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { spies } = renderMembers();

    fireEvent.click(screen.getByRole("button", { name: /refresh invite/i }));
    expect(spies.revealInvite).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /copy invite/i }));
    expect(writeText).toHaveBeenCalledWith("ducktape-invite-blob");

    fireEvent.change(screen.getByLabelText("Joiner public key"), {
      target: { value: ` ${joinerKey} ` },
    });
    fireEvent.click(screen.getByRole("button", { name: /admit joiner/i }));
    expect(spies.admitMember).toHaveBeenCalledWith(joinerKey);

    cleanup();
    renderMembers({
      workspace: { ...workspace, founder: false, member: false },
      inviteBlob: null,
    });
    expect(
      screen.getByText("Invite and admission controls require an admitted workspace."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /reveal invite/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /admit joiner/i })).not.toBeInTheDocument();
  });

  it("offers an in-app confirmed removal per row but never for this node itself", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderMembers();

    try {
      // The peer gets a removal control; this node (the local key) never does.
      const removePeer = screen.getByRole("button", {
        name: /remove Ben Validator from validator set/i,
      });
      expect(removePeer).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /remove Founder Rae from validator set/i }),
      ).not.toBeInTheDocument();

      fireEvent.click(removePeer);
      const dialog = screen.getByRole("dialog", { name: /remove Ben Validator/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /remove from validators/i }));
      expect(spies.demoteMember).toHaveBeenCalledWith(peerKey);
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("aborts the removal when the dialog is cancelled", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { spies } = renderMembers();

    try {
      fireEvent.click(
        screen.getByRole("button", { name: /remove Ben Validator from validator set/i }),
      );
      const dialog = screen.getByRole("dialog", { name: /remove Ben Validator/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      fireEvent.click(within(dialog).getByRole("button", { name: /cancel/i }));
      expect(spies.demoteMember?.mock.calls ?? []).toHaveLength(0);
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("hides the removal control when this workspace cannot administer", () => {
    renderMembers({
      workspace: { ...workspace, founder: false, member: false },
    });
    expect(
      screen.queryByRole("button", { name: /remove Ben Validator from validator set/i }),
    ).not.toBeInTheDocument();
  });

  it("renders resident standing with in-app confirmed promote and revoke actions", () => {
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderMembers({
      residents: [residentKey],
      authorNames: {
        [localKey]: "Founder Rae",
        [peerKey]: "Ben Validator",
        [residentKey]: "Olive Resident",
      },
    });

    expect(screen.getByText("Olive Resident")).toBeInTheDocument();
    expect(screen.getByText("Resident")).toBeInTheDocument();
    // Resident rows govern standing, not a quorum seat — no removal control.
    expect(
      screen.queryByRole("button", { name: /remove Olive Resident from validator set/i }),
    ).not.toBeInTheDocument();

    try {
      fireEvent.click(
        screen.getByRole("button", { name: /promote Olive Resident into the validator set/i }),
      );
      let dialog = screen.getByRole("dialog", { name: /promote Olive Resident/i });
      expect(nativeConfirm).not.toHaveBeenCalled();
      fireEvent.click(within(dialog).getByRole("button", { name: /promote to validator/i }));
      expect(spies.promoteMember).toHaveBeenCalledWith(residentKey);

      fireEvent.click(
        screen.getByRole("button", { name: /revoke resident standing from Olive Resident/i }),
      );
      dialog = screen.getByRole("dialog", { name: /revoke Olive Resident/i });
      fireEvent.click(within(dialog).getByRole("button", { name: /revoke standing/i }));
      expect(spies.removeResident).toHaveBeenCalledWith(residentKey);
      expect(nativeConfirm).not.toHaveBeenCalled();
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("hides the resident controls when this workspace cannot administer", () => {
    renderMembers({
      residents: [residentKey],
      workspace: { ...workspace, founder: false, member: false },
    });
    expect(screen.getByText("Resident")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /promote/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /revoke resident standing/i }),
    ).not.toBeInTheDocument();
  });

  it("lets this node rename itself inline, but offers no rename for peers", () => {
    const { spies } = renderMembers();

    // Exactly one rename control — the local node's own row.
    const renameButtons = screen.getAllByRole("button", { name: /rename yourself/i });
    expect(renameButtons).toHaveLength(1);

    fireEvent.click(renameButtons[0]);
    const input = screen.getByLabelText("Edit your display name");
    // seeded with the current profile name.
    expect(input).toHaveValue("Founder Rae");

    fireEvent.change(input, { target: { value: "  Rae the Founder  " } });
    fireEvent.click(screen.getByRole("button", { name: /save display name/i }));
    // trimmed and written through the origin-gated profiles action.
    expect(spies.setDisplayName).toHaveBeenCalledWith("Rae the Founder");
  });

  it("discards an inline rename on Escape without writing", () => {
    const { spies } = renderMembers();

    fireEvent.click(screen.getByRole("button", { name: /rename yourself/i }));
    const input = screen.getByLabelText("Edit your display name");
    fireEvent.change(input, { target: { value: "Nope" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.queryByLabelText("Edit your display name")).not.toBeInTheDocument();
    expect(spies.setDisplayName?.mock.calls ?? []).toHaveLength(0);
    // the original name is back on the row.
    expect(screen.getByText("Founder Rae")).toBeInTheDocument();
  });

  it("keeps residents out of the Validators filter but in All", () => {
    renderMembers({
      residents: [residentKey],
      authorNames: {
        [residentKey]: "Olive Resident",
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Validators" }));
    expect(screen.queryByText("Olive Resident")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "All" }));
    expect(screen.getByText("Olive Resident")).toBeInTheDocument();
  });

  it("groups a multi-device user under one header with device-key rows, collapsing single-device users flat", () => {
    const deviceAKey = "e".repeat(64);
    const deviceBKey = "f".repeat(64);
    const soloBoundKey = "9".repeat(64);
    const unboundKey = "7".repeat(64);
    renderMembers({
      members: [deviceAKey, deviceBKey, soloBoundKey, unboundKey],
      // Mirror the provider overlay: a bound node's authorNames entry IS the
      // user's display name (identity overlays profiles at each bound key).
      authorNames: {
        [deviceAKey]: "Casey",
        [deviceBKey]: "Casey",
        [soloBoundKey]: "Solo Sam",
      },
      nodeUsers: {
        [deviceAKey]: { userKey: "user-casey", name: "Casey" },
        [deviceBKey]: { userKey: "user-casey", name: "Casey" },
        [soloBoundKey]: { userKey: "user-sam", name: "Solo Sam" },
      },
    });

    // Two-device user: exactly ONE "Casey" — the group header. The nested
    // rows label by device key, so the name never doubles up.
    expect(screen.getAllByText("Casey")).toHaveLength(1);
    expect(screen.getByText("2 devices")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: `Open member ${shortKey(deviceAKey)}` }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: `Open member ${shortKey(deviceBKey)}` }),
    ).toBeInTheDocument();

    // Single-device user: flat row with the display name, NO group header.
    expect(
      screen.getByRole("button", { name: "Open member Solo Sam" }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Solo Sam")).toHaveLength(1);
    expect(screen.queryByText("1 device")).not.toBeInTheDocument();

    // The unbound key renders exactly as today — standalone, no group.
    expect(
      screen.getByRole("button", { name: `Open member ${shortKey(unboundKey)}` }),
    ).toBeInTheDocument();
  });

  it("shows each node's announced capabilities as chips", () => {
    renderMembers({
      capabilitiesByNode: new Map([[peerKey, ["codex", "claude"]]]),
    });
    // The peer announced two executors — both render as chips on its row.
    expect(screen.getByText("codex")).toBeInTheDocument();
    expect(screen.getByText("claude")).toBeInTheDocument();
  });
});
