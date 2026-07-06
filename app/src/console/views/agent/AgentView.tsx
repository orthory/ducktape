// The agents surface over the `agent` registry and the `runs` module — the
// collaboration loop's record book and its actor. It stays render-only over
// useDucktape: roster, watches, pending runs, and composers all submit
// through the store action facade. Run lifecycle lives in the dispatch
// module; this surface shows only the in-flight entries (pruned when a
// result delivers).
//
// No optimistic state: every write goes through the store's submit-then-refresh.

import { useEffect, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { AgentRecord, SagaOrigin } from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import type { PendingRun, TurnPolicy, WatchView } from "../../../domain/runs-client";
import type { Channel } from "../../../domain/chat-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

// ── Static labels ───────────────────────────────────────

const ACTION_LABEL: Record<string, string> = {
  "chat.post": "Post to chat",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
};

// Permission checkboxes read as plain abilities ("what this agent can do"),
// not as the wire action ids they map to.
const ACTION_HINT: Record<string, string> = {
  "chat.post": "Reply in chat",
  "tasks.create": "Create tasks",
  "tasks.update_status": "Update task status",
};

const POLICY_KINDS = ["mention", "all", "round_robin", "assigned"] as const;
type PolicyKind = (typeof POLICY_KINDS)[number];

// "When to reply" options — plain language for the dispatch turn policy.
const POLICY_LABEL: Record<PolicyKind, string> = {
  mention: "When mentioned",
  all: "Every message",
  round_robin: "Take turns",
  assigned: "Only a chosen agent",
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

/** Present a lowercase executor tag ("codex") as a friendly label ("Codex").
 *  The raw tag stays the stored value — this is display only. */
const titleCase = (tag: string): string =>
  tag ? tag.charAt(0).toUpperCase() + tag.slice(1) : tag;

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

const ownerText = (origin: SagaOrigin): string => {
  if (origin === "system") return "system";
  if ("module" in origin) return `module:${origin.module}`;
  return `external:${shortHex(origin.external)}`;
};

const channelLabel = (channels: Channel[], channelId: string): string =>
  channels.find((channel) => channel.id === channelId)?.name ?? channelId;

const agentLabel = (agents: AgentRecord[], agentId: string): string =>
  agents.find((agent) => agent.agent_id === agentId)?.display_name ?? agentId;

const policyText = (policy: TurnPolicy, agents: AgentRecord[]): string => {
  if (policy === "mention") return "When mentioned";
  if (policy === "all") return "Every message";
  if (policy === "round_robin") return "Take turns";
  return `Only ${agentLabel(agents, policy.assigned)}`;
};

/** Every listed entry is by definition awaiting its dispatch delivery — the
 *  node prunes entries the moment a result lands. */
const runDetail = (run: PendingRun): string =>
  `dispatch ${run.dispatch_id.slice(0, 12)}…`;

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

/** "Runs on" — which executor backs this agent. Reads the network's announced
 *  executor registry (`state.capabilities`) so the user picks a real one
 *  instead of typing a routing tag blind. Degrades by registry size:
 *   - none announced & nothing chosen → a labelled text field (never blocks
 *     setup before a host has announced);
 *   - one or more announced → a select; when the current value is empty it
 *     defaults to the first, so a single-executor node needs no choice at all.
 *  An already-stored tag absent from the registry (its host went offline) is
 *  pinned so an edit never silently rewrites which executor the agent runs on. */
function RunsOnField({
  id,
  value,
  capabilities,
  onChange,
}: {
  id: string;
  value: string;
  capabilities: string[];
  onChange: (next: string) => void;
}) {
  const known = capabilities;

  // Adopt a sane default once the registry loads: an empty value with executors
  // available picks the first, so the common single-executor case is one fewer
  // decision. Never overrides a value the user (or the record) already set.
  useEffect(() => {
    if (value === "" && known.length > 0) onChange(known[0]);
  }, [value, known, onChange]);

  // Nothing announced and nothing chosen yet: fall back to free text so a
  // first-time operator can still register before any host announces.
  if (known.length === 0 && value === "") {
    return (
      <>
        <input
          id={id}
          name={id}
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="e.g. codex"
          style={monoInputStyle}
        />
        <div
          style={{
            marginTop: 5,
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            lineHeight: 1.4,
          }}
        >
          Name of an executor your node can run (for example codex or claude).
        </div>
      </>
    );
  }

  const offline = (tag: string) => known.length > 0 && !known.includes(tag);
  const optionLabel = (tag: string) =>
    offline(tag) ? `${titleCase(tag)} (offline)` : titleCase(tag);
  // Pin an off-registry stored value at the front so it stays selectable.
  const options = value !== "" && !known.includes(value) ? [value, ...known] : known;

  return (
    <select
      id={id}
      name={id}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      style={{ ...inputStyle, cursor: "pointer" }}
    >
      {value === "" && (
        <option value="" disabled>
          Choose an executor…
        </option>
      )}
      {options.map((tag) => (
        <option key={tag} value={tag}>
          {optionLabel(tag)}
        </option>
      ))}
    </select>
  );
}

// ── Agents roster + detail ──────────────────────────────

function AgentListButton({
  agent,
  selected,
  op,
  onSelect,
}: {
  agent: AgentRecord;
  selected: boolean;
  /** The agent's finalization record — the status line draws the mark. */
  op: OpRecord | undefined;
  onSelect: (agentId: string) => void;
}) {
  const active = agent.status === "active";
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
          <FinalizationMark op={op} />
        </span>
      </span>
    </button>
  );
}

function AgentDetail({
  agent,
  channels,
  capabilities,
  onPause,
  onResume,
  onUpdate,
  onRequestRun,
}: {
  agent: AgentRecord | null;
  channels: Channel[];
  capabilities: string[];
  onPause: (agentId: string) => void;
  onResume: (agentId: string) => void;
  onUpdate: (params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    prompt?: string;
    allowedActions?: string[];
  }) => void;
  onRequestRun: (params: { agentId: string; channelId: string; anchorSeq: number }) => void;
}) {
  const [editing, setEditing] = useState(false);

  if (!agent) {
    return (
      <section aria-label="Agent detail" style={{ minWidth: 0 }}>
        <SectionLabel>AGENT DETAIL</SectionLabel>
        <GroupCard style={{ marginTop: 9 }}>
          <EmptyState
            icon="agent"
            title="No agent selected"
            body="Add an agent, or pick one from the list to see its settings."
          />
        </GroupCard>
      </section>
    );
  }

  const active = agent.status === "active";
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
            <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
              <button
                type="button"
                onClick={() => setEditing((open) => !open)}
                aria-expanded={editing}
                style={secondaryButton}
              >
                {editing ? "Close edit" : "Edit"}
              </button>
              <button
                type="button"
                onClick={() => (active ? onPause(agent.agent_id) : onResume(agent.agent_id))}
                style={{
                  ...secondaryButton,
                  color: active ? color.amber : color.green,
                }}
              >
                {active ? "Pause agent" : "Resume agent"}
              </button>
            </div>
          </div>

          <div
            style={{
              marginTop: 15,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 180px), 1fr))",
              gap: 8,
            }}
          >
            <InfoRow label="runs on" value={titleCase(agent.capability)} />
            <InfoRow label="owner" value={ownerText(agent.owner)} />
            <InfoRow label="prompt" value={shortHex(agent.prompt_hash)} />
            <InfoRow label="updated" value={String(agent.updated_at)} />
          </div>

          <div style={{ marginTop: 15 }}>
            <SectionLabel>PERMISSIONS</SectionLabel>
            <div style={{ marginTop: 8, display: "flex", gap: 7, flexWrap: "wrap" }}>
              {agent.allowed_actions.length === 0 ? (
                <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
                  Can't take any actions yet.
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

          {editing && (
            <AgentEditForm
              key={agent.agent_id}
              agent={agent}
              capabilities={capabilities}
              onUpdate={onUpdate}
              onClose={() => setEditing(false)}
            />
          )}
        </div>
        <RunRequestForm agent={agent} channels={channels} onRequestRun={onRequestRun} />
      </GroupCard>
    </section>
  );
}

