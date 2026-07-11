import { describe, expect, it } from "vitest";

import type { ActivityLogEntry, ActivityLogRow } from "./run-log-lines";
import { parseActivityLog } from "./run-log-lines";

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
});
