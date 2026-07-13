// Pure helpers over MessageView arrays: same-author grouping (Slack-style
// compaction), day dividers, and author-key comparisons used to tell "mine"
// messages and reaction ownership apart. No store/transport dependencies —
// everything here is a function of data already in `ConsoleState`.

import type { AuthorRef, Channel, MessageView } from "../../../domain/chat-client";
import { wallClockMillisOf } from "../../../domain/wire";

/** Consecutive messages from the same author within this window (and the same
 *  calendar day) compact into one visual group. Mirrors Slack's ~5 minute
 *  window. */
export const GROUP_WINDOW_MS = 5 * 60_000;

/** A stable string key for an AuthorRef, comparable with `===`. */
export const authorKey = (author: AuthorRef): string => {
  if (author === "system") return "system";
  if ("user" in author) return `user:${author.user.join(",")}`;
  if ("agent" in author) return `agent:${author.agent.module}/${author.agent.agent_id}`;
  return `module:${author.module}`;
};

export const isAgentAuthor = (author: AuthorRef): boolean =>
  typeof author === "object" && "agent" in author;

/** The local author's key, in the same shape `authorKey` produces for a
 *  `User` author. Callers pass `selfAuthorBytes(status, author)` — the node
 *  pubkey on a networked node (the submit lane SIGNS frames; committed
 *  authorship is the node's key and the origin string is ignored), the
 *  origin bytes on the embedded daemon (which stores them verbatim). */
export const selfAuthorKeyOf = (selfBytes: number[]): string =>
  `user:${selfBytes.join(",")}`;

/** May this viewer rename / archive / unarchive the channel? Mirrors the
 *  module's `check_channel_admin`: an owned channel admits only its owner among
 *  User origins, an owner-less one (module/system-minted, or a legacy record)
 *  admits any user. `selfKey` is `selfAuthorKeyOf(selfAuthorBytes(...))`. */
export const canAdministerChannel = (channel: Channel, selfKey: string): boolean =>
  !channel.owner || selfAuthorKeyOf(channel.owner) === selfKey;

export const hasReacted = (message: MessageView, emoji: string, selfKey: string): boolean =>
  message.reactions.some(
    (reaction) => reaction.emoji === emoji && reaction.reactors.some((a) => authorKey(a) === selfKey),
  );

// Wire timestamps (`MessageHead.created_at` / `edited_at`) are the node's
// `consensus_time` verbatim — wall-clock unix MILLIS on the embedded daemon
// and simnode, a block-height counter on the networked validator (see
// domain/wire.ts). Everything here renders through `wallClockMillisOf`: a
// counter never gets a date/time label, and stamps from DIFFERENT timebases
// are never compared — that comparison is exactly what flashed a bogus day
// divider over a just-sent (locally stamped) preconf echo.

const dayKeyOf = (ms: number): string => new Date(ms).toDateString();

/** "Today" / "Yesterday" / a short locale date — the day-divider label.
 *  `ms` is a wall-clock stamp already normalized to JS millis. */
export const dayLabelOf = (ms: number): string => {
  const now = Date.now();
  const oneDayMs = 24 * 60 * 60 * 1000;
  if (dayKeyOf(ms) === dayKeyOf(now)) return "Today";
  if (dayKeyOf(ms) === dayKeyOf(now - oneDayMs)) return "Yesterday";
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
};

/** The grouping gap between two stamps, in millis. Two wall-clock stamps
 *  compare as real time; two counters keep the legacy `×1000` reading (a
 *  validator ticks ~1 block/s, so the window still spans ~5 min of chain
 *  time); mixed timebases are incomparable — infinite gap, new group. */
const gapMsOf = (a: number, b: number): number => {
  const aMs = wallClockMillisOf(a);
  const bMs = wallClockMillisOf(b);
  if (aMs !== null && bMs !== null) return bMs - aMs;
  if (aMs === null && bMs === null) return (b - a) * 1000;
  return Number.POSITIVE_INFINITY;
};

export interface StreamRow {
  message: MessageView;
  /** False → compact under the previous row (same author header/avatar, no
   *  repeat). True → render the full avatar+name+time header. */
  groupStart: boolean;
  /** Divider label to render immediately above this row, or null. */
  dayDivider: string | null;
}

/** Walks an ordered (by seq) list of ROOT messages into render rows: day
 *  dividers on calendar-day boundaries, and a `groupStart` flag for the
 *  same-author compaction. Never groups across a day boundary. */
export const buildStreamRows = (roots: MessageView[]): StreamRow[] => {
  const rows: StreamRow[] = [];
  let prev: MessageView | null = null;
  for (const message of roots) {
    const ms = wallClockMillisOf(message.head.created_at);
    const prevMs = prev === null ? null : wallClockMillisOf(prev.head.created_at);
    // A day boundary exists only between two REAL wall-clock stamps. Mixed
    // timebases never divide: a locally stamped preconf echo above
    // height-stamped history is the same day until committed truth says
    // otherwise. (A wall-clock FIRST row still gets its top-of-stream label.)
    const dayChanged = prevMs !== null && ms !== null && dayKeyOf(prevMs) !== dayKeyOf(ms);
    const dayDivider = (prev === null || dayChanged) && ms !== null ? dayLabelOf(ms) : null;
    const groupStart =
      prev === null ||
      dayChanged ||
      authorKey(prev.head.author) !== authorKey(message.head.author) ||
      gapMsOf(prev.head.created_at, message.head.created_at) >= GROUP_WINDOW_MS;
    rows.push({ message, groupStart, dayDivider });
    prev = message;
  }
  return rows;
};
