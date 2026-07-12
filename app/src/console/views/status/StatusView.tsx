// Node operations panel: the client's honest view of the node it talks to.
// It mirrors the reference Node screen structure while staying wired only to
// real console state: connection, workspace role, height, app hash, and module
// roots.

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

import {
  blocksPerSecond,
  formatLatency,
  formatRate,
  quantile,
  type NodeMetrics,
} from "../../../domain/metrics";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";
import { HealthBar, HealthLegend } from "./HealthBar";
import { LogsTab } from "./LogsTab";
import { commitHealth, healthSegments, nodeLiveness } from "./node-health";
import { NodeFactsCard } from "./NodeFactsCard";
import { PeersTab } from "./PeersTab";
import { SandboxTab } from "./SandboxTab";

type TabId = "overview" | "peers" | "sandbox" | "permissions" | "logs";

const TABS: ReadonlyArray<readonly [TabId, string]> = [
  ["overview", "Overview"],
  ["peers", "Connections"],
  ["sandbox", "Sandbox"],
  ["permissions", "Permissions"],
  ["logs", "Logs"],
];

const sectionLabelStyle = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
} as const;

const STATUS_PILLS = {
  synced: {
    label: "Synced",
    text: tint(color.green).text,
    dot: color.green,
    bg: tint(color.green).bg,
    border: tint(color.green).border,
  },
  stopped: {
    label: "Stopped",
    text: tint(color.red).text,
    dot: color.red,
    bg: tint(color.red).bg,
    border: tint(color.red).border,
  },
  offline: {
    label: "Offline",
    text: tint(color.amber).text,
    dot: color.amber,
    bg: tint(color.amber).bg,
    border: tint(color.amber).border,
  },
} as const;

const statusPill = (connected: boolean, managed: boolean) =>
  connected ? STATUS_PILLS.synced : managed ? STATUS_PILLS.stopped : STATUS_PILLS.offline;

const shortValue = (value: string | null | undefined, start = 12, end = 8): string => {
  if (!value) return "—";
  return value.length > start + end + 1
    ? `${value.slice(0, start)}…${value.slice(-end)}`
    : value;
};

const numberValue = (value: number | null | undefined): string =>
  typeof value === "number" ? value.toLocaleString() : "—";

function workspaceRole(workspace: {
  name: string;
  pubkey: string;
  member: boolean;
  founder: boolean;
} | null) {
  if (workspace?.founder) {
    return {
      id: "genesis",
      pill: "genesis · validator",
      title: "Genesis validator",
      badge: "VALIDATOR",
      tint: color.dark,
      bg: tint(color.accent).bg,
      border: tint(color.accent).border,
      body:
        "This node created the network at genesis. That is provenance only: it validates committed state as an equal member and holds no special governance authority.",
      validator: true,
    } as const;
  }
  if (workspace?.member) {
    return {
      id: "member",
      pill: "member · validator",
      title: "Member validator",
      badge: "VALIDATOR",
      tint: color.accentAlt2,
      bg: tint(color.green).bg,
      border: tint(color.green).border,
      body:
        "This workspace is admitted as a member and runs a validator for the network.",
      validator: true,
    } as const;
  }
  return {
    id: "guest",
    pill: "guest",
    title: "Guest",
    badge: "READ",
    tint: tint(color.amber).text,
    bg: tint(color.amber).bg,
    border: tint(color.amber).border,
    body:
      "No desktop workspace validator identity is loaded. This view can still inspect committed node state.",
    validator: false,
  } as const;
}

function StatusPill({ connected, managed }: { connected: boolean; managed: boolean }) {
  const pill = statusPill(connected, managed);
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        background: pill.bg,
        border: `1px solid ${pill.border}`,
        borderRadius: radius.sm,
        padding: "3px 9px",
        font: `600 11px ${font.mono}`,
        color: pill.text,
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          background: pill.dot,
        }}
      />
      {pill.label}
    </span>
  );
}

