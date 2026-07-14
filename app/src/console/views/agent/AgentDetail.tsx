// The detail pane for the selected agent: identity header, pause/resume,
// curated skills, permissions, and the inline edit toggle. Asking an agent to
// respond lives on the message in chat now — this pane manages the record only.

import { useState } from "react";

import type { AgentRecord, ResourceCaps, SkillRef } from "../../../domain/agent-client";
import { agentAddress } from "../../../domain/agent-client";
import { Icon } from "../../components/Icon";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";
import { AgentEditForm } from "./AgentEditForm";
import {
  ACTION_LABEL,
  AgentAvatar,
  CapabilityStrip,
  Chip,
  EmptyState,
  FILLED_IDENTITY_TEXT_PERCENT,
  filledForeground,
  filledMix,
  GroupCard,
  InfoRow,
  onDarkButton,
  ownerText,
  primaryButton,
  secondaryButton,
  SectionLabel,
  statusTone,
} from "./parts";
import { cleanPrefix, skillDocPath, skillsSummary } from "./skills";

/** One curated skill, read-only: what it is, where it lives, and whether it is
 *  part of the agent's soul (always loaded) or something it reaches for. */
function SkillRow({ skill }: { skill: SkillRef }) {
  const { actions } = useDucktape();
  const always = skill.load === "always";
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 9,
        minWidth: 0,
        padding: "8px 10px",
        borderRadius: radius.sm,
        border: `1px solid ${always ? statusTone.agent.border : color.border}`,
        background: always ? statusTone.agent.bg : color.paper,
      }}
    >
      <Chip
        text={always ? "ALWAYS" : "ON DEMAND"}
        tone={always ? statusTone.agent : statusTone.neutral}
      />
      <span style={{ font: `600 12px ${font.sans}`, color: color.ink, flexShrink: 0 }}>
        {skill.name}
      </span>
      <span
        translate="no"
        style={{
          flex: 1,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          font: `400 10.5px ${font.mono}`,
          color: color.muted2,
        }}
      >
        {skillDocPath(skill.source_prefix)}
      </span>
      <button
        type="button"
        onClick={() => actions.openFiles(cleanPrefix(skill.source_prefix))}
        style={{ ...secondaryButton, minHeight: 24, padding: "2px 8px", flexShrink: 0 }}
      >
        Open
      </button>
    </div>
  );
}

