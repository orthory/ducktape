// The agents surface over the node's `agent` module — the collaboration-loop
// orchestrator. It stays render-only over useDucktape: roster, watches, recent
// runs, and composers all submit through the store action facade.
//
// No optimistic state: every write goes through the store's submit-then-refresh.

import { useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type {
  AgentRecord,
  RunStatus,
  RunView,
  SagaOrigin,
  TurnPolicy,
  WatchView,
} from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

// ── Static labels ───────────────────────────────────────

const ACTION_LABEL: Record<string, string> = {
  "chat.post": "Post to chat",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
};

const ACTION_HINT: Record<string, string> = {
  "chat.post": "Allow chat replies",
  "tasks.create": "Allow creating tasks",
  "tasks.update_status": "Allow task status updates",
};

const POLICY_KINDS = ["Mention", "All", "RoundRobin", "Assigned"] as const;
type PolicyKind = (typeof POLICY_KINDS)[number];

const POLICY_LABEL: Record<PolicyKind, string> = {
  Mention: "Mention",
  All: "All",
  RoundRobin: "Round-robin",
  Assigned: "Assigned",
};

type Tone = { text: string; bg: string; border: string };

const statusTone = {
  success: { text: "#5f9e74", bg: "#eef5f0", border: "#cfe3d7" },
  warning: { text: "#a07b32", bg: "#fbf4e6", border: "#ecdcae" },
  danger: { text: "#a35248", bg: "#fbeeec", border: "#eccfc9" },
  neutral: { text: "#7a6f9e", bg: "#f1edf5", border: "#ddd2e6" },
  blue: { text: "#5f7a9e", bg: "#edf2f7", border: "#cfdae7" },
  agent: { text: accentVar, bg: "#f9f1ea", border: "#e7d2c4" },
} satisfies Record<string, Tone>;

const inputStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "9px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12.5px ${font.sans}`,
  color: color.ink,
};

const monoInputStyle: CSSProperties = {
  ...inputStyle,
  font: `400 12px ${font.mono}`,
};

const secondaryButton: CSSProperties = {
  appearance: "none",
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.sm,
  background: color.paper,
  color: color.inkSoft,
  cursor: "pointer",
  minHeight: 32,
  padding: "0 12px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 7,
  font: `600 12px ${font.sans}`,
  whiteSpace: "nowrap",
  touchAction: "manipulation",
};

const primaryButton = (enabled: boolean): CSSProperties => ({
  ...secondaryButton,
  borderColor: enabled ? color.dark : color.chip,
  background: enabled ? color.dark : color.chip,
  color: enabled ? color.onDark : color.muted2,
  cursor: enabled ? "pointer" : "default",
});

// ── Wire → display helpers ──────────────────────────────

const slug = (raw: string): string =>
  raw
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");

const initialsOf = (name: string): string => {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "AI";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return `${parts[0][0]}${parts[parts.length - 1][0]}`.toUpperCase();
};

const hexOf = (bytes: number[]): string =>
  bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");

const shortHex = (bytes: number[]): string => {
  const hex = hexOf(bytes);
  return hex.length > 18 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex || "—";
};

const shortText = (value: string): string =>
  value.length > 32 ? `${value.slice(0, 18)}…${value.slice(-8)}` : value;

const ownerText = (origin: SagaOrigin): string => {
  if (origin === "System") return "system";
  if ("Module" in origin) return `module:${origin.Module}`;
  return `external:${shortHex(origin.External)}`;
};

const channelLabel = (channels: Channel[], channelId: string): string =>
  channels.find((channel) => channel.id === channelId)?.name ?? channelId;

const agentLabel = (agents: AgentRecord[], agentId: string): string =>
  agents.find((agent) => agent.agent_id === agentId)?.display_name ?? agentId;

const policyText = (policy: TurnPolicy, agents: AgentRecord[]): string => {
  if (policy === "Mention") return "Mention";
  if (policy === "All") return "All agents";
  if (policy === "RoundRobin") return "Round-robin";
  return `Assigned · ${agentLabel(agents, policy.Assigned)}`;
};