function AgentEditForm({
  agent,
  capabilities,
  onUpdate,
  onClose,
}: {
  agent: AgentRecord;
  capabilities: string[];
  onUpdate: (params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    prompt?: string;
    allowedActions?: string[];
  }) => void;
  onClose: () => void;
}) {
  const [displayName, setDisplayName] = useState(agent.display_name);
  const [capability, setCapability] = useState(agent.capability);
  const [prompt, setPrompt] = useState("");
  const [allowedActions, setAllowedActions] = useState<string[]>(agent.allowed_actions);

  const toggle = (name: string) =>
    setAllowedActions((prev) =>
      prev.includes(name) ? prev.filter((action) => action !== name) : [...prev, name],
    );

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onUpdate({
      agentId: agent.agent_id,
      displayName: displayName.trim(),
      capability: capability.trim(),
      allowedActions,
      ...(prompt.trim() ? { prompt } : {}),
    });
    onClose();
  };

  return (
    <form
      onSubmit={submit}
      aria-label="Edit agent"
      style={{
        marginTop: 15,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.sidebar,
        padding: 14,
      }}
    >
      <SectionLabel>EDIT AGENT</SectionLabel>
      <div
        style={{
          marginTop: 9,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 150px), 1fr))",
          gap: 9,
        }}
      >
        <div>
          <FieldLabel htmlFor="agent-edit-display-name">Edit display name</FieldLabel>
          <input
            id="agent-edit-display-name"
            name="agent-edit-display-name"
            type="text"
            autoComplete="off"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            style={inputStyle}
          />
        </div>
        <div>
          <FieldLabel htmlFor="agent-edit-capability">Runs on</FieldLabel>
          <RunsOnField
            id="agent-edit-capability"
            value={capability}
            capabilities={capabilities}
            onChange={setCapability}
          />
        </div>
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
                  name="agent-edit-capability"
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

      <div style={{ marginTop: 10 }}>
        <FieldLabel htmlFor="agent-edit-prompt">New prompt</FieldLabel>
        <textarea
          id="agent-edit-prompt"
          name="agent-edit-prompt"
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          rows={4}
          placeholder="Leave blank to keep the current prompt"
          style={{
            ...inputStyle,
            resize: "vertical",
            minHeight: 80,
            lineHeight: 1.45,
          }}
        />
      </div>

      <div
        style={{
          marginTop: 12,
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          gap: 8,
        }}
      >
        <button type="button" onClick={onClose} style={secondaryButton}>
          Cancel
        </button>
        <button type="submit" style={primaryButton(true)}>
          Save changes
        </button>
      </div>
    </form>
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
  // Anchor defaults to the channel's latest message; only the "Options"
  // disclosure lets you reply from an earlier point.
  const [anchorInput, setAnchorInput] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
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
        <SectionLabel>ASK TO RESPOND</SectionLabel>
        {!hasMessages && (
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>
            no messages here yet
          </span>
        )}
        <button
          type="button"
          onClick={() => setShowAdvanced((open) => !open)}
          aria-expanded={showAdvanced}
          style={{
            marginLeft: "auto",
            appearance: "none",
            border: 0,
            background: "transparent",
            cursor: "pointer",
            padding: 0,
            font: `600 10px ${font.mono}`,
            letterSpacing: ".05em",
            color: color.muted2,
          }}
        >
          {showAdvanced ? "Hide options" : "Options"}
        </button>
      </div>
      <div
        style={{
          marginTop: 9,
          display: "grid",
          gridTemplateColumns: "minmax(0, 1fr) auto",
          gap: 8,
          alignItems: "end",
        }}
      >
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor="agent-run-channel">Channel</FieldLabel>
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
        <button type="submit" disabled={!ready} style={primaryButton(ready)}>
          Ask to respond
        </button>
      </div>
      {showAdvanced && (
        <div style={{ marginTop: 10 }}>
          <FieldLabel htmlFor="agent-run-anchor">Reply from message #</FieldLabel>
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
          <div
            style={{
              marginTop: 5,
              font: `400 10.5px ${font.sans}`,
              color: color.muted2,
            }}
          >
            Defaults to the latest message.
          </div>
        </div>
      )}
    </form>
  );
}

