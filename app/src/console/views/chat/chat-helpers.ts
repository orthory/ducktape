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
  if (author === "System") return "system";
  if ("User" in author) return `user:${author.User.join(",")}`;
  if ("Agent" in author) return `agent:${author.Agent.module}/${author.Agent.agent_id}`;
  return `module:${author.Module}`;
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
    const dayDivider = prev === null || dayChanged ? dayLabelOf(message.head.created_at) : null;
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
