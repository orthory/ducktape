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
import type {
  PageBlock,
  PageMeta,
  PageSearchHit,
  TargetThreads,
} from "../../domain/pages-client";
import type { BlockRecord, NodeStatus } from "../../domain/transport";
import type { VoiceError } from "../../domain/voice-session";
import type { OpLedger } from "./finalization";
import type { PhaseReport, Workspace } from "../../domain/workspace-client";

/** The two sidebar partitions the view-mode toggle switches between: the
 *  participant "user" apps and the "operator" node/network surfaces. Neither
 *  side confers authority — it is purely which surfaces the rail shows. */
export type ViewMode = "user" | "operator";

/** The ephemeral voice-huddle slice. Lives OUTSIDE ConsoleSnapshot (like
 *  telemetry): the roster is committed consensus state on the channel, but
 *  whether THIS client is in a live audio session — and its mic/connection
 *  state — is per-client and never re-projected from the node. `channelId` is
 *  the channel we're huddling in (null = not in a huddle). */
export interface VoiceSlice {
  channelId: string | null;
  muted: boolean;
  status: "idle" | "connecting" | "live" | "error";
  /** Why `status` is "error" — picks the dock's message. Null otherwise. */
  error: VoiceError | null;
  /** The huddle lives in its own desktop window right now — the in-app card
   *  yields to it (desktop only; see store/huddle-window.ts). */
  popped: boolean;
  /** Local camera state (ephemeral, beaconed to peers — never consensus). */
  cameraOn: boolean;
  /** Per-peer ephemeral call state from 1 Hz beacons, keyed by NODE hex.
   *  Staleness (no beacon for >10 s) drives the sweep affordance. */
  peers: Record<string, { muted: boolean; cameraOn: boolean; atMs: number }>;
}

/** One search round-trip across the modules that ship materialized views —
 *  chat and docs (the `pages` module) searched with the same text, grouped.
 *  `docs` holds the page-block hits — pages is the console's docs surface. */
export interface SearchResults {
  query: string;
  chat: ChatSearchHit[];
  docs: PageSearchHit[];
}

/** A managed (app-spawned) node failed to START or CONNECT — the dedicated
 *  "Node failed to start" surface reads this instead of leaving the developer
 *  on a hollow, disconnected shell. `reason` is the human headline (the Rust
 *  `Err` string, which already folds in the node's exit reason, or the boot
 *  timeout); `logTail` is the daemon.log content behind it; `logPath` powers the
 *  "Open daemon.log" affordance; `workspaceId` lets Retry re-connect the SAME
 *  workspace idempotently (never re-minting one). Null when there is no boot
 *  failure. Distinct from `error` (transient, dismissible op failures) and from
 *  a joiner's `onboardingPhase: fatal` (shown in the waiting room). */
export interface BootError {
  workspaceId: string | null;
  reason: string;
  logPath: string | null;
  logTail: string;
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
  /** This client's live voice-huddle session — ephemeral, never in the
   *  committed snapshot (see VoiceSlice). */
  voice: VoiceSlice;

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
  /** Ordered ids of the open document tabs. `activePage` is the active tab.
   *  Persisted (loadDocTabs) and reconciled against the live enumeration. */
  openTabs: string[];
  /** Comment threads for the open page's blocks + the page itself, grouped by
   *  target. Loaded on page open and after any comment op. Not per-block
   *  snapshot state. */
  pageThreads: TargetThreads[];

  // ── Agents ──
  /** Every registered agent, re-queried per block like tasks. */
  agents: AgentRecord[];
  /** Distinct executor tags announced network-wide (the `capability` registry),
   *  sorted. Feeds the agent view's "Runs on" picker; empty when no host has
   *  announced or the node predates the module (best-effort in the snapshot). */
  capabilities: string[];
  /** Every channel watch and its turn policy. */
  watches: WatchView[];
  /** In-flight runs (dispatches awaiting delivery), newest-first. terminal
   *  history lives in the dispatch module, not here. */
  pendingRuns: PendingRun[];
  /** hex node key -> the executor tags that node announced (the `capability`
   *  registry, kept per-node instead of flattened). Members view shows what
   *  each member runs; empty when nothing is announced. */
  capabilitiesByNode: Map<string, string[]>;
  /** run_id -> hex node key currently executing it (the saga assignee, via the
   *  dispatch read facade). Only in-flight runs appear; empty otherwise. */
  runAssignee: Map<string, string>;

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