// ── Register flow ───────────────────────────────────────

function RegisterAgentForm({
  capabilities,
  onRegister,
  onDone,
}: {
  capabilities: string[];
  onRegister: (params: {
    displayName: string;
    agentId: string;
    capability: string;
    prompt: string;
    allowedActions: string[];
  }) => void;
  /** Called after a successful submit (and by Cancel) so the host can close
   *  the create pane. */
  onDone?: () => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [agentIdInput, setAgentIdInput] = useState("");
  const [capability, setCapability] = useState("");
  const [prompt, setPrompt] = useState("");
  const [allowedActions, setAllowedActions] = useState<string[]>(["chat.post"]);
  // The id is derived from the name by default; this reveals the override.
  const [showAdvanced, setShowAdvanced] = useState(false);

  const agentId = slug(agentIdInput || displayName);
  const ready =
    displayName.trim() !== "" &&
    agentId !== "" &&
    capability.trim() !== "" &&
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
      capability: capability.trim(),
      prompt,
      allowedActions,
    });
    setDisplayName("");
    setAgentIdInput("");
    setCapability("");
    setPrompt("");
    setAllowedActions(["chat.post"]);
    setShowAdvanced(false);
    onDone?.();
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
                Add an agent
              </div>
              <div
                style={{
                  marginTop: 3,
                  font: `400 11.5px ${font.sans}`,
                  color: color.muted2,
                  lineHeight: 1.45,
                }}
              >
                Give it a name, pick what it runs on, and describe its job.
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
              <FieldLabel htmlFor="agent-capability">Runs on</FieldLabel>
              <RunsOnField
                id="agent-capability"
                value={capability}
                capabilities={capabilities}
                onChange={setCapability}
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
              PERMISSIONS
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

          {showAdvanced && (
            <div style={{ marginTop: 12 }}>
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
              <div
                style={{
                  marginTop: 5,
                  font: `400 10.5px ${font.sans}`,
                  color: color.muted2,
                }}
              >
                Used in @mentions and the API. Defaults to the name.
              </div>
            </div>
          )}

          <div
            style={{
              marginTop: 14,
              display: "flex",
              alignItems: "center",
              gap: 10,
              minWidth: 0,
            }}
          >
            <button
              type="button"
              onClick={() => setShowAdvanced((open) => !open)}
              aria-expanded={showAdvanced}
              style={{
                appearance: "none",
                border: 0,
                background: "transparent",
                cursor: "pointer",
                padding: 0,
                font: `600 10px ${font.mono}`,
                letterSpacing: ".05em",
                color: color.muted2,
                flexShrink: 0,
              }}
            >
              {showAdvanced ? "Hide advanced" : "Advanced"}
            </button>
            <span
              translate="no"
              style={{
                flex: 1,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                textAlign: "right",
                font: `400 11px ${font.mono}`,
                color: color.muted2,
              }}
            >
              saved as {agentId || "—"}
            </span>
            {onDone && (
              <button type="button" onClick={onDone} style={secondaryButton}>
                Cancel
              </button>
            )}
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
  op,
  onUnwatch,
}: {
  watch: WatchView;
  channels: Channel[];
  agents: AgentRecord[];
  /** The watch's finalization record (watch/unwatch key by channel). */
  op: OpRecord | undefined;
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
        <div
          style={{
            marginTop: 2,
            display: "flex",
            alignItems: "center",
            gap: 6,
            font: `400 11.5px ${font.sans}`,
            color: color.muted2,
          }}
        >
          {policyText(watch.policy, agents)}
          <FinalizationMark op={op} />
        </div>
      </div>
      <button
        type="button"
        onClick={() => onUnwatch(watch.channel_id)}
        aria-label={`Stop watching ${label}`}
        style={{ ...secondaryButton, minHeight: 30, color: color.red }}
      >
        Turn off
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
  const [kind, setKind] = useState<PolicyKind>("mention");
  const [assigned, setAssigned] = useState("");

  const policy: TurnPolicy | null =
    kind === "assigned" ? (assigned ? { assigned: assigned } : null) : kind;
  const ready = channelId !== "" && policy !== null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready || !policy) return;
    onWatch({ channelId, policy });
    setChannelId("");
    setKind("mention");
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
            kind === "assigned"
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
          <FieldLabel htmlFor="agent-watch-policy">When to reply</FieldLabel>
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
        {kind === "assigned" && (
          <div style={{ minWidth: 0 }}>
            <FieldLabel htmlFor="agent-watch-assigned">Which agent</FieldLabel>
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
          Add auto-reply
        </button>
      </div>
    </form>
  );
}

