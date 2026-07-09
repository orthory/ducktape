// The Activity tab: the jobs-worker toggle and the in-progress runs timeline.
// Every listed run is awaiting its dispatch delivery — the node prunes
// entries the moment a result lands.

import { useEffect, useState } from "react";

import type { AgentRecord } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import { displayNameForKey, shortKey } from "../../../domain/names";
import type { PendingRun } from "../../../domain/runs-client";
import {
  isRunOutputTailItem,
  runOutputTopic,
  type RunStream,
} from "../../../domain/stream";
import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, shadow } from "../../theme/tokens";
import {
  agentLabel,
  channelLabel,
  Chip,
  EmptyState,
  GroupCard,
  initialsOf,
  runDetail,
  runIsMine,
  secondaryButton,
  SectionLabel,
  statusTone,
  StatusPill,
} from "./parts";

type OutputLine =
  | { id: number; kind: "line"; stream: RunStream; text: string }
  | { id: number; kind: "gap"; text: string };

const MAX_OUTPUT_LINES = 1_000;

function RunOutputPane({ run }: { run: PendingRun }) {
  const { transport } = useDucktape();
  const [lines, setLines] = useState<OutputLine[]>([]);

  useEffect(() => {
    setLines([]);
    if (!transport) return;
    let nextId = 0;
    const topic = runOutputTopic(run.dispatch_id);
    return transport.subscribe([topic], {
      onTail: (frame) => {
        if (frame.topic !== topic || !isRunOutputTailItem(frame.item)) return;
        const line: OutputLine = {
          id: nextId++,
          kind: "line",
          stream: frame.item.stream,
          text: frame.item.line,
        };
        setLines((prev) => [...prev, line].slice(-MAX_OUTPUT_LINES));
      },
      onLagged: (laggedTopic, cursor) => {
        if (laggedTopic !== topic) return;
        const line: OutputLine = {
          id: nextId++,
          kind: "gap",
          text: `output gap: dropped older lines before cursor ${cursor}`,
        };
        setLines((prev) => [...prev, line].slice(-MAX_OUTPUT_LINES));
      },
    });
  }, [transport, run.dispatch_id]);

  return (
    <div
      style={{
        marginTop: 10,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: 6,
        background: "#fbfaf7",
        padding: "8px 10px",
        maxHeight: 220,
        overflow: "auto",
      }}
    >
      {lines.length === 0 ? (
        <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          waiting for output…
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          {lines.map((line) =>
            line.kind === "gap" ? (
              <div
                key={line.id}
                style={{
                  font: `600 10px ${font.mono}`,
                  color: color.amber,
                  padding: "2px 0",
                }}
              >
                {line.text}
              </div>
            ) : (
              <div
                key={line.id}
                style={{
                  display: "grid",
                  gridTemplateColumns: "44px 1fr",
                  gap: 8,
                  font: `500 11px ${font.mono}`,
                  color: line.stream === "stderr" ? color.red : color.inkSoft,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                <span
                  style={{
                    color: color.muted2,
                    font: `700 9px ${font.mono}`,
                    textAlign: "right",
                    userSelect: "none",
                  }}
                >
                  {line.stream}
                </span>
                <span>{line.text || " "}</span>
              </div>
            ),
          )}
        </div>
      )}
    </div>
  );
}

/** The daemon-lifecycle switch for job-board pickup — its own row on the
 *  Activity tab, where background work lives. */
export function JobsWorkerRow({
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

function RunRow({
  run,
  agents,
  channels,
  op,
  onCancel,
  assigneeName,
  mine,
}: {
  run: PendingRun;
  agents: AgentRecord[];
  channels: Channel[];
  /** The run's finalization record (a cancel keys by run id). */
  op: OpRecord | undefined;
  onCancel: (id: string) => void;
  /** Display name of the node executing this run, or null when unknown. */
  assigneeName?: string | null;
  /** This run was requested by the local user. */
  mine?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
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
            {assigneeName ? <Chip text={`on ${assigneeName}`} tone={statusTone.agent} /> : null}
            {mine ? <Chip text="you" tone={statusTone.neutral} /> : null}
            <button
              type="button"
              aria-expanded={expanded}
              onClick={() => setExpanded((v) => !v)}
              style={{
                ...secondaryButton,
                minHeight: 22,
                padding: "2px 8px",
                font: `600 10px ${font.sans}`,
              }}
            >
              Output
            </button>
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
          {expanded && <RunOutputPane run={run} />}
        </div>
      </GroupCard>
    </div>
  );
}

export function RunsTimeline({
  runs,
  agents,
  channels,
  ops,
  onCancel,
  runAssignee,
  authorNames,
  workspacePubkey,
}: {
  runs: PendingRun[];
  agents: AgentRecord[];
  channels: Channel[];
  /** The store's finalization ledger — run rows draw their marks. */
  ops: OpLedger;
  onCancel: (id: string) => void;
  /** run_id -> hex node key executing it (the saga assignee). */
  runAssignee: Map<string, string>;
  /** hex key -> display name, for the executor badge. */
  authorNames: Record<string, string>;
  /** The local user's pubkey, for the "you" marker. */
  workspacePubkey: string | null;
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
          {runs.map((run) => {
            const assigneeKey = runAssignee.get(run.run_id) ?? null;
            const assigneeName = assigneeKey
              ? (displayNameForKey(assigneeKey, authorNames) ?? shortKey(assigneeKey))
              : null;
            return (
              <RunRow
                key={run.run_id}
                run={run}
                agents={agents}
                channels={channels}
                op={ops[opKey.run(run.run_id)]}
                onCancel={onCancel}
                assigneeName={assigneeName}
                mine={runIsMine(run, workspacePubkey)}
              />
            );
          })}
        </div>
      )}
    </section>
  );
}
