// Node operations panel: the client's honest view of the node it talks to.
// It mirrors the reference Node screen structure while staying wired only to
// real console state: connection, workspace role, height, app hash, and module
// roots.

import { useState, type CSSProperties, type ReactNode } from "react";

import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

type TabId = "overview" | "permissions";

const TABS: ReadonlyArray<readonly [TabId, string]> = [
  ["overview", "Overview"],
  ["permissions", "Permissions"],
];

const sectionLabelStyle = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
} as const;

const statusPill = (connected: boolean) =>
  connected
    ? {
        text: "Synced",
        color: "#5f9e74",
        dot: color.green,
        bg: "#eef5f0",
        border: "#cfe3d7",
      }
    : {
        text: "Stopped",
        color: color.red,
        dot: "#cf6a5e",
        bg: "#fbeeec",
        border: "#eccfc9",
      };

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
      id: "founder",
      pill: "admin · validator",
      title: "Admin validator",
      badge: "VALIDATOR",
      tint: color.dark,
      bg: "#f4f1ec",
      border: "#ded6ca",
      body:
        "This workspace founded the network and is admitted to validate committed state.",
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
      bg: "#f3f8f4",
      border: "#d7e3d9",
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
    tint: color.amber,
    bg: "#fbf4e6",
    border: "#ecdcae",
    body:
      "No desktop workspace validator identity is loaded. This view can still inspect committed node state.",
    validator: false,
  } as const;
}

function StatusPill({ connected }: { connected: boolean }) {
  const pill = statusPill(connected);
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
        color: pill.color,
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
      {pill.text}
    </span>
  );
}

function RolePill({ text, active }: { text: string; active: boolean }) {
  return (
    <span
      style={{
        font: `600 9.5px ${font.mono}`,
        letterSpacing: ".05em",
        color: active ? color.onDark : color.muted3,
        background: active ? color.dark : color.paper,
        border: `1px solid ${active ? color.dark : color.borderStrong}`,
        borderRadius: radius.sm,
        padding: "4px 9px",
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
      ? { fg: color.red, bg: color.paper, bd: "#e7cdc8" }
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
        <StatusPill connected={state.connected} />
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
          background: active ? "#e7f1ea" : color.titlebar,
          color: active ? "#5f9e74" : color.muted2,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `700 10px ${font.sans}`,
          flexShrink: 0,
        }}
      >
        {active ? "✓" : "–"}
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
        <CheckRow text="Validate blocks" active={role.validator} />
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
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
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
          color: color.dark,
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
  return (
    <button
      type="button"
      disabled={!hasValue}
      onClick={onCopy}
      title={value ?? undefined}
      style={{
        all: "unset",
        cursor: hasValue ? "pointer" : "default",
        display: "grid",
        gridTemplateColumns: prominent ? "1fr" : "128px minmax(0, 1fr) auto",
        gap: prominent ? 7 : 12,
        alignItems: prominent ? "start" : "center",
        padding: prominent ? "13px 14px" : "10px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: prominent ? color.sunken : color.paper,
        minWidth: 0,
        boxSizing: "border-box",
      }}
    >
      <span
        style={{
          font: `700 8.5px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
        }}
      >
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
          app hash + per-module merkle roots
        </span>
      </div>

      <div
        style={{
          marginTop: 9,
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          background: color.paper,
          padding: 10,
          display: "flex",
          flexDirection: "column",
          gap: 9,
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
            {status ? `${status.modules.length} modules` : "waiting"}
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

function OverviewTab() {
  const { state } = useDucktape();

  return (
    <>
      <SectionLabel>YOUR ACCESS</SectionLabel>
      <div style={{ marginTop: 9 }}>
        <AccessCard />
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
        <StatCard label="ROUND" value="—" hint="not exposed by /v1/status" />
        <StatCard label="PEERS" value="—" hint="not exposed by /v1/status" />
        <StatCard label="FINALITY" value="—" hint="not exposed by /v1/status" />
      </div>

      <StateCommitment />
    </>
  );
}

const permissionRows = [
  { label: "Read committed node status", validator: true, guest: true },
  { label: "Inspect app hash and module roots", validator: true, guest: true },
  { label: "Run as an admitted validator", validator: true, guest: false },
  { label: "Admit joiners from a member workspace", validator: true, guest: false },
];

function MatrixCell({ on }: { on: boolean }) {
  return (
    <div style={{ textAlign: "center", padding: "11px 0" }}>
      <span
        style={{
          color: on ? "#5f9e74" : color.muted2,
          font: `700 13px ${font.sans}`,
        }}
      >
        {on ? "✓" : "–"}
      </span>
    </div>
  );
}

function PermissionsTab() {
  const { state } = useDucktape();
  const role = workspaceRole(state.workspace);

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
        This panel only distinguishes roles the app can honestly derive today:
        an admitted workspace validator, or a guest connection with no local
        workspace identity loaded.
      </div>

      <div
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
          style={{
            display: "grid",
            gridTemplateColumns: "minmax(180px, 1fr) 112px 112px",
            alignItems: "center",
            background: color.sunken,
          }}
        >
          <div style={{ font: `600 11px ${font.sans}`, color: color.muted, padding: "10px 14px" }}>
            capability
          </div>
          <div
            style={{
              textAlign: "center",
              padding: "10px 0",
              font: `700 10.5px ${font.sans}`,
              color: color.inkSoft,
              background: role.validator ? "#f6f3ee" : "transparent",
            }}
          >
            Validator
          </div>
          <div
            style={{
              textAlign: "center",
              padding: "10px 0",
              font: `700 10.5px ${font.sans}`,
              color: color.inkSoft,
              background: !role.validator ? "#f6f3ee" : "transparent",
            }}
          >
            Guest
          </div>
        </div>

        {permissionRows.map((row) => (
          <div
            key={row.label}
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(180px, 1fr) 112px 112px",
              alignItems: "center",
              borderTop: `1px solid ${color.borderSoft}`,
            }}
          >
            <div style={{ font: `400 12px ${font.sans}`, color: color.inkSoft, padding: "11px 14px" }}>
              {row.label}
            </div>
            <MatrixCell on={row.validator} />
            <MatrixCell on={row.guest} />
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
          <Icon name="tasks" size={18} strokeWidth={1.7} />
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
        background: "#fcfcfc",
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
        {activeTab === "permissions" ? <PermissionsTab /> : <OverviewTab />}
      </div>
    </div>
  );
}
