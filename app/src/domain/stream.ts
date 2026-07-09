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
export type TailFrame = Extract<ServerFrame, { type: "tail" }>;
export type LaggedFrame = Extract<ServerFrame, { type: "lagged" }>;
export type HeartbeatFrame = Extract<ServerFrame, { type: "heartbeat" }>;
export type ErrorFrame = Extract<ServerFrame, { type: "error" }>;

export type LogTailItem = Extract<TailItem, { line: string }>;
export type FileChangeTailItem = Extract<TailItem, { paths: string[] }>;
export type RunOutputTailItem = Extract<TailItem, { stream: RunStream; line: string }>;

export const LOGS_TOPIC = "logs";
export const FILES_WATCH_TOPIC = "files:watch";

export const moduleTopic = (id: string): string => `module:${id}`;
export const runOutputTopic = (dispatchId: string): string =>
  `run-output:${dispatchId}`;

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

export const isServerFrame = (value: unknown): value is ServerFrame =>
  isSubscribedFrame(value) ||
  isEventFrame(value) ||
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
