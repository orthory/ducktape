// The agents surface over the `agent` registry and the `runs` module — the
// collaboration loop's record book and its actor. It stays render-only over
// useDucktape: roster, watches, pending runs, and composers all submit
// through the store action facade. Run lifecycle lives in the dispatch
// module; this surface shows only the in-flight entries (pruned when a
// result delivers).
//
// No optimistic state: every write goes through the store's submit-then-refresh.
//
// This file is the thin shell — tabs, layout, store wiring. The sections live
// in sibling files; parts.tsx holds the shared atoms.

import { useEffect, useState } from "react";

import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";
import { AgentDetail, MissingAgentPane, NoAgentsPane } from "./AgentDetail";
import { primaryButton, runIsMine, secondaryButton, statusTone } from "./parts";
import { RegisterAgentForm } from "./RegisterAgentForm";
import { RosterList } from "./RosterList";
import { JobsWorkerRow, RunsTimeline } from "./RunsTimeline";
import { UsageCard } from "./UsageCard";
import { WatchesPanel } from "./WatchesPanel";

export { runIsMine };

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
        color: active ? accentVar : color.muted2,
        whiteSpace: "nowrap",
      }}
    >
      {label}
      <span
        style={{
          minWidth: 16,
          padding: "0 5px",
          borderRadius: 999,
          textAlign: "center",
          background: active ? statusTone.agent.bg : color.sidebar,
          font: `600 10px ${font.mono}`,
          color: active ? accentVar : color.muted2,
        }}
      >
        {count}
      </span>
    </button>
  );
}

export function AgentView() {
  const { state, actions } = useDucktape();
  const [tab, setTab] = useState<AgentTab>("agents");
  const [runFilter, setRunFilter] = useState<"all" | "mine">("all");
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [jobWorkerOn, setJobWorkerOn] = useState(false);
  // With nothing selected, the first agent is a fine default. But once a
  // selection is EXPLICIT — the roster, or a clicked @agent mention naming an
  // agent that has since been removed — falling back to the first agent would
  // silently show a DIFFERENT agent's pane as if it were the one asked for.
  // An id we can't resolve is a miss, not a redirect. (`agents` is empty until
  // the roster loads, which is a miss too — NoAgentsPane covers that.)
  const explicitMiss =
    selectedAgentId !== null &&
    state.agents.length > 0 &&
    !state.agents.some((agent) => agent.agent_id === selectedAgentId);
  const selectedAgent =
    selectedAgentId === null
      ? (state.agents[0] ?? null)
      : (state.agents.find((agent) => agent.agent_id === selectedAgentId) ?? null);

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

  // Consume a clicked @agent mention's hand-off (state.agentFocus). One-shot:
  // cleared on consume, so the same agent can be clicked again after browsing
  // away. Selection is by id only — the roster may not hold it yet, and the
  // detail pane already falls back to the first agent when it doesn't.
  const { agentFocus } = state;
  useEffect(() => {
    if (agentFocus === null) return;
    setTab("agents");
    setAdding(false);
    setSelectedAgentId(agentFocus);
    actions.clearAgentFocus();
  }, [agentFocus, actions]);

  return (
    <div
      data-screen-label="Agents"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
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
        <h1
          style={{
            margin: 0,
            font: `650 18px ${font.sans}`,
            letterSpacing: "0",
            color: color.ink,
          }}
        >
          Agents
        </h1>

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
          <RosterList
            agents={state.agents}
            selectedId={adding ? null : (selectedAgent?.agent_id ?? null)}
            ops={state.ops}
            onSelect={selectAgent}
          />

          <main style={{ flex: 1, minWidth: 0, minHeight: 0, overflowY: "auto", padding: 22 }}>
            <div style={{ maxWidth: 640, margin: "0 auto" }}>
              {adding ? (
                <RegisterAgentForm
                  capabilities={state.capabilities}
                  capabilitiesStatus={state.capabilitiesStatus}
                  onRetryCapabilities={
                    state.connected ? actions.refreshCapabilities : undefined
                  }
                  onRegister={actions.registerAgent}
                  onDone={() => setAdding(false)}
                />
              ) : selectedAgent ? (
                <AgentDetail
                  agent={selectedAgent}
                  capabilities={state.capabilities}
                  capabilitiesStatus={state.capabilitiesStatus}
                  onPause={actions.pauseAgent}
                  onResume={actions.resumeAgent}
                  onUpdate={actions.updateAgent}
                />
              ) : explicitMiss ? (
                <MissingAgentPane agentId={selectedAgentId!} onBack={() => setSelectedAgentId(null)} />
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
            <UsageCard refreshKey={state.pendingRuns.map((run) => run.run_id).join("\n")} />
            <div style={{ display: "flex", gap: 6, marginBottom: 12 }}>
              {(["all", "mine"] as const).map((f) => (
                <button
                  key={f}
                  type="button"
                  onClick={() => setRunFilter(f)}
                  style={{
                    ...secondaryButton,
                    minHeight: 26,
                    padding: "3px 10px",
                    background: runFilter === f ? color.dark : color.paper,
                    color: runFilter === f ? color.onDark : color.muted2,
                  }}
                >
                  {f === "all" ? "All" : "Requested by you"}
                </button>
              ))}
            </div>
            <RunsTimeline
              runs={
                runFilter === "mine"
                  ? state.pendingRuns.filter((run) =>
                      runIsMine(run, state.workspace?.pubkey ?? null),
                    )
                  : state.pendingRuns
              }
              agents={state.agents}
              channels={state.channels}
              ops={state.ops}
              onCancel={actions.cancelRun}
              onReassign={actions.reassignRun}
              runLease={state.runLease}
              currentHeight={state.status?.height ?? state.lastBlock ?? 0}
              authorNames={state.authorNames}
              workspacePubkey={state.workspace?.pubkey ?? null}
            />
          </div>
        </main>
      )}
    </div>
  );
}
