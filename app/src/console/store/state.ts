// Console state — a client-side projection of the node's committed state
// (channels/messages/tasks/status re-queried per block) plus the global ui
// state that must survive screen boundaries (screen, accent, author identity,
// thread panel).

import type { AgentRecord } from "../../domain/agent-client";
import type { PendingRun, WatchView } from "../../domain/runs-client";
import type {
  Channel,
  ChatSearchHit,
  ChatThread,
  MessageView,
} from "../../domain/chat-client";
import type { Manifest } from "../../domain/files-client";
import type { ProposalView } from "../../domain/governance-client";
import type { PageBlock, PageMeta, PageSearchHit } from "../../domain/pages-client";
import type { BlockRecord, NodeStatus, TelemetryFrame } from "../../domain/transport";
import type { OpLedger } from "./finalization";
import type { PhaseReport, Workspace } from "../../domain/workspace-client";

/** The two sidebar partitions the view-mode toggle switches between: the
 *  participant "user" apps and the "operator" node/network surfaces. Neither
 *  side confers authority — it is purely which surfaces the rail shows. */
export type ViewMode = "user" | "operator";

/** One search round-trip across the modules that ship materialized views —
 *  chat and docs (the `pages` module) searched with the same text, grouped.
 *  `docs` holds the page-block hits — pages is the console's docs surface. */
export interface SearchResults {
  query: string;
  chat: ChatSearchHit[];
  docs: PageSearchHit[];
}

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

  // ── Members / validator roster ──
  /** Hex-encoded validator public keys from the `valset` module. */
  members: string[];
  /** Hex-encoded observer public keys from the `valset` module — the
   *  staged-admission tier (mesh + statesync, no quorum seat). Disjoint from
   *  `members`: valset's Grant refuses validators, Join clears standing. */
  observers: string[];

  // ── Governance ──
  /** Every proposal from the `governance` module, sorted by id. Re-queried per
   *  block like the roster; empty when the node exposes no governance surface. */
  proposals: ProposalView[];

  // ── Forge ──
  /** forge HEAD commit oid, or null on an unborn repo (no commits yet). */
  forgeHead: string | null;

  // ── Docs (block-tree notebook over the `pages` module) ──
  /** Every page (id + live title), from ListPages, re-queried per block.
   *  Empty when the node predates the pages module. */
  pages: PageMeta[];
  /** The page whose block tree is loaded, or null when none is open. */
  activePage: string | null;
  /** Preorder blocks of the active page — root first — re-queried per block /
   *  on open. The view derives depth/indent from the parent links. */
  activePageBlocks: PageBlock[];

  // ── Agents ──
  /** Every registered agent, re-queried per block like tasks. */
  agents: AgentRecord[];
  /** Every channel watch and its turn policy. */
  watches: WatchView[];
  /** In-flight runs (dispatches awaiting delivery), newest-first. terminal
   *  history lives in the dispatch module, not here. */
  pendingRuns: PendingRun[];

  // ── Search (cross-module reads over the node's derived index) ──
  /** The last search's results, or null before any search ran. Query-driven —
   *  never part of the per-block snapshot. */
  search: SearchResults | null;
  /** A search round-trip is in flight (the module views fan out). */
  searchPending: boolean;
  /** The ⌘K command-palette search overlay is open. Global UI, not per-block. */
  searchOpen: boolean;

  // ── Files (content-addressed manifests) ──
  /** Every file manifest (List, prefix ""), re-queried per block. */
  files: Manifest[];

  /** Recent per-block node telemetry, oldest-first (the view renders newest
   *  first). Node-local observability — never re-queried from committed state;
   *  backfilled from the node's ring on connect, then followed live over ws. */
  telemetry: TelemetryFrame[];

  /** Recent NON-EMPTY blocks, oldest-first (the explorer renders newest
   *  first). Node-local observability like telemetry — re-pulled from the
   *  node's ring on every refresh; empty on a node without the surface. */
  blocks: BlockRecord[];

  /** Height the explorer should open on next render — the finalization-mark
   *  cross-link's hand-off (openExplorerAt sets it, the explorer consumes it
   *  once `blocks` has data and clears it). Null when nothing is pending. */
  explorerFocus: number | null;

  /** Per-operation finalization ledger (entity key → newest op touching that
   *  row): pending while a write is in flight, then finalized with the
   *  inclusion height + addressable op hash from the submit receipt. Client
   *  bookkeeping, never committed state — node switches reset it. */
  ops: OpLedger;

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
  /** The last guarded forget couldn't confirm the node left its valset (node
   *  down/bricked) — reveal the force-forget override so a workspace whose node
   *  can never start is still removable. Cleared on any fresh forget attempt. */
  forgetNeedsForce: boolean;
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

