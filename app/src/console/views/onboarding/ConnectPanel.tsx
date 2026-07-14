// The connect panel (epic W1): the dismissible modal that adds a network — the
// create / join / remote forms relocated out of the deleted full-screen "Add
// workspace" gate. Opened from the rail "+" and the account home CTAs; the
// account home is the base surface behind it, so it is always closable.
//
// Two paths mint a local node — found a new network, or join one from an invite
// blob — plus a Remote tab that dials someone else's node (#587 client mode).
// On submit the store mints the node key + network and connects; a joiner falls
// through to JoinProgress while its node parks.

import { useEffect, useState } from "react";

import { color, font, radius, shadow } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";
import type { Workspace } from "../../../domain/workspace-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";

type Mode = "create" | "join" | "remote";

// An invite blob is 🦆 + base64url — whitespace is never part of it. Terminal
// and chat copies hard-wrap the long blob, so a paste arrives with embedded
// newlines/spaces (and chat apps sometimes inject zero-width characters, which
// \s does not cover); strip them all so the field always holds the canonical
// single-line blob.
const sanitizeInviteBlob = (raw: string): string =>
  raw.replace(/[\s\u200B-\u200D\u2060]+/g, "");

const inputStyle: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "9px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.sunken,
  font: `500 12.5px ${font.sans}`,
  color: color.ink,
};

const deleteButtonStyle: React.CSSProperties = {
  all: "unset",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  padding: "0 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.paper,
  font: `600 10.5px ${font.sans}`,
  color: color.red,
  whiteSpace: "nowrap",
};

function Tab({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        flex: 1,
        textAlign: "center",
        padding: "8px 0",
        borderRadius: radius.sm,
        background: active ? color.paper : "transparent",
        boxShadow: active ? shadow.card : "none",
        font: `600 12px ${font.sans}`,
        color: active ? color.ink : color.muted,
      }}
    >
      {label}
    </button>
  );
}

