// The agents surface over the node's `agent` module — the collaboration-loop
// orchestrator. It is render-only over useDucktape: roster, watches, recent
// runs, and the local composers all submit through the store action facade.
//
// No optimistic state: every write goes through the store's submit-then-refresh.

import { useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type {
  AgentRecord,
  RunStatus,
  RunView,
  TurnPolicy,
  WatchView,
} from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

// ── Static labels ───────────────────────────────────────

/** Friendly labels for the action vocabulary (KNOWN_ACTIONS). */
const ACTION_LABEL: Record<string, string> = {
  "chat.post": "Post to chat",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
};

/** The turn policies a watch composer can pick. `Assigned` needs an agent. */
const POLICY_KINDS = ["Mention", "All", "RoundRobin", "Assigned"] as const;
type PolicyKind = (typeof POLICY_KINDS)[number];

const POLICY_LABEL: Record<PolicyKind, string> = {
  Mention: "Mention",
  All: "All",
  RoundRobin: "Round-robin",
  Assigned: "Assigned",
};

const statusTone = {
  success: { text: "#5f9e74", bg: "#eef5f0", border: "#cfe3d7" },
  warning: { text: "#a07b32", bg: "#fbf4e6", border: "#ecdcae" },
  danger: { text: "#a35248", bg: "#fbeeec", border: "#eccfc9" },
  neutral: { text: "#7a6f9e", bg: "#f1edf5", border: "#ddd2e6" },
  agent: { text: accentVar, bg: "#f9f1ea", border: "#e7d2c4" },
} as const;

const fieldStyle: CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const ghostButton: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  padding: "7px 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.border}`,
  background: color.paper,
  color: color.inkSoft,
  font: `600 11.5px ${font.sans}`,
};

// ── Wire → display helpers ──────────────────────────────

/** A wire-safe slug for an agent id — same rule as channelIdOf / docIdOf. */
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
  return `${parts[0][0]}${parts[1][0]}`.toUpperCase();
};

const hexPreview = (bytes: number[]): string => {
  const hex = bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return hex.length > 18 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex;
};

const policyText = (policy: TurnPolicy): string => {
  if (policy === "Mention") return "Mention";
  if (policy === "All") return "All";
  if (policy === "RoundRobin") return "Round-robin";
  return `Assigned · ${policy.Assigned}`;
};

const runTone = (status: RunStatus): (typeof statusTone)[keyof typeof statusTone] => {
  if (status === "Done") return statusTone.success;
  if (status === "Cancelled") return statusTone.neutral;
  if ("AwaitingOracle" in status) return statusTone.warning;
  return statusTone.danger;
};

const runLabel = (status: RunStatus): string => {
  if (status === "Done") return "DONE";
  if (status === "Cancelled") return "CANCELLED";
  if ("AwaitingOracle" in status) return "AWAITING ORACLE";
  return "FAILED";
};

const runDetail = (status: RunStatus): string => {
  if (typeof status === "object" && "Failed" in status) return status.Failed.reason;
  if (typeof status === "object" && "AwaitingOracle" in status) {
    return `saga ${status.AwaitingOracle.saga_id}`;
  }
  return status.toLowerCase();
};

/** Only an awaiting run can be cancelled — the sole non-terminal state. */
const isAwaiting = (status: RunStatus): boolean =>
  status !== "Done" && status !== "Cancelled" && "AwaitingOracle" in status;

// ── Small local bits ────────────────────────────────────

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

