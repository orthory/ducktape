// The agents surface over the node's `agent` module — the collaboration-loop
// orchestrator. Three regions, all render-only over useDucktape:
//
//   - Agents roster: every registered agent as a card (status pill, model,
//     granted actions) with a Pause/Resume toggle, plus a "New agent" composer.
//     Registering UPLOADS the prompt text to the node's blob store and commits
//     RegisterAgent with the resulting 32-byte digest as prompt_hash — see the
//     store's registerAgent action.
//   - Watches: the channels the module watches and their turn policy, with an
//     Unwatch button, plus a "Watch channel" composer (channel + policy picker).
//   - Runs: a newest-first timeline of runs (the store reverses the ascending
//     wire order), each tinted by status, with a Cancel button while a run is
//     still awaiting its oracle and a badge when it is job-backed.
//
// No optimistic state: every write goes through the store's submit-then-refresh.

import { useState } from "react";
import type { FormEvent } from "react";

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

const fieldStyle = {
  padding: "6px 9px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12px ${font.sans}`,
  color: color.ink,
  width: "100%",
} as const;

// ── Wire → display helpers ──────────────────────────────

/** A wire-safe slug for an agent id — same rule as channelIdOf / docIdOf. */
const slug = (raw: string): string =>
  raw
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");

const policyText = (policy: TurnPolicy): string => {
  if (policy === "Mention") return "Mention";
  if (policy === "All") return "All";
  if (policy === "RoundRobin") return "Round-robin";
  return `Assigned → ${policy.Assigned}`;
};

/** A run's tint by lifecycle: running amber, done green, failed red, else grey. */
const runTint = (status: RunStatus): string => {
  if (status === "Done") return color.green;
  if (status === "Cancelled") return color.muted2;
  if ("AwaitingOracle" in status) return color.amber;
  return color.red;
};

const runLabel = (status: RunStatus): string => {
  if (status === "Done") return "done ✓";
  if (status === "Cancelled") return "cancelled";
  if ("AwaitingOracle" in status) return "· running";
  return `failed · ${status.Failed.reason}`;
};

/** Only an awaiting run can be cancelled — the sole non-terminal state. */
const isAwaiting = (status: RunStatus): boolean =>
  status !== "Done" && status !== "Cancelled" && "AwaitingOracle" in status;

// ── Small shared bits ───────────────────────────────────

function SectionLabel({ text }: { text: string }) {
  return (
    <span
      style={{
        font: `600 10px ${font.sans}`,
        color: color.muted,
        letterSpacing: ".06em",
      }}
    >
      {text}
    </span>
  );
}

function Chip({ text, tint = color.muted3 }: { text: string; tint?: string }) {
  return (
    <span
      style={{
        padding: "2px 7px",
        borderRadius: radius.sm,
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

function PillButton({
  label,
  tint,
  onClick,
}: {
  label: string;
  tint: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        padding: "3px 10px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        background: color.paper,
        font: `600 10.5px ${font.sans}`,
        color: tint,
      }}
    >
      {label}
    </button>
  );
}

// ── Agents roster ───────────────────────────────────────

function AgentCard({
  agent,
  onPause,
  onResume,
}: {
  agent: AgentRecord;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
}) {
  const active = agent.status === "Active";
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 7,
        padding: 11,
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
        animation: "ik-fade .16s ease-out",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>
          {agent.display_name}
        </span>
        <span
          style={{
            display: "flex",
            alignItems: "center",
            gap: 5,
            padding: "2px 8px",
            borderRadius: radius.sm,
            background: color.sunken,
            font: `600 9.5px ${font.mono}`,
            letterSpacing: ".04em",
            color: active ? color.green : color.amber,
          }}
        >
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: active ? color.green : color.amber,
            }}
          />
          {active ? "ACTIVE" : "PAUSED"}
        </span>
        <span style={{ marginLeft: "auto" }}>
          {active ? (
            <PillButton label="Pause" tint={color.amber} onClick={() => onPause(agent.agent_id)} />
          ) : (
            <PillButton label="Resume" tint={color.green} onClick={() => onResume(agent.agent_id)} />
          )}
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
        <Chip text={agent.agent_id} />
        <Chip text={agent.model_ref} tint={color.blue} />
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 5, flexWrap: "wrap" }}>
        {agent.allowed_actions.length === 0 ? (
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>
            no actions granted
          </span>
        ) : (
          agent.allowed_actions.map((name) => (
            <Chip key={name} text={ACTION_LABEL[name] ?? name} />
          ))
        )}
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
    <form
      onSubmit={submit}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 11,
        borderRadius: radius.md,
        border: `1px dashed ${color.borderStrong}`,
        background: color.sunken,
      }}
    >
      <div style={{ display: "flex", gap: 7 }}>
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
        rows={3}
        placeholder="System prompt — uploaded to the node's blob store, committed as prompt_hash"
        style={{ ...fieldStyle, resize: "vertical", minHeight: 54 }}
      />
      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
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
                padding: "3px 9px",
                borderRadius: radius.sm,
                border: `1px solid ${on ? accentVar : color.border}`,
                background: on ? accentVar : color.paper,
                color: on ? "#fff" : color.muted3,
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
            padding: "5px 13px",
            borderRadius: radius.sm,
            background: ready ? accentVar : color.chip,
            color: "#fff",
            font: `600 11.5px ${font.sans}`,
          }}
        >
          Register agent
        </button>
      </div>
    </form>
  );
}

