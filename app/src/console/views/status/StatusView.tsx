// The node status surface: the client's honest view of the node it talks to —
// connection, height, the global app-hash, and each module's authenticated
// root. Read-only; there is no node to manage from here.

import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        padding: "11px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        minWidth: 120,
      }}
    >
      <span style={{ font: `600 10px ${font.sans}`, color: color.muted, letterSpacing: ".05em" }}>
        {label}
      </span>
      <span style={{ font: `600 13px ${font.mono}`, color: color.ink }}>{value}</span>
    </div>
  );
}

const shortHash = (hex: string): string =>
  hex.length > 16 ? `${hex.slice(0, 8)}…${hex.slice(-8)}` : hex || "—";

export function StatusView() {
  const { state } = useDucktape();
  const status = state.status;

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
        Node
      </div>

      <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 17, overflowY: "auto" }}>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <Stat label="CONNECTION" value={state.connected ? "connected" : "disconnected"} />
          <Stat label="HEIGHT" value={(status?.height ?? 0).toLocaleString()} />
          <Stat label="TARGET" value={"__TAURI_INTERNALS__" in window ? "embedded" : "remote"} />
        </div>

        <div
          style={{
            padding: "11px 13px",
            borderRadius: radius.md,
            border: `1px solid ${color.border}`,
            background: color.sunken,
          }}
        >
          <div style={{ font: `600 10px ${font.sans}`, color: color.muted, letterSpacing: ".05em" }}>
            APP-HASH
          </div>
          <div
            style={{
              marginTop: 4,
              font: `500 12px ${font.mono}`,
              color: color.inkSoft,
              wordBreak: "break-all",
            }}
          >
            {status?.appHash ?? "—"}
          </div>
        </div>

        <div>
          <div
            style={{
              font: `600 10px ${font.sans}`,
              color: color.muted,
              letterSpacing: ".05em",
              margin: "0 0 7px 2px",
            }}
          >
            MODULE ROOTS
          </div>
          <div
            style={{
              borderRadius: radius.md,
              border: `1px solid ${color.border}`,
              overflow: "hidden",
            }}
          >
            {(status?.modules ?? []).map((mod, index) => (
              <div
                key={mod.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 13,
                  padding: "9px 13px",
                  background: color.paper,
                  borderTop: index > 0 ? `1px solid ${color.borderSoft}` : "none",
                }}
              >
                <span style={{ font: `600 12px ${font.sans}`, color: color.ink, width: 110 }}>
                  {mod.id}
                </span>
                <span
                  title={mod.root}
                  style={{ font: `400 11.5px ${font.mono}`, color: color.muted3 }}
                >
                  {shortHash(mod.root)}
                </span>
              </div>
            ))}
            {!status && (
              <div style={{ padding: "9px 13px", font: `400 12px ${font.sans}`, color: color.muted2 }}>
                Waiting for the node…
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