function StatusPill({
  label,
  tone,
}: {
  label: string;
  tone: (typeof statusTone)[keyof typeof statusTone];
}) {
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
      style={{
        width: size,
        height: size,
        flexShrink: 0,
        borderRadius: 8,
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

function EmptyState({ icon, title, body }: { icon: "agent" | "hash"; title: string; body: string }) {
  return (
    <div
      style={{
        padding: "26px 18px",
        borderRadius: radius.lg,
        border: `1px dashed ${color.borderStrong}`,
        background: color.sidebar,
        display: "flex",
        alignItems: "center",
        gap: 13,
      }}
    >
      <div
        style={{
          width: 38,
          height: 38,
          borderRadius: 9,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted2,
          flexShrink: 0,
        }}
      >
        <Icon name={icon} size={18} />
      </div>
      <div style={{ minWidth: 0 }}>
        <div style={{ font: `600 13px ${font.sans}`, color: color.ink }}>{title}</div>
        <div style={{ marginTop: 2, font: `400 12px ${font.sans}`, color: color.muted2 }}>
          {body}
        </div>
      </div>
    </div>
  );
}

function Chip({ text, tint = color.muted3 }: { text: string; tint?: string }) {
  return (
    <span
      title={text}
      style={{
        minWidth: 0,
        maxWidth: 220,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        padding: "3px 7px",
        borderRadius: 5,
        background: color.sunken,
        border: `1px solid ${color.border}`,
        font: `500 10px ${font.mono}`,
        color: tint,
      }}
    >
      {text}
    </span>
  );
}

// ── Agents roster ───────────────────────────────────────

function AgentRow({
  agent,
  last,
  onPause,
  onResume,
}: {
  agent: AgentRecord;
  last?: boolean;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
}) {
  const active = agent.status === "Active";
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "13px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
        animation: "ik-fade .16s ease-out",
      }}
    >
      <AgentAvatar name={agent.display_name} />

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
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
        </div>
        <div
          style={{
            marginTop: 3,
            display: "flex",
            alignItems: "center",
            gap: 7,
            minWidth: 0,
            flexWrap: "wrap",
          }}
        >
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            {agent.agent_id}
          </span>
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.blue }}>
            {agent.model_ref}
          </span>
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            prompt {hexPreview(agent.prompt_hash)}
          </span>
        </div>
      </div>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          gap: 9,
          flexShrink: 0,
        }}
      >
        <StatusPill
          label={active ? "ACTIVE" : "PAUSED"}
          tone={active ? statusTone.success : statusTone.warning}
        />
        <button
          type="button"
          onClick={() => (active ? onPause(agent.agent_id) : onResume(agent.agent_id))}
          style={{
            ...ghostButton,
            color: active ? color.amber : color.green,
            minWidth: 58,
            textAlign: "center",
          }}
        >
          {active ? "Pause" : "Resume"}
        </button>
      </div>
    </div>
  );
}

function NewAgentForm({
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
  const [agentId, setAgentId] = useState("");
  const [modelRef, setModelRef] = useState("gpt-5.3-codex-spark");
  const [prompt, setPrompt] = useState("");
  const [actions, setActions] = useState<string[]>(["chat.post"]);

  const id = slug(agentId);
  const ready = displayName.trim() !== "" && id !== "" && modelRef.trim() !== "";

  const toggle = (name: string) =>
    setActions((prev) =>
      prev.includes(name) ? prev.filter((a) => a !== name) : [...prev, name],
    );

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    onRegister({ displayName, agentId: id, modelRef, prompt, allowedActions: actions });
    setDisplayName("");
    setAgentId("");
    setModelRef("gpt-5.3-codex-spark");
    setPrompt("");
    setActions(["chat.post"]);
  };

  return (
    <GroupCard>
      <form
        onSubmit={submit}
        style={{ padding: 15, display: "flex", flexDirection: "column", gap: 11 }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <div>
            <div style={{ font: `600 13px ${font.sans}`, color: color.ink }}>
              Register agent
            </div>
            <div style={{ marginTop: 2, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
              Prompt text is uploaded, then registered by its hash.
            </div>
          </div>
          <AgentAvatar name={displayName || "AI"} size={30} />
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 160px), 1fr))",
            gap: 8,
          }}
        >
          <input
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Display name"
            style={fieldStyle}
          />
          <input
            value={agentId}
            onChange={(event) => setAgentId(event.target.value)}
            placeholder="agent-id"
            style={{ ...fieldStyle, font: `400 12px ${font.mono}` }}
          />
          <input
            value={modelRef}
            onChange={(event) => setModelRef(event.target.value)}
            placeholder="model"
            style={{ ...fieldStyle, font: `400 12px ${font.mono}` }}
          />
        </div>

        <textarea
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          rows={4}
          placeholder="System prompt"
          style={{
            ...fieldStyle,
            resize: "vertical",
            minHeight: 78,
            lineHeight: 1.45,
          }}
        />

        <div style={{ display: "flex", alignItems: "center", gap: 7, flexWrap: "wrap" }}>
          {KNOWN_ACTIONS.map((name) => {
            const on = actions.includes(name);
            return (
              <button
                key={name}
                type="button"
                onClick={() => toggle(name)}
                style={{
                  all: "unset",
                  cursor: "pointer",
                  padding: "5px 9px",
                  borderRadius: radius.sm,
                  border: `1px solid ${on ? statusTone.agent.border : color.border}`,
                  background: on ? statusTone.agent.bg : color.paper,
                  color: on ? accentVar : color.muted3,
                  font: `600 10.5px ${font.sans}`,
                }}
              >
                {ACTION_LABEL[name]}
              </button>
            );
          })}
          <button
            type="submit"
            disabled={!ready}
            style={{
              all: "unset",
              cursor: ready ? "pointer" : "default",
              marginLeft: "auto",
              padding: "8px 15px",
              borderRadius: radius.sm,
              background: ready ? color.dark : color.chip,
              color: color.onDark,
              font: `600 12px ${font.sans}`,
            }}
          >
            Register
          </button>
        </div>
      </form>
    </GroupCard>
  );
}

