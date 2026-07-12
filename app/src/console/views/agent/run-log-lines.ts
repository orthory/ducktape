// Provider output arrives as one JSON object per line. The live Activity pane
// is deliberately a presentation boundary: the provider wire format stays
// useful for parsing, but never leaks into the operator-facing log.

export type ActivityLogStream = "stdout" | "stderr";

export type ActivityLogEntry =
  | { kind: "line"; stream: ActivityLogStream; text: string }
  | { kind: "gap"; text: string };

export type ActivityLogRowKind =
  | "message"
  | "command"
  | "output"
  | "status"
  | "exit"
  | "file"
  | "tool"
  | "text"
  | "gap"
  | "blank";

export interface ActivityLogRow {
  kind: ActivityLogRowKind;
  stream?: ActivityLogStream;
  text: string;
}

/** Hard presentation budgets. Provider output is untrusted and one JSONL
 * completion can contain megabytes of aggregated command output. These caps
 * keep parsing and the semantic DOM bounded while preserving an honest tail. */
export const MAX_ACTIVITY_ENTRIES = 120;
export const MAX_ACTIVITY_ROWS = 500;
export const MAX_FIELD_LINES = 120;
export const MAX_ROW_CHARS = 4_000;
export const MAX_RAW_ENTRY_CHARS = 256_000;
const MAX_FIELD_CHARS = 128_000;
const FIELD_HEAD_LINES = 20;
const FIELD_TAIL_LINES = MAX_FIELD_LINES - FIELD_HEAD_LINES - 1;
const OMISSION_ROW = /^… \d+ (?:characters|lines) omitted …$/;
const ENTRY_TAIL_GAP = /^live log tail: (\d+) older events omitted$/;

/** Append one stream event without losing the fact that older events fell out
 * of the client tail. The synthetic gap consumes one slot once truncation
 * begins, so memory remains bounded at MAX_ACTIVITY_ENTRIES. */
export const appendActivityEntry = (
  entries: readonly ActivityLogEntry[],
  entry: ActivityLogEntry,
): ActivityLogEntry[] => {
  const boundedEntry: ActivityLogEntry =
    entry.kind === "line" && entry.text.length > MAX_RAW_ENTRY_CHARS
      ? {
          kind: "gap",
          text: `provider event omitted: ${entry.text.length - MAX_RAW_ENTRY_CHARS} characters over limit`,
        }
      : entry;
  const first = entries[0];
  const match = first?.kind === "gap" ? ENTRY_TAIL_GAP.exec(first.text) : null;
  const alreadyDropped = match ? Number(match[1]) : 0;
  const content = [...(match ? entries.slice(1) : entries), boundedEntry];
  if (alreadyDropped === 0 && content.length <= MAX_ACTIVITY_ENTRIES) return content;
  const overflow = Math.max(0, content.length - (MAX_ACTIVITY_ENTRIES - 1));
  const dropped = alreadyDropped + overflow;
  return [
    { kind: "gap", text: `live log tail: ${dropped} older events omitted` },
    ...content.slice(-(MAX_ACTIVITY_ENTRIES - 1)),
  ];
};

type JsonRecord = Record<string, unknown>;

const isRecord = (value: unknown): value is JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const stringField = (record: JsonRecord, key: string): string | null =>
  typeof record[key] === "string" ? record[key] : null;

/** JSON.parse already decodes escaped newlines. This also handles plain-text
 *  fallback lines that contain the two literal characters `\\n`/`\\r`. */
const normalizeText = (value: string): string =>
  value
    .replace(/\\r\\n/g, "\n")
    .replace(/\\n/g, "\n")
    .replace(/\\r/g, "\r")
    .replace(/\r\n?/g, "\n")
    .replace(/\n{3,}/g, "\n\n");

const clipRowText = (value: string): string => {
  if (value.length <= MAX_ROW_CHARS) return value;
  const marker = ` … ${value.length - MAX_ROW_CHARS} characters omitted … `;
  const available = MAX_ROW_CHARS - marker.length;
  const head = Math.max(1, Math.floor(available / 3));
  const tail = Math.max(1, available - head);
  return `${value.slice(0, head)}${marker}${value.slice(-tail)}`;
};

const clipFieldText = (value: string): string => {
  if (value.length <= MAX_FIELD_CHARS) return value;
  const half = Math.floor(MAX_FIELD_CHARS / 2);
  const omitted = value.length - half * 2;
  return `${value.slice(0, half)}\n… ${omitted} characters omitted …\n${value.slice(-half)}`;
};

