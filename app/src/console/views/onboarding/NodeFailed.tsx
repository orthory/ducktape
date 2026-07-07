// The dedicated "Node failed to start" body. When a MANAGED node fails to spawn
// or connect, the app used to drop to a hollow, disconnected shell whose error
// toast then vanished — no on-screen reason, no way back. This surface shows the
// REAL reason (from daemon.log via workspace_log_tail), an idempotent Retry, the
// log to read, and a way to pick another workspace. Reuses the huddle-card
// danger vocabulary (theme tokens) rather than a new toast system.

import { useState, type CSSProperties } from "react";

import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const primaryBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 11px ${font.sans}`,
  color: "#fff",
  background: color.danger,
  borderRadius: radius.md,
  padding: "7px 14px",
};

const ghostBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 11px ${font.sans}`,
  color: color.inkSoft,
  background: color.paper,
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.md,
  padding: "6px 12px",
};

export function NodeFailed() {
  const { state, actions } = useDucktape();
  const [showLog, setShowLog] = useState(false);
  const boot = state.bootError;
  if (!boot) return null;

  const name = state.workspace?.name ?? "this workspace";
  const hasLog = boot.logTail.trim().length > 0;

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
        background: color.paper,
        overflow: "auto",
      }}
    >
      <div
        style={{
          width: 520,
          maxWidth: "100%",
          border: `1px solid ${color.dangerBorder}`,
          background: color.dangerSoft,
          borderRadius: radius.lg,
          padding: 20,
        }}
      >
        <div style={{ font: `600 13px ${font.sans}`, color: color.danger, marginBottom: 6 }}>
          The node for “{name}” failed to start
        </div>
        <div
          style={{
            font: `500 11.5px ${font.mono}`,
            color: color.ink,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            userSelect: "text",
            marginBottom: 14,
          }}
        >
          {boot.reason}
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button onClick={() => actions.retryConnect()} style={primaryBtn}>
            Retry
          </button>
          <button onClick={() => actions.newWorkspace()} style={ghostBtn}>
            Choose another workspace
          </button>
          {hasLog && (
            <button onClick={() => setShowLog((v) => !v)} style={ghostBtn}>
              {showLog ? "Hide" : "Open"} daemon.log
            </button>
          )}
        </div>

        {hasLog && showLog && (
          <pre
            style={{
              margin: "14px 0 0",
              maxHeight: 260,
              overflow: "auto",
              background: color.paper,
              border: `1px solid ${color.border}`,
              borderRadius: radius.md,
              padding: 10,
              font: `500 10.5px ${font.mono}`,
              color: color.inkSoft,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              userSelect: "text",
            }}
          >
            {boot.logTail}
          </pre>
        )}
        {boot.logPath && (
          <div
            style={{
              marginTop: 10,
              font: `500 10px ${font.mono}`,
              color: color.muted,
              userSelect: "text",
              wordBreak: "break-all",
            }}
          >
            {boot.logPath}
          </div>
        )}
      </div>
    </div>
  );
}