// ── Watches ─────────────────────────────────────────────

function WatchRow({ watch, onUnwatch }: { watch: WatchView; onUnwatch: (id: string) => void }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "12px 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <div
        style={{
          width: 30,
          height: 30,
          borderRadius: 8,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted2,
          flexShrink: 0,
        }}
      >
        <Icon name="hash" size={15} />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          title={watch.channel_id}
          style={{
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            font: `600 12px ${font.mono}`,
            color: color.ink,
          }}
        >
          {watch.channel_id}
        </div>
        <div style={{ marginTop: 2, font: `400 11px ${font.sans}`, color: color.muted2 }}>
          {policyText(watch.policy)}
        </div>
      </div>
      <button
        type="button"
        onClick={() => onUnwatch(watch.channel_id)}
        title="Unwatch"
        style={{ ...ghostButton, padding: "6px 10px", color: color.muted3 }}
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

  const buildPolicy = (): TurnPolicy | null => {
    if (kind === "Assigned") return assigned ? { Assigned: assigned } : null;
    return kind;
  };

  const policy = buildPolicy();
  const ready = channelId !== "" && policy !== null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!channelId || !policy) return;
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
        display: "flex",
        flexDirection: "column",
        gap: 9,
      }}
    >
      <div style={{ display: "flex", gap: 8 }}>
        <select
          value={channelId}
          onChange={(event) => setChannelId(event.target.value)}
          style={{ ...fieldStyle, cursor: "pointer" }}
        >
          <option value="">Channel…</option>
          {channels.map((channel) => (
            <option key={channel.id} value={channel.id}>
              {channel.name}
            </option>
          ))}
        </select>
        {kind === "Assigned" && (
          <select
            value={assigned}
            onChange={(event) => setAssigned(event.target.value)}
            style={{ ...fieldStyle, cursor: "pointer" }}
          >
            <option value="">Agent…</option>
            {agents.map((agent) => (
              <option key={agent.agent_id} value={agent.agent_id}>
                {agent.display_name}
              </option>
            ))}
          </select>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
        {POLICY_KINDS.map((option) => {
          const on = option === kind;
          return (
            <button
              key={option}
              type="button"
              onClick={() => setKind(option)}
              style={{
                all: "unset",
                cursor: "pointer",
                padding: "5px 9px",
                borderRadius: radius.sm,
                border: `1px solid ${on ? statusTone.agent.border : color.border}`,
                background: on ? statusTone.agent.bg : color.paper,
                color: on ? accentVar : color.muted3,
                font: `600 10.5px ${font.sans}`,
              }}
            >
              {POLICY_LABEL[option]}
            </button>
          );
        })}
        <button
          type="submit"
          disabled={!ready}
          style={{
            all: "unset",
            cursor: ready ? "pointer" : "default",
            marginLeft: "auto",
            padding: "7px 12px",
            borderRadius: radius.sm,
            background: ready ? color.dark : color.chip,
            color: color.onDark,
            font: `600 11.5px ${font.sans}`,
          }}
        >
          Watch
        </button>
      </div>
    </form>
  );
}

// ── Runs timeline ───────────────────────────────────────