const pushRow = (
  rows: ActivityLogRow[],
  row: ActivityLogRow,
): void => {
  const text = clipRowText(row.text);
  const blank = text.trim() === "";
  const next = blank ? { ...row, kind: "blank" as const, text: "" } : { ...row, text };
  if (next.kind === "blank" && rows[rows.length - 1]?.kind === "blank") return;
  rows.push(next);
};

/** Add a possibly multiline provider field without leaving escaped newline
 *  noise in the UI. A trailing newline is a transport delimiter, not a row. */
const pushTextRows = (
  rows: ActivityLogRow[],
  kind: Exclude<ActivityLogRowKind, "gap" | "blank">,
  stream: ActivityLogStream,
  text: string,
): void => {
  let parts = normalizeText(clipFieldText(text)).split("\n");
  while (parts.length > 1 && parts[parts.length - 1] === "") parts.pop();
  if (parts.length === 1 && parts[0] === "") return;
  if (parts.length > MAX_FIELD_LINES) {
    const omitted = parts.length - FIELD_HEAD_LINES - FIELD_TAIL_LINES;
    parts = [
      ...parts.slice(0, FIELD_HEAD_LINES),
      `… ${omitted} lines omitted …`,
      ...parts.slice(-FIELD_TAIL_LINES),
    ];
  }
  for (const part of parts) {
    if (OMISSION_ROW.test(part)) pushRow(rows, { kind: "gap", text: part });
    else pushRow(rows, { kind, stream, text: part });
  }
};

const describeChanges = (changes: unknown): string[] => {
  if (!Array.isArray(changes)) return [];
  return changes.map((change) => {
    if (typeof change === "string") return change;
    if (!isRecord(change)) return "changed file";
    return (
      stringField(change, "path") ??
      stringField(change, "file") ??
      stringField(change, "filename") ??
      "changed file"
    );
  });
};

const itemLabel = (item: JsonRecord): { kind: ActivityLogRowKind; text: string } | null => {
  const type = stringField(item, "type");
  if (!type) return null;

  switch (type) {
    case "agent_message": {
      const text = stringField(item, "text");
      return text === null ? null : { kind: "message", text };
    }
    case "command_execution": {
      const command = stringField(item, "command");
      return command === null ? null : { kind: "command", text: command };
    }
    case "file_change": {
      const files = describeChanges(item.changes);
      return {
        kind: "file",
        text: files.length > 0 ? `files: ${files.join(", ")}` : "file changes",
      };
    }
    case "mcp_tool_call": {
      const server = stringField(item, "server");
      const tool = stringField(item, "tool") ?? stringField(item, "name");
      const name = server && tool ? `${server}/${tool}` : tool ?? server;
      return { kind: "tool", text: name ? `MCP tool: ${name}` : "MCP tool call" };
    }
    default:
      return { kind: "text", text: `item: ${type}` };
  }
};

const itemKey = (item: JsonRecord): string | null => {
  const type = stringField(item, "type");
  if (!type) return null;
  const id = stringField(item, "id");
  if (id) return `${type}:id:${id}`;
  const primary = itemLabel(item);
  if (primary) return `${type}:value:${primary.text}`;
  return type;
};

const takeStarted = (started: Map<string, number>, key: string | null): boolean => {
  if (!key) return false;
  const count = started.get(key) ?? 0;
  if (count <= 1) started.delete(key);
  else started.set(key, count - 1);
  return count > 0;
};

const rememberStarted = (started: Map<string, number>, key: string | null): void => {
  if (!key) return;
  started.set(key, (started.get(key) ?? 0) + 1);
};

const pushStatus = (
  rows: ActivityLogRow[],
  stream: ActivityLogStream,
  text: string,
): void => pushRow(rows, { kind: "status", stream, text });

const pushItemDetails = (
  rows: ActivityLogRow[],
  stream: ActivityLogStream,
  item: JsonRecord,
): void => {
  const type = stringField(item, "type");
  if (type === "command_execution") {
    const output = stringField(item, "aggregated_output") ?? stringField(item, "output");
    if (output !== null) pushTextRows(rows, "output", stream, output);
  } else if (type === "mcp_tool_call") {
    const result = stringField(item, "result") ?? stringField(item, "error");
    if (result !== null) pushTextRows(rows, "output", stream, result);
  }

  const status = stringField(item, "status");
  if (status !== null) pushStatus(rows, stream, `status: ${status}`);

  const exitCode = item.exit_code;
  if (typeof exitCode === "number" || typeof exitCode === "string") {
    pushRow(rows, { kind: "exit", stream, text: `exit: ${exitCode}` });
  }
};

