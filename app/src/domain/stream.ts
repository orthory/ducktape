import type {
  ServerFrame,
  TailItem,
  RunStream,
  StreamErrorCode,
  StreamOpRow,
} from "./stream.gen";

export type { ServerFrame, TailItem, RunStream, StreamErrorCode, StreamOpRow };

export type SubscribedFrame = Extract<ServerFrame, { type: "subscribed" }>;
export type EventFrame = Extract<ServerFrame, { type: "event" }>;
export type TermCommandLogFrame = Extract<ServerFrame, { type: "termCommandLog" }>;
export type TailFrame = Extract<ServerFrame, { type: "tail" }>;
export type LaggedFrame = Extract<ServerFrame, { type: "lagged" }>;
export type HeartbeatFrame = Extract<ServerFrame, { type: "heartbeat" }>;
export type ErrorFrame = Extract<ServerFrame, { type: "error" }>;

export type LogTailItem = Extract<TailItem, { line: string }>;
export type FileChangeTailItem = Extract<TailItem, { paths: string[] }>;
export type RunOutputTailItem = Extract<TailItem, { stream: RunStream; line: string }>;
export type MetricsTailItem = Extract<TailItem, { text: string }>;

export const LOGS_TOPIC = "logs";
export const FILES_WATCH_TOPIC = "files:watch";
export const METRICS_TOPIC = "metrics";

export const moduleTopic = (id: string): string => `module:${id}`;
export const runOutputTopic = (dispatchId: string): string =>
  `run-output:${dispatchId}`;

// ── Interactive terminal sessions (docs/…/interactive-terminal-sessions) ──
// A term session's output rides the existing ws stream on `term:<sessionId>`
// as an event frame carrying a base64 chunk in `item` (a ring replays it on
// subscribe, exactly like run-output). Input rides the SAME socket as two new
// ClientMsg ops. These two op shapes are FINAL and are being added to noded's
// `ClientMsg` enum in parallel; once ts-rs regenerates stream.gen.ts the
// generated `ClientMsg` will cover them and `TermClientMsg` can fold in.
export const TERM_TOPIC_PREFIX = "term:";
export const termTopic = (sessionId: string): string =>
  `${TERM_TOPIC_PREFIX}${sessionId}`;

// A SHARED session's ordered command log rides a second topic, `term-cmd:<id>`,
// carrying `TermCommandLog` frames (the total-order seq + attributed text). The
// node replays the whole command ring on (re)subscribe, exactly like `term:`.
export const TERM_CMD_TOPIC_PREFIX = "term-cmd:";
export const commandTopic = (sessionId: string): string =>
  `${TERM_CMD_TOPIC_PREFIX}${sessionId}`;

/** The app→node terminal ops, hand-authored to stay independent of
 *  stream.gen.ts regeneration. `data` is base64 of the keystroke bytes;
 *  a `termCommand` carries plain command `text` + the author `origin` (the
 *  shared-session lane — the node stores the text verbatim). */
export type TermClientMsg =
  | { op: "termInput"; session: string; data: string }
  | { op: "termResize"; session: string; cols: number; rows: number }
  | { op: "termCommand"; session: string; text: string; origin: string };

/** A terminal output chunk: an event frame on a `term:` topic whose `item` is
 *  base64 of raw terminal bytes and whose `cursor` resumes the byte ring
 *  without replay. Distinct from the op-carrying `EventFrame` (no `op`), so
 *  the transport routes it separately. */
export interface TermChunkFrame {
  type: "event";
  topic: string;
  cursor: string;
  item: string;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

const hasString = (value: Record<string, unknown>, key: string): boolean =>
  typeof value[key] === "string";

const hasNumber = (value: Record<string, unknown>, key: string): boolean =>
  typeof value[key] === "number" && Number.isFinite(value[key]);

export const isSubscribedFrame = (value: unknown): value is SubscribedFrame =>
  isRecord(value) && value.type === "subscribed" && isRecord(value.topics);

export const isEventFrame = (value: unknown): value is EventFrame =>
  isRecord(value) &&
  value.type === "event" &&
  hasString(value, "topic") &&
  hasString(value, "cursor") &&
  isRecord(value.op) &&
  hasNumber(value.op, "height") &&
  hasNumber(value.op, "seq") &&
  hasNumber(value.op, "time");

export const isTailFrame = (value: unknown): value is TailFrame =>
  isRecord(value) &&
  value.type === "tail" &&
  hasString(value, "topic") &&
  hasString(value, "cursor") &&
  isRecord(value.item);

export const isLaggedFrame = (value: unknown): value is LaggedFrame =>
  isRecord(value) &&
  value.type === "lagged" &&
  hasString(value, "topic") &&
  hasString(value, "cursor");

export const isHeartbeatFrame = (value: unknown): value is HeartbeatFrame =>
  isRecord(value) &&
  value.type === "heartbeat" &&
  hasNumber(value, "height") &&
  hasString(value, "appHash") &&
  hasNumber(value, "timeMs") &&
  hasNumber(value, "intervalMs");

export const isErrorFrame = (value: unknown): value is ErrorFrame =>
  isRecord(value) &&
  value.type === "error" &&
  hasString(value, "topic") &&
  hasString(value, "code") &&
  hasString(value, "detail");

export const isTermChunkFrame = (value: unknown): value is TermChunkFrame =>
  isRecord(value) &&
  value.type === "event" &&
  hasString(value, "topic") &&
  (value.topic as string).startsWith(TERM_TOPIC_PREFIX) &&
  hasString(value, "cursor") &&
  hasString(value, "item");

export const isTermCommandLogFrame = (value: unknown): value is TermCommandLogFrame =>
  isRecord(value) &&
  value.type === "termCommandLog" &&
  hasString(value, "topic") &&
  hasNumber(value, "seq") &&
  hasString(value, "origin") &&
  hasString(value, "text");

export const isServerFrame = (value: unknown): value is ServerFrame =>
  isSubscribedFrame(value) ||
  isEventFrame(value) ||
  isTermCommandLogFrame(value) ||
  isTailFrame(value) ||
  isLaggedFrame(value) ||
  isHeartbeatFrame(value) ||
  isErrorFrame(value);

export const isLogTailItem = (value: TailItem): value is LogTailItem =>
  "line" in value && typeof value.line === "string";

export const isFileChangeTailItem = (value: TailItem): value is FileChangeTailItem =>
  "paths" in value &&
  Array.isArray(value.paths) &&
  value.paths.every((path: unknown) => typeof path === "string");

export const isRunOutputTailItem = (value: TailItem): value is RunOutputTailItem =>
  "stream" in value &&
  (value.stream === "stdout" || value.stream === "stderr") &&
  typeof value.line === "string";

export const isMetricsTailItem = (value: TailItem): value is MetricsTailItem =>
  "text" in value &&
  typeof value.text === "string" &&
  "timeMs" in value &&
  typeof value.timeMs === "number";