  /** The newest finalized height seen on the ws block stream — updated
   *  UNGATED (unlike the refresh the same stream drives, which is held while
   *  an op is in flight), so the console always knows the chain moved. Null
   *  until the first frame on this connection. */
  lastBlock: number | null;

  /** Recent NON-EMPTY blocks, oldest-first (the explorer renders newest
   *  first). Node-local observability — re-pulled from the node's ring on
   *  every refresh; empty on a node without the surface. */
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

  /** A managed node failed to start/connect — routes the console to the
   *  dedicated "Node failed to start" body (see BootError). Null on success. */
  bootError: BootError | null;

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
  /** The picker-row counterpart of `forgetNeedsForce`: the id of the workspace
   *  whose guarded delete couldn't confirm its node left the valset, so its row
   *  offers the force override. Null when no delete is awaiting escalation. */
  deleteNeedsForce: string | null;
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

// ── Doc tab persistence ─────────────────────────────────
//
// The open Docs tabs survive restart as a single id list; on load they are
// filtered against the live page enumeration (a stale id from another workspace
// simply drops), so no per-workspace keying is needed.
const DOC_TABS_KEY = "ducktape.docTabs";

export const loadDocTabs = (): string[] => {
  try {
    const raw = localStorage.getItem(DOC_TABS_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
};

export const saveDocTabs = (tabs: string[]): void => {
  try {
    localStorage.setItem(DOC_TABS_KEY, JSON.stringify(tabs));
  } catch {
    // persistence is best-effort; a failed write just doesn't survive restart.
  }
};

/** Append `id` if absent (order preserved). */
export const addTab = (tabs: string[], id: string): string[] =>
  tabs.includes(id) ? tabs : [...tabs, id];

/** Remove `id`; if it was active, pick the following neighbor (else previous,
 *  else null) as the next active tab. */
export const removeTab = (
  tabs: string[],
  active: string | null,
  id: string,
): { tabs: string[]; active: string | null } => {
  const idx = tabs.indexOf(id);
  const next = tabs.filter((t) => t !== id);
  if (active !== id) return { tabs: next, active };
  const neighbor = next[idx] ?? next[idx - 1] ?? null;
  return { tabs: next, active: neighbor };
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
    voice: {
      channelId: null,
      muted: false,
      status: "idle",
      error: null,
      popped: false,
      cameraOn: false,
      peers: {},
    },
    members: [],
    observers: [],
    proposals: [],
    forgeHead: null,
    pages: [],
    activePage: null,
    activePageBlocks: [],
    openTabs: loadDocTabs(),
    pageThreads: [],
    agents: [],
    capabilities: [],
    watches: [],
    pendingRuns: [],
    capabilitiesByNode: new Map(),
    runAssignee: new Map(),
    search: null,
    searchPending: false,
    searchOpen: false,
    files: [],
    lastBlock: null,
    blocks: [],
    explorerFocus: null,
    ops: {},
    error: null,
    bootError: null,
    workspaces: [],
    workspace: null,
    needsOnboarding: false,
    onboardingBusy: false,
    forgetNeedsForce: false,
    deleteNeedsForce: null,
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
  capabilities: string[];
  watches: WatchView[];
  pendingRuns: PendingRun[];
  capabilitiesByNode: Map<string, string[]>;
  runAssignee: Map<string, string>;
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
  capabilities: snapshot.capabilities,
  watches: snapshot.watches,
  pendingRuns: snapshot.pendingRuns,
  capabilitiesByNode: snapshot.capabilitiesByNode,
  runAssignee: snapshot.runAssignee,
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

