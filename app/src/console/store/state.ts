// Console state — a client-side projection of the node's committed state
// (channels/messages/tasks/status re-queried per block) plus local ui state
// (screen, accent, author identity, thread panel).

import type { ChatChannel, ChatMessage, ChatThread } from "../../domain/chat-client";
import type { Task, TaskStatus } from "../../domain/tasks-client";
import type { NodeStatus } from "../../domain/transport";

// ── State shape ─────────────────────────────────────────

export interface ConsoleState {
  screen: string;
  accent: string;
  author: string;
  /** The node answered the last status query. */
  connected: boolean;
  /** The daemon url this build resolved to (null until bootstrap finishes). */
  nodeUrl: string | null;
  /** True when this app owns the daemon lifecycle (desktop build). */
  managed: boolean;
  status: NodeStatus | null;
  channels: ChatChannel[];
  activeChannel: string | null;
  /** Messages of the active channel only. */
  messages: ChatMessage[];
  activeThread: ChatThread | null;
  tasks: Task[];
  error: string | null;
}

export const DEFAULT_ACCENT = "#a05a3c";

export const createInitialState = (): ConsoleState => ({
  screen: "chat",
  accent: DEFAULT_ACCENT,
  author: "operator",
  connected: false,
  nodeUrl: null,
  managed: false,
  status: null,
  channels: [],
  activeChannel: null,
  messages: [],
  activeThread: null,
  tasks: [],
  error: null,
});

// ── Pure helpers ────────────────────────────────────────

/** A channel id from a display name: lowercase, dash-separated, wire-safe. */
export const channelIdOf = (name: string): string =>
  name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");

/** The task lifecycle is a one-way lane; Done stays Done. */
export const nextTaskStatus = (status: TaskStatus): TaskStatus => {
  switch (status) {
    case "Open":
      return "InProgress";
    case "InProgress":
      return "Done";
    case "Done":
      return "Done";
  }
};
