// Small pure formatters shared by the duckfs browser and its panels. Kept in
// their own module so FilesView / FilePreview / HistoryPanel don't import each
// other just to reuse a helper (which would be a cycle).

import { wallClockMillisOf } from "../../../domain/wire";

/** A byte count as a short human string (B/KB/MB/GB). */
export const humanBytes = (n: number): string => {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rendered = unit === 0 ? String(Math.round(value)) : value.toFixed(value < 10 ? 1 : 0);
  return `${rendered} ${units[unit]}`;
};

/** The message out of an unknown thrown value — the node's `files: …` rejection
 *  reads straight through (NodeError carries it as the message). */
export const errMsg = (err: unknown): string =>
  err instanceof Error ? err.message : String(err);

/** A snapshot id / object hash, shortened for a chip (first 10 hex). */
export const shortHash = (hash: string): string =>
  hash.length > 12 ? `${hash.slice(0, 10)}…` : hash || "—";

/** A consensus_time stamp → a compact local timestamp, or "—" when the stamp
 *  isn't wall-clock (a validator height counter — see domain/wire.ts). */
export const formatTime = (stamp: number): string => {
  const ms = wallClockMillisOf(stamp);
  if (ms === null) return "—";
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
};