// ── Watches ─────────────────────────────────────────────

function WatchRow({ watch, onUnwatch }: { watch: WatchView; onUnwatch: (id: string) => void }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "8px 11px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
      }}
    >
      <Icon name="hash" size={13} color={color.muted2} />
      <span style={{ font: `500 12px ${font.mono}`, color: color.ink }}>
        {watch.channel_id}
      </span>
      <Chip text={policyText(watch.policy)} tint={color.purple} />
      <button
        onClick={() => onUnwatch(watch.channel_id)}
        title="Unwatch"
        style={{
          all: "unset",
          cursor: "pointer",
          marginLeft: "auto",
          font: `600 10.5px ${font.sans}`,
          color: color.muted3,
        }}
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

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const policy = buildPolicy();
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
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 11,
        borderRadius: radius.md,
        border: `1px dashed ${color.borderStrong}`,
        background: color.sunken,
      }}
    >
      <div style={{ display: "flex", gap: 7 }}>
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
                padding: "3px 9px",
                borderRadius: radius.sm,
                border: `1px solid ${on ? accentVar : color.border}`,
                background: on ? accentVar : color.paper,
                color: on ? "#fff" : color.muted3,
                font: `600 10.5px ${font.sans}`,
              }}
            >
              {POLICY_LABEL[option]}
            </button>
          );
        })}
        <button
          type="submit"
          style={{
            all: "unset",
            cursor: "pointer",
            marginLeft: "auto",
            padding: "5px 13px",
            borderRadius: radius.sm,
            background: accentVar,
            color: "#fff",
            font: `600 11.5px ${font.sans}`,
          }}
        >
          Watch channel
        </button>
      </div>
    </form>
  );
}

// ── Runs timeline ───────────────────────────────────────

function RunRow({ run, onCancel }: { run: RunView; onCancel: (id: string) => void }) {
  const tint = runTint(run.status);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 9,
        padding: "9px 11px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        borderLeft: `3px solid ${tint}`,
        background: color.paper,
        boxShadow: shadow.card,
        animation: "ik-fade .16s ease-out",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
          <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>
            {run.agent_id}
          </span>
          <span style={{ font: `400 11px ${font.mono}`, color: color.muted3 }}>
            {run.channel_id} @{run.anchor_seq}
          </span>
          {run.job_id && <Chip text="job" tint={color.blue} />}
        </div>
        <span style={{ font: `500 10.5px ${font.mono}`, color: tint }}>
          {runLabel(run.status)}
        </span>
      </div>
      {isAwaiting(run.status) && (
        <button
          onClick={() => onCancel(run.run_id)}
          title="Cancel run"
          style={{
            all: "unset",
            cursor: "pointer",
            marginLeft: "auto",
            font: `600 10.5px ${font.sans}`,
            color: color.red,
          }}
        >
          Cancel
        </button>
      )}
    </div>
  );
}

// ── The view ────────────────────────────────────────────

export function AgentView() {
  const { state, actions } = useDucktape();

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 7,
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <Icon name="agent" size={15} color={color.muted} />
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Agents</span>
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          padding: 17,
          display: "flex",
          flexDirection: "column",
          gap: 22,
        }}
      >
        {/* ── Agents roster + new-agent composer ── */}
        <section style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <SectionLabel text="ROSTER" />
          {state.agents.length === 0 ? (
            <span style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
              No agents yet — register one below.
            </span>
          ) : (
            state.agents.map((agent) => (
              <AgentCard
                key={agent.agent_id}
                agent={agent}
                onPause={actions.pauseAgent}
                onResume={actions.resumeAgent}
              />
            ))
          )}
          <NewAgentForm onRegister={actions.registerAgent} />
        </section>

        {/* ── Watched channels + watch composer ── */}
        <section style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <SectionLabel text="WATCHES" />
          {state.watches.length === 0 ? (
            <span style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
              No watched channels — watch one to engage agents on new posts.
            </span>
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
        </section>

        {/* ── Runs timeline (newest-first) ── */}
        <section style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <SectionLabel text="RUNS" />
          {state.runs.length === 0 ? (
            <span style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
              No runs yet — an engaged post or a watched channel starts one.
            </span>
          ) : (
            state.runs.map((run) => (
              <RunRow key={run.run_id} run={run} onCancel={actions.cancelRun} />
            ))
          )}
        </section>
      </div>
    </div>
  );
}