// ── Remote node persistence ─────────────────────────────
//
// The last remote node url the user dialed, so the desktop app reconnects to it
// on launch instead of falling back to the onboarding gate. A remote choice
// supersedes the local active-workspace pointer (which lives in the Rust
// registry) — connecting a workspace clears this, connecting a remote sets it —
// so whichever the user chose last is what we reconnect to.
const REMOTE_URL_KEY = "ducktape.remoteUrl";

export const loadRemoteUrl = (): string | null => {
  try {
    return localStorage.getItem(REMOTE_URL_KEY);
  } catch {
    return null; // storage unavailable — no remembered remote
  }
};

export const saveRemoteUrl = (url: string): void => {
  try {
    localStorage.setItem(REMOTE_URL_KEY, url);
  } catch {
    // best-effort; a failed write just doesn't survive restart.
  }
};

export const clearRemoteUrl = (): void => {
  try {
    localStorage.removeItem(REMOTE_URL_KEY);
  } catch {
    // best-effort; nothing to clean up if storage is unavailable.
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
    members: [],
    observers: [],
    proposals: [],
    forgeHead: null,
    pages: [],
    activePage: null,
    activePageBlocks: [],
    agents: [],
    watches: [],
    pendingRuns: [],
    search: null,
    searchPending: false,
    searchOpen: false,
    files: [],
    telemetry: [],
    blocks: [],
    explorerFocus: null,
    ops: {},
    error: null,
    workspaces: [],
    workspace: null,
    needsOnboarding: false,
    onboardingBusy: false,
    forgetNeedsForce: false,
    onboardingPhase: null,
    inviteBlob: null,
  };
};

export interface ConsoleSnapshot {
  connected: boolean;
  status: NodeStatus | null;
  channels: Channel[];
  members: string[];
  observers: string[];
  proposals: ProposalView[];
  forgeHead: string | null;
  activeChannel: string | null;
  messages: MessageView[];
  authorNames: Record<string, string>;
  pages: PageMeta[];
  activePageBlocks: PageBlock[];
  agents: AgentRecord[];
  watches: WatchView[];
  pendingRuns: PendingRun[];
  files: Manifest[];
  blocks: BlockRecord[];
}

/** Project a committed node snapshot onto store data fields. Global UI,
 *  workspace/onboarding, and error state are intentionally left untouched. */
export const applySnapshot = (snapshot: ConsoleSnapshot): Partial<ConsoleState> => ({
  connected: snapshot.connected,
  status: snapshot.status,
  channels: snapshot.channels,
  members: snapshot.members,
  observers: snapshot.observers,
  proposals: snapshot.proposals,
  forgeHead: snapshot.forgeHead,
  activeChannel: snapshot.activeChannel,
  messages: snapshot.messages,
  authorNames: snapshot.authorNames,
  pages: snapshot.pages,
  activePageBlocks: snapshot.activePageBlocks,
  agents: snapshot.agents,
  watches: snapshot.watches,
  pendingRuns: snapshot.pendingRuns,
  files: snapshot.files,
  blocks: snapshot.blocks,
});

// ── Pure helpers ────────────────────────────────────────

/** A channel id from a display name: lowercase, dash-separated, wire-safe. */
export const channelIdOf = (name: string): string =>
  name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");