const isAwaiting = (
  status: RunStatus,
): status is { AwaitingOracle: { saga_id: string } } =>
  typeof status === "object" && "AwaitingOracle" in status;

const runTone = (status: RunStatus): Tone => {
  if (status === "Done") return statusTone.success;
  if (status === "Cancelled") return statusTone.neutral;
  if (isAwaiting(status)) return statusTone.warning;
  return statusTone.danger;
};

const runLabel = (status: RunStatus): string => {
  if (status === "Done") return "DONE";
  if (status === "Cancelled") return "CANCELLED";
  if (isAwaiting(status)) return "AWAITING ORACLE";
  return "FAILED";
};

const runDetail = (status: RunStatus): string => {
  if (status === "Done") return "completed";
  if (status === "Cancelled") return "cancelled";
  if (isAwaiting(status)) return `saga ${status.AwaitingOracle.saga_id}`;
  return status.Failed.reason;
};

// ── Shared UI atoms ─────────────────────────────────────

function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".11em",
        color: color.muted2,
      }}
    >
      {children}
    </div>
  );
}

function GroupCard({
  children,
  style,
}: {
  children: ReactNode;
  style?: CSSProperties;
}) {
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        boxShadow: shadow.card,
        overflow: "hidden",
        ...style,
      }}
    >
      {children}
    </div>
  );
}

function StatusPill({ label, tone }: { label: string; tone: Tone }) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: 5,
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.text,
        padding: "3px 7px",
        font: `600 9px ${font.mono}`,
        letterSpacing: ".06em",
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

