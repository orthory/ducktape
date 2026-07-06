import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Workspace } from "../../../domain/workspace-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { MembersView } from "./MembersView";

const localKey = "a".repeat(64);
const peerKey = "b".repeat(64);
const joinerKey = "c".repeat(64);
const observerKey = "d".repeat(64);

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

  it("offers a confirmed removal per row but never for this node itself", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderMembers();

    // The peer gets a removal control; this node (the local key) never does.
    const removePeer = screen.getByRole("button", {
      name: /remove Ben Validator from validator set/i,
    });
    expect(removePeer).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /remove Founder Rae from validator set/i }),
    ).not.toBeInTheDocument();

    fireEvent.click(removePeer);
    expect(confirm).toHaveBeenCalledOnce();
    expect(spies.demoteMember).toHaveBeenCalledWith(peerKey);

    confirm.mockRestore();
  });

  it("aborts the removal when the confirm is dismissed", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { spies } = renderMembers();

    fireEvent.click(
      screen.getByRole("button", { name: /remove Ben Validator from validator set/i }),
    );
    expect(confirm).toHaveBeenCalledOnce();
    // Dismissed confirm -> demoteMember is never even referenced, so the proxy
    // never minted a spy for it.
    expect(spies.demoteMember?.mock.calls ?? []).toHaveLength(0);

    confirm.mockRestore();
  });

  it("hides the removal control when this workspace cannot administer", () => {
    renderMembers({
      workspace: { ...workspace, founder: false, member: false },
    });
    expect(
      screen.queryByRole("button", { name: /remove Ben Validator from validator set/i }),
    ).not.toBeInTheDocument();
  });

  it("renders observer standing with confirmed promote and revoke actions", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderMembers({
      observers: [observerKey],
      authorNames: {
        [localKey]: "Founder Rae",
        [peerKey]: "Ben Validator",
        [observerKey]: "Olive Observer",
      },
    });

    expect(screen.getByText("Olive Observer")).toBeInTheDocument();
    expect(screen.getByText("Observer")).toBeInTheDocument();
    // Observer rows govern standing, not a quorum seat — no removal control.
    expect(
      screen.queryByRole("button", { name: /remove Olive Observer from validator set/i }),
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: /promote Olive Observer into the validator set/i }),
    );
    expect(confirm).toHaveBeenCalledOnce();
    expect(spies.promoteMember).toHaveBeenCalledWith(observerKey);

    fireEvent.click(
      screen.getByRole("button", { name: /revoke observer standing from Olive Observer/i }),
    );
    expect(spies.removeObserver).toHaveBeenCalledWith(observerKey);

    confirm.mockRestore();
  });

  it("hides the observer controls when this workspace cannot administer", () => {
    renderMembers({
      observers: [observerKey],
      workspace: { ...workspace, founder: false, member: false },
    });
    expect(screen.getByText("Observer")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /promote/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /revoke observer standing/i }),
    ).not.toBeInTheDocument();
  });

  it("keeps observers out of the Validators filter but in All", () => {
    renderMembers({
      observers: [observerKey],
      authorNames: {
        [observerKey]: "Olive Observer",
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "Validators" }));
    expect(screen.queryByText("Olive Observer")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "All" }));
    expect(screen.getByText("Olive Observer")).toBeInTheDocument();
  });
});
