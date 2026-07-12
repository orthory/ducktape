import { describe, expect, it } from "vitest";

import type { ActivityLogEntry, ActivityLogRow } from "./run-log-lines";
import {
  MAX_ACTIVITY_ENTRIES,
  MAX_ACTIVITY_ROWS,
  MAX_RAW_ENTRY_CHARS,
  MAX_ROW_CHARS,
  appendActivityEntry,
  parseActivityLog,
} from "./run-log-lines";

const json = (value: unknown, stream: "stdout" | "stderr" = "stdout"): ActivityLogEntry => ({
  kind: "line",
  stream,
  text: JSON.stringify(value),
});

const line = (text: string, stream: "stdout" | "stderr" = "stdout"): ActivityLogEntry => ({
  kind: "line",
  stream,
  text,
});

const summary = (rows: ActivityLogRow[]) => rows.map(({ kind, stream, text }) => ({ kind, stream, text }));

describe("parseActivityLog", () => {
  it("renders the live Codex JSONL event shapes as readable rows", () => {
    const rows = parseActivityLog([
      json({ type: "thread.started", thread_id: "thread-123" }),
      json({ type: "turn.started" }),
      json({
        type: "item.started",
        item: { type: "command_execution", command: "cargo test -p app" },
      }),
      json({
        type: "item.completed",
        item: {
          type: "command_execution",
          command: "cargo test -p app",
          aggregated_output: "running tests\n\n\nfinished\n",
          exit_code: 0,
          status: "completed",
        },
      }),
      json({
        type: "item.started",
        item: { type: "agent_message", text: "all tests passed" },
      }),
      json({
        type: "item.completed",
        item: { type: "agent_message", text: "all tests passed" },
      }),
      json({
        type: "item.started",
        item: {
          type: "file_change",
          changes: [{ path: "app/src/console/views/agent/run-log-lines.ts" }],
          status: "in_progress",
        },
      }),
      json({
        type: "item.completed",
        item: {
          type: "file_change",
          changes: [{ path: "app/src/console/views/agent/run-log-lines.ts" }],
          status: "completed",
        },
      }),
      json({
        type: "item.started",
        item: { type: "mcp_tool_call", server: "lattice", tool: "search" },
      }),
      json({
        type: "item.completed",
        item: {
          type: "mcp_tool_call",
          server: "lattice",
          tool: "search",
          status: "completed",
        },
      }),
      json({ type: "turn.completed" }),
    ]);

    expect(summary(rows)).toEqual([
      { kind: "status", stream: "stdout", text: "thread started: thread-123" },
      { kind: "status", stream: "stdout", text: "turn started" },
      { kind: "command", stream: "stdout", text: "cargo test -p app" },
      { kind: "output", stream: "stdout", text: "running tests" },
      { kind: "blank", stream: "stdout", text: "" },
      { kind: "output", stream: "stdout", text: "finished" },
      { kind: "status", stream: "stdout", text: "status: completed" },
      { kind: "exit", stream: "stdout", text: "exit: 0" },
      { kind: "message", stream: "stdout", text: "all tests passed" },
      {
        kind: "file",
        stream: "stdout",
        text: "files: app/src/console/views/agent/run-log-lines.ts",
      },
      { kind: "status", stream: "stdout", text: "status: completed" },
      { kind: "tool", stream: "stdout", text: "MCP tool: lattice/search" },
      { kind: "status", stream: "stdout", text: "status: completed" },
      { kind: "status", stream: "stdout", text: "turn completed" },
    ]);
    expect(rows.filter((row) => row.kind === "command")).toHaveLength(1);
    expect(rows.map((row) => row.text).join("\n")).not.toContain("item.completed");
  });

  it("preserves stderr and gaps, while falling back to plain text", () => {
    const rows = parseActivityLog([
      line("warning from provider", "stderr"),
      { kind: "gap", text: "output gap: dropped older lines before cursor 9" },
      line("plain\\ntext"),
      line(""),
      line(""),
      line("next line"),
    ]);

    expect(summary(rows)).toEqual([
      { kind: "text", stream: "stderr", text: "warning from provider" },
      {
        kind: "gap",
        stream: undefined,
        text: "output gap: dropped older lines before cursor 9",
      },
      { kind: "text", stream: "stdout", text: "plain" },
      { kind: "text", stream: "stdout", text: "text" },
      { kind: "blank", stream: "stdout", text: "" },
      { kind: "text", stream: "stdout", text: "next line" },
    ]);
    expect(rows.map((row) => row.text).join("\n")).not.toContain("\\n");
  });

  it("does not expose wrappers for an unknown JSON event", () => {
    const [row] = parseActivityLog([json({ type: "future.event", payload: { secret: true } })]);

    expect(row).toEqual({ kind: "status", stream: "stdout", text: "event: future.event" });
    expect(row.text).not.toContain("{");
  });

  it("bounds huge command output while preserving the command result tail", () => {
    const output = Array.from({ length: 10_000 }, (_, index) => `output line ${index}`).join("\n");
    const rows = parseActivityLog([
      json({
        type: "item.started",
        item: { id: "cmd-1", type: "command_execution", command: "cargo test --workspace" },
      }),
      json({
        type: "item.completed",
        item: {
          id: "cmd-1",
          type: "command_execution",
          command: "cargo test --workspace",
          aggregated_output: output,
          exit_code: 101,
          status: "failed",
        },
      }),
    ]);

    expect(rows.length).toBeLessThanOrEqual(MAX_ACTIVITY_ROWS);
    expect(rows.some((row) => row.kind === "command" && row.text === "cargo test --workspace")).toBe(true);
    expect(rows.some((row) => row.text.includes("lines omitted"))).toBe(true);
    expect(rows.some((row) => row.text === "output line 9999")).toBe(true);
    expect(rows.slice(-2)).toEqual([
      { kind: "status", stream: "stdout", text: "status: failed" },
      { kind: "exit", stream: "stdout", text: "exit: 101" },
    ]);
  });

  it("clips a newline-less megabyte without exposing an unbounded DOM row", () => {
    const rows = parseActivityLog([
      json({
        type: "item.completed",
        item: {
          type: "command_execution",
          command: "generate output",
          aggregated_output: "x".repeat(200_000),
          exit_code: 0,
          status: "completed",
        },
      }),
    ]);

    expect(Math.max(...rows.map((row) => row.text.length))).toBeLessThanOrEqual(MAX_ROW_CHARS);
    expect(rows.some((row) => row.text.includes("characters omitted"))).toBe(true);
    expect(rows[rows.length - 1]).toEqual({ kind: "exit", stream: "stdout", text: "exit: 0" });
  });

  it("rejects an oversized raw provider event before JSON parsing", () => {
    const rows = parseActivityLog([line("x".repeat(MAX_RAW_ENTRY_CHARS + 10_000))]);

    expect(rows).toEqual([
      {
        kind: "gap",
        text: "provider event omitted: 10000 characters over limit",
      },
    ]);
  });

  it("does not retain an oversized provider payload in live component state", () => {
    const oversized = line("x".repeat(MAX_RAW_ENTRY_CHARS + 10_000));
    const entries = appendActivityEntry([], oversized);

    expect(entries).toEqual([
      {
        kind: "gap",
        text: "provider event omitted: 10000 characters over limit",
      },
    ]);
    expect(entries[0].text.length).toBeLessThan(100);
  });

  it("keeps only a bounded, explicit tail of a long event history", () => {
    const entries = Array.from({ length: 1_000 }, (_, index) =>
      line(`plain event ${index}`),
    );
    const rows = parseActivityLog(entries);

    expect(rows.length).toBeLessThanOrEqual(MAX_ACTIVITY_ROWS);
    expect(rows[0]).toEqual({
      kind: "gap",
      text: `live log tail: ${1_000 - MAX_ACTIVITY_ENTRIES} older events omitted`,
    });
    expect(rows.some((row) => row.text === "plain event 0")).toBe(false);
    expect(rows[rows.length - 1]).toEqual({
      kind: "text",
      stream: "stdout",
      text: "plain event 999",
    });
  });

  it("keeps an exact rolling omission count while appending stream events", () => {
    let entries: ActivityLogEntry[] = [];
    for (let index = 0; index < 1_000; index += 1) {
      entries = appendActivityEntry(entries, line(`event ${index}`));
    }

    expect(entries).toHaveLength(MAX_ACTIVITY_ENTRIES);
    expect(entries[0]).toEqual({
      kind: "gap",
      text: `live log tail: ${1_000 - (MAX_ACTIVITY_ENTRIES - 1)} older events omitted`,
    });
    expect(entries[1]).toEqual(line(`event ${1_000 - (MAX_ACTIVITY_ENTRIES - 1)}`));
    expect(entries[entries.length - 1]).toEqual(line("event 999"));
  });

  it("preserves event and rendered-row omission provenance together", () => {
    const output = Array.from({ length: 120 }, (_, index) => `line ${index}`).join("\n");
    let entries: ActivityLogEntry[] = [];
    for (let index = 0; index < 1_000; index += 1) {
      entries = appendActivityEntry(entries, json({
        type: "item.completed",
        item: {
          id: `cmd-${index}`,
          type: "command_execution",
          command: `command ${index}`,
          aggregated_output: output,
          exit_code: 0,
          status: "completed",
        },
      }));
    }

    const rows = parseActivityLog(entries);
    expect(rows).toHaveLength(MAX_ACTIVITY_ROWS);
    expect(rows[0].kind).toBe("gap");
    expect(rows[0].text).toContain("881 older events");
    expect(rows[0].text).toContain("rendered rows");
    expect(rows[rows.length - 1]).toEqual({ kind: "exit", stream: "stdout", text: "exit: 0" });
  });
});
