// YOUR IDENTITY — the person: display name (the canonical editor for the
// origin-gated profiles SetName), role tier badge, and this device's node key.

import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { copyText, HoverButton, outlineButton, smallMono } from "./parts";

const initialsOf = (name: string): string => {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "?";
  return parts
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
};

function workspaceRole(workspace: {
  founder: boolean;
  member: boolean;
} | null) {
  if (workspace?.founder) {
    return {
      role: "genesis validator",
      title: "Genesis validator",
      tier: "GENESIS",
      fg: color.onDark,
      bg: color.dark,
      bd: color.dark,
    } as const;
  }
  if (workspace?.member) {
    return {
      role: "member validator",
      title: "Member validator",
      tier: "MEMBER",
      fg: color.accentAlt2,
      bg: "#eef5f0",
      bd: "#cfe3d7",
    } as const;
  }
  return {
    role: "guest",
    title: "Guest",
    tier: "GUEST",
    fg: color.amber,
    bg: "#fbf4e6",
    bd: "#ecdcae",
  } as const;
}

export function IdentityCard() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const role = workspaceRole(workspace);
  const key = workspace?.pubkey ?? "";
  const keyLine = key
    ? `${key} · key on this device`
    : "no workspace key loaded";

  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        padding: 15,
        display: "flex",
        alignItems: "center",
        gap: 13,
        background: color.paper,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 40,
          height: 40,
          borderRadius: "50%",
          background: "#cdcdcd",
          color: color.muted3,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          font: `600 15px ${font.sans}`,
        }}
      >
        {initialsOf(state.author)}
      </span>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          <input
            aria-label="Display name"
            value={state.author}
            onChange={(event) => actions.setAuthor(event.target.value)}
            onBlur={(event) => actions.setDisplayName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            style={{
              all: "unset",
              width: Math.max(58, Math.min(230, state.author.length * 8 + 12)),
              font: `600 13.5px ${font.sans}`,
              color: color.ink,
            }}
          />
          <span
            title={
              workspace?.founder
                ? "Founding node — created the network at genesis. Provenance only; it confers no special governance authority."
                : undefined
            }
            style={{
              font: `600 9px ${font.mono}`,
              color: role.fg,
              background: role.bg,
              border: `1px solid ${role.bd}`,
              borderRadius: 4,
              padding: "2px 6px",
              letterSpacing: ".04em",
            }}
          >
            {role.tier}
          </span>
          <FinalizationMark op={state.ops[opKey.profile()]} />
        </div>
        <div style={{ ...smallMono, marginTop: 3 }} title={keyLine}>
          {keyLine}
        </div>
      </div>

      <HoverButton
        ariaLabel="Copy key"
        onClick={() => copyText(key)}
        hoverBg={color.titlebar}
        disabled={!key}
        style={outlineButton}
      >
        Copy key
      </HoverButton>
    </div>
  );
}
