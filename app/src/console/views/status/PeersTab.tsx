// The Node view's Connections tab — this node's operational read of the mesh it
// participates in. Rows are the committed valset roster (validators + the
// warming resident tier), but seen through the node's lens: each validator's
// liveness is DERIVED from which blocks it verifiably proposed in the recent
// ring, so a leader that's been quiet reads as quiet — honestly — rather than
// the roster's flat "presence unavailable". Residents hold no quorum seat and
// never propose, so they carry statesync standing, not a fabricated liveness.

import { useMemo, useState, type CSSProperties, type ReactNode } from "react";

import { providersOf } from "../../../domain/capability-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";
import { buildPeers, proposalWindow, type PeerVM } from "./node-health";

type FilterId = "all" | "validators" | "residents" | "active";

const FILTERS: ReadonlyArray<{ id: FilterId; label: string }> = [
  { id: "all", label: "All" },
  { id: "validators", label: "Validators" },
  { id: "residents", label: "Residents" },
  { id: "active", label: "Leading" },
];

const sectionLabel: CSSProperties = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
};

const copyText = (text: string): void => {
  if (!text) return;
  void navigator.clipboard?.writeText(text).catch(() => {});
};

// ── liveness presentation ───────────────────────────────────

type Liveness = { dot: string; label: string; note: string };

function livenessOf(peer: PeerVM, windowTotal: number): Liveness {
  if (peer.tier === "resident") {
    return {
      dot: color.blue,
      label: "statesync",
      note: "resident standing · no quorum seat",
    };
  }
  if (peer.activity) {
    const pct = windowTotal > 0 ? Math.round((peer.activity.count / windowTotal) * 100) : 0;
    return {
      dot: color.green,
      label: "leading",
      note: `led #${peer.activity.lastHeight.toLocaleString()} · ${peer.activity.count} block${
        peer.activity.count === 1 ? "" : "s"
      } (${pct}%)`,
    };
  }
  return {
    dot: color.muted2,
    label: "quiet",
    note: "no block led in the recent ring",
  };
}

// ── atoms ───────────────────────────────────────────────────

function Avatar({ peer }: { peer: PeerVM }) {
  const bg = peer.isFounder ? color.dark : peer.isSelf ? tint(color.accentAlt2).bg : color.chip;
  const fg = peer.isFounder ? color.onDark : peer.isSelf ? tint(color.accentAlt2).text : color.muted3;
  return (
    <span
      aria-hidden="true"
      style={{
        width: 34,
        height: 34,
        borderRadius: "50%",
        background: bg,
        color: fg,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 12px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {peer.initials}
    </span>
  );
}

function RolePill({ peer }: { peer: PeerVM }) {
  const spec = peer.isFounder
    ? { text: color.onDark, bg: color.dark, border: color.dark, label: "genesis" }
    : peer.tier === "validator"
      ? { text: tint(color.green).text, bg: tint(color.green).bg, border: tint(color.green).border, label: "validator" }
      : { text: tint(color.amber).text, bg: tint(color.amber).bg, border: tint(color.amber).border, label: "resident" };
  return (
    <span
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".05em",
        color: spec.text,
        background: spec.bg,
        border: `1px solid ${spec.border}`,
        borderRadius: 5,
        padding: "2px 7px",
        textTransform: "uppercase",
        whiteSpace: "nowrap",
      }}
    >
      {spec.label}
    </span>
  );
}

/** A thin participation bar — the share of the recent window this peer led. */
function ShareBar({ share }: { share: number }) {
  return (
    <div
      style={{
        width: 54,
        height: 6,
        borderRadius: 3,
        background: color.sunken,
        position: "relative",
        flexShrink: 0,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          width: `${Math.max(share * 100, share > 0 ? 6 : 0)}%`,
          borderRadius: 3,
          background: color.green,
        }}
      />
    </div>
  );
}

// One chip per provider, not per announced tag: a node lists a tag for every
// model×effort combo, so the raw set floods the row. Show WHICH providers it
// runs (with a model count), the model list on hover.
function CapChips({ tags }: { tags: string[] }) {
  const groups = providersOf(tags);
  if (groups.length === 0) return null;
  const shown = groups.slice(0, 4);
  const extra = groups.length - shown.length;
  return (
    <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
      {shown.map((group) => (
        <span
          key={group.provider}
          title={(group.models.length ? group.models : group.tags).join("\n")}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            font: `500 9.5px ${font.mono}`,
            color: color.muted3,
            background: color.sunken,
            border: `1px solid ${color.borderSoft}`,
            borderRadius: 5,
            padding: "1px 6px",
          }}
        >
          {group.label}
          {group.models.length > 1 && (
            <span style={{ color: color.muted2 }}>{group.models.length}</span>
          )}
        </span>
      ))}
      {extra > 0 && (
        <span style={{ font: `500 9.5px ${font.mono}`, color: color.muted2 }}>+{extra}</span>
      )}
    </div>
  );
}

// ── row ─────────────────────────────────────────────────────

