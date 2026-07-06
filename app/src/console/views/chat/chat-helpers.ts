// Pure helpers over MessageView arrays: same-author grouping (Slack-style
// compaction), day dividers, and author-key comparisons used to tell "mine"
// messages and reaction ownership apart. No store/transport dependencies —
// everything here is a function of data already in `ConsoleState`.

import type { AuthorRef, MessageView } from "../../../domain/chat-client";

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
  typeof author === "object" && "Agent" in author;

/** The local author's key, in the same shape `authorKey` produces for a
 *  `User` author — the origin string crosses the wire as `Origin::External`
 *  bytes, which the chat module stores verbatim as `AuthorRef::User`, so this
 *  is just `authorKey` applied to our own submitted identity. */
export const selfAuthorKeyOf = (origin: string): string =>
  `user:${Array.from(new TextEncoder().encode(origin)).join(",")}`;

export const hasReacted = (message: MessageView, emoji: string, selfKey: string): boolean =>
  message.reactions.some(
    (reaction) => reaction.emoji === emoji && reaction.reactors.some((a) => authorKey(a) === selfKey),
  );

// Wire timestamps (`MessageHead.created_at` / `edited_at`) are UNIX SECONDS —
// they come straight from the node's consensus_time. Every Date built from one
// MUST multiply by 1000 to get JS milliseconds, or every message renders as
// "Jan 1, 1970".
const toMillis = (unixSeconds: number): number => unixSeconds * 1000;

/** True when a timestamp is a plausible real wall-clock time (after 2001-01-01),
 *  vs a genesis-relative counter. The node's `consensus_time` is currently a
 *  chain-relative value (small: seconds since genesis), so rendering it as an
 *  absolute clock/date yields nonsense like "Jan 1, 1970" / "20637d ago".
 *  Display code guards on this and simply omits the time when it isn't real —
 *  honest, and self-adjusting: the moment the node stamps real wall-clock time
 *  (values > 2001), timestamps light up on their own. Relative ORDERING and
 *  grouping stay correct either way (they use time DIFFERENCES, not the label). */
export const isWallClock = (unixSeconds: number): boolean => unixSeconds > 978_307_200;

const dayKeyOf = (unixSeconds: number): string => new Date(toMillis(unixSeconds)).toDateString();

/** "Today" / "Yesterday" / a short locale date — the day-divider label.
 *  `unixSeconds` is a `created_at`-shaped UNIX-seconds timestamp. */
export const dayLabelOf = (unixSeconds: number): string => {
  const nowSeconds = Date.now() / 1000;
  const oneDaySeconds = 24 * 60 * 60;
  if (dayKeyOf(unixSeconds) === dayKeyOf(nowSeconds)) return "Today";
  if (dayKeyOf(unixSeconds) === dayKeyOf(nowSeconds - oneDaySeconds)) return "Yesterday";
  return new Date(toMillis(unixSeconds)).toLocaleDateString([], { month: "short", day: "numeric" });
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
    const dayChanged = prev !== null && dayKeyOf(prev.head.created_at) !== dayKeyOf(message.head.created_at);
    // Only render a day divider when the timestamp is real wall-clock — a
    // genesis-relative counter would label every divider "Jan 1, 1970".
    const dayDivider =
      (prev === null || dayChanged) && isWallClock(message.head.created_at)
        ? dayLabelOf(message.head.created_at)
        : null;
    const groupStart =
      prev === null ||
      dayChanged ||
      authorKey(prev.head.author) !== authorKey(message.head.author) ||
      toMillis(message.head.created_at) - toMillis(prev.head.created_at) >= GROUP_WINDOW_MS;
    rows.push({ message, groupStart, dayDivider });
    prev = message;
  }
  return rows;
};