export function AgentDetail({
  agent,
  capabilities,
  capabilitiesStatus,
  op,
  onPause,
  onResume,
  onUpdate,
}: {
  agent: AgentRecord | null;
  capabilities: string[];
  capabilitiesStatus: "loading" | "ready" | "error";
  op?: OpRecord;
  onPause: (agentId: string) => void;
  onResume: (agentId: string) => void;
  onUpdate: (params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    allowedActions?: string[];
    caps?: ResourceCaps;
    skills?: SkillRef[];
  }) => Promise<boolean>;
}) {
  const [editing, setEditing] = useState(false);
  const pending = op?.phase === "pending";

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
      <div
        style={{
          border: `1px solid ${color.border}`,
          borderRadius: radius.lg,
          background: color.paper,
          boxShadow: shadow.card,
          overflow: "hidden",
        }}
      >
        {/* Identity band — a dark plate so the agent reads as a first-class actor,
            not a form row. The accent avatar is the one bright object on it. */}
        <div
          style={{
            background: color.dark,
            padding: "18px 18px 17px",
            display: "flex",
            alignItems: "flex-start",
            gap: 14,
          }}
        >
          <AgentAvatar name={agent.display_name} size={50} tone="accent" />
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2
              style={{
                margin: 0,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                font: `650 20px ${font.sans}`,
                letterSpacing: "-.01em",
                color: color.onDark,
              }}
            >
              {agent.display_name}
            </h2>
            <div
              style={{
                marginTop: 6,
                display: "flex",
                alignItems: "center",
                gap: 9,
                flexWrap: "wrap",
              }}
            >
              <span
                translate="no"
                style={{
                  font: `400 11px ${font.mono}`,
                  color: filledMix(FILLED_IDENTITY_TEXT_PERCENT),
                }}
              >
                {agent.agent_id}
              </span>
              <span
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 5,
                  padding: "2px 9px",
                  borderRadius: 999,
                  background: filledMix(8),
                  border: `1px solid ${filledMix(16)}`,
                  font: `700 9px ${font.mono}`,
                  letterSpacing: ".07em",
                  textTransform: "uppercase",
                  color: active
                    ? filledForeground(color.green)
                    : filledForeground(color.amber),
                }}
              >
                <span
                  style={{
                    width: 5,
                    height: 5,
                    borderRadius: "50%",
                    background: "currentColor",
                  }}
                />
                {active ? "Active" : "Paused"}
              </span>
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
            <button
              type="button"
              disabled={pending}
              onClick={() => setEditing((open) => !open)}
              aria-expanded={editing}
              style={{
                ...onDarkButton,
                cursor: pending ? "default" : "pointer",
                opacity: pending ? 0.6 : 1,
              }}
            >
              {editing ? "Close edit" : "Edit"}
            </button>
            <button
              type="button"
              disabled={pending}
              onClick={() => (active ? onPause(agent.agent_id) : onResume(agent.agent_id))}
              style={{
                ...onDarkButton,
                cursor: pending ? "default" : "pointer",
                opacity: pending ? 0.6 : 1,
                color: active
                  ? filledForeground(color.amber)
                  : filledForeground(color.green),
              }}
            >
              {active ? "Pause agent" : "Resume agent"}
            </button>
          </div>
        </div>

        {/* Body */}
        <div style={{ padding: 18 }}>
          <SectionLabel>RUNS ON</SectionLabel>
          <div
            style={{
              marginTop: 8,
              padding: "12px 14px",
              borderRadius: radius.md,
              border: `1px solid ${color.border}`,
              background: color.sunken,
              display: "flex",
              alignItems: "center",
              minWidth: 0,
            }}
          >
            <CapabilityStrip capability={agent.capability} />
          </div>

          <div
            style={{
              marginTop: 15,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 170px), 1fr))",
              gap: 8,
            }}
          >
            {/* a legacy agent has no label-shaped id, so no address it can be
                reached at — see `agentAddress`. show none, never a false one. */}
            {agentAddress(agent.agent_id) && (
              <InfoRow label="address" value={agentAddress(agent.agent_id)} />
            )}
            <InfoRow label="owner" value={ownerText(agent.owner)} />
            <InfoRow label="skills" value={skillsSummary(agent.skills ?? [])} />
            <InfoRow label="updated" value={String(agent.updated_at)} />
          </div>

          <div style={{ marginTop: 15 }}>
            <SectionLabel>SKILLS</SectionLabel>
            <div
              style={{
                marginTop: 4,
                font: `400 10.5px ${font.sans}`,
                color: color.muted2,
                lineHeight: 1.5,
              }}
            >
              Always-loaded documents are pasted into every run — they are this agent's
              persona. The others are listed by name and opened only when the job calls
              for one.
            </div>
            <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 6 }}>
              {(agent.skills ?? []).length === 0 ? (
                <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
                  No skills curated — this agent runs on the task instructions alone.
                </span>
              ) : (
                (agent.skills ?? []).map((skill) => (
                  <SkillRow key={`${skill.name}:${skill.source_prefix}`} skill={skill} />
                ))
              )}
            </div>
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
              capabilitiesStatus={capabilitiesStatus}
              pending={pending}
              onUpdate={onUpdate}
              onClose={() => setEditing(false)}
            />
          )}
        </div>
      </div>
    </section>
  );
}

/** The right pane when an EXPLICIT selection names an agent the roster doesn't
 *  hold — a clicked @mention of an agent that has since been removed. Says so,
 *  rather than quietly showing the first agent's pane as if it were the one
 *  asked for. */
export function MissingAgentPane({ agentId, onBack }: { agentId: string; onBack: () => void }) {
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
            background: color.sunken,
            color: color.muted2,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <Icon name="agent" size={22} color="currentColor" strokeWidth={1.6} />
        </span>
        <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Agent not found</div>
        <div style={{ maxWidth: 340, font: `400 12px ${font.sans}`, color: color.muted2, lineHeight: 1.5 }}>
          <span style={{ font: `500 12px ${font.mono}`, color: color.muted3 }}>{agentId}</span> isn’t in
          this workspace’s roster — it may have been removed since it was mentioned.
        </div>
        <button type="button" onClick={onBack} style={{ ...primaryButton(true), marginTop: 4 }}>
          Back to the roster
        </button>
      </div>
    </GroupCard>
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
