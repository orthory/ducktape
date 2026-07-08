// The roster aside on the Agents tab: one row per registered agent with its
// live status and finalization mark, plus the list chrome (count, empty
// state). Selection stays in the shell — this list only reports clicks.

import type { AgentRecord } from "../../../domain/agent-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { accentVar, color, font } from "../../theme/tokens";
import { AgentAvatar, capabilityShort, EmptyState, SectionLabel } from "./parts";

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
        padding: "12px 14px",
        background: selected ? "#faf4ef" : "transparent",
        cursor: "pointer",
        textAlign: "left",
        boxShadow: selected ? `inset 3px 0 0 ${accentVar}` : undefined,
      }}
    >
      <AgentAvatar name={agent.display_name} size={36} />
      <span style={{ flex: 1, minWidth: 0 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
          <span
            style={{
              flex: 1,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `600 13.5px ${font.sans}`,
              color: selected ? color.dark : color.ink,
            }}
          >
            {agent.display_name}
          </span>
          <FinalizationMark op={op} />
        </span>
        <span
          style={{
            marginTop: 3,
            display: "flex",
            alignItems: "center",
            gap: 6,
            minWidth: 0,
          }}
        >
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: active ? color.green : color.amber,
              flexShrink: 0,
            }}
          />
          <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted3, flexShrink: 0 }}>
            {active ? "Active" : "Paused"}
          </span>
          <span style={{ color: color.iconIdle, flexShrink: 0 }}>·</span>
          <span
            translate="no"
            title={agent.capability}
            style={{
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `500 10.5px ${font.mono}`,
              color: color.muted2,
            }}
          >
            {capabilityShort(agent.capability)}
          </span>
        </span>
      </span>
    </button>
  );
}

export function RosterList({
  agents,
  selectedId,
  ops,
  onSelect,
}: {
  agents: AgentRecord[];
  /** The highlighted agent id — null while the Add pane is open. */
  selectedId: string | null;
  /** The store's finalization ledger — rows draw their marks. */
  ops: OpLedger;
  onSelect: (agentId: string) => void;
}) {
  return (
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
          {agents.length} total
        </span>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {agents.length === 0 ? (
          <EmptyState icon="agent" title="No agents yet" body="Add an agent to get started." />
        ) : (
          agents.map((agent) => (
            <AgentListButton
              key={agent.agent_id}
              agent={agent}
              selected={selectedId === agent.agent_id}
              op={ops[opKey.agent(agent.agent_id)]}
              onSelect={onSelect}
            />
          ))
        )}
      </div>
    </aside>
  );
}