function RolePill({ text, active }: { text: string; active: boolean }) {
  return (
    <span
      style={{
        font: `600 9.5px ${font.mono}`,
        letterSpacing: ".05em",
        color: active ? color.onDark : tint(color.amber).text,
        background: active ? color.dark : tint(color.amber).bg,
        border: `1px solid ${active ? color.dark : tint(color.amber).border}`,
        borderRadius: radius.sm,
        padding: "4px 9px",
        textTransform: "uppercase",
      }}
    >
      {text}
    </span>
  );
}

function NodeButton({
  children,
  tone = "neutral",
  onClick,
}: {
  children: ReactNode;
  tone?: "neutral" | "danger" | "primary";
  onClick: () => void;
}) {
  const palette =
    tone === "danger"
      ? { fg: color.red, bg: color.paper, bd: tint(color.red).border }
      : tone === "primary"
        ? { fg: color.onDark, bg: color.dark, bd: color.dark }
        : { fg: color.inkSoft, bg: color.paper, bd: color.borderStrong };
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        font: `600 11.5px ${font.sans}`,
        color: palette.fg,
        background: palette.bg,
        border: `1px solid ${palette.bd}`,
        borderRadius: radius.sm,
        padding: "7px 13px",
      }}
    >
      {children}
    </button>
  );
}

function NodeHeader({
  activeTab,
  setActiveTab,
}: {
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;
}) {
  const { state, actions } = useDucktape();
  const status = state.status;
  const role = workspaceRole(state.workspace);
  const peer = shortValue(state.workspace?.pubkey, 12, 8);
  const version = status?.version ? `v${status.version}` : "v—";

  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          flexWrap: "wrap",
        }}
      >
        <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
          This node
        </div>
        <StatusPill connected={state.connected} managed={state.managed} />
        <RolePill text={role.pill} active={role.validator} />

        {state.managed && (
          <div style={{ marginLeft: "auto", display: "flex", gap: 7 }}>
            {state.connected ? (
              <NodeButton tone="danger" onClick={actions.stopNode}>
                Stop
              </NodeButton>
            ) : (
              <NodeButton tone="primary" onClick={actions.startNode}>
                Start
              </NodeButton>
            )}
          </div>
        )}
      </div>

      <div
        style={{
          font: `400 10.5px ${font.mono}`,
          color: color.muted2,
          marginTop: 7,
        }}
      >
        peer {peer} · ducktape-node {version}
      </div>

      <div
        style={{
          marginTop: 15,
          display: "inline-flex",
          gap: 3,
          background: color.titlebar,
          border: `1px solid ${color.borderSoft}`,
          borderRadius: radius.lg,
          padding: 3,
        }}
      >
        {TABS.map(([id, label]) => {
          const active = activeTab === id;
          return (
            <button
              key={id}
              type="button"
              onClick={() => setActiveTab(id)}
              aria-pressed={active}
              style={{
                all: "unset",
                cursor: "pointer",
                font: `600 11.5px ${font.sans}`,
                color: active ? color.dark : color.muted2,
                background: active ? color.paper : "transparent",
                border: `1px solid ${active ? color.borderStrong : "transparent"}`,
                boxShadow: active ? shadow.card : "none",
                borderRadius: radius.md,
                padding: "6px 17px",
              }}
            >
              {label}
            </button>
          );
        })}
      </div>
    </>
  );
}

function SectionLabel({
  children,
  style,
}: {
  children: ReactNode;
  style?: CSSProperties;
}) {
  return <div style={{ ...sectionLabelStyle, ...style }}>{children}</div>;
}

function CheckRow({ text, active }: { text: string; active: boolean }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        font: `400 12px ${font.sans}`,
        color: active ? color.inkSoft : color.muted2,
      }}
    >
      <span
        style={{
          width: 17,
          height: 17,
          borderRadius: "50%",
          background: active ? tint(color.green).bg : color.titlebar,
          color: active ? tint(color.green).text : color.muted2,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `700 10px ${font.sans}`,
          flexShrink: 0,
        }}
      >
        {active ? "✓" : "−"}
      </span>
      {text}
    </div>
  );
}

