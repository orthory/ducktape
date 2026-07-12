// Shared wire knowledge for the module clients: reply decoding, and the one
// timestamp discipline every rendering surface must go through.
//
// Every `*Reply` enum serializes as a single-variant object
// (`{"messages": [...]}`) or, for unit variants, a bare string. replyVariant
// unwraps the expected variant or throws — a mismatch means the module and
// this client disagree about the interface, which must surface loudly.

export const replyVariant = <T>(reply: unknown, variant: string): T => {
  if (
    typeof reply === "object" &&
    reply !== null &&
    variant in (reply as Record<string, unknown>)
  ) {
    return (reply as Record<string, T>)[variant];
  }
  throw new Error(`unexpected module reply: wanted ${variant}`);
};

// ── Consensus-time timebases ────────────────────────────
// Module timestamps (`created_at`, `edited_at`, `joined_at`, search-hit
// `time`, …) are the node's `consensus_time` verbatim, and the system has
// THREE timebases in live use:
//   - the embedded daemon (noded) and simnode stamp wall-clock unix MILLIS;
//   - the networked validator stamps the block HEIGHT (a bare counter);
//   - older rows may carry legacy unix SECONDS.
// `wallClockMillisOf` is the single conversion display code renders through:
// it tells the lanes apart by magnitude and returns JS millis, or null for a
// counter — a counter must never render as clock time (better no label than
// a fake "09:21 AM" or a "Jan 1, 1970" divider), and the moment a node
// stamps real wall-clock time, labels light up on their own.

/** 2001-01-01 as unix seconds / unix millis. No node ran before 2001, so a
 *  stamp under the seconds floor is a counter; one between the floors is
 *  seconds (as millis it would predate 2001); one above the millis floor is
 *  millis (as seconds it would be past year 30000). */
const WALL_CLOCK_SECONDS_FLOOR = 978_307_200;
const WALL_CLOCK_MILLIS_FLOOR = 978_307_200_000;

/** A wire stamp as JS millis when it is real wall-clock time, else null. */
export const wallClockMillisOf = (stamp: number): number | null => {
  if (!Number.isFinite(stamp)) return null;
  if (stamp > WALL_CLOCK_MILLIS_FLOOR) return stamp; // millis lane
  if (stamp > WALL_CLOCK_SECONDS_FLOOR) return stamp * 1000; // legacy seconds lane
  return null; // counter lane (validator height / genesis-relative)
};

/** True when a wire stamp is renderable as wall-clock time at all. */
export const isWallClock = (stamp: number): boolean => wallClockMillisOf(stamp) !== null;