function AgentAvatar({ name, size = 34 }: { name: string; size?: number }) {
  return (
    <span
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        flexShrink: 0,
        borderRadius: Math.max(7, Math.round(size * 0.22)),
        background: color.dark,
        color: color.onDark,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 ${Math.max(10, Math.round(size * 0.31))}px ${font.mono}`,
        letterSpacing: 0,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}

function EmptyState({
  icon,
  title,
  body,
}: {
  icon: "agent" | "hash";
  title: string;
  body: string;
}) {
  return (
    <div
      style={{
        minHeight: 170,
        padding: "30px 18px",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        gap: 8,
        color: color.muted2,
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted,
        }}
      >
        <Icon name={icon} size={17} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div style={{ maxWidth: 300, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
        {body}
      </div>
    </div>
  );
}

function FieldLabel({ htmlFor, children }: { htmlFor: string; children: ReactNode }) {
  return (
    <label
      htmlFor={htmlFor}
      style={{
        display: "block",
        marginBottom: 5,
        font: `600 10px ${font.mono}`,
        letterSpacing: ".05em",
        color: color.muted2,
      }}
    >
      {children}
    </label>
  );
}

function InfoRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        gap: 14,
        border: `1px solid ${color.border}`,
        borderRadius: radius.sm,
        padding: "9px 11px",
        font: `400 11px ${font.mono}`,
        color: color.muted3,
        minWidth: 0,
      }}
    >
      <span style={{ color: color.muted2, whiteSpace: "nowrap" }}>{label}</span>
      <span
        title={typeof value === "string" ? value : undefined}
        translate="no"
        style={{
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          textAlign: "right",
        }}
      >
        {value}
      </span>
    </div>
  );
}

function Chip({ text, tone = statusTone.neutral }: { text: string; tone?: Tone }) {
  return (
    <span
      title={text}
      translate="no"
      style={{
        minWidth: 0,
        maxWidth: 230,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        padding: "3px 7px",
        borderRadius: 5,
        background: tone.bg,
        border: `1px solid ${tone.border}`,
        font: `500 10px ${font.mono}`,
        color: tone.text,
      }}
    >
      {text}
    </span>
  );
}

// ── Agents roster + detail ──────────────────────────────

function AgentListButton({
  agent,
  selected,
  onSelect,
}: {
  agent: AgentRecord;
  selected: boolean;
  onSelect: (agentId: string) => void;
}) {
  const active = agent.status === "Active";
  return (
    <button
      type="button"
      aria-label={`Open details for ${agent.display_name}`}
      onClick={() => onSelect(agent.agent_id)}
      style={{
        appearance: "none",
        border: 0,
        borderBottom: `1px solid ${color.borderSoft}`,
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "13px 14px",
        background: selected ? color.paper : "transparent",
        cursor: "pointer",
        textAlign: "left",
        boxShadow: selected ? `inset 3px 0 0 ${accentVar}` : undefined,
      }}
    >
      <AgentAvatar name={agent.display_name} />
      <span style={{ flex: 1, minWidth: 0 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
          <span
            style={{
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `600 13.5px ${font.sans}`,
              color: color.ink,
            }}
          >
            {agent.display_name}
          </span>
          <StatusPill label="AGENT" tone={statusTone.agent} />
        </span>
        <span
          style={{
            marginTop: 3,
            display: "flex",
            alignItems: "center",
            gap: 7,
            minWidth: 0,
          }}
        >
          <span
            translate="no"
            style={{
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `400 10.5px ${font.mono}`,
              color: color.muted2,
            }}
          >
            {agent.agent_id}
          </span>
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: active ? color.green : color.amber,
              flexShrink: 0,
            }}
          />
          <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted3 }}>
            {active ? "Active" : "Paused"}
          </span>
        </span>
      </span>
    </button>
  );
}

function AgentDetail({
  agent,
  channels,
  onPause,
  onResume,
  onRequestRun,
}: {
  agent: AgentRecord | null;
  channels: Channel[];
  onPause: (agentId: string) => void;
  onResume: (agentId: string) => void;
  onRequestRun: (params: { agentId: string; channelId: string; anchorSeq: number }) => void;
}) {
  if (!agent) {
    return (
      <section aria-label="Agent detail" style={{ minWidth: 0 }}>
        <SectionLabel>AGENT DETAIL</SectionLabel>
        <GroupCard style={{ marginTop: 9 }}>
          <EmptyState
            icon="agent"
            title="No agent selected"
            body="Register an agent or select one from the roster to inspect its backing data."
          />
        </GroupCard>
      </section>
    );
  }

  const active = agent.status === "Active";
  return (
    <section aria-label="Agent detail" style={{ minWidth: 0 }}>
      <SectionLabel>AGENT DETAIL</SectionLabel>
      <GroupCard style={{ marginTop: 9 }}>
        <div style={{ padding: 16 }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: 14 }}>
            <AgentAvatar name={agent.display_name} size={52} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 7, flexWrap: "wrap" }}>
                <h2
                  style={{
                    margin: 0,
                    minWidth: 0,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    font: `600 16px ${font.sans}`,
                    color: color.dark,
                  }}
                >
                  {agent.display_name}
                </h2>
                <StatusPill label="AGENT" tone={statusTone.agent} />
                <StatusPill
                  label={active ? "ACTIVE" : "PAUSED"}
                  tone={active ? statusTone.success : statusTone.warning}
                />
              </div>
              <div
                translate="no"
                style={{
                  marginTop: 4,
                  font: `400 11px ${font.mono}`,
                  color: color.muted2,
                  overflowWrap: "anywhere",
                }}
              >
                {agent.agent_id}
              </div>
            </div>
            <button
              type="button"
              onClick={() => (active ? onPause(agent.agent_id) : onResume(agent.agent_id))}
              style={{
                ...secondaryButton,
                color: active ? color.amber : color.green,
                flexShrink: 0,
              }}
            >
              {active ? "Pause agent" : "Resume agent"}
            </button>
          </div>

          <div
            style={{
              marginTop: 15,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 180px), 1fr))",
              gap: 8,
            }}
          >
            <InfoRow label="model" value={agent.model_ref} />
            <InfoRow label="owner" value={ownerText(agent.owner)} />
            <InfoRow label="prompt" value={shortHex(agent.prompt_hash)} />
            <InfoRow label="updated" value={String(agent.updated_at)} />
          </div>

          <div style={{ marginTop: 15 }}>
            <SectionLabel>CAPABILITIES</SectionLabel>
            <div style={{ marginTop: 8, display: "flex", gap: 7, flexWrap: "wrap" }}>
              {agent.allowed_actions.length === 0 ? (
                <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
                  No write actions granted.
                </span>
              ) : (
                agent.allowed_actions.map((action) => (
                  <Chip
                    key={action}
                    text={ACTION_LABEL[action] ?? action}
                    tone={statusTone.agent}
                  />
                ))
              )}
            </div>
          </div>
        </div>
        <RunRequestForm agent={agent} channels={channels} onRequestRun={onRequestRun} />
      </GroupCard>
    </section>
  );
}

function RunRequestForm({
  agent,
  channels,
  onRequestRun,
}: {
  agent: AgentRecord;
  channels: Channel[];
  onRequestRun: (params: { agentId: string; channelId: string; anchorSeq: number }) => void;
}) {
  const defaultChannelId = channels[0]?.id ?? "";
  const [channelId, setChannelId] = useState(defaultChannelId);
  const [anchorInput, setAnchorInput] = useState("");
  const selectedChannel = channels.find((channel) => channel.id === channelId) ?? null;
  const defaultAnchor = selectedChannel?.head_seq ?? 0;
  const anchorSeq = anchorInput.trim() === "" ? defaultAnchor : Number(anchorInput);
  const hasMessages = defaultAnchor > 0;
  const ready = channelId !== "" && hasMessages && Number.isFinite(anchorSeq) && anchorSeq > 0;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    onRequestRun({ agentId: agent.agent_id, channelId, anchorSeq });
  };

  return (
    <form
      onSubmit={submit}
      style={{
        borderTop: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        padding: 14,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>REQUEST RUN</SectionLabel>
        {!hasMessages && (
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>
            no channel messages yet
          </span>
        )}
      </div>
      <div
        style={{
          marginTop: 9,
          display: "grid",
          gridTemplateColumns: "minmax(0, 1.3fr) minmax(110px, .7fr) auto",
          gap: 8,
          alignItems: "end",
        }}
      >
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-run-channel">Run channel</FieldLabel>
          <select
            id="agent-run-channel"
            name="agent-run-channel"
            value={channelId}
            onChange={(event) => {
              setChannelId(event.target.value);
              setAnchorInput("");
            }}
            disabled={channels.length === 0}
            style={{ ...inputStyle, cursor: channels.length > 0 ? "pointer" : "default" }}
          >
            {channels.length === 0 ? (
              <option value="">No channels</option>
            ) : (
              channels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))
            )}
          </select>
        </div>
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-run-anchor">Anchor sequence</FieldLabel>
          <input
            id="agent-run-anchor"
            name="agent-run-anchor"
            type="number"
            inputMode="numeric"
            min={1}
            max={selectedChannel?.head_seq || undefined}
            value={anchorInput || (defaultAnchor > 0 ? String(defaultAnchor) : "")}
            onChange={(event) => setAnchorInput(event.target.value)}
            disabled={!hasMessages}
            style={monoInputStyle}
          />
        </div>
        <button type="submit" disabled={!ready} style={primaryButton(ready)}>
          Request run
        </button>
      </div>
    </form>
  );
}

// ── Register flow ───────────────────────────────────────

function RegisterAgentForm({
  onRegister,
}: {
  onRegister: (params: {
    displayName: string;
    agentId: string;
    modelRef: string;
    prompt: string;
    allowedActions: string[];
  }) => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [agentIdInput, setAgentIdInput] = useState("");
  const [modelRef, setModelRef] = useState("gpt-5.5-codex");
  const [prompt, setPrompt] = useState("");
  const [allowedActions, setAllowedActions] = useState<string[]>(["chat.post"]);

  const agentId = slug(agentIdInput || displayName);
  const ready =
    displayName.trim() !== "" &&
    agentId !== "" &&
    modelRef.trim() !== "" &&
    prompt.trim() !== "" &&
    allowedActions.length > 0;

  const toggle = (name: string) =>
    setAllowedActions((prev) =>
      prev.includes(name) ? prev.filter((action) => action !== name) : [...prev, name],
    );

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    onRegister({
      displayName: displayName.trim(),
      agentId,
      modelRef: modelRef.trim(),
      prompt,
      allowedActions,
    });
    setDisplayName("");
    setAgentIdInput("");
    setModelRef("gpt-5.5-codex");
    setPrompt("");
    setAllowedActions(["chat.post"]);
  };

  return (
    <section aria-label="Register agent" style={{ minWidth: 0 }}>
      <SectionLabel>REGISTER AGENT</SectionLabel>
      <GroupCard style={{ marginTop: 9 }}>
        <form onSubmit={submit} style={{ padding: 16 }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
            <AgentAvatar name={displayName || "AI"} size={40} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>
                New collaboration worker
              </div>
              <div
                style={{
                  marginTop: 3,
                  font: `400 11.5px ${font.sans}`,
                  color: color.muted2,
                  lineHeight: 1.45,
                }}
              >
                Prompt text is stored by the node, then the registry records its hash.
              </div>
            </div>
            <StatusPill label="AGENT" tone={statusTone.agent} />
          </div>

          <div
            style={{
              marginTop: 14,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 150px), 1fr))",
              gap: 9,
            }}
          >
            <div>
              <FieldLabel htmlFor="agent-display-name">Agent display name</FieldLabel>
              <input
                id="agent-display-name"
                name="agent-display-name"
                type="text"
                autoComplete="off"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
                placeholder="Triage Agent…"
                style={inputStyle}
              />
            </div>
            <div>
              <FieldLabel htmlFor="agent-id">Agent ID</FieldLabel>
              <input
                id="agent-id"
                name="agent-id"
                type="text"
                autoComplete="off"
                spellCheck={false}
                value={agentIdInput}
                onChange={(event) => setAgentIdInput(event.target.value)}
                placeholder={agentId || "triage-agent…"}
                style={monoInputStyle}
              />
            </div>
            <div>
              <FieldLabel htmlFor="agent-model-ref">Model reference</FieldLabel>
              <input
                id="agent-model-ref"
                name="agent-model-ref"
                type="text"
                autoComplete="off"
                spellCheck={false}
                value={modelRef}
                onChange={(event) => setModelRef(event.target.value)}
                placeholder="model…"
                style={monoInputStyle}
              />
            </div>
          </div>

          <div style={{ marginTop: 10 }}>
            <FieldLabel htmlFor="agent-system-prompt">System prompt</FieldLabel>
            <textarea
              id="agent-system-prompt"
              name="agent-system-prompt"
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              rows={5}
              placeholder="Describe what this agent may do…"
              style={{
                ...inputStyle,
                resize: "vertical",
                minHeight: 96,
                lineHeight: 1.45,
              }}
            />
          </div>

          <fieldset
            style={{
              margin: "12px 0 0",
              padding: 0,
              border: 0,
              display: "flex",
              flexDirection: "column",
              gap: 7,
            }}
          >
            <legend
              style={{
                marginBottom: 2,
                padding: 0,
                font: `600 10px ${font.mono}`,
                letterSpacing: ".05em",
                color: color.muted2,
              }}
            >
              CAPABILITIES
            </legend>
            <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
              {KNOWN_ACTIONS.map((name) => {
                const checked = allowedActions.includes(name);
                return (
                  <label
                    key={name}
                    style={{
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 7,
                      border: `1px solid ${checked ? statusTone.agent.border : color.border}`,
                      borderRadius: radius.sm,
                      background: checked ? statusTone.agent.bg : color.paper,
                      padding: "6px 9px",
                      cursor: "pointer",
                      font: `600 10.5px ${font.sans}`,
                      color: checked ? accentVar : color.muted3,
                    }}
                  >
                    <input
                      type="checkbox"
                      name="agent-capability"
                      checked={checked}
                      onChange={() => toggle(name)}
                      style={{ margin: 0 }}
                    />
                    <span>{ACTION_HINT[name] ?? ACTION_LABEL[name] ?? name}</span>
                  </label>
                );
              })}
            </div>
          </fieldset>

          <div
            style={{
              marginTop: 14,
              display: "flex",
              alignItems: "center",
              gap: 10,
              minWidth: 0,
            }}
          >
            <span
              translate="no"
              style={{
                flex: 1,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                font: `400 11px ${font.mono}`,
                color: color.muted2,
              }}
            >
              id {agentId || "—"}
            </span>
            <button type="submit" disabled={!ready} style={primaryButton(ready)}>
              Register agent
            </button>
          </div>
        </form>
      </GroupCard>
    </section>
  );
}

// ── Watches ─────────────────────────────────────────────

function WatchRow({
  watch,
  channels,
  agents,
  onUnwatch,
}: {
  watch: WatchView;
  channels: Channel[];
  agents: AgentRecord[];
  onUnwatch: (id: string) => void;
}) {
  const label = channelLabel(channels, watch.channel_id);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 11,
        padding: "12px 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <span
        style={{
          width: 31,
          height: 31,
          borderRadius: radius.sm,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted2,
          flexShrink: 0,
        }}
      >
        <Icon name="hash" size={15} />
      </span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          title={watch.channel_id}
          translate="no"
          style={{
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            font: `600 12px ${font.mono}`,
            color: color.ink,
          }}
        >
          {label}
        </div>
        <div style={{ marginTop: 2, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          {policyText(watch.policy, agents)}
        </div>
      </div>
      <button
        type="button"
        onClick={() => onUnwatch(watch.channel_id)}
        aria-label={`Stop watching ${label}`}
        style={{ ...secondaryButton, minHeight: 30, color: color.red }}
      >
        Unwatch
      </button>
    </div>
  );
}

function WatchForm({
  channels,
  agents,
  onWatch,
}: {
  channels: Channel[];
  agents: AgentRecord[];
  onWatch: (params: { channelId: string; policy: TurnPolicy }) => void;
}) {
  const [channelId, setChannelId] = useState("");
  const [kind, setKind] = useState<PolicyKind>("Mention");
  const [assigned, setAssigned] = useState("");

  const policy: TurnPolicy | null =
    kind === "Assigned" ? (assigned ? { Assigned: assigned } : null) : kind;
  const ready = channelId !== "" && policy !== null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready || !policy) return;
    onWatch({ channelId, policy });
    setChannelId("");
    setKind("Mention");
    setAssigned("");
  };

  return (
    <form
      onSubmit={submit}
      style={{
        padding: 14,
        borderTop: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns:
            kind === "Assigned"
              ? "minmax(0, 1fr) minmax(0, 1fr) minmax(120px, .8fr) auto"
              : "minmax(0, 1.1fr) minmax(130px, .8fr) auto",
          gap: 8,
          alignItems: "end",
        }}
      >
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-watch-channel">Channel to watch</FieldLabel>
          <select
            id="agent-watch-channel"
            name="agent-watch-channel"
            value={channelId}
            onChange={(event) => setChannelId(event.target.value)}
            disabled={channels.length === 0}
            style={{ ...inputStyle, cursor: channels.length > 0 ? "pointer" : "default" }}
          >
            <option value="">{channels.length === 0 ? "No channels" : "Choose channel…"}</option>
            {channels.map((channel) => (
              <option key={channel.id} value={channel.id}>
                {channel.name}
              </option>
            ))}
          </select>
        </div>
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-watch-policy">Turn policy</FieldLabel>
          <select
            id="agent-watch-policy"
            name="agent-watch-policy"
            value={kind}
            onChange={(event) => setKind(event.target.value as PolicyKind)}
            style={{ ...inputStyle, cursor: "pointer" }}
          >
            {POLICY_KINDS.map((option) => (
              <option key={option} value={option}>
                {POLICY_LABEL[option]}
              </option>
            ))}
          </select>
        </div>
        {kind === "Assigned" && (
          <div style={{ minWidth: 0 }}>
            <FieldLabel htmlFor="agent-watch-assigned">Assigned agent</FieldLabel>
            <select
              id="agent-watch-assigned"
              name="agent-watch-assigned"
              value={assigned}
              onChange={(event) => setAssigned(event.target.value)}
              disabled={agents.length === 0}
              style={{ ...inputStyle, cursor: agents.length > 0 ? "pointer" : "default" }}
            >
              <option value="">{agents.length === 0 ? "No agents" : "Choose agent…"}</option>
              {agents.map((agent) => (
                <option key={agent.agent_id} value={agent.agent_id}>
                  {agent.display_name}
                </option>
              ))}
            </select>
          </div>
        )}
        <button type="submit" disabled={!ready} style={primaryButton(ready)}>
          Watch channel
        </button>
      </div>
    </form>
  );
}

function WatchesPanel({
  channels,
  agents,
  watches,
  onWatch,
  onUnwatch,
}: {
  channels: Channel[];
  agents: AgentRecord[];
  watches: WatchView[];
  onWatch: (params: { channelId: string; policy: TurnPolicy }) => void;
  onUnwatch: (id: string) => void;
}) {
  return (
    <section aria-label="Watches" style={{ minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>WATCHES</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {watches.length}
        </span>
      </div>
      <GroupCard style={{ marginTop: 9 }}>
        {watches.length === 0 ? (
          <EmptyState
            icon="hash"
            title="No watched channels"
            body="Add a real channel watch to let registered agents engage posts by policy."
          />
        ) : (
          watches.map((watch) => (
            <WatchRow
              key={watch.channel_id}
              watch={watch}
              channels={channels}
              agents={agents}
              onUnwatch={onUnwatch}
            />
          ))
        )}
        <WatchForm channels={channels} agents={agents} onWatch={onWatch} />
      </GroupCard>
    </section>
  );
}

// ── Runs timeline ───────────────────────────────────────

function RunRow({
  run,
  agents,
  channels,
  onCancel,
}: {
  run: RunView;
  agents: AgentRecord[];
  channels: Channel[];
  onCancel: (id: string) => void;
}) {
  const tone = runTone(run.status);
  const awaiting = isAwaiting(run.status);
  const agentName = agentLabel(agents, run.agent_id);
  const label = channelLabel(channels, run.channel_id);
  return (
    <div
      style={{
        position: "relative",
        padding: "0 0 16px 38px",
      }}
    >
      <span
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          width: 28,
          height: 28,
          borderRadius: 8,
          background: color.dark,
          color: color.onDark,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `600 9.5px ${font.mono}`,
          boxShadow: "0 0 0 3px #fcfcfc",
        }}
      >
        {initialsOf(agentName)}
      </span>
      <GroupCard>
        <div
          style={{
            background: color.sidebar,
            borderBottom: `1px solid ${color.borderSoft}`,
            padding: "8px 12px",
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <span
            style={{
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `600 12px ${font.sans}`,
              color: color.dark,
            }}
          >
            {agentName}
          </span>
          <StatusPill label={runLabel(run.status)} tone={tone} />
          <StatusPill label={run.job_id ? "JOB" : "CHAT"} tone={run.job_id ? statusTone.agent : statusTone.blue} />
          {awaiting && (
            <button
              type="button"
              onClick={() => onCancel(run.run_id)}
              aria-label={`Cancel run ${run.run_id}`}
              style={{ ...secondaryButton, marginLeft: "auto", minHeight: 28, color: color.red }}
            >
              Cancel
            </button>
          )}
        </div>
        <div style={{ padding: "11px 12px" }}>
          <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
            <Chip text={shortText(run.run_id)} />
            <Chip text={`${label} @${run.anchor_seq}`} tone={statusTone.blue} />
            {run.thread_root !== null && <Chip text={`thread ${run.thread_root}`} />}
            {run.job_id && <Chip text={shortText(run.job_id)} tone={statusTone.agent} />}
          </div>
          <div
            title={runDetail(run.status)}
            style={{
              marginTop: 7,
              font: `400 11px ${font.mono}`,
              color: color.muted2,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {runDetail(run.status)} · updated {run.updated_at}
          </div>
        </div>
      </GroupCard>
    </div>
  );
}

function RunsTimeline({
  runs,
  agents,
  channels,
  onCancel,
}: {
  runs: RunView[];
  agents: AgentRecord[];
  channels: Channel[];
  onCancel: (id: string) => void;
}) {
  return (
    <section aria-label="Runs timeline" style={{ minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>RUNS TIMELINE</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {runs.length}
        </span>
      </div>
      {runs.length === 0 ? (
        <GroupCard style={{ marginTop: 9 }}>
          <EmptyState
            icon="agent"
            title="No runs yet"
            body="Watched channels and explicit requests will appear here newest-first."
          />
        </GroupCard>
      ) : (
        <div style={{ position: "relative", marginTop: 12 }}>
          <div
            style={{
              position: "absolute",
              left: 13,
              top: 10,
              bottom: 20,
              width: 2,
              background: color.border,
            }}
          />
          {runs.map((run) => (
            <RunRow
              key={run.run_id}
              run={run}
              agents={agents}
              channels={channels}
              onCancel={onCancel}
            />
          ))}
        </div>
      )}
    </section>
  );
}

// ── The view ────────────────────────────────────────────

export function AgentView() {
  const { state, actions } = useDucktape();
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const selectedAgent =
    state.agents.find((agent) => agent.agent_id === selectedAgentId) ??
    state.agents[0] ??
    null;
  const activeCount = state.agents.filter((agent) => agent.status === "Active").length;

  return (
    <div
      data-screen-label="Agents"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: "#fcfcfc",
      }}
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
        <span
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.sm,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="agent" size={16} color="currentColor" strokeWidth={1.7} />
        </span>
        <h1 style={{ margin: 0, font: `600 16px ${font.sans}`, color: color.dark }}>
          Agents
        </h1>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {state.agents.length}
        </span>
        <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 8 }}>
          <StatusPill label={`${activeCount} ACTIVE`} tone={statusTone.success} />
          <StatusPill label={`${state.watches.length} WATCHES`} tone={statusTone.neutral} />
          <StatusPill label={`${state.runs.length} RUNS`} tone={statusTone.warning} />
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <aside
          aria-label="Agent roster"
          style={{
            width: "clamp(260px, 31%, 318px)",
            minWidth: 250,
            flexShrink: 0,
            borderRight: `1px solid ${color.borderSoft}`,
            background: color.sidebar,
            display: "flex",
            flexDirection: "column",
          }}
        >
          <div
            style={{
              padding: "14px 14px 9px",
              display: "flex",
              alignItems: "center",
              gap: 8,
            }}
          >
            <SectionLabel>ROSTER</SectionLabel>
            <span style={{ marginLeft: "auto", font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
              {state.agents.length} total
            </span>
          </div>
          <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
            {state.agents.length === 0 ? (
              <EmptyState
                icon="agent"
                title="No agents registered"
                body="Register an agent before configuring runs or assigned watches."
              />
            ) : (
              state.agents.map((agent) => (
                <AgentListButton
                  key={agent.agent_id}
                  agent={agent}
                  selected={selectedAgent?.agent_id === agent.agent_id}
                  onSelect={setSelectedAgentId}
                />
              ))
            )}
          </div>
        </aside>

        <main
          style={{
            flex: 1,
            minWidth: 0,
            minHeight: 0,
            overflowY: "auto",
            padding: 22,
          }}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 360px), 1fr))",
              gap: 18,
              alignItems: "start",
            }}
          >
            <AgentDetail
              agent={selectedAgent}
              channels={state.channels}
              onPause={actions.pauseAgent}
              onResume={actions.resumeAgent}
              onRequestRun={actions.requestRun}
            />
            <RegisterAgentForm onRegister={actions.registerAgent} />
          </div>

          <div
            style={{
              marginTop: 18,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 360px), 1fr))",
              gap: 18,
              alignItems: "start",
            }}
          >
            <WatchesPanel
              channels={state.channels}
              agents={state.agents}
              watches={state.watches}
              onWatch={actions.watchChannel}
              onUnwatch={actions.unwatchChannel}
            />
            <RunsTimeline
              runs={state.runs}
              agents={state.agents}
              channels={state.channels}
              onCancel={actions.cancelRun}
            />
          </div>
        </main>
      </div>
    </div>
  );
}