export function ConnectPanel() {
  const { state, actions } = useDucktape();
  const [mode, setMode] = useState<Mode>("create");
  const [name, setName] = useState("");
  const [blob, setBlob] = useState("");
  const [url, setUrl] = useState("");
  const [pendingDelete, setPendingDelete] = useState<{ workspace: Workspace; force: boolean } | null>(null);

  // the invitee's JOIN CODE: fetched once the join tab opens so it is ready to
  // hand the inviter (every invite is locked to it). keygen semantics — the
  // staged identity is reused, so a repeat fetch is harmless.
  useEffect(() => {
    if (mode === "join" && LIVE_JOIN_SUPPORTED && !state.joinCode) actions.joinCode();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  // Escape closes the panel — the account home is always behind it.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") actions.dismissOnboarding();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [actions]);

  const busy = state.onboardingBusy;
  // live join is enabled (LIVE_JOIN_SUPPORTED); joinGated is the kill-switch
  // path that disables the join form should the flag ever be turned back off.
  const joinGated = mode === "join" && !LIVE_JOIN_SUPPORTED;
  const canSubmit =
    !joinGated &&
    (mode === "create"
      ? name.trim().length > 0
      : mode === "join"
        ? name.trim().length > 0 && blob.trim().length > 0
        : url.trim().length > 0);

  const submit = () => {
    if (busy || !canSubmit) return;
    if (mode === "create") actions.createWorkspace(name);
    else if (mode === "join") actions.joinWorkspace(name, blob);
    else actions.connectRemote(url);
  };

  const confirmDelete = (w: Workspace): void => {
    setPendingDelete({ workspace: w, force: false });
  };

  const confirmForceDelete = (w: Workspace): void => {
    setPendingDelete({ workspace: w, force: true });
  };

  const title =
    mode === "create"
      ? "Name your network"
      : mode === "join"
        ? "Join a network"
        : "Connect to a remote node";
  const subtitle =
    mode === "create"
      ? "Found a new network — your account becomes its first member; this device runs its first node."
      : mode === "join"
        ? joinGated
          ? "Joining an existing network is temporarily unavailable."
          : "Paste an invite from a member — this device joins their network with a fresh node key, owned by your account."
        : "Enter the http address of a node running on another device. It stays running there — this app just connects to it.";

  return (
    <div
      // The panel covers the content area (right of the rail) with a scrim; the
      // rail stays visible so the switcher is never trapped behind it.
      role="dialog"
      aria-modal="true"
      aria-label="Add a network"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) actions.dismissOnboarding();
      }}
      style={{
        position: "absolute",
        inset: 0,
        zIndex: 40,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
        background: color.scrimSoft,
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        style={{
          width: 440,
          maxWidth: "100%",
          maxHeight: "100%",
          overflowY: "auto",
          background: color.sidebar,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          boxShadow: shadow.pop,
          padding: 24,
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}
      >
        <div style={{ display: "flex", alignItems: "flex-start", gap: 10 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 5, flex: 1, minWidth: 0 }}>
            <span style={{ font: `600 16px ${font.sans}`, color: color.ink }}>
              {title}
            </span>
            <span style={{ font: `500 12px ${font.sans}`, color: color.muted }}>
              {subtitle}
            </span>
          </div>
          <button
            onClick={() => actions.dismissOnboarding()}
            aria-label="Close"
            title="Close"
            style={{
              all: "unset",
              cursor: "pointer",
              flexShrink: 0,
              width: 26,
              height: 26,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: radius.sm,
              color: color.muted,
              font: `500 16px ${font.sans}`,
            }}
          >
            ✕
          </button>
        </div>

        <div
          style={{
            display: "flex",
            gap: 4,
            padding: 4,
            borderRadius: radius.md,
            background: color.panel,
          }}
        >
          <Tab label="Create" active={mode === "create"} onClick={() => setMode("create")} />
          <Tab label="Join" active={mode === "join"} onClick={() => setMode("join")} />
          <Tab label="Remote" active={mode === "remote"} onClick={() => setMode("remote")} />
        </div>

        {joinGated ? (
          <div
            style={{
              padding: "12px 13px",
              borderRadius: radius.md,
              border: `1px solid ${color.border}`,
              background: color.sunken,
              font: `500 11.5px ${font.sans}`,
              color: color.muted3,
              lineHeight: 1.5,
            }}
          >
            Joining a running network is temporarily unavailable. Found a new
            network to get started, and invite others from the Members view.
          </div>
        ) : (
          <>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {mode === "remote" ? (
                <input
                  value={url}
                  placeholder="http://192.168.1.50:8844"
                  onChange={(event) => setUrl(event.target.value)}
                  onKeyDown={(event) => event.key === "Enter" && submit()}
                  autoCapitalize="off"
                  spellCheck={false}
                  style={{ ...inputStyle, font: `500 11.5px ${font.mono}` }}
                />
              ) : (
                <>
                  <input
                    value={name}
                    placeholder="Network name"
                    onChange={(event) => setName(event.target.value)}
                    onKeyDown={(event) => event.key === "Enter" && mode === "create" && submit()}
                    style={inputStyle}
                  />
                  {mode === "join" && (
                    <>
                      <div
                        style={{
                          padding: "10px 11px",
                          borderRadius: radius.sm,
                          border: `1px solid ${color.border}`,
                          background: color.paper,
                          display: "flex",
                          flexDirection: "column",
                          gap: 6,
                        }}
                      >
                        <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted2, letterSpacing: ".04em" }}>
                          YOUR JOIN CODE
                        </span>
                        <span style={{ font: `500 11px ${font.sans}`, color: color.muted }}>
                          Send this code to whoever is inviting you — invites are locked to it.
                        </span>
                        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                          <code
                            style={{
                              flex: 1,
                              minWidth: 0,
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                              font: `500 10.5px ${font.mono}`,
                              color: color.inkSoft,
                            }}
                          >
                            {state.joinCode ?? "generating…"}
                          </code>
                          <button
                            onClick={() => state.joinCode && navigator.clipboard?.writeText(state.joinCode)}
                            disabled={!state.joinCode}
                            style={{
                              all: "unset",
                              cursor: state.joinCode ? "pointer" : "default",
                              padding: "5px 10px",
                              borderRadius: radius.sm,
                              border: `1px solid ${color.border}`,
                              background: color.sunken,
                              font: `600 10.5px ${font.sans}`,
                              color: state.joinCode ? color.inkSoft : color.muted3,
                            }}
                          >
                            Copy
                          </button>
                        </div>
                      </div>
                      <textarea
                        value={blob}
                        placeholder="Paste invite blob (🦆…)"
                        onChange={(event) => setBlob(sanitizeInviteBlob(event.target.value))}
                        rows={3}
                        style={{ ...inputStyle, resize: "vertical", font: `500 11px ${font.mono}` }}
                      />
                    </>
                  )}
                </>
              )}
            </div>

            {state.error && (
              <span style={{ font: `500 11.5px ${font.mono}`, color: color.red }}>
                {state.error}
              </span>
            )}

            <button
              onClick={submit}
              disabled={busy || !canSubmit}
              style={{
                all: "unset",
                textAlign: "center",
                cursor: busy || !canSubmit ? "default" : "pointer",
                padding: "10px 0",
                borderRadius: radius.md,
                background: busy || !canSubmit ? color.chip : color.dark,
                color: busy || !canSubmit ? color.muted3 : color.onDark,
                font: `600 12.5px ${font.sans}`,
              }}
            >
              {busy
                ? "Setting up…"
                : mode === "create"
                  ? "Create network"
                  : mode === "join"
                    ? "Join network"
                    : "Connect"}
            </button>
          </>
        )}

        {state.workspaces.length > 0 && (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 6,
              paddingTop: 14,
              borderTop: `1px solid ${color.border}`,
            }}
          >
            <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted2, letterSpacing: ".04em" }}>
              YOUR NETWORKS
            </span>
            {state.workspaces.map((w) => (
              <div key={w.id} style={{ display: "flex", alignItems: "stretch", gap: 6 }}>
                <button
                  onClick={() => actions.selectWorkspace(w.id)}
                  style={{
                    all: "unset",
                    cursor: "pointer",
                    flex: 1,
                    minWidth: 0,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 10,
                    padding: "9px 11px",
                    borderRadius: radius.sm,
                    border: `1px solid ${color.border}`,
                    background: color.paper,
                  }}
                >
                  <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>{w.name}</span>
                  <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
                    {w.chainId}
                  </span>
                </button>
                {state.deleteNeedsForce === w.id ? (
                  <button
                    aria-label={`Force delete network ${w.name}`}
                    title="The node couldn't confirm it left its network — force skips that check"
                    onClick={() => confirmForceDelete(w)}
                    style={{ ...deleteButtonStyle, background: color.red, color: color.onDark }}
                  >
                    Force delete
                  </button>
                ) : (
                  <button
                    aria-label={`Delete network ${w.name}`}
                    title="Stop its node and delete this network locally"
                    onClick={() => confirmDelete(w)}
                    style={deleteButtonStyle}
                  >
                    Delete
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {pendingDelete && (
        <ConfirmDialog
          title={
            pendingDelete.force
              ? `Force-delete ${pendingDelete.workspace.name}?`
              : `Delete ${pendingDelete.workspace.name}?`
          }
          confirmLabel={pendingDelete.force ? "Force delete" : "Delete network"}
          onCancel={() => setPendingDelete(null)}
          onConfirm={() => {
            actions.deleteWorkspace(pendingDelete.workspace.id, pendingDelete.force);
            setPendingDelete(null);
          }}
        >
          {pendingDelete.force ? (
            <>
              Its node could not confirm it has left its validator set. Forcing deletes
              the network without that confirmation: its directory, node key,
              and registry entry are removed for good. Only do this for a solo or
              defunct network.
            </>
          ) : (
            <>
              This stops its node and deletes the network locally: directory,
              node key, and registry entry. It is refused while its node is still
              a current validator of a network with other members.
            </>
          )}
        </ConfirmDialog>
      )}
    </div>
  );
}
