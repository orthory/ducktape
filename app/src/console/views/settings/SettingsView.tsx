// Local console preferences (author identity, accent) plus — on desktop — the
// active workspace surface: its network + identity, the invite blob to share,
// admitting a joiner, and switching workspaces. Preferences touch nothing on
// the node; the workspace actions drive the ~/.ducktape registry.

import { useState } from "react";

import { color, font, radius } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";

const ACCENTS = ["#a05a3c", "#3d63b8", "#3f7d54", "#7a6f9e", "#a35248"];

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 13,
        padding: "13px 15px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
      }}
    >
      <span style={{ font: `500 12.5px ${font.sans}`, color: color.ink }}>{label}</span>
      {children}
    </div>
  );
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span
      style={{
        marginTop: 8,
        font: `600 10.5px ${font.sans}`,
        color: color.muted2,
        letterSpacing: ".04em",
      }}
    >
      {children}
    </span>
  );
}

const mono: React.CSSProperties = {
  font: `500 11px ${font.mono}`,
  color: color.muted3,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  maxWidth: 240,
};

const smallButton: React.CSSProperties = {
  all: "unset",
  cursor: "pointer",
  padding: "6px 12px",
  borderRadius: radius.sm,
  background: color.dark,
  color: color.onDark,
  font: `600 11.5px ${font.sans}`,
};

function WorkspaceSection() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const [pubkey, setPubkey] = useState("");
  if (!workspace) return null;

  return (
    <>
      <SectionLabel>WORKSPACE</SectionLabel>

      <Row label="Network">
        <span style={mono} title={workspace.chainId}>
          {workspace.chainId}
        </span>
      </Row>

      <Row label="Your identity">
        <span style={mono} title={workspace.pubkey}>
          {workspace.pubkey}
        </span>
      </Row>

      {/* inviting + admitting members drives the node's live-admission path
          (landed in PR #77); gated on LIVE_JOIN_SUPPORTED so the same kill-switch
          hides invite/admit if live join is ever turned back off. */}
      {LIVE_JOIN_SUPPORTED && (
        <>
          <Row label="Invite a member">
            <button onClick={actions.revealInvite} style={smallButton}>
              {state.inviteBlob ? "Refresh invite" : "Reveal invite"}
            </button>
          </Row>

          {state.inviteBlob && (
            <textarea
              readOnly
              value={state.inviteBlob}
              rows={2}
              onFocus={(event) => event.currentTarget.select()}
              style={{
                width: "100%",
                boxSizing: "border-box",
                padding: "9px 11px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderStrong}`,
                background: color.sunken,
                font: `500 10.5px ${font.mono}`,
                color: color.inkSoft,
                resize: "vertical",
              }}
            />
          )}

          <Row label="Admit a joiner">
            <div style={{ display: "flex", gap: 7 }}>
              <input
                value={pubkey}
                placeholder="joiner pubkey"
                onChange={(event) => setPubkey(event.target.value)}
                style={{
                  width: 150,
                  padding: "6px 10px",
                  borderRadius: radius.sm,
                  border: `1px solid ${color.borderStrong}`,
                  background: color.sunken,
                  font: `500 11px ${font.mono}`,
                  color: color.ink,
                }}
              />
              <button
                onClick={() => {
                  actions.admitMember(pubkey);
                  setPubkey("");
                }}
                style={smallButton}
              >
                Admit
              </button>
            </div>
          </Row>
        </>
      )}

      <Row label="Switch workspace">
        <button onClick={actions.newWorkspace} style={smallButton}>
          Workspaces
        </button>
      </Row>
    </>
  );
}

export function SettingsView() {
  const { state, actions } = useDucktape();

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
          font: `600 13px ${font.sans}`,
          color: color.ink,
        }}
      >
        Settings
      </div>

      <div
        style={{
          padding: 17,
          display: "flex",
          flexDirection: "column",
          gap: 10,
          maxWidth: 520,
          overflowY: "auto",
        }}
      >
        <Row label="Display name">
          <input
            value={state.author}
            onChange={(event) => actions.setAuthor(event.target.value)}
            style={{
              width: 180,
              padding: "6px 10px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.sunken,
              font: `500 12px ${font.sans}`,
              color: color.ink,
              textAlign: "right",
            }}
          />
        </Row>

        <Row label="Accent">
          <div style={{ display: "flex", gap: 7 }}>
            {ACCENTS.map((accent) => (
              <button
                key={accent}
                onClick={() => actions.setAccent(accent)}
                title={accent}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  width: 22,
                  height: 22,
                  borderRadius: "50%",
                  background: accent,
                  boxShadow:
                    state.accent === accent
                      ? `0 0 0 2px ${color.paper}, 0 0 0 4px ${accent}`
                      : "none",
                }}
              />
            ))}
          </div>
        </Row>

        <WorkspaceSection />
      </div>
    </div>
  );
}