function RunRow({ run, onCancel }: { run: RunView; onCancel: (id: string) => void }) {
  const tone = runTone(run.status);
  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        alignItems: "flex-start",
        gap: 12,
        padding: "0 0 18px",
        animation: "ik-fade .16s ease-out",
      }}
    >
      <div
        style={{
          width: 10,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          alignSelf: "stretch",
          paddingTop: 14,
        }}
      >
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: tone.text,
            boxShadow: `0 0 0 4px ${tone.bg}`,
            flexShrink: 0,
          }}
        />
        <span style={{ width: 1, flex: 1, marginTop: 8, background: color.borderSoft }} />
      </div>

      <GroupCard style={{ flex: 1, minWidth: 0 }}>
        <div style={{ padding: "13px 14px", display: "flex", gap: 12 }}>
          <AgentAvatar name={run.agent_id} size={30} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 7, flexWrap: "wrap" }}>
              <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>
                {run.agent_id}
              </span>
              <StatusPill label={runLabel(run.status)} tone={tone} />
              {run.job_id && <StatusPill label="JOB" tone={statusTone.agent} />}
            </div>
            <div
              style={{
                marginTop: 5,
                display: "flex",
                alignItems: "center",
                gap: 7,
                flexWrap: "wrap",
              }}
            >
              <Chip text={run.run_id} />
              <Chip text={`${run.channel_id} @${run.anchor_seq}`} tint={color.blue} />
              {run.thread_root !== null && <Chip text={`thread ${run.thread_root}`} />}
              {run.job_id && <Chip text={run.job_id} tint={color.blue} />}
            </div>
            <div
              title={runDetail(run.status)}
              style={{
                marginTop: 6,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                font: `400 11px ${font.mono}`,
                color: color.muted2,
              }}
            >
              {runDetail(run.status)}
            </div>
          </div>
          {isAwaiting(run.status) && (
            <button
              type="button"
              onClick={() => onCancel(run.run_id)}
              title="Cancel run"
              style={{ ...ghostButton, color: color.red, padding: "6px 10px" }}
            >
              Cancel
            </button>
          )}
        </div>
      </GroupCard>
    </div>
  );
}

// ── The view ────────────────────────────────────────────

export function AgentView() {
  const { state, actions } = useDucktape();

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
        }}
      >
        <Icon name="agent" size={17} color={color.muted} />
        <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Agents</div>
        <div style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {state.agents.length}
        </div>
        <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", gap: 8 }}>
          <StatusPill label={`${state.watches.length} WATCHES`} tone={statusTone.neutral} />
          <StatusPill label={`${state.runs.length} RUNS`} tone={statusTone.warning} />
        </div>
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          padding: 22,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 360px), 1fr))",
          gap: 18,
          alignItems: "start",
        }}
      >
        <div style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: 18 }}>
          <section style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            <SectionLabel>ROSTER</SectionLabel>
            <GroupCard>
              {state.agents.length === 0 ? (
                <div style={{ padding: 14 }}>
                  <EmptyState
                    icon="agent"
                    title="No agents registered"
                    body="Register one below to give the node a collaboration-loop worker."
                  />
                </div>
              ) : (
                state.agents.map((agent, index) => (
                  <AgentRow
                    key={agent.agent_id}
                    agent={agent}
                    last={index === state.agents.length - 1}
                    onPause={actions.pauseAgent}
                    onResume={actions.resumeAgent}
                  />
                ))
              )}
            </GroupCard>
          </section>

          <section style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            <SectionLabel>NEW AGENT</SectionLabel>
            <NewAgentForm onRegister={actions.registerAgent} />
          </section>
        </div>

        <div style={{ minWidth: 0, display: "flex", flexDirection: "column", gap: 18 }}>
          <section style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            <SectionLabel>WATCHES</SectionLabel>
            <GroupCard>
              {state.watches.length === 0 ? (
                <div style={{ padding: 14 }}>
                  <EmptyState
                    icon="hash"
                    title="No watched channels"
                    body="Watch a channel to let agents engage new posts by policy."
                  />
                </div>
              ) : (
                state.watches.map((watch) => (
                  <WatchRow
                    key={watch.channel_id}
                    watch={watch}
                    onUnwatch={actions.unwatchChannel}
                  />
                ))
              )}
              <WatchForm
                channels={state.channels}
                agents={state.agents}
                onWatch={actions.watchChannel}
              />
            </GroupCard>
          </section>

          <section style={{ display: "flex", flexDirection: "column", gap: 9 }}>
            <SectionLabel>RUNS</SectionLabel>
            {state.runs.length === 0 ? (
              <EmptyState
                icon="agent"
                title="No runs yet"
                body="Engaged posts and watched channels will appear here newest-first."
              />
            ) : (
              <div style={{ display: "flex", flexDirection: "column" }}>
                {state.runs.map((run) => (
                  <RunRow key={run.run_id} run={run} onCancel={actions.cancelRun} />
                ))}
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
