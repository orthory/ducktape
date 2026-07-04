// The jobs surface over the node's `jobs` module: a consensus-native work
// board. A submitter posts a job, any worker claims it, exactly one claim
// wins by consensus order, the claimant processes off-platform and reports a
// result — this view exposes that whole lifecycle explicitly, one action at
// a time, with no state hidden behind an implicit transition.

import { useState } from "react";
import type { CSSProperties, FormEvent } from "react";

import type { BoardCounts, Job, JobStatus } from "../../../domain/jobs-client";
import { isTerminal } from "../../../domain/jobs-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const STATUS_ORDER: JobStatus[] = ["Pending", "Processing", "Done", "Failed", "Cancelled"];

const COUNTS_KEY: Record<JobStatus, keyof BoardCounts> = {
  Pending: "pending",
  Processing: "processing",
  Done: "done",
  Failed: "failed",
  Cancelled: "cancelled",
};

const STATUS_TONE: Record<
  JobStatus,
  { label: string; countLabel: string; text: string; bg: string; border: string }
> = {
  Pending: {
    label: "Pending",
    countLabel: "pending",
    text: color.amber,
    bg: "#fbf4e6",
    border: "#ecdcae",
  },
  Processing: {
    label: "Processing",
    countLabel: "processing",
    text: color.blue,
    bg: "#eef1f6",
    border: "#d1dbe9",
  },
  Done: {
    label: "Done",
    countLabel: "done",
    text: color.green,
    bg: "#eef5f0",
    border: "#cfe3d7",
  },
  Failed: {
    label: "Failed",
    countLabel: "failed",
    text: color.red,
    bg: color.dangerSoft,
    border: color.dangerBorder,
  },
  Cancelled: {
    label: "Cancelled",
    countLabel: "cancelled",
    text: color.muted2,
    bg: color.sunken,
    border: color.borderSoft,
  },
};

const inputBase: CSSProperties = {
  width: "100%",
  minWidth: 0,
  height: 36,
  padding: "0 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 13px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const textareaBase: CSSProperties = {
  ...inputBase,
  height: 60,
  padding: "8px 12px",
  resize: "vertical",
  font: `400 12.5px ${font.mono}`,
};

const smallInputBase: CSSProperties = {
  height: 28,
  padding: "0 8px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 11.5px ${font.mono}`,
  color: color.ink,
  outline: "none",
};

const shortId = (id: string): string =>
  id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id || "—";

const countFor = (counts: BoardCounts | null, status: JobStatus): number =>
  counts ? counts[COUNTS_KEY[status]] : 0;

function StatusPill({ status }: { status: JobStatus }) {
  const tone = STATUS_TONE[status];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        borderRadius: 999,
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.text,
        padding: "3px 8px",
        font: `700 9.5px ${font.mono}`,
        whiteSpace: "nowrap",
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          background: tone.text,
          flexShrink: 0,
        }}
      />
      {tone.label}
    </span>
  );
}

