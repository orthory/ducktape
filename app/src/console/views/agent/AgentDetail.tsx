// The detail pane for the selected agent: identity header, pause/resume,
// permissions, and the inline edit toggle. Asking an agent to respond lives
// on the message in chat now — this pane manages the record only.

import { useState } from "react";

import type { AgentRecord } from "../../../domain/agent-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { AgentEditForm } from "./AgentEditForm";
import {
  ACTION_LABEL,
  AgentAvatar,
  Chip,
  EmptyState,
  GroupCard,
  InfoRow,
  ownerText,
  primaryButton,
  secondaryButton,
  SectionLabel,
  shortHex,
  statusTone,
  StatusPill,
  titleCase,
} from "./parts";

export function AgentDetail({
  agent,
  capabilities,
  onPause,
  onResume,
  onUpdate,
}: {
  agent: AgentRecord | null;
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
      </GroupCard>
    </section>
  );
}

/** The right pane when there are no agents at all — a single call to action
 *  instead of an always-present form. */
export function NoAgentsPane({ onAdd }: { onAdd: () => void }) {
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