function AccessCard() {
  const { state } = useDucktape();
  const role = workspaceRole(state.workspace);
  const workspaceName = state.workspace?.name ?? "Remote node";
  const peer = shortValue(state.workspace?.pubkey, 14, 8);

  return (
    <div
      style={{
        border: `1px solid ${role.border}`,
        background: role.bg,
        borderRadius: radius.lg,
        padding: "16px 18px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <span
          style={{
            width: 36,
            height: 36,
            borderRadius: radius.md,
            background: role.validator ? color.dark : color.paper,
            color: role.validator ? color.onDark : role.tint,
            border: role.validator ? "none" : `1px solid ${role.border}`,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="node" size={18} strokeWidth={1.7} />
        </span>

        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              flexWrap: "wrap",
            }}
          >
            <span style={{ font: `600 14px ${font.sans}`, color: color.dark }}>
              {role.title}
            </span>
            <span
              style={{
                font: `700 8.5px ${font.mono}`,
                letterSpacing: ".06em",
                color: role.validator ? color.onDark : role.tint,
                background: role.validator ? color.dark : color.paper,
                border: `1px solid ${role.validator ? color.dark : role.border}`,
                borderRadius: 5,
                padding: "2px 7px",
              }}
            >
              {role.badge}
            </span>
          </div>
          <div
            style={{
              font: `400 10.5px ${font.mono}`,
              color: color.muted3,
              marginTop: 3,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
            title={state.workspace?.pubkey ?? undefined}
          >
            {workspaceName} · peer {peer}
          </div>
        </div>
      </div>

      <div
        style={{
          marginTop: 14,
          font: `400 12px ${font.sans}`,
          color: color.inkSofter,
          lineHeight: 1.5,
        }}
      >
        {role.body}
      </div>

      <div
        style={{
          marginTop: 14,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))",
          gap: 9,
        }}
      >
        <CheckRow text="Read node status" active={state.connected} />
        <CheckRow text="Verify committed roots" active={Boolean(state.status)} />
        <CheckRow text="Submit module messages" active={state.connected} />
        <CheckRow text="Validate blocks" active={role.validator} />
        <CheckRow text="Admit waiting workspaces" active={role.validator} />
        <CheckRow text="Local daemon controls" active={role.validator && state.managed} />
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  const unavailable = value === "—";
  return (
    <div
      style={{
        border: `1px solid ${unavailable ? color.borderSoft : color.border}`,
        borderRadius: radius.lg,
        background: unavailable ? color.sunken : color.paper,
        padding: "12px 14px",
        minWidth: 0,
      }}
    >
      <div
        style={{
          font: `700 8.5px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
        }}
      >
        {label}
      </div>
      <div
        style={{
          font: `700 20px ${font.mono}`,
          color: unavailable ? color.muted2 : color.dark,
          marginTop: 4,
          minHeight: 24,
        }}
      >
        {value}
      </div>
      {hint && (
        <div
          style={{
            font: `400 11px ${font.sans}`,
            color: color.muted2,
            marginTop: 4,
            lineHeight: 1.35,
          }}
        >
          {hint}
        </div>
      )}
    </div>
  );
}

function CopyValue({
  label,
  value,
  copied,
  onCopy,
  prominent = false,
}: {
  label: string;
  value: string | null | undefined;
  copied: boolean;
  onCopy: () => void;
  prominent?: boolean;
}) {
  const hasValue = Boolean(value);
  const labelText = label.toLowerCase().replace(/[_-]+/g, " ");
  return (
    <button
      type="button"
      disabled={!hasValue}
      onClick={onCopy}
      title={value ?? undefined}
      aria-label={`${copied ? "Copied" : "Copy"} ${labelText}`}
      style={{
        all: "unset",
        cursor: hasValue ? "pointer" : "default",
        display: "grid",
        gridTemplateColumns: prominent ? "1fr" : "132px minmax(0, 1fr) 64px",
        gap: prominent ? 7 : 12,
        alignItems: prominent ? "start" : "center",
        padding: prominent ? "13px 14px" : "10px 13px",
        borderRadius: radius.md,
        border: `1px solid ${copied ? tint(color.green).border : color.border}`,
        background: copied ? tint(color.green).bg : prominent ? color.sunken : color.paper,
        minWidth: 0,
        boxSizing: "border-box",
      }}
    >
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 6,
          font: `700 8.5px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
        }}
      >
        {prominent && <Icon name="hash" size={12} strokeWidth={1.7} />}
        {label}
      </span>
      <span
        style={{
          font: `600 ${prominent ? "13px" : "11.5px"} ${font.mono}`,
          color: hasValue ? color.inkSoft : color.muted2,
          wordBreak: "break-all",
          minWidth: 0,
        }}
      >
        {shortValue(value, prominent ? 24 : 14, prominent ? 16 : 10)}
      </span>
      {!prominent && (
        <span
          style={{
            font: `600 9px ${font.mono}`,
            color: copied ? color.accentAlt2 : color.muted2,
            letterSpacing: ".05em",
          }}
        >
          {copied ? "COPIED" : hasValue ? "COPY" : ""}
        </span>
      )}
      {prominent && (
        <span
          style={{
            justifySelf: "start",
            font: `600 9px ${font.mono}`,
            color: copied ? color.accentAlt2 : color.muted2,
            letterSpacing: ".05em",
          }}
        >
          {copied ? "COPIED" : hasValue ? "CLICK TO COPY" : "WAITING"}
        </span>
      )}
    </button>
  );
}

function StateCommitment() {
  const { state } = useDucktape();
  const status = state.status;
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const copy = (key: string, value: string | null | undefined) => {
    if (!value) return;
    setCopiedKey(key);
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(value).catch(() => {});
    }
    globalThis.setTimeout(() => {
      setCopiedKey((current) => (current === key ? null : current));
    }, 1200);
  };

  return (
    <>
      <div
        style={{
          marginTop: 22,
          display: "flex",
          alignItems: "baseline",
          justifyContent: "space-between",
          gap: 12,
        }}
      >
        <SectionLabel>STATE COMMITMENT</SectionLabel>
        <span style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
          {status
            ? `h ${status.height.toLocaleString()} · ${status.modules.length} modules`
            : "waiting for /v1/status"}
        </span>
      </div>

      <div
        style={{
          marginTop: 9,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          background: color.paper,
          padding: 11,
          display: "flex",
          flexDirection: "column",
          gap: 9,
          boxShadow: shadow.card,
        }}
      >
        <CopyValue
          label="APP HASH"
          value={status?.appHash}
          copied={copiedKey === "appHash"}
          onCopy={() => copy("appHash", status?.appHash)}
          prominent
        />

        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 10,
            padding: "3px 3px 0",
          }}
        >
          <span style={sectionLabelStyle}>MODULE ROOTS</span>
          <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
            {status ? "click any root to copy" : "waiting"}
          </span>
        </div>

        {status?.modules.length ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
            {status.modules.map((mod) => (
              <CopyValue
                key={mod.id}
                label={mod.id}
                value={mod.root}
                copied={copiedKey === `module:${mod.id}`}
                onCopy={() => copy(`module:${mod.id}`, mod.root)}
              />
            ))}
          </div>
        ) : (
          <div
            style={{
              borderRadius: radius.md,
              border: `1px solid ${color.borderSoft}`,
              background: color.sunken,
              padding: "11px 13px",
              font: `400 12px ${font.sans}`,
              color: color.muted2,
            }}
          >
            {status ? "No module roots reported by this node." : "Waiting for /v1/status…"}
          </div>
        )}
      </div>
    </>
  );
}

