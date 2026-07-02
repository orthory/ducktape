// Local console preferences: the author identity stamped on outgoing messages
// and the accent color. Nothing here touches the node.

import { color, font, radius } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";

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

      <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 10, maxWidth: 520 }}>
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
      </div>
    </div>
  );
}
