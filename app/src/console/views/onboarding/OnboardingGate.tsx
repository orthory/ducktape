// The front door (desktop): shown when there is no active workspace, or when
// the user asks to add/switch one. Two paths — found a new network, or join one
// from an invite blob — plus a list of existing workspaces to jump back into.
// On submit the store mints identity + workspace and connects; a joiner then
// falls through to JoinProgress while its node parks.

import { useState } from "react";

import { color, font, radius, shadow } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";

type Mode = "create" | "join";

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

export function OnboardingGate() {
  const { state, actions } = useDucktape();
  const [mode, setMode] = useState<Mode>("create");
  const [name, setName] = useState("");
  const [blob, setBlob] = useState("");

  const busy = state.onboardingBusy;
  // live join is enabled (LIVE_JOIN_SUPPORTED); joinGated is the kill-switch
  // path that disables the join form should the flag ever be turned back off.
  const joinGated = mode === "join" && !LIVE_JOIN_SUPPORTED;
  const canSubmit =
    !joinGated &&
    (mode === "create"
      ? name.trim().length > 0
      : name.trim().length > 0 && blob.trim().length > 0);

  const submit = () => {
    if (busy || !canSubmit) return;
    if (mode === "create") actions.createWorkspace(name);
    else actions.joinWorkspace(name, blob);
  };

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: color.paper,
        padding: 24,
      }}
    >
      <div
        style={{
          width: 440,
          maxWidth: "100%",
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
        <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
          <span style={{ font: `600 16px ${font.sans}`, color: color.ink }}>
            {mode === "create" ? "Name your workspace" : "Join a workspace"}
          </span>
          <span style={{ font: `500 12px ${font.sans}`, color: color.muted }}>
            {mode === "create"
              ? "Found a new network — you become its first member, with a fresh identity."
              : joinGated
                ? "Joining an existing network is temporarily unavailable."
                : "Paste an invite from a member to join their network with a new identity."}
          </span>
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
            network to get started, and invite others from Settings.
          </div>
        ) : (
          <>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <input
                value={name}
                placeholder="Workspace name"
                onChange={(event) => setName(event.target.value)}
                onKeyDown={(event) => event.key === "Enter" && mode === "create" && submit()}
                style={inputStyle}
              />
              {mode === "join" && (
                <textarea
                  value={blob}
                  placeholder="Paste invite blob (ducktape-invite-v1:…)"
                  onChange={(event) => setBlob(event.target.value)}
                  rows={3}
                  style={{ ...inputStyle, resize: "vertical", font: `500 11px ${font.mono}` }}
                />
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
                  ? "Create workspace"
                  : "Join workspace"}
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
              YOUR WORKSPACES
            </span>
            {state.workspaces.map((w) => (
              <button
                key={w.id}
                onClick={() => actions.selectWorkspace(w.id)}
                style={{
                  all: "unset",
                  cursor: "pointer",
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
            ))}
          </div>
        )}

        {state.workspace && (
          <button
            onClick={actions.dismissOnboarding}
            style={{
              all: "unset",
              cursor: "pointer",
              textAlign: "center",
              font: `600 11px ${font.sans}`,
              color: color.muted,
            }}
          >
            ← back to {state.workspace.name}
          </button>
        )}
      </div>
    </div>
  );
}