/** How often the Node overview re-scrapes /metrics for its live cadence and
 *  apply-latency readout. Slower than the Metrics dashboard's 2 s — this is a
 *  glance, not a chart. */
const METRICS_POLL_MS = 2_500;

interface LiveMetrics {
  latest: NodeMetrics | null;
  /** blocks/sec across the last two scrapes, null until a second read lands. */
  blocksPerSec: number | null;
}

/** Poll /metrics while mounted + connected, deriving a live block rate from
 *  successive counter reads. Mirrors MetricsView's poller, trimmed to the two
 *  numbers this overview shows. Resets when the node changes. */
function useLiveMetrics(): LiveMetrics {
  const { state, actions } = useDucktape();
  const { connected, nodeUrl } = state;
  const [latest, setLatest] = useState<NodeMetrics | null>(null);
  const [blocksPerSec, setBlocksPerSec] = useState<number | null>(null);
  const prev = useRef<{ t: number; blocks: number } | null>(null);

  useEffect(() => {
    setLatest(null);
    setBlocksPerSec(null);
    prev.current = null;
    if (!connected) return;
    let cancelled = false;
    const poll = () => {
      actions.readMetrics().then((m) => {
        if (cancelled || !m) return;
        setLatest(m);
        if (!m.present) return;
        const now = Date.now();
        if (prev.current) {
          setBlocksPerSec(blocksPerSecond(prev.current.blocks, m.blocksTotal, now - prev.current.t));
        }
        prev.current = { t: now, blocks: m.blocksTotal };
      });
    };
    poll();
    const timer = setInterval(poll, METRICS_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [connected, nodeUrl, actions]);

  return { latest, blocksPerSec };
}

const LIVENESS_DOT: Record<string, string> = {
  live: color.green,
  idle: color.amber,
  stopped: color.red,
  // a distinct brighter yellow — keeps offline visually apart from idle (both
  // otherwise amber); a vivid status dot reads fine on either theme.
  offline: "#e3b443",
};

/** The status-page commit-health card: a liveness headline, the recent-block
 *  health bar, and its applied/rejected legend. */
function CommitHealthCard() {
  const { state } = useDucktape();
  const segments = useMemo(() => healthSegments(state.blocks, 48), [state.blocks]);
  const health = commitHealth(segments);
  const liveness = nodeLiveness({
    connected: state.connected,
    managed: state.managed,
    tip: state.lastBlock,
  });
  const span =
    segments.length > 0
      ? `#${segments[0].height.toLocaleString()} – #${segments[segments.length - 1].height.toLocaleString()}`
      : null;

  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        padding: "14px 16px",
        boxShadow: shadow.card,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          marginBottom: 12,
        }}
      >
        <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
          <span
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: LIVENESS_DOT[liveness.tone] ?? color.muted2,
              animation: liveness.tone === "live" ? "ik-pulse 1.6s ease-in-out infinite" : undefined,
            }}
          />
          <span style={{ font: `600 13px ${font.sans}`, color: color.dark }}>{liveness.label}</span>
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>
            {liveness.detail}
          </span>
        </span>
        <span style={{ font: `600 11px ${font.mono}`, color: color.muted2 }}>
          {health.total > 0 ? `${health.total} commits` : "—"}
        </span>
      </div>

      {health.total > 0 ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <HealthBar segments={segments} slots={48} live={state.connected} />
          <HealthLegend applied={health.applied} rejected={health.rejected} span={span} />
        </div>
      ) : (
        <div
          style={{
            borderRadius: radius.md,
            border: `1px solid ${color.borderSoft}`,
            background: color.sunken,
            padding: "13px 14px",
            font: `400 12px ${font.sans}`,
            color: color.muted2,
          }}
        >
          {state.connected
            ? "No non-empty blocks committed yet — the health bar fills as real ops land (heartbeat blocks are skipped)."
            : "Not connected — commit health streams from the node's block ring once it's reachable."}
        </div>
      )}
    </div>
  );
}

