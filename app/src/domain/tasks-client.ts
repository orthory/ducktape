// Typed client for the node's `tasks` module — the TS mirror of
// `crates/apps/tasks-interface`. Same contract as chat-client: camelCase
// params in, verbatim serde wire out, pure functions over an injected
// NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (TaskReply payloads, verbatim) ───────────

export type TaskStatus = "Open" | "InProgress" | "Done";

export interface Task {
  id: string;
  title: string;
  status: TaskStatus;
  created_at: number;
  updated_at: number;
}

const TARGET = "tasks";

// ── Msgs (writes) ───────────────────────────────────────

export const createTask = (
  transport: NodeTransport,
  params: { taskId: string; title: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    CreateTask: { task_id: params.taskId, title: params.title },
  });

export const updateStatus = (
  transport: NodeTransport,
  params: { taskId: string; status: TaskStatus },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    UpdateStatus: { task_id: params.taskId, status: params.status },
  });

// ── Queries (reads) ─────────────────────────────────────

export const listTasks = (transport: NodeTransport): Promise<Task[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "List"))
    .then((reply) => replyVariant<Task[]>(reply, "Tasks"));
