// The Activity tab: the jobs-worker toggle, the in-progress runs timeline,
// and the delivered-runs history (the node's last-100 ring, recorded at
// delivery). Every IN PROGRESS run is awaiting its dispatch delivery — the
// node prunes entries the moment a result lands, at which point the run
// reappears under HISTORY.

import { useEffect, useMemo, useRef, useState } from "react";

import type { AgentRecord } from "../../../domain/agent-client";
import type { Channel } from "../../../domain/chat-client";
import type { RunLease } from "../../../domain/dispatch-client";
import { forgeItemTarget } from "../../../domain/forge-client";
import { displayNameForKey, shortKey } from "../../../domain/names";
import { dispatchIdForRun, recentRuns } from "../../../domain/runs-client";
import type { PendingRun, RunRecord } from "../../../domain/runs-client";
import {
  isRunOutputTailItem,
  runOutputTopic,
} from "../../../domain/stream";
import { FinalizationMark } from "../../components/FinalizationMark";
import { opKey } from "../../store/finalization";
import type { OpLedger, OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import { wallClockMillisOf } from "../../../domain/wire";
import { relTime } from "../forge/ui";
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
import {
  appendActivityEntry,
  parseActivityLog,
  type ActivityLogEntry,
  type ActivityLogRow,
} from "./run-log-lines";

// ── Consensus-counter rendering ──────────────────────────
// `created_at`/`delivered_at` are whatever the lane stamps: the embedded
// daemon writes unix MILLIS, legacy rows unix seconds, a validator its block
// height. Raw values must never render as clock time; `wallClockMillisOf`
// (domain/wire.ts) owns the lane sniffing, and durations always come from
// the created→delivered DIFF.

/** The counter as unix seconds when it is a real wall-clock stamp, else null
 *  (a height counter — not renderable as time). */
const wallClockSecs = (counter: number): number | null => {
  const ms = wallClockMillisOf(counter);
  return ms === null ? null : ms / 1000;
};

const formatSeconds = (secs: number): string => {
  if (secs < 1) return "<1s";
  if (secs < 60) return `${Math.round(secs)}s`;
  const minutes = Math.floor(secs / 60);
  if (minutes < 60) return `${minutes}m ${Math.round(secs % 60)}s`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
};

/** created→delivered as an honest duration: seconds for wall-clock lanes
 *  (both endpoints normalized via wallClockSecs), a block count for height
 *  lanes. */
const runDuration = (rec: RunRecord): string => {
  const start = wallClockSecs(rec.created_at);
  const end = wallClockSecs(rec.delivered_at);
  if (start !== null && end !== null) return formatSeconds(Math.max(0, end - start));
  const diff = Math.max(0, rec.delivered_at - rec.created_at);
  return diff === 1 ? "1 block" : `${diff} blocks`;
};

/** `branch@oid` with the oid clipped to 8 hex chars; other refs clipped flat. */
const shortOutputRef = (ref: string): string => {
  const at = ref.indexOf("@");
  if (at >= 0 && ref.length > at + 9) return `${ref.slice(0, at + 9)}…`;
  return ref.length > 16 ? `${ref.slice(0, 16)}…` : ref;
};

function RunOutputPane({
  runLabel,
  dispatchId,
  panelId,
  terminal = false,
}: {
  runLabel: string;
  dispatchId: string;
  panelId: string;
  terminal?: boolean;
}) {
  const { transport } = useDucktape();
  const [entries, setEntries] = useState<ActivityLogEntry[]>([]);
  const [unavailable, setUnavailable] = useState(!transport);
  const pane = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setEntries([]);
    setUnavailable(!transport);
    if (!transport) return;
    let lastCursor = 0;
    const topic = runOutputTopic(dispatchId);
    return transport.subscribe([topic], {
      onTail: (frame) => {
        if (frame.topic !== topic || !isRunOutputTailItem(frame.item)) return;
        const cursor = Number(frame.cursor);
        if (!Number.isSafeInteger(cursor) || cursor <= lastCursor) return;
        lastCursor = cursor;
        const line: ActivityLogEntry = {
          kind: "line",
          stream: frame.item.stream,
          text: frame.item.line,
        };
        setEntries((prev) => appendActivityEntry(prev, line));
      },
      onLagged: (laggedTopic, cursor) => {
        if (laggedTopic !== topic) return;
        const nextCursor = Number(cursor);
        if (!Number.isSafeInteger(nextCursor) || nextCursor <= lastCursor) return;
        lastCursor = nextCursor;
        const line: ActivityLogEntry = {
          kind: "gap",
          text: `output gap: dropped older lines before cursor ${cursor}`,
        };
        setEntries((prev) => appendActivityEntry(prev, line));
      },
      onRefused: (refusedTopic) => {
        if (refusedTopic === topic) setUnavailable(true);
      },
    });
  }, [transport, dispatchId]);

  const rows = useMemo(() => parseActivityLog(entries), [entries]);

  useEffect(() => {
    if (pane.current) pane.current.scrollTop = pane.current.scrollHeight;
  }, [rows.length]);

  useEffect(() => {
    pane.current?.focus();
  }, []);

  const rowLabel = (row: ActivityLogRow): string =>
    row.kind === "blank" ? "" : row.kind.toUpperCase();

  return (
    <div
      id={panelId}
      ref={pane}
      role="log"
      aria-label={`Execution log for run ${runLabel}`}
      aria-live="polite"
      tabIndex={0}
      style={{
        marginTop: 10,
        border: `1px solid ${color.borderSoft}`,
        borderRadius: 6,
        background: color.canvas,
        padding: "8px 10px",
        maxHeight: 220,
        overflow: "auto",
        flexBasis: "100%",
      }}
    >
      {rows.length === 0 ? (
        <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          {unavailable
            ? "Run output unavailable."
            : terminal
              ? "No retained output received — older output may have been evicted."
              : "Waiting for retained output…"}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          {rows.map((row, index) =>
            row.kind === "gap" ? (
              <div
                key={`${row.kind}-${index}`}
                style={{
                  font: `600 10px ${font.mono}`,
                  color: color.amber,
                  padding: "2px 0",
                }}
              >
                {row.text}
              </div>
            ) : (
              <div
                key={`${row.kind}-${index}`}
                style={{
                  display: "grid",
                  gridTemplateColumns: "48px 64px 1fr",
                  gap: 8,
                  font: `500 11px ${font.mono}`,
                  color: row.stream === "stderr" ? color.red : color.inkSoft,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                  minHeight: row.kind === "blank" ? 5 : undefined,
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
                  {row.stream ?? ""}
                </span>
                <span
                  style={{
                    color: row.kind === "status" ? color.muted2 : color.accentAlt1,
                    font: `700 9px ${font.mono}`,
                    userSelect: "none",
                  }}
                >
                  {rowLabel(row)}
                </span>
                <span>{row.text || " "}</span>
              </div>
            ),
          )}
        </div>
      )}
    </div>
  );
}

/** The daemon-lifecycle controls for job-board pickup — their own row on the
 *  Activity tab, where background work lives. */
export function JobsWorkerRow({
  op,
  onToggle,
}: {
  op: OpRecord | undefined;
  onToggle: (enabled: boolean) => void;
}) {
  const pending = op?.phase === "pending";
  const actionStyle = {
    ...secondaryButton,
    cursor: pending ? "default" : "pointer",
    opacity: pending ? 0.6 : 1,
  };

  return (
    <GroupCard style={{ marginBottom: 16 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 14px" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>Jobs worker</div>
          <div style={{ marginTop: 2, font: `400 11px ${font.sans}`, color: color.muted2 }}>
            {pending
              ? "Waiting for confirmation…"
              : "Current committed status is not readable on this network."}
          </div>
        </div>
        <FinalizationMark op={op} />
        <div style={{ display: "flex", gap: 6 }}>
          <button
            type="button"
            disabled={pending}
            onClick={() => onToggle(true)}
            style={actionStyle}
          >
            Enable worker
          </button>
          <button
            type="button"
            disabled={pending}
            onClick={() => onToggle(false)}
            style={actionStyle}
          >
            Disable worker
          </button>
        </div>
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
  onReassign,
  assigneeName,
  lease,
  currentHeight,
  mine,
}: {
  run: PendingRun;
  agents: AgentRecord[];
  channels: Channel[];
  /** The run's finalization record (a cancel keys by run id). */
  op: OpRecord | undefined;
  onCancel: (id: string) => void;
  onReassign: (id: string, attempt: number) => void;
  /** Display name of the node executing this run, or null when unknown. */
  assigneeName?: string | null;
  lease?: RunLease | null;
  currentHeight: number;
  /** This run was requested by the local user. */
  mine?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const logPanelId = `run-log-${run.dispatch_id}`;
  const leaseRemaining =
    lease?.expiresAt === null || lease?.expiresAt === undefined
      ? null
      : Math.max(0, lease.expiresAt - currentHeight);
  const heartbeatAge =
    lease?.updatedAt === null || lease?.updatedAt === undefined
      ? null
      : Math.max(0, currentHeight - lease.updatedAt);
  const canReassign =
    lease?.reassignable === true &&
    lease?.assigneeHex !== null &&
    lease?.assigneeHex !== undefined &&
    lease.attempt + 1 < lease.maxAttempts;
  const agentName = agentLabel(agents, run.agent_id);
  const label = run.job_id
    ? `job ${run.job_id}`
    : `${channelLabel(channels, run.channel_id)} @${run.anchor_seq}`;
  const runLabel = `${agentName} — ${label}`.replace(/\p{Cc}+/gu, " ").trim();
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
          boxShadow: `0 0 0 3px ${color.canvas}`,
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
          <StatusPill
            label={leaseRemaining === 0 ? "LEASE EXPIRED" : "WORKING…"}
            tone={leaseRemaining === 0 ? statusTone.danger : statusTone.warning}
          />
          <StatusPill label={run.job_id ? "JOB" : "CHAT"} tone={run.job_id ? statusTone.agent : statusTone.blue} />
          {canReassign && (
            <button
              type="button"
              onClick={() => onReassign(run.run_id, lease.attempt)}
              aria-label={`Force reassign run ${runLabel}`}
              style={{ ...secondaryButton, marginLeft: "auto", minHeight: 28 }}
            >
              Force reassign
            </button>
          )}
          <button
            type="button"
            onClick={() => onCancel(run.run_id)}
            aria-label={`Cancel run ${runLabel}`}
            style={{
              ...secondaryButton,
              marginLeft: canReassign ? 0 : "auto",
              minHeight: 28,
              color: color.red,
            }}
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
            {lease ? <Chip text={`attempt ${lease.attempt + 1}/${lease.maxAttempts}`} /> : null}
            {leaseRemaining !== null ? <Chip text={`lease ${leaseRemaining} views`} /> : null}
            {heartbeatAge !== null ? <Chip text={`heartbeat ${heartbeatAge} views ago`} /> : null}
            {mine ? <Chip text="you" tone={statusTone.neutral} /> : null}
            <button
              type="button"
              aria-expanded={expanded}
              aria-controls={logPanelId}
              aria-label={`${expanded ? "Hide" : "Show"} live log for run ${runLabel}`}
              onClick={() => setExpanded((v) => !v)}
              style={{
                ...secondaryButton,
                minHeight: 22,
                padding: "2px 8px",
                font: `600 10px ${font.sans}`,
              }}
            >
              Live log
            </button>
          </div>
          <div
            title={`run ${runLabel} · ${runDetail(run)}`}
            style={{
              marginTop: 7,
              font: `400 11px ${font.mono}`,
              color: color.muted2,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {/* created_at is a consensus counter — a height lane's raw value
                is NOT a time, so it renders the dispatch detail instead. */}
            {wallClockSecs(run.created_at) !== null
              ? `started ${relTime(wallClockSecs(run.created_at)!)}`
              : runDetail(run)}
          </div>
          {expanded && (
            <RunOutputPane
              runLabel={runLabel}
              dispatchId={run.dispatch_id}
              panelId={logPanelId}
            />
          )}
        </div>
      </GroupCard>
    </div>
  );
}

function HistoryRow({
  rec,
  agents,
  channels,
  authorNames,
}: {
  rec: RunRecord;
  agents: AgentRecord[];
  channels: Channel[];
  authorNames: Record<string, string>;
}) {
  const { actions } = useDucktape();
  const [expanded, setExpanded] = useState(false);
  const delivered = rec.outcome === "delivered";
  const agentName = agentLabel(agents, rec.agent_id);
  const anchor = rec.channel_id
    ? `${channelLabel(channels, rec.channel_id)} @${rec.anchor_seq}`
    : "job";
  const runLabel = `${agentName} — ${anchor}`.replace(/\p{Cc}+/gu, " ").trim();
  const nodeName =
    rec.executing_node !== "unknown"
      ? (displayNameForKey(rec.executing_node, authorNames) ?? shortKey(rec.executing_node))
      : null;
  const forgeTarget = rec.channel_id
    ? forgeItemTarget(rec.channel_id, { messageSeq: rec.anchor_seq })
    : null;
  const prNumber = rec.pr_number;
  const dispatchId = dispatchIdForRun(rec.run_id);
  const logPanelId = `run-log-${dispatchId}`;
  const openAnchor = () => {
    if (forgeTarget) actions.openForgeItem(forgeTarget);
    else if (rec.channel_id && rec.anchor_seq > 0) {
      actions.focusMessage(rec.channel_id, rec.anchor_seq);
    }
  };
  return (
    <div
      title={`run ${runLabel}`}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 7,
        flexWrap: "wrap",
        padding: "9px 12px",
        borderTop: `1px solid ${color.borderSoft}`,
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
      <StatusPill
        label={delivered ? "DELIVERED" : "FAILED"}
        tone={delivered ? statusTone.success : statusTone.danger}
      />
      {rec.degraded && <StatusPill label="DEGRADED" tone={statusTone.warning} />}
      {rec.channel_id && rec.anchor_seq > 0 ? (
        <button
          type="button"
          onClick={openAnchor}
          aria-label={`Open ${anchor}`}
          style={{
            ...secondaryButton,
            minHeight: 22,
            padding: "2px 7px",
            color: statusTone.blue.text,
          }}
        >
          {anchor}
        </button>
      ) : (
        <Chip text={anchor} tone={statusTone.blue} />
      )}
      <Chip text={runDuration(rec)} />
      {nodeName && <Chip text={`on ${nodeName}`} tone={statusTone.agent} />}
      {prNumber !== null && forgeTarget ? (
        <button
          type="button"
          onClick={() =>
            actions.openForgeItem({ repo: forgeTarget.repo, number: prNumber })
          }
          aria-label={`Open PR #${prNumber}`}
          style={{
            ...secondaryButton,
            minHeight: 22,
            padding: "2px 7px",
            color: statusTone.success.text,
          }}
        >
          PR #{prNumber}
        </button>
      ) : prNumber !== null ? (
        <Chip text={`PR #${prNumber}`} tone={statusTone.success} />
      ) : null}
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={logPanelId}
        aria-label={`${expanded ? "Hide" : "Show"} execution log for run ${runLabel}`}
        onClick={() => setExpanded((value) => !value)}
        style={{
          ...secondaryButton,
          minHeight: 22,
          padding: "2px 8px",
          font: `600 10px ${font.sans}`,
        }}
      >
        Log
      </button>
      {rec.output_ref && (
        <span
          title={rec.output_ref}
          style={{ font: `500 10px ${font.mono}`, color: color.muted2, marginLeft: "auto" }}
        >
          {shortOutputRef(rec.output_ref)}
        </span>
      )}
      {expanded && (
        <RunOutputPane
          runLabel={runLabel}
          dispatchId={dispatchId}
          panelId={logPanelId}
          terminal
        />
      )}
    </div>
  );
}

/** The delivered-runs history: the node's last-100 ring, re-pulled on every
 *  finalized block (a delivery lands a block, so the list stays live). */
function RunHistory({
  agents,
  channels,
  authorNames,
}: {
  agents: AgentRecord[];
  channels: Channel[];
  authorNames: Record<string, string>;
}) {
  const { state, transport } = useDucktape();
  const [history, setHistory] = useState<RunRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!transport) return;
    let alive = true;
    recentRuns(transport)
      .then((records) => {
        if (!alive) return;
        setError(null);
        setHistory(records);
      })
      .catch((e) => {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [transport, state.lastBlock]);

  return (
    <section aria-label="Run history" style={{ minWidth: 0, marginTop: 22 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionLabel>HISTORY</SectionLabel>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
          {history?.length ?? 0}
        </span>
      </div>
      {error && (
        <div style={{ marginTop: 8, font: `500 11px ${font.sans}`, color: color.red }}>
          run history unavailable: {error}
        </div>
      )}
      {!error && (history === null || history.length === 0) && (
        <div style={{ marginTop: 8, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          {history === null
            ? "Loading run history..."
            : "No delivered runs yet — finished runs land here (the node keeps the last 100)."}
        </div>
      )}
      {!error && history !== null && history.length > 0 && (
        <GroupCard style={{ marginTop: 9 }}>
          {history.map((rec) => (
            <HistoryRow
              key={rec.run_id}
              rec={rec}
              agents={agents}
              channels={channels}
              authorNames={authorNames}
            />
          ))}
        </GroupCard>
      )}
    </section>
  );
}

export function RunsTimeline({
  runs,
  agents,
  channels,
  ops,
  onCancel,
  onReassign,
  runLease,
  currentHeight,
  authorNames,
  workspacePubkey,
}: {
  runs: PendingRun[];
  agents: AgentRecord[];
  channels: Channel[];
  /** The store's finalization ledger — run rows draw their marks. */
  ops: OpLedger;
  onCancel: (id: string) => void;
  onReassign: (id: string, attempt: number) => void;
  /** run_id -> current saga lease. */
  runLease: Map<string, RunLease>;
  currentHeight: number;
  /** hex key -> display name, for the executor badge. */
  authorNames: Record<string, string>;
  /** The local user's pubkey, for the "you" marker. */
  workspacePubkey: string | null;
}) {
  return (
    <>
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
              const lease = runLease.get(run.run_id) ?? null;
              const assigneeKey = lease?.assigneeHex ?? null;
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
                  onReassign={onReassign}
                  assigneeName={assigneeName}
                  lease={lease}
                  currentHeight={currentHeight}
                  mine={runIsMine(run, workspacePubkey)}
                />
              );
            })}
          </div>
        )}
      </section>
      <RunHistory agents={agents} channels={channels} authorNames={authorNames} />
    </>
  );
}