function OverviewTab() {
  const { state } = useDucktape();
  const { latest, blocksPerSec } = useLiveMetrics();

  const p50 = latest?.present ? quantile(latest.latency, 0.5) : null;
  const cadenceValue = blocksPerSec !== null ? formatRate(blocksPerSec) : "—";
  const cadenceHint = !state.connected
    ? "node offline"
    : latest === null
      ? "reading /metrics…"
      : !latest.present
        ? "no block metrics"
        : p50 !== null
          ? `apply p50 ${formatLatency(p50)}`
          : "warming up";

  return (
    <>
      <SectionLabel>YOUR ACCESS</SectionLabel>
      <div style={{ marginTop: 9 }}>
        <AccessCard />
      </div>
      <div style={{ marginTop: 10 }}>
        <NodeFactsCard />
      </div>

      <SectionLabel style={{ marginTop: 22 }}>NETWORK</SectionLabel>
      <div
        style={{
          marginTop: 9,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(138px, 1fr))",
          gap: 10,
        }}
      >
        <StatCard label="HEIGHT" value={numberValue(state.status?.height)} />
        <StatCard label="VALIDATORS" value={numberValue(state.members.length)} hint="consensus quorum" />
        <StatCard label="RESIDENTS" value={numberValue(state.residents.length)} hint="statesync tier" />
        <StatCard label="CADENCE" value={cadenceValue} hint={cadenceHint} />
      </div>

      <SectionLabel style={{ marginTop: 22 }}>COMMIT HEALTH</SectionLabel>
      <CommitHealthCard />

      <StateCommitment />
    </>
  );
}