const pushItem = (
  rows: ActivityLogRow[],
  stream: ActivityLogStream,
  item: JsonRecord,
  includePrimary: boolean,
  includeDetails: boolean,
): void => {
  if (includePrimary) {
    const primary = itemLabel(item);
    if (primary) {
      if (primary.kind === "message" || primary.kind === "output") {
        pushTextRows(rows, primary.kind, stream, primary.text);
      } else {
        pushRow(rows, { ...primary, stream });
      }
    }
  }
  if (includeDetails) pushItemDetails(rows, stream, item);
};

const parseJsonEvent = (
  line: string,
  stream: ActivityLogStream,
  rows: ActivityLogRow[],
  started: Map<string, number>,
): boolean => {
  if (line.length > MAX_RAW_ENTRY_CHARS) {
    pushRow(rows, {
      kind: "gap",
      text: `provider event omitted: ${line.length - MAX_RAW_ENTRY_CHARS} characters over limit`,
    });
    return true;
  }
  let value: unknown;
  try {
    value = JSON.parse(line.trim());
  } catch {
    return false;
  }

  if (typeof value === "string") {
    pushTextRows(rows, "text", stream, value);
    return true;
  }
  if (!isRecord(value)) {
    pushStatus(rows, stream, "event: JSON value");
    return true;
  }

  const type = stringField(value, "type");
  switch (type) {
    case "thread.started": {
      const threadId = stringField(value, "thread_id");
      pushStatus(rows, stream, threadId ? `thread started: ${threadId}` : "thread started");
      return true;
    }
    case "turn.started":
      pushStatus(rows, stream, "turn started");
      return true;
    case "turn.completed":
      pushStatus(rows, stream, "turn completed");
      return true;
    case "item.started": {
      const item = isRecord(value.item) ? value.item : null;
      if (!item) {
        pushStatus(rows, stream, "item started");
        return true;
      }
      rememberStarted(started, itemKey(item));
      pushItem(rows, stream, item, true, false);
      return true;
    }
    case "item.completed": {
      const item = isRecord(value.item) ? value.item : null;
      if (!item) {
        pushStatus(rows, stream, "item completed");
        return true;
      }
      const hadStarted = takeStarted(started, itemKey(item));
      pushItem(rows, stream, item, !hadStarted, true);
      return true;
    }
    default: {
      const message =
        stringField(value, "message") ?? stringField(value, "text") ?? stringField(value, "error");
      if (message !== null) pushTextRows(rows, "text", stream, message);
      else pushStatus(rows, stream, type ? `event: ${type}` : "event: JSON object");
      return true;
    }
  }
};

/** Convert provider JSONL and plain output into rows suitable for the live
 *  Activity pane. Started/completed command pairs share one command row; the
 *  completion contributes output, status, and exit details. */
export function parseActivityLog(entries: readonly ActivityLogEntry[]): ActivityLogRow[] {
  const rows: ActivityLogRow[] = [];
  const started = new Map<string, number>();
  const visibleEntries = entries.slice(-MAX_ACTIVITY_ENTRIES);

  for (const entry of visibleEntries) {
    if (entry.kind === "gap") {
      pushRow(rows, { kind: "gap", text: normalizeText(entry.text) });
      continue;
    }
    if (entry.text.length <= MAX_RAW_ENTRY_CHARS && entry.text.trim() === "") {
      pushRow(rows, { kind: "blank", stream: entry.stream, text: "" });
      continue;
    }
    if (!parseJsonEvent(entry.text, entry.stream, rows, started)) {
      pushTextRows(rows, "text", entry.stream, entry.text);
    }
  }

  const droppedEntries = entries.length - visibleEntries.length;
  const leadingEntryGap =
    rows[0]?.kind === "gap" ? ENTRY_TAIL_GAP.exec(rows[0].text) : null;
  const retainedRows = leadingEntryGap ? rows.slice(1) : rows;
  const recordedDroppedEntries = leadingEntryGap ? Number(leadingEntryGap[1]) : 0;
  const totalDroppedEntries = droppedEntries + recordedDroppedEntries;
  const droppedRows = Math.max(0, retainedRows.length - (MAX_ACTIVITY_ROWS - 1));
  if (droppedEntries === 0 && droppedRows === 0) return rows;

  const reasons = [
    totalDroppedEntries > 0 ? `${totalDroppedEntries} older events` : null,
    droppedRows > 0 ? `${droppedRows} rendered rows` : null,
  ].filter((reason): reason is string => reason !== null);
  return [
    { kind: "gap", text: `live log tail: ${reasons.join(" and ")} omitted` },
    ...retainedRows.slice(-(MAX_ACTIVITY_ROWS - 1)),
  ];
}
