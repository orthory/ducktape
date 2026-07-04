// Console state — a client-side projection of the node's committed state
// (channels/messages/tasks/status re-queried per block) plus the global ui
// state that must survive screen boundaries (screen, accent, author identity,
// thread panel).

import type { AgentRecord, RunView, WatchView } from "../../domain/agent-client";
import type { Channel, ChatThread, MessageView } from "../../domain/chat-client";
import type { Block } from "../../domain/document-client";
import type { ProposalView } from "../../domain/governance-client";
import type { Task, TaskStatus } from "../../domain/tasks-client";
import type { NodeStatus, TelemetryFrame } from "../../domain/transport";
import type { PhaseReport, Workspace } from "../../domain/workspace-client";

/** The two sidebar partitions the view-mode toggle switches between: the
 *  participant "user" apps and the "operator" node/network surfaces. Neither
 *  side confers authority — it is purely which surfaces the rail shows. */
export type ViewMode = "user" | "operator";

// ── State shape ─────────────────────────────────────────

export interface ConsoleState {
  // ── Session / node core ──
  screen: string;
  /** Which sidebar rail is shown. Persisted across sessions (see loadViewMode).
   *  Kept in sync with `screen`: navigating to a surface adopts its section. */
  viewMode: ViewMode;
  accent: string;
  author: string;
  /** The node answered the last status query. */
  connected: boolean;
  /** The daemon url this build resolved to (null until bootstrap finishes). */
  nodeUrl: string | null;
  /** True when this app owns the daemon lifecycle (desktop build). */
  managed: boolean;
  status: NodeStatus | null;

  // ── Chat ──
  channels: Channel[];
  activeChannel: string | null;
  /** Messages of the active channel only (all sequences; views filter). */
  messages: MessageView[];
  activeThread: ChatThread | null;
  /** hex(user key bytes) → display name, from the `profiles` module; threaded
   *  into author rendering so messages show chosen names, not hex handles. */
  authorNames: Record<string, string>;

  // ── Tasks ──
  tasks: Task[];

  // ── Members / validator roster ──
  /** Hex-encoded validator public keys from the `valset` module. */
  members: string[];

  // ── Governance ──
  /** Every proposal from the `governance` module, sorted by id. Re-queried per
   *  block like the roster; empty when the node exposes no governance surface. */
  proposals: ProposalView[];

  // ── Forge ──
  /** forge HEAD commit oid, or null on an unborn repo (no commits yet). */
  forgeHead: string | null;

  // ── Documents ──
  /** Known doc-ids — a client-side registry, since the document module has no
   *  "list docs" query (its store is keyed by sha256(doc_id) and can't
   *  enumerate). Persisted per node url by the provider. */
  docIds: string[];
  /** The doc whose blocks are loaded, or null when none is open. */
  activeDoc: string | null;
  /** Ordered blocks of the active doc (re-queried per block / on open). */
  activeDocBlocks: Block[];

  // ── Agents ──
  /** Every registered agent, re-queried per block like tasks. */
  agents: AgentRecord[];
  /** Every channel watch and its turn policy. */
  watches: WatchView[];
  /** Recent runs across all channels, newest-first for the timeline. */
  runs: RunView[];

  /** Recent per-block node telemetry, oldest-first (the view renders newest
   *  first). Node-local observability — never re-queried from committed state;
   *  backfilled from the node's ring on connect, then followed live over ws. */
  telemetry: TelemetryFrame[];

  error: string | null;

  // ── Workspace / onboarding ──
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

// ── View-mode persistence ───────────────────────────────
//
// The chosen rail survives restarts. The screen itself is NOT persisted, so on
// boot we land on the persisted rail's default surface. These two ids duplicate
// the registry's first-in-section screens (chat / members) rather than import
// the registry into this low-level state module, keeping the store free of the
// views graph.
const VIEW_MODE_KEY = "ducktape.viewMode";
export const DEFAULT_USER_SCREEN = "chat";
export const DEFAULT_OPERATOR_SCREEN = "members";

export const loadViewMode = (): ViewMode => {
  try {
    return localStorage.getItem(VIEW_MODE_KEY) === "operator" ? "operator" : "user";
  } catch {
    return "user"; // storage unavailable (private mode / quota) — default rail
  }
};

export const saveViewMode = (mode: ViewMode): void => {
  try {
    localStorage.setItem(VIEW_MODE_KEY, mode);
  } catch {
    // persistence is best-effort; a failed write just doesn't survive restart.
  }
};

export const createInitialState = (): ConsoleState => {
  const viewMode = loadViewMode();
  return {
    screen: viewMode === "operator" ? DEFAULT_OPERATOR_SCREEN : DEFAULT_USER_SCREEN,
    viewMode,
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
    authorNames: {},
    tasks: [],
    members: [],
    proposals: [],
    forgeHead: null,
    docIds: [],
    activeDoc: null,
    activeDocBlocks: [],
    agents: [],
    watches: [],
    runs: [],
    telemetry: [],
    error: null,
    workspaces: [],
    workspace: null,
    needsOnboarding: false,
    onboardingBusy: false,
    onboardingPhase: null,
    inviteBlob: null,
  };
};

export interface ConsoleSnapshot {
  connected: boolean;
  status: NodeStatus | null;
  channels: Channel[];
  tasks: Task[];
  members: string[];
  proposals: ProposalView[];
  forgeHead: string | null;
  activeChannel: string | null;
  messages: MessageView[];
  authorNames: Record<string, string>;
  activeDocBlocks: Block[];
  agents: AgentRecord[];
  watches: WatchView[];
  runs: RunView[];
}

/** Project a committed node snapshot onto store data fields. Global UI, doc
 *  registry, workspace/onboarding, and error state are intentionally left
 *  untouched. */
export const applySnapshot = (snapshot: ConsoleSnapshot): Partial<ConsoleState> => ({
  connected: snapshot.connected,
  status: snapshot.status,
  channels: snapshot.channels,
  tasks: snapshot.tasks,
  members: snapshot.members,
  proposals: snapshot.proposals,
  forgeHead: snapshot.forgeHead,
  activeChannel: snapshot.activeChannel,
  messages: snapshot.messages,
  authorNames: snapshot.authorNames,
  activeDocBlocks: snapshot.activeDocBlocks,
  agents: snapshot.agents,
  watches: snapshot.watches,
  runs: snapshot.runs,
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