const permissionRows = (managed: boolean) => [
  {
    label: "Read committed node status",
    detail: "Connection, version, height, app hash, and module roots.",
    validator: true,
    guest: true,
  },
  {
    label: "Inspect app hash and module roots",
    detail: "Copy the committed state hash and every reported module root.",
    validator: true,
    guest: true,
  },
  {
    label: "Submit module messages",
    detail: "Uses the node API; module policies may still reject a write.",
    validator: true,
    guest: true,
  },
  {
    label: "Validate/finalize blocks",
    detail: "Requires an admitted workspace validator identity.",
    validator: true,
    guest: false,
  },
  {
    label: "Start/stop managed daemon",
    detail: managed
      ? "Available because this desktop app owns the local daemon lifecycle."
      : "Unavailable when this app is only connected to a remote node.",
    validator: managed,
    guest: false,
  },
  {
    label: "Admit waiting workspaces",
    detail: "Runs through the active member workspace, not a public guest link.",
    validator: true,
    guest: false,
  },
];

const permissionGrid = "minmax(220px, 1fr) 116px 116px";

function MatrixCell({
  on,
  active,
  label,
}: {
  on: boolean;
  active: boolean;
  label: string;
}) {
  return (
    <div
      role="cell"
      aria-label={`${label}: ${on ? "available" : "not available"}`}
      style={{
        textAlign: "center",
        padding: "12px 0",
        background: active ? color.canvas : "transparent",
      }}
    >
      <span
        style={{
          color: on ? tint(color.green).text : color.muted2,
          font: `700 13px ${font.sans}`,
        }}
      >
        {on ? "✓" : "−"}
      </span>
    </div>
  );
}

function HeaderCell({
  label,
  active,
}: {
  label: string;
  active: boolean;
}) {
  return (
    <div
      role="columnheader"
      style={{
        textAlign: "center",
        padding: "10px 0",
        font: `700 10.5px ${font.sans}`,
        color: color.inkSoft,
        background: active ? color.hover : "transparent",
      }}
    >
      {label}
    </div>
  );
}

