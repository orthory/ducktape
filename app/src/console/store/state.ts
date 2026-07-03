// Console state — a client-side projection of the node's committed state
// (channels/messages/tasks/status re-queried per block) plus local ui state
// (screen, accent, author identity, thread panel).

import type { AgentRecord, RunView, WatchView } from "../../domain/agent-client";
import type { Channel, ChatThread, MessageView } from "../../domain/chat-client";
import type { Block } from "../../domain/document-client";
import type { Task, TaskStatus } from "../../domain/tasks-client";
import type { NodeStatus } from "../../domain/transport";
import type { PhaseReport, Workspace } from "../../domain/workspace-client";

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
  channels: Channel[];
  activeChannel: string | null;
  /** Messages of the active channel only (all sequences; views filter). */
  messages: MessageView[];
  activeThread: ChatThread | null;
  tasks: Task[];
  /** forge HEAD commit oid, or null on an unborn repo (no commits yet). */
  forgeHead: string | null;

  // ── Documents (block store; see document-client) ──
  /** Known doc-ids — a client-side registry, since the document module has no
   *  "list docs" query (its store is keyed by sha256(doc_id) and can't
   *  enumerate). Persisted per node url by the provider. */
  docIds: string[];
  /** The doc whose blocks are loaded, or null when none is open. */
  activeDoc: string | null;
  /** Ordered blocks of the active doc (re-queried per block / on open). */
  activeDocBlocks: Block[];

  // ── Agents (collaboration loop; see agent-client) ──
  /** Every registered agent, re-queried per block like tasks. */
  agents: AgentRecord[];
  /** Every channel watch and its turn policy. */
  watches: WatchView[];
  /** Recent runs across all channels, newest-first for the timeline. */
  runs: RunView[];

  error: string | null;

  // ── Workspaces / onboarding (desktop only; inert on web) ──
  /** Every registered workspace, for the switcher. Empty on web. */
  workspaces: Workspace[];
  /** The active workspace whose node we talk to. Null on web / pre-onboarding. */
  workspace: Workspace | null;
  /** Desktop with no active workspace → show the onboarding gate. */
  needsOnboarding: boolean;
  /** An onboarding step is running (create/join/select) — disables the gate. */
  onboardingBusy: boolean;
  /** A joiner's live park→promote phase while its node is not yet a ready
   *  validator; null on the founder/member path and once the node answers. */
  onboardingPhase: PhaseReport | null;
  /** The active workspace's invite blob, once revealed for sharing. */
  inviteBlob: string | null;
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
  forgeHead: null,
  docIds: [],
  activeDoc: null,
  activeDocBlocks: [],
  agents: [],
  watches: [],
  runs: [],
  error: null,
  workspaces: [],
  workspace: null,
  needsOnboarding: false,
  onboardingBusy: false,
  onboardingPhase: null,
  inviteBlob: null,
});

// ── Pure helpers ────────────────────────────────────────

/** A channel id from a display name: lowercase, dash-separated, wire-safe. */
export const channelIdOf = (name: string): string =>
  name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");

/** A doc id from user input (new-doc or open-by-id): lowercase, dash-separated,
 *  wire-safe. Slugging both entry points keeps "My Notes" and "my-notes" the
 *  same document, mirroring channelIdOf. */
export const docIdOf = (raw: string): string =>
  raw
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
