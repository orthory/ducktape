// The tasks client mirrors tasks-interface: TaskMsg encoding + TaskReply
// decoding, including the unit-variant "List" query.

import { describe, expect, it, vi } from "vitest";

import { createTask, listTasks, updateStatus } from "./tasks-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn(),
  getBlob: vi.fn(),
  status: vi.fn(),
  telemetry: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
  onTelemetry: vi.fn(),
});

describe("task msgs", () => {
  it("encodes CreateTask", async () => {
    const transport = stubTransport();
    await createTask(transport, { taskId: "t1", title: "port the app" });
    expect(transport.submit).toHaveBeenCalledWith("tasks", {
      CreateTask: { task_id: "t1", title: "port the app" },
    });
  });

  it("encodes UpdateStatus with the enum status verbatim", async () => {
    const transport = stubTransport();
    await updateStatus(transport, { taskId: "t1", status: "InProgress" });
    expect(transport.submit).toHaveBeenCalledWith("tasks", {
      UpdateStatus: { task_id: "t1", status: "InProgress" },
    });
  });
});

describe("task queries", () => {
  it("sends the unit variant List and decodes Tasks", async () => {
    const wire = [
      { id: "t1", title: "port the app", status: "Open", created_at: 1, updated_at: 1 },
    ];
    const transport = stubTransport({ Tasks: wire });
    await expect(listTasks(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("tasks", "List");
  });
});