function PermissionsTab() {
  const { state } = useDucktape();
  const role = workspaceRole(state.workspace);
  const rows = permissionRows(state.managed);
  const validatorCount = state.members.length;

  return (
    <>
      <div
        style={{
          font: `400 12px ${font.sans}`,
          color: color.muted3,
          lineHeight: 1.55,
          maxWidth: 680,
        }}
      >
        This panel only distinguishes the roles this app can derive today: an
        admitted workspace validator, or a guest client with no local validator
        identity loaded. Consensus round, peer count, and finality timing are
        intentionally not invented.
      </div>

      <div
        role="table"
        aria-label="Node capability matrix"
        style={{
          marginTop: 13,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          overflow: "hidden",
          maxWidth: 680,
          background: color.paper,
        }}
      >
        <div
          role="row"
          style={{
            display: "grid",
            gridTemplateColumns: permissionGrid,
            alignItems: "center",
            background: color.sunken,
          }}
        >
          <div
            role="columnheader"
            style={{ font: `600 11px ${font.sans}`, color: color.muted, padding: "10px 14px" }}
          >
            capability
          </div>
          <HeaderCell label="Validator" active={role.validator} />
          <HeaderCell label="Guest client" active={!role.validator} />
        </div>

        {rows.map((row) => (
          <div
            key={row.label}
            role="row"
            style={{
              display: "grid",
              gridTemplateColumns: permissionGrid,
              alignItems: "center",
              borderTop: `1px solid ${color.borderSoft}`,
            }}
          >
            <div role="cell" style={{ padding: "11px 14px" }}>
              <div style={{ font: `400 12px ${font.sans}`, color: color.inkSoft }}>
                {row.label}
              </div>
              <div
                style={{
                  font: `400 10.5px ${font.sans}`,
                  color: color.muted2,
                  lineHeight: 1.35,
                  marginTop: 2,
                }}
              >
                {row.detail}
              </div>
            </div>
            <MatrixCell
              on={row.validator}
              active={role.validator}
              label={`${row.label} for validator`}
            />
            <MatrixCell
              on={row.guest}
              active={!role.validator}
              label={`${row.label} for guest client`}
            />
          </div>
        ))}
      </div>

      <div
        style={{
          marginTop: 14,
          maxWidth: 680,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          padding: "15px 17px",
          display: "flex",
          alignItems: "center",
          gap: 13,
          background: color.paper,
        }}
      >
        <span
          style={{
            width: 36,
            height: 36,
            borderRadius: radius.md,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="node" size={18} strokeWidth={1.7} />
        </span>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ font: `600 13px ${font.sans}`, color: color.dark }}>
            Current role: {role.title}
          </div>
          <div
            style={{
              font: `400 11px ${font.sans}`,
              color: color.muted2,
              marginTop: 3,
              lineHeight: 1.5,
            }}
          >
            Daemon controls only appear for managed desktop workspaces; web and
            remote-node builds stay read/submit clients over the configured node.
          </div>
        </div>
        <div style={{ textAlign: "right", flexShrink: 0 }}>
          <div style={{ font: `700 17px ${font.mono}`, color: color.dark }}>
            {validatorCount ? validatorCount.toLocaleString() : "—"}
          </div>
          <div
            style={{
              font: `600 8px ${font.mono}`,
              letterSpacing: ".06em",
              color: color.muted2,
              marginTop: 1,
            }}
          >
            VALIDATORS
          </div>
        </div>
      </div>
    </>
  );
}

export function StatusView() {
  const [activeTab, setActiveTab] = useState<TabId>("overview");

  return (
    <div
      data-screen-label="Node"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
      }}
    >
      <div style={{ flexShrink: 0, padding: "20px 22px 0" }}>
        <NodeHeader activeTab={activeTab} setActiveTab={setActiveTab} />
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          padding: "18px 22px 22px",
        }}
      >
        {activeTab === "peers" ? (
          <PeersTab />
        ) : activeTab === "sandbox" ? (
          <SandboxTab />
        ) : activeTab === "permissions" ? (
          <PermissionsTab />
        ) : activeTab === "logs" ? (
          <LogsTab />
        ) : (
          <OverviewTab />
        )}
      </div>
    </div>
  );
}