function CountPill({ status, count }: { status: JobStatus; count: number }) {
  const tone = STATUS_TONE[status];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        borderRadius: 999,
        border: `1px solid ${tone.border}`,
        background: tone.bg,
        color: tone.text,
        padding: "4px 9px",
        font: `600 10.5px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: tone.text }} />
      {count} {tone.countLabel}
    </span>
  );
}

function ActionButton({
  label,
  tone = "neutral",
  disabled,
  onClick,
}: {
  label: string;
  tone?: "neutral" | "accent" | "danger";
  disabled?: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  const palette =
    tone === "accent"
      ? { text: color.paper, bg: accentVar, border: "transparent", hoverBg: color.dark }
      : tone === "danger"
        ? {
            text: color.danger,
            bg: color.dangerSoft,
            border: color.dangerBorder,
            hoverBg: color.dangerBorder,
          }
        : { text: color.inkSoft, bg: color.paper, border: color.borderStrong, hoverBg: color.hover };

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        height: 28,
        padding: "0 10px",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: radius.sm,
        border: `1px solid ${disabled ? color.borderSoft : palette.border}`,
        background: disabled ? color.chip : hover ? palette.hoverBg : palette.bg,
        color: disabled ? color.muted2 : palette.text,
        cursor: disabled ? "default" : "pointer",
        font: `600 11px ${font.sans}`,
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
      {label}
    </button>
  );
}

function JobCard({
  job,
  op,
  onClaim,
  onFinalize,
  onRelease,
  onReclaim,
  onCancel,
  onPrune,
}: {
  job: Job;
  /** The job's finalization record — the meta line draws the inline mark. */
  op: OpRecord | undefined;
  onClaim: (jobId: string, leaseViews: number) => void;
  onFinalize: (jobId: string, ok: boolean, payload: string) => void;
  onRelease: (jobId: string) => void;
  onReclaim: (jobId: string) => void;
  onCancel: (jobId: string) => void;
  onPrune: (jobId: string) => void;
}) {
  const [leaseViews, setLeaseViews] = useState(32);
  const [payload, setPayload] = useState("");

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: "13px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
        <span
          style={{
            font: `600 14px ${font.sans}`,
            color: color.ink,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={job.kind}
        >
          {job.kind}
        </span>
        <StatusPill status={job.status} />
      </div>

      <div
        style={{
          font: `400 11px ${font.mono}`,
          color: color.muted2,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
          #{shortId(job.job_id)} · from {shortId(job.submitter)} · attempt {job.attempt} · h
          {job.created_at_height}
          <FinalizationMark op={op} />
        </span>
      </div>

      <div
        style={{
          padding: "8px 10px",
          borderRadius: radius.sm,
          border: `1px solid ${color.borderSoft}`,
          background: color.sunken,
          color: color.inkSoft,
          font: `400 11.5px ${font.mono}`,
          display: "-webkit-box",
          WebkitLineClamp: 3,
          WebkitBoxOrient: "vertical",
          overflow: "hidden",
        }}
        title={job.spec}
      >
        {job.spec}
      </div>

      {job.claim ? (
        <div style={{ font: `400 11px ${font.mono}`, color: color.muted3 }}>
          claimed by {shortId(job.claim.worker)} · lease {job.claim.lease_views}v
        </div>
      ) : null}

      {job.result ? (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            font: `400 11.5px ${font.mono}`,
            color: job.result.ok ? color.green : color.red,
          }}
        >
          <Icon name={job.result.ok ? "check" : "close"} size={12} strokeWidth={2} />
          {job.result.payload}
        </div>
      ) : null}

      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginTop: 2 }}>
        {job.status === "Pending" ? (
          <>
            <input
              type="number"
              aria-label={`Lease views for ${job.kind}`}
              min={1}
              value={leaseViews}
              onChange={(event) => setLeaseViews(Math.max(1, Number(event.target.value) || 1))}
              style={{ ...smallInputBase, width: 68 }}
            />
            <ActionButton
              label="Claim"
              tone="accent"
              onClick={() => onClaim(job.job_id, leaseViews)}
            />
            <ActionButton label="Cancel" onClick={() => onCancel(job.job_id)} />
          </>
        ) : null}

        {job.status === "Processing" ? (
          <>
            <input
              type="text"
              aria-label={`Result payload for ${job.kind}`}
              placeholder="payload"
              value={payload}
              onChange={(event) => setPayload(event.target.value)}
              style={{ ...smallInputBase, width: 140, font: `400 11.5px ${font.sans}` }}
            />
            <ActionButton
              label="Finalize ✓"
              tone="accent"
              onClick={() => onFinalize(job.job_id, true, payload)}
            />
            <ActionButton
              label="Finalize ✗"
              tone="danger"
              onClick={() => onFinalize(job.job_id, false, payload)}
            />
            <ActionButton label="Release" onClick={() => onRelease(job.job_id)} />
            <ActionButton label="Reclaim" onClick={() => onReclaim(job.job_id)} />
          </>
        ) : null}

        {isTerminal(job.status) ? (
          <ActionButton label="Prune" tone="danger" onClick={() => onPrune(job.job_id)} />
        ) : null}
      </div>
    </div>
  );
}

function CenterState({ title, detail, muted }: { title: string; detail: string; muted?: boolean }) {
  return (
    <div
      style={{
        minHeight: 280,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: muted ? color.sunken : "#eef5f0",
          color: muted ? color.muted : color.green,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="jobs" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.55,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

export function JobsView() {
  const { state, actions } = useDucktape();
  const [kind, setKind] = useState("");
  const [spec, setSpec] = useState("");
  const [kindFocus, setKindFocus] = useState(false);
  const [buttonHover, setButtonHover] = useState(false);

  const loading = state.status === null;
  const backed = Boolean(state.status?.modules.some((mod) => mod.id === "jobs"));
  const jobCount = state.jobs.length;
  const canSubmit = backed && kind.trim().length > 0;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmedKind = kind.trim();
    if (!backed || !trimmedKind) return;
    actions.submitJob({ kind: trimmedKind, spec: spec.trim() });
    setKind("");
    setSpec("");
  };

  const sortedJobs = [...state.jobs].sort((a, b) => b.created_at_height - a.created_at_height);

  return (
    <div
      data-screen-label="Jobs"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Jobs</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>{jobCount}</span>
        <div
          style={{
            marginLeft: "auto",
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          {STATUS_ORDER.map((status) => (
            <CountPill key={status} status={status} count={countFor(state.jobCounts, status)} />
          ))}
        </div>
      </div>

      <form
        onSubmit={submit}
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "flex-end",
          gap: 10,
          padding: "13px 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.sidebar,
        }}
      >
        <label
          htmlFor="job-kind"
          style={{
            width: 180,
            flexShrink: 0,
            display: "grid",
            gap: 6,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".08em",
            color: backed ? color.muted2 : color.muted,
          }}
        >
          KIND
          <input
            id="job-kind"
            value={kind}
            disabled={!backed}
            onChange={(event) => setKind(event.target.value)}
            onFocus={() => setKindFocus(true)}
            onBlur={() => setKindFocus(false)}
            placeholder={loading ? "Loading…" : "e.g. render"}
            style={{
              ...inputBase,
              borderColor: kindFocus ? accentVar : color.borderStrong,
              background: backed ? color.paper : color.sunken,
              color: backed ? color.ink : color.muted2,
            }}
          />
        </label>
        <label
          htmlFor="job-spec"
          style={{
            flex: 1,
            minWidth: 0,
            display: "grid",
            gap: 6,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".08em",
            color: backed ? color.muted2 : color.muted,
          }}
        >
          SPEC
          <textarea
            id="job-spec"
            value={spec}
            disabled={!backed}
            onChange={(event) => setSpec(event.target.value)}
            placeholder={loading ? "Loading…" : "Describe the work"}
            style={{
              ...textareaBase,
              background: backed ? color.paper : color.sunken,
              color: backed ? color.ink : color.muted2,
            }}
          />
        </label>
        <button
          type="submit"
          aria-label="Post job"
          disabled={!canSubmit}
          onMouseEnter={() => setButtonHover(true)}
          onMouseLeave={() => setButtonHover(false)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            height: 36,
            padding: "0 14px",
            borderRadius: radius.sm,
            background: canSubmit ? (buttonHover ? color.dark : accentVar) : color.chip,
            color: canSubmit ? color.paper : color.muted2,
            border: `1px solid ${canSubmit ? "transparent" : color.borderStrong}`,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: 6,
            flexShrink: 0,
            cursor: canSubmit ? "pointer" : "default",
            font: `600 12px ${font.sans}`,
            whiteSpace: "nowrap",
          }}
        >
          <Icon name="plus" size={14} strokeWidth={1.9} />
          Post job
        </button>
      </form>

      <div
        style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 18, background: color.sidebar }}
      >
        <div
          style={{
            minHeight: "100%",
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            boxShadow: shadow.card,
            overflow: "hidden",
          }}
        >
          {loading ? (
            <CenterState title="Loading the board…" detail="Waiting for this node's committed job board." muted />
          ) : !backed ? (
            <CenterState
              title="Jobs module is not available"
              detail="This node did not report a jobs module, so the work board is disabled."
              muted
            />
          ) : sortedJobs.length === 0 ? (
            <CenterState
              title="The board is empty"
              detail="The board is empty — post a job above."
            />
          ) : (
            sortedJobs.map((job) => (
              <JobCard
                key={job.job_id}
                job={job}
                op={state.ops[opKey.job(job.job_id)]}
                onClaim={(jobId, leaseViews) => actions.claimJob({ jobId, leaseViews })}
                onFinalize={(jobId, ok, payload) => actions.finalizeJob({ jobId, ok, payload })}
                onRelease={(jobId) => actions.releaseJob(jobId)}
                onReclaim={(jobId) => actions.reclaimJob(jobId)}
                onCancel={(jobId) => actions.cancelJob(jobId)}
                onPrune={(jobId) => actions.pruneJob(jobId)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
