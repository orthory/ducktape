// Validator roster over the `valset` module. The node exposes validator keys
// only, so this view intentionally avoids fake presence, roles, or member kind.

import { useMemo } from "react";

import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

const shortKey = (hex: string): string =>
  hex.length > 18 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex || "—";

const initialsOf = (name: string): string => {
  const trimmed = name.trim();
  if (!trimmed) return "?";
  const parts = trimmed.split(/\s+/).filter(Boolean);
  if (parts.length >= 2) return `${parts[0][0]}${parts[1][0]}`.toUpperCase();
  return trimmed.slice(0, 2).toUpperCase();
};

function Avatar({ name }: { name: string }) {
  return (
    <span
      style={{
        width: 32,
        height: 32,
        borderRadius: "50%",
        background: color.chip,
        color: color.muted3,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 12px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}

function ValidatorBadge() {
  return (
    <span
      style={{
        font: `700 9px ${font.mono}`,
        letterSpacing: ".04em",
        color: color.onDark,
        background: color.dark,
        border: `1px solid ${color.dark}`,
        borderRadius: 5,
        padding: "3px 7px",
        flexShrink: 0,
      }}
    >
      VALIDATOR
    </span>
  );
}

function MemberRow({ hex, name }: { hex: string; name: string }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "12px 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.card,
      }}
    >
      <Avatar name={name} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            font: `600 13.5px ${font.sans}`,
            color: color.ink,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {name}
        </div>
        <div
          title={hex}
          style={{
            marginTop: 2,
            font: `400 10.5px ${font.mono}`,
            color: color.muted2,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {shortKey(hex)}
        </div>
      </div>
      <ValidatorBadge />
    </div>
  );
}

export function MembersView() {
  const { state } = useDucktape();
  const rows = useMemo(
    () =>
      state.members.map((hex) => ({
        hex,
        name: state.authorNames[hex] ?? shortKey(hex),
      })),
    [state.authorNames, state.members],
  );

  return (
    <div
      data-screen-label="Members"
      style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column" }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Members</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {rows.length}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          gap: 7,
          padding: "12px 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          flexShrink: 0,
        }}
      >
        <span
          style={{
            font: `500 11.5px ${font.sans}`,
            color: color.ink,
            background: "#e9e9e9",
            borderRadius: radius.sm,
            padding: "5px 11px",
          }}
        >
          All
        </span>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "6px 12px" }}>
        {rows.length === 0 ? (
          <div
            style={{
              padding: "30px 12px",
              textAlign: "center",
              font: `400 12.5px ${font.sans}`,
              color: color.muted2,
            }}
          >
            No validators reported by this node.
          </div>
        ) : (
          rows.map((member) => (
            <MemberRow key={member.hex} hex={member.hex} name={member.name} />
          ))
        )}
      </div>
    </div>
  );
}