function WatchesPanel({
  channels,
  agents,
  watches,
  ops,
  onWatch,
  onUnwatch,
}: {
  channels: Channel[];
  agents: AgentRecord[];
  watches: WatchView[];
  /** The store's finalization ledger — watch rows draw their marks. */
  ops: OpLedger;
  onWatch: (params: { channelId: string; policy: TurnPolicy }) => void;
  onUnwatch: (id: string) => void;
}) {
  return (
    <section aria-label="Watches" style={{ minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>AUTO-REPLY</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {watches.length}
        </span>
      </div>
      <GroupCard style={{ marginTop: 9 }}>
        {watches.length === 0 ? (
          <EmptyState
            icon="hash"
            title="No auto-reply set up"
            body="Pick a channel and your agents will answer there on the rule you choose."
          />
        ) : (
          watches.map((watch) => (
            <WatchRow
              key={watch.channel_id}
              watch={watch}
              channels={channels}
              agents={agents}
              op={ops[opKey.watch(watch.channel_id)]}
              onUnwatch={onUnwatch}
            />
          ))
        )}
        <WatchForm channels={channels} agents={agents} onWatch={onWatch} />
      </GroupCard>
    </section>
  );
}

// ── Pending runs timeline ───────────────────────────────

function RunRow({
  run,
  agents,
  channels,
  op,
  onCancel,
}: {
  run: PendingRun;
  agents: AgentRecord[];
  channels: Channel[];
  /** The run's finalization record (a cancel keys by run id). */
  op: OpRecord | undefined;
  onCancel: (id: string) => void;
}) {
  const agentName = agentLabel(agents, run.agent_id);
  const label = run.job_id
    ? `job ${run.job_id}`
    : `${channelLabel(channels, run.channel_id)} @${run.anchor_seq}`;
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
          <StatusPill label="WORKING…" tone={statusTone.warning} />
          <StatusPill label={run.job_id ? "JOB" : "CHAT"} tone={run.job_id ? statusTone.agent : statusTone.blue} />
          <button
            type="button"
            onClick={() => onCancel(run.run_id)}
            aria-label={`Cancel run ${run.run_id}`}
            style={{ ...secondaryButton, marginLeft: "auto", minHeight: 28, color: color.red }}
          >
            Cancel
          </button>
        </div>
        <div style={{ padding: "11px 12px" }}>
          <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
            <FinalizationMark op={op} />
            <Chip text={label} tone={statusTone.blue} />
            {run.thread_root !== null && <Chip text={`thread ${run.thread_root}`} />}
          </div>
          <div
            title={`run ${run.run_id} · ${runDetail(run)}`}
            style={{
              marginTop: 7,
              font: `400 11px ${font.mono}`,
              color: color.muted2,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            started {run.created_at}
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
  ops,
  onCancel,
}: {
  runs: PendingRun[];
  agents: AgentRecord[];
  channels: Channel[];
  /** The store's finalization ledger — run rows draw their marks. */
  ops: OpLedger;
  onCancel: (id: string) => void;
}) {
  return (
    <section aria-label="Pending runs" style={{ minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>IN PROGRESS</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {runs.length}
        </span>
      </div>
      {runs.length === 0 ? (
        <GroupCard style={{ marginTop: 9 }}>
          <EmptyState
            icon="agent"
            title="Nothing running"
            body="When an agent is working on a reply, it shows here until it finishes."
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
              op={ops[opKey.run(run.run_id)]}
              onCancel={onCancel}
            />
          ))}
        </div>
      )}
    </section>
  );
}

// ── The view ────────────────────────────────────────────

type AgentTab = "agents" | "auto-reply" | "activity";

/** One pill in the top segmented switch — carries a live count so the operator
 *  sees how much lives behind each surface without opening it. */
function TabButton({
  label,
  count,
  active,
  onClick,
}: {
  label: string;
  count: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      style={{
        appearance: "none",
        border: 0,
        borderRadius: radius.sm,
        background: active ? color.paper : "transparent",
        boxShadow: active ? shadow.card : undefined,
        cursor: "pointer",
        padding: "6px 12px",
        display: "inline-flex",
        alignItems: "center",
        gap: 7,
        font: `600 12px ${font.sans}`,
        color: active ? color.dark : color.muted2,
        whiteSpace: "nowrap",
      }}
    >
      {label}
      <span style={{ font: `600 10px ${font.mono}`, color: active ? accentVar : color.muted2 }}>
        {count}
      </span>
    </button>
  );
}

/** The daemon-lifecycle switch for job-board pickup — its own row on the
 *  Activity tab, where background work lives. */
function JobsWorkerRow({
  on,
  op,
  onToggle,
}: {
  on: boolean;
  op: OpRecord | undefined;
  onToggle: () => void;
}) {
  return (
    <GroupCard style={{ marginBottom: 16 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 14px" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>Jobs worker</div>
          <div style={{ marginTop: 2, font: `400 11px ${font.sans}`, color: color.muted2 }}>
            Let agents pick up background jobs.
          </div>
        </div>
        <FinalizationMark op={op} />
        <button
          type="button"
          role="switch"
          aria-checked={on}
          aria-label="Jobs worker"
          onClick={onToggle}
          style={{
            appearance: "none",
            cursor: "pointer",
            width: 40,
            height: 22,
            flexShrink: 0,
            padding: 2,
            borderRadius: 999,
            border: `1px solid ${on ? color.dark : color.borderStrong}`,
            background: on ? color.dark : color.chip,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: on ? "flex-end" : "flex-start",
            transition: "background .12s, border-color .12s",
          }}
        >
          <span
            aria-hidden="true"
            style={{
              width: 16,
              height: 16,
              borderRadius: "50%",
              background: on ? color.onDark : color.muted,
              boxShadow: shadow.card,
            }}
          />
        </button>
      </div>
    </GroupCard>
  );
}

/** The right pane when there are no agents at all — a single call to action
 *  instead of an always-present form. */
function NoAgentsPane({ onAdd }: { onAdd: () => void }) {
  return (
    <GroupCard>
      <div
        style={{
          minHeight: 240,
          padding: "40px 24px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          textAlign: "center",
          gap: 10,
        }}
      >
        <span
          style={{
            width: 46,
            height: 46,
            borderRadius: radius.md,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="agent" size={22} color="currentColor" strokeWidth={1.6} />
        </span>
        <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>No agents yet</div>
        <div style={{ maxWidth: 320, font: `400 12px ${font.sans}`, color: color.muted2, lineHeight: 1.5 }}>
          Add your first agent to start automating chats and tasks.
        </div>
        <button type="button" onClick={onAdd} style={{ ...primaryButton(true), marginTop: 4 }}>
          + Add agent
        </button>
      </div>
    </GroupCard>
  );
}

export function AgentView() {
  const { state, actions } = useDucktape();
  const [tab, setTab] = useState<AgentTab>("agents");
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [jobWorkerOn, setJobWorkerOn] = useState(false);
  const selectedAgent =
    state.agents.find((agent) => agent.agent_id === selectedAgentId) ??
    state.agents[0] ??
    null;

  const toggleJobWorker = () => {
    const next = !jobWorkerOn;
    setJobWorkerOn(next);
    actions.enableJobWorker(next);
  };

  const startAdding = () => {
    setTab("agents");
    setAdding(true);
  };
  const selectAgent = (id: string) => {
    setSelectedAgentId(id);
    setAdding(false);
  };

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
          gap: 12,
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
        <h1 style={{ margin: 0, font: `600 16px ${font.sans}`, color: color.dark }}>Agents</h1>

        <div
          role="tablist"
          aria-label="Agent views"
          style={{
            marginLeft: "auto",
            display: "flex",
            alignItems: "center",
            gap: 4,
            background: color.sidebar,
            border: `1px solid ${color.border}`,
            borderRadius: radius.md,
            padding: 3,
          }}
        >
          <TabButton
            label="Agents"
            count={state.agents.length}
            active={tab === "agents"}
            onClick={() => setTab("agents")}
          />
          <TabButton
            label="Auto-reply"
            count={state.watches.length}
            active={tab === "auto-reply"}
            onClick={() => setTab("auto-reply")}
          />
          <TabButton
            label="Activity"
            count={state.pendingRuns.length}
            active={tab === "activity"}
            onClick={() => setTab("activity")}
          />
        </div>

        <button type="button" onClick={startAdding} style={primaryButton(true)}>
          + Add agent
        </button>
      </div>

      {tab === "agents" ? (
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
            <div style={{ padding: "14px 14px 9px", display: "flex", alignItems: "center", gap: 8 }}>
              <SectionLabel>ROSTER</SectionLabel>
              <span
                style={{ marginLeft: "auto", font: `400 10.5px ${font.mono}`, color: color.muted2 }}
              >
                {state.agents.length} total
              </span>
            </div>
            <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
              {state.agents.length === 0 ? (
                <EmptyState icon="agent" title="No agents yet" body="Add an agent to get started." />
              ) : (
                state.agents.map((agent) => (
                  <AgentListButton
                    key={agent.agent_id}
                    agent={agent}
                    selected={!adding && selectedAgent?.agent_id === agent.agent_id}
                    op={state.ops[opKey.agent(agent.agent_id)]}
                    onSelect={selectAgent}
                  />
                ))
              )}
            </div>
          </aside>

          <main style={{ flex: 1, minWidth: 0, minHeight: 0, overflowY: "auto", padding: 22 }}>
            <div style={{ maxWidth: 640, margin: "0 auto" }}>
              {adding ? (
                <RegisterAgentForm
                  capabilities={state.capabilities}
                  onRegister={actions.registerAgent}
                  onDone={() => setAdding(false)}
                />
              ) : selectedAgent ? (
                <AgentDetail
                  agent={selectedAgent}
                  channels={state.channels}
                  capabilities={state.capabilities}
                  onPause={actions.pauseAgent}
                  onResume={actions.resumeAgent}
                  onUpdate={actions.updateAgent}
                  onRequestRun={actions.requestRun}
                />
              ) : (
                <NoAgentsPane onAdd={startAdding} />
              )}
            </div>
          </main>
        </div>
      ) : tab === "auto-reply" ? (
        <main style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 22 }}>
          <div style={{ maxWidth: 720, margin: "0 auto" }}>
            <WatchesPanel
              channels={state.channels}
              agents={state.agents}
              watches={state.watches}
              ops={state.ops}
              onWatch={actions.watchChannel}
              onUnwatch={actions.unwatchChannel}
            />
          </div>
        </main>
      ) : (
        <main style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 22 }}>
          <div style={{ maxWidth: 720, margin: "0 auto" }}>
            <JobsWorkerRow
              on={jobWorkerOn}
              op={state.ops[opKey.jobWorker()]}
              onToggle={toggleJobWorker}
            />
            <RunsTimeline
              runs={state.pendingRuns}
              agents={state.agents}
              channels={state.channels}
              ops={state.ops}
              onCancel={actions.cancelRun}
            />
          </div>
        </main>
      )}
    </div>
  );
}
