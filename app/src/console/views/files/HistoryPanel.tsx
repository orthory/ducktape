// The history panel: the bounded commit window (newest-first). Selecting a
// snapshot switches the browser to read that historical tree; while one is
// selected the panel also shows its path-level diff against the live head.

import { useEffect, useState } from "react";

import { diff, history } from "../../../domain/files-client";
import type { FileDiffEntry, FileSnapshot } from "../../../domain/files-client";
import type { NodeTransport } from "../../../domain/transport";
import { Icon } from "../../components/Icon";
import { color, font } from "../../theme/tokens";
import { errMsg, formatTime, shortHash } from "./files-format";

/** How many commits the panel pulls (the module's window is bounded anyway). */
const HISTORY_LIMIT = 100;

const DIFF_TONE: Record<FileDiffEntry["kind"], string> = {
  added: color.green,
  removed: color.danger,
  modified: color.amber,
};

function SnapshotRow({
  snapshot,
  active,
  onSelect,
}: {
  snapshot: FileSnapshot;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={`Browse snapshot ${shortHash(snapshot.id)}`}
      aria-pressed={active}
      onClick={onSelect}
      style={{
        all: "unset",
        boxSizing: "border-box",
        width: "100%",
        cursor: "pointer",
        padding: "9px 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
        borderLeft: `2px solid ${active ? color.accent : "transparent"}`,
        background: active ? color.sidebar : "transparent",
        display: "flex",
        flexDirection: "column",
        gap: 3,
      }}
    >
      <span
        style={{
          font: `600 12px ${font.sans}`,
          color: color.ink,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {snapshot.message || "(no message)"}
      </span>
      <span style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>
        h{snapshot.height} · {formatTime(snapshot.consensus_time)} · {shortHash(snapshot.id)}
      </span>
    </button>
  );
}

export function HistoryPanel({
  transport,
  head,
  snapshot,
  reloadToken,
  onSelectSnapshot,
  onClose,
}: {
  transport: NodeTransport;
  head: string | null;
  /** The currently-browsed snapshot id, or null for the live head. */
  snapshot: string | null;
  /** Bumped by the container after a write, so a new commit shows up. */
  reloadToken: number;
  onSelectSnapshot: (id: string | null) => void;
  onClose: () => void;
}) {
  const [snapshots, setSnapshots] = useState<FileSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [diffEntries, setDiffEntries] = useState<FileDiffEntry[] | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setError(null);
    history(transport, { limit: HISTORY_LIMIT })
      .then((rows) => {
        if (!alive) return;
        setSnapshots(rows);
        setLoading(false);
      })
      .catch((err) => {
        if (!alive) return;
        setError(errMsg(err));
        setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [transport, reloadToken]);

  // Diff the browsed snapshot against head (only while a past snapshot is open).
  useEffect(() => {
    if (!snapshot || !head || snapshot === head) {
      setDiffEntries(null);
      return;
    }
    let alive = true;
    setDiffLoading(true);
    diff(transport, { from: snapshot, to: head })
      .then((rows) => {
        if (!alive) return;
        setDiffEntries(rows);
        setDiffLoading(false);
      })
      .catch(() => {
        if (!alive) return;
        setDiffEntries(null);
        setDiffLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [transport, snapshot, head]);

  return (
    <div
      style={{
        width: 300,
        flexShrink: 0,
        borderLeft: `1px solid ${color.border}`,
        background: color.paper,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "12px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <Icon name="metrics" size={15} strokeWidth={1.7} />
        <span style={{ flex: 1, font: `600 13px ${font.sans}`, color: color.ink }}>History</span>
        <button
          type="button"
          aria-label="Close history panel"
          onClick={onClose}
          style={{ all: "unset", cursor: "pointer", color: color.muted2, display: "inline-flex", padding: 2 }}
        >
          <Icon name="close" size={14} strokeWidth={1.9} />
        </button>
      </div>

      <button
        type="button"
        aria-label="Browse the live head"
        aria-pressed={snapshot === null}
        onClick={() => onSelectSnapshot(null)}
        style={{
          all: "unset",
          boxSizing: "border-box",
          width: "100%",
          cursor: "pointer",
          padding: "9px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
          borderLeft: `2px solid ${snapshot === null ? color.accent : "transparent"}`,
          background: snapshot === null ? color.sidebar : "transparent",
          font: `600 12px ${font.sans}`,
          color: color.ink,
        }}
      >
        Live head
        <span style={{ marginLeft: 8, font: `400 10px ${font.mono}`, color: color.muted2 }}>
          {head ? shortHash(head) : "empty"}
        </span>
      </button>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {loading ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            Loading history…
          </div>
        ) : error ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.danger }}>
            {error}
          </div>
        ) : snapshots.length === 0 ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            No commits yet.
          </div>
        ) : (
          snapshots.map((snap) => (
            <SnapshotRow
              key={snap.id}
              snapshot={snap}
              active={snapshot === snap.id}
              onSelect={() => onSelectSnapshot(snap.id)}
            />
          ))
        )}
      </div>

      {snapshot && snapshot !== head ? (
        <div
          style={{
            flexShrink: 0,
            maxHeight: "38%",
            overflowY: "auto",
            borderTop: `1px solid ${color.border}`,
            background: color.sunken,
            padding: "10px 14px",
          }}
        >
          <div style={{ font: `600 10.5px ${font.sans}`, color: color.muted2, marginBottom: 6 }}>
            DIFF VS HEAD
          </div>
          {diffLoading ? (
            <div style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>Diffing…</div>
          ) : !diffEntries || diffEntries.length === 0 ? (
            <div style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>No changes.</div>
          ) : (
            diffEntries.map((entry) => (
              <div
                key={`${entry.kind}:${entry.path}`}
                style={{
                  display: "flex",
                  gap: 8,
                  alignItems: "baseline",
                  font: `400 11px ${font.mono}`,
                  color: color.muted3,
                  padding: "2px 0",
                }}
              >
                <span
                  style={{
                    width: 58,
                    flexShrink: 0,
                    font: `600 9.5px ${font.sans}`,
                    color: DIFF_TONE[entry.kind],
                    textTransform: "uppercase",
                  }}
                >
                  {entry.kind}
                </span>
                <span
                  style={{
                    minWidth: 0,
                    wordBreak: "break-all",
                  }}
                  title={entry.path}
                >
                  {entry.path}
                </span>
              </div>
            ))
          )}
        </div>
      ) : (
        <div
          style={{
            flexShrink: 0,
            borderTop: `1px solid ${color.borderSoft}`,
            padding: "10px 14px",
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
          }}
        >
          Select a snapshot to browse it and diff it against head.
        </div>
      )}
    </div>
  );
}