function PeerRow({ peer, windowTotal }: { peer: PeerVM; windowTotal: number }) {
  const live = livenessOf(peer, windowTotal);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "11px 13px",
        borderRadius: radius.md,
        border: `1px solid ${peer.isSelf ? tint(color.green).border : color.border}`,
        background: peer.isSelf ? tint(color.green).bg : color.paper,
      }}
    >
      <Avatar peer={peer} />

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ font: `600 13px ${font.sans}`, color: color.dark }}>
            {peer.displayName}
          </span>
          <RolePill peer={peer} />
          {peer.isSelf && (
            <span
              style={{
                font: `600 9px ${font.mono}`,
                letterSpacing: ".05em",
                color: tint(color.green).text,
                background: tint(color.green).bg,
                border: `1px solid ${tint(color.green).border}`,
                borderRadius: 5,
                padding: "2px 7px",
                textTransform: "uppercase",
              }}
            >
              this node
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={() => copyText(peer.key)}
          title={`${peer.key}\nClick to copy`}
          style={{
            all: "unset",
            cursor: "pointer",
            font: `400 10.5px ${font.mono}`,
            color: color.muted2,
            marginTop: 3,
          }}
        >
          {peer.shortKey}
        </button>
        {peer.capabilities.length > 0 && (
          <div style={{ marginTop: 6 }}>
            <CapChips tags={peer.capabilities} />
          </div>
        )}
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "flex-end",
          gap: 5,
          flexShrink: 0,
          minWidth: 0,
        }}
      >
        <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: "50%",
              background: live.dot,
              flexShrink: 0,
            }}
          />
          <span style={{ font: `600 10.5px ${font.mono}`, color: color.inkSoft }}>
            {live.label}
          </span>
        </span>
        {peer.tier === "validator" && peer.activity && <ShareBar share={peer.share} />}
        <span
          style={{
            font: `400 9.5px ${font.sans}`,
            color: color.muted2,
            textAlign: "right",
            whiteSpace: "nowrap",
          }}
        >
          {live.note}
        </span>
      </div>
    </div>
  );
}

// ── summary chip ────────────────────────────────────────────

function CountChip({ label, value, tint }: { label: string; value: number; tint?: string }) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 2,
        padding: "9px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        minWidth: 0,
        flex: "1 1 92px",
      }}
    >
      <span style={{ font: `700 17px ${font.mono}`, color: tint ?? color.dark }}>
        {value.toLocaleString()}
      </span>
      <span
        style={{
          font: `700 8px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
          textTransform: "uppercase",
        }}
      >
        {label}
      </span>
    </div>
  );
}

// ── tab ─────────────────────────────────────────────────────

export function PeersTab() {
  const { state } = useDucktape();
  const [filter, setFilter] = useState<FilterId>("all");

  const window = useMemo(() => proposalWindow(state.blocks), [state.blocks]);
  const peers = useMemo(
    () =>
      buildPeers({
        members: state.members,
        residents: state.residents,
        authorNames: state.authorNames,
        workspace: state.workspace,
        capabilitiesByNode: state.capabilitiesByNode,
        window,
      }),
    [state.members, state.residents, state.authorNames, state.workspace, state.capabilitiesByNode, window],
  );

  const validatorCount = state.members.length;
  const residentCount = state.residents.length;
  const activeCount = peers.reduce((n, p) => n + (p.activity ? 1 : 0), 0);

  const visible = peers.filter((p) => {
    switch (filter) {
      case "validators":
        return p.tier === "validator";
      case "residents":
        return p.tier === "resident";
      case "active":
        return Boolean(p.activity);
      default:
        return true;
    }
  });

  const windowNote =
    window.total > 0 && window.low !== null && window.high !== null
      ? `liveness derived from ${window.total} committed block${
          window.total === 1 ? "" : "s"
        } · #${window.low.toLocaleString()}–#${window.high.toLocaleString()}`
      : "liveness appears once this node commits non-empty blocks";

  return (
    <>
      <SectionLabelRow>CONNECTIONS</SectionLabelRow>
      <div style={{ marginTop: 9, display: "flex", gap: 9, flexWrap: "wrap" }}>
        <CountChip label="Peers" value={validatorCount + residentCount} />
        <CountChip label="Validators" value={validatorCount} tint={color.green} />
        <CountChip label="Residents" value={residentCount} tint={color.amber} />
        <CountChip label="Leading" value={activeCount} tint={color.dark} />
      </div>

      <div
        style={{
          marginTop: 15,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          flexWrap: "wrap",
        }}
      >
        <div
          style={{
            display: "inline-flex",
            gap: 3,
            background: color.titlebar,
            border: `1px solid ${color.borderSoft}`,
            borderRadius: radius.lg,
            padding: 3,
          }}
        >
          {FILTERS.map(({ id, label }) => {
            const active = filter === id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setFilter(id)}
                aria-pressed={active}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  font: `600 11px ${font.sans}`,
                  color: active ? color.dark : color.muted2,
                  background: active ? color.paper : "transparent",
                  border: `1px solid ${active ? color.borderStrong : "transparent"}`,
                  boxShadow: active ? shadow.card : "none",
                  borderRadius: radius.md,
                  padding: "5px 13px",
                }}
              >
                {label}
              </button>
            );
          })}
        </div>
        <span style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>{windowNote}</span>
      </div>

      <div style={{ marginTop: 11, display: "flex", flexDirection: "column", gap: 7 }}>
        {visible.length === 0 ? (
          <EmptyState connected={state.connected} filter={filter} />
        ) : (
          visible.map((peer) => (
            <PeerRow key={peer.key} peer={peer} windowTotal={window.total} />
          ))
        )}
      </div>
    </>
  );
}

function SectionLabelRow({ children }: { children: ReactNode }) {
  return <div style={sectionLabel}>{children}</div>;
}

function EmptyState({ connected, filter }: { connected: boolean; filter: FilterId }) {
  const message = !connected
    ? "Not connected — the connection roster loads from the node's valset once it's reachable."
    : filter === "active"
      ? "No validator has led a block in the recent ring yet."
      : filter === "residents"
        ? "No residents — no workspace is warming into the set right now."
        : "No members reported by this node's valset.";
  return (
    <div
      style={{
        borderRadius: radius.md,
        border: `1px solid ${color.borderSoft}`,
        background: color.sunken,
        padding: "14px 15px",
        font: `400 12px ${font.sans}`,
        color: color.muted2,
      }}
    >
      {message}
    </div>
  );
}
