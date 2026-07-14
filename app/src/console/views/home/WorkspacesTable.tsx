// Your networks, as a table — the account home's list, echoing the far-left
// rail. One row per network on this machine: its name, its network id, and (for
// the CONNECTED one only, since standing is chain-scoped) whether this device's
// node is a validator, a resident, or has no seat. Clicking anywhere on an
// inactive row — or its Enter button, the keyboard path — routes through
// selectWorkspace, the single node-swap. "+ Add network" opens the connect panel.

import { useState } from "react";

import { normalizeKey } from "../../../domain/names";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, tint } from "../../theme/tokens";
import { HoverButton, outlineButton, SectionLabel } from "../settings/parts";

type Standing = "Validator" | "Resident" | "No seat";

function standingChip(standing: Standing) {
  const palette =
    standing === "Validator"
      ? { fg: color.onDark, bg: color.dark, bd: color.dark }
      : standing === "Resident"
        ? { fg: tint(color.green).text, bg: tint(color.green).bg, bd: tint(color.green).border }
        : { fg: color.muted2, bg: color.sunken, bd: color.border };
  return (
    <span
      style={{
        font: `600 9px ${font.mono}`,
        color: palette.fg,
        background: palette.bg,
        border: `1px solid ${palette.bd}`,
        borderRadius: 4,
        padding: "2px 6px",
        letterSpacing: ".04em",
        whiteSpace: "nowrap",
      }}
    >
      {standing}
    </span>
  );
}

const cell: React.CSSProperties = {
  padding: "9px 12px",
  borderBottom: `1px solid ${color.borderSoft}`,
  font: `500 12px ${font.sans}`,
  color: color.ink,
  textAlign: "left",
  verticalAlign: "middle",
};

const headCell: React.CSSProperties = {
  ...cell,
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".06em",
  color: color.muted,
};

export function WorkspacesTable() {
  const { state, actions } = useDucktape();
  const [hovered, setHovered] = useState<string | null>(null);

  const validators = new Set(state.members.map(normalizeKey));
  const residents = new Set(state.residents.map(normalizeKey));
  const standingOf = (nodeHex: string): Standing =>
    validators.has(normalizeKey(nodeHex))
      ? "Validator"
      : residents.has(normalizeKey(nodeHex))
        ? "Resident"
        : "No seat";

  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginTop: 18,
        }}
      >
        <SectionLabel>YOUR NETWORKS</SectionLabel>
        <HoverButton
          ariaLabel="Add network"
          onClick={() => actions.newWorkspace()}
          hoverBg={color.titlebar}
          style={outlineButton}
        >
          + Add network
        </HoverButton>
      </div>

      <div
        style={{
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          // `overflowX:auto`, NOT `hidden`: the window frame is `overflow:hidden`
          // end to end, so there is no global scrollbar to catch this table when
          // the mono chainId column forces it past the container — the right-hand
          // Active/actions columns were simply clipped away, silently. Scrolling
          // on this axis still clips to the rounded corners (a scroll container
          // always does), so the rounded box is preserved.
          overflowX: "auto",
          background: color.paper,
        }}
      >
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th style={headCell}>Network</th>
              <th style={headCell}>Network ID</th>
              <th style={headCell}>Your standing</th>
              <th style={headCell}>Active</th>
              <th style={{ ...headCell, textAlign: "right" }} />
            </tr>
          </thead>
          <tbody>
            {state.workspaces.map((w, i) => {
              const active = state.workspace?.id === w.id;
              const last = i === state.workspaces.length - 1;
              const rowCell = last ? { ...cell, borderBottom: undefined } : cell;
              return (
                <tr
                  key={w.id}
                  // The whole row enters the workspace; the Enter button is kept
                  // as the keyboard/AT path and fires its own click, so ignore a
                  // click that came from it rather than connecting twice
                  // (connectActive is not idempotent — it drops the transport).
                  onClick={(e) => {
                    if ((e.target as HTMLElement).closest("button")) return;
                    actions.selectWorkspace(w.id);
                  }}
                  onMouseEnter={() => setHovered(w.id)}
                  onMouseLeave={() => setHovered((h) => (h === w.id ? null : h))}
                  style={{
                    cursor: "pointer",
                    background: hovered === w.id ? color.titlebar : undefined,
                  }}
                >
                  <td style={rowCell}>{w.name}</td>
                  <td style={{ ...rowCell, font: `500 11px ${font.mono}`, color: color.muted }}>
                    {w.chainId}
                  </td>
                  <td style={rowCell}>
                    {active ? standingChip(standingOf(w.pubkey)) : <span style={{ color: color.muted3 }}>—</span>}
                  </td>
                  <td style={rowCell}>
                    {active ? (
                      <span
                        aria-label="Active workspace"
                        style={{
                          font: `600 9px ${font.mono}`,
                          color: tint(color.green).text,
                          background: tint(color.green).bg,
                          border: `1px solid ${tint(color.green).border}`,
                          borderRadius: 4,
                          padding: "2px 6px",
                          letterSpacing: ".04em",
                        }}
                      >
                        ACTIVE
                      </span>
                    ) : (
                      <span style={{ color: color.muted3 }}>—</span>
                    )}
                  </td>
                  <td style={{ ...rowCell, textAlign: "right" }}>
                    <HoverButton
                      ariaLabel={`Enter ${w.name}`}
                      onClick={() => actions.selectWorkspace(w.id)}
                      hoverBg={color.titlebar}
                      style={outlineButton}
                    >
                      Enter
                    </HoverButton>
                  </td>
                </tr>
              );
            })}
            {state.workspaces.length === 0 && (
              <tr>
                <td style={{ ...cell, borderBottom: undefined, color: color.muted }} colSpan={5}>
                  No networks yet — add one to get started.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </>
  );
}
