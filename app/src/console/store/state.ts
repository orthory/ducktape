// Console state — a client-side projection of the node's committed state
// (channels/messages/tasks/status re-queried per block) plus the global ui
// state that must survive screen boundaries (screen, accent, author identity,
// thread panel).

import type { AgentRecord } from "../../domain/agent-client";
import type { BootErrorKind } from "../../domain/boot-error";
import type { PendingRun, WatchView } from "../../domain/runs-client";
import type { RunLease } from "../../domain/dispatch-client";
import type {
  Channel,
  ChatSearchHit,
  ChatTagRow,
  ChatThread,
  MessageView,
} from "../../domain/chat-client";
import type { FileEntry } from "../../domain/files-client";
import type { MemberKeyView } from "../../domain/identity-client";
import type { ForgeItemSummary, ForgeRefHead } from "../../domain/forge-client";
import type { ProposalView, SharesView } from "../../domain/governance-client";
import type {
  PageBlock,
  PageMeta,
  PageSearchHit,
  TargetThreads,
} from "../../domain/pages-client";
import type { BlockRecord, NodeStatus } from "../../domain/transport";
import type { VoiceError } from "../../domain/voice-session";
import type { VideoCapability } from "../../domain/video-capability";
import { loadDevicePrefs } from "../../domain/media-devices";
import type { DevicePrefs, HuddleDevices } from "../../domain/media-devices";
import type { OpLedger } from "./finalization";
// type-only: nav-history value-imports from this module, so the reverse edge
// must stay erased at runtime.
import type { NavStack } from "./nav-history";
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
  /** "reconnecting" = the live session dropped unexpectedly and ONE automatic
   *  media re-establish is in flight — consensus membership is kept. */
  status: "idle" | "connecting" | "reconnecting" | "live" | "error";
  /** Why `status` is "error" — picks the dock's message. Null otherwise. */
  error: VoiceError | null;
  /** The node's own refusal sentence, when it sent one (no call hub, overlay
   *  down). Shown UNDER the message `error` picks: only the node knows which of
   *  the several ways a huddle can fail actually happened. Null otherwise. */
  errorNote: string | null;
  /** A transient media failure note (camera/screen acquire failed) — shown for a
   *  few seconds by the card surfaces, then auto-cleared. Never fatal. */
  mediaNote: "camera-failed" | "screen-failed" | null;
  /** The huddle lives in its own desktop window right now — the in-app card
   *  yields to it (desktop only; see store/huddle-window.ts). */
  popped: boolean;
  /** Local camera state (ephemeral, beaconed to peers — never consensus). */
  cameraOn: boolean;
  /** Whether OUR video lane is a screen share rather than the camera (camera XOR
   *  screen — ephemeral, beaconed, never consensus). */
  sharing: boolean;
  /** Per-peer ephemeral call state from 1 Hz beacons, keyed by NODE hex.
   *  Staleness (no beacon for >10 s) drives the sweep affordance. */
  peers: Record<string, { muted: boolean; cameraOn: boolean; sharing: boolean; atMs: number }>;
  /** Epoch ms our current session started (set on join, null when idle) — the
   *  staleness baseline for a never-beaconed member. Shared so the dock and the
   *  popped window agree on who is sweepable. */
  sessionStartMs: number | null;
  /** Whether OUR mic is currently above the speaking threshold (drives the self
   *  speaking ring + the "you're muted while talking" banner). Detected off the
   *  capture frames, so it's true even while muted. */
  speaking: boolean;
  /** OUR mic input level, 0..1 (throttled). Drives the solo self-check meter so a
   *  lone user can see the mic responds; detected even while muted. */
  level: number;
}

/** One search round-trip across the modules that ship materialized views —
 *  chat and docs (the `pages` module) searched with the same text, grouped.
 *  `docs` holds the page-block hits — pages is the console's docs surface. */
export interface SearchResults {
  query: string;
  chat: ChatSearchHit[];
  docs: PageSearchHit[];
}

/** The active #tag filter on the chat surface: while set, the message pane
 *  renders the tag's `tagSearch` hits instead of the live slice. `tag` keeps
 *  the as-typed display form (the node's index normalizes); `channelId` is
 *  the channel the filter was set in — switching channels clears it. */
export interface TagFilter {
  tag: string;
  channelId: string | null;
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
  kind: BootErrorKind;
  workspaceId: string | null;
  reason: string;
  logPath: string | null;
  logTail: string;
}

/** A mid-session connection warning: either the stream proved the node went
 *  away (crash, stop, remote unplug, wrong node on the port), or bounded status
 *  probes found an established node temporarily busy while its stream remains
 *  alive. Drives the persistent recovery banner; only authoritative stream
 *  `down` makes the session disconnected. Distinct from `bootError` (never
 *  connected) and `error` (transient op failures). */
export interface ConnectionDown {
  reason: string;
  /** True when a DIFFERENT node answered the reused port (identity mismatch on
   *  recovery) rather than our node merely being unreachable — a stronger
   *  warning, and Restart won't help. */
  impostor?: boolean;
}

// ── State shape ─────────────────────────────────────────

export interface ConsoleState {
  // ── Session / node core ──
  screen: string;
  /** Which sidebar rail is shown. Persisted across sessions (see loadViewMode).
   *  Kept in sync with `screen`: navigating to a surface adopts its section. */
  viewMode: ViewMode;
  accent: string;
  /** Light/dark color theme. Reflected onto <html data-theme> by the provider. */
  theme: ThemeMode;
  notifyPrefs: NotifyPrefs;
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
  /** The active #tag filter (see TagFilter), or null for the live view. */
  tagFilter: TagFilter | null;
  /** The active tag filter's hits (newest first) — query-driven, like
   *  `search`; never part of the per-block snapshot. */
  tagHits: ChatSearchHit[];
  /** A tagSearch round-trip is in flight. */
  tagHitsPending: boolean;
  /** The active channel's tag catalog (count-ordered), loaded on demand for
   *  the header's tag dropdown. Cleared on channel switch. */
  channelTags: ChatTagRow[];
  /** hex(node key bytes) → canonical account display name, projected from
   *  `identity` for author rendering. Unbound nodes deliberately have no
   *  replicated display-name record. */
  authorNames: Record<string, string>;
  /** hex(node key bytes) → its owning user, from the `identity` module — the
   *  node/user split's resolver: `name` is that user's chosen display name
   *  (null if unset), already folded into `authorNames` when present. */
  nodeUsers: Record<string, { accountId: string; name: string | null }>;
  /** hex(account id) → the account's collected member keys (of any scheme),
   *  from the `identity` module. `nodeUsers`/`authorNames` carry the shared
   *  display name; this is the key list the account settings surface renders. */
  accountKeys: Record<string, MemberKeyView[]>;
  /** hex(account id) → its optional DuckDNS handle (without `.duck`). Identity
   *  exists independently when no entry is registered. */
  accountHandles: Record<string, string>;
  /** This client's live voice-huddle session — ephemeral, never in the
   *  committed snapshot (see VoiceSlice). */
  voice: VoiceSlice;
  /** Runtime VP8 encode/decode support, resolved once from a real codec probe.
   *  Stable (a device capability, not session state), so it lives OUTSIDE the
   *  voice slice — the huddle-reset paths must never wipe it. `canEncode` gates
   *  the camera control; `canDecode` gates peer-tile rendering. */
  videoCapability: VideoCapability;
  /** The user's chosen huddle input/output devices (persisted; undefined =
   *  system default). Stable across sessions, so it lives OUTSIDE the voice slice
   *  — a leave/rejoin keeps the selection. */
  devicePrefs: DevicePrefs;
  /** The enumerated mic/camera/speaker options for the picker — refreshed on
   *  demand (labels appear only after a media-permission grant). */
  deviceOptions: HuddleDevices;

  // ── Members / validator roster ──
  /** Hex-encoded validator public keys from the `valset` module. */
  members: string[];
  /** Hex-encoded resident public keys from the `valset` module — the
   *  staged-admission tier (mesh + statesync, no quorum seat). Disjoint from
   *  `members`: valset's Grant refuses validators, Join clears standing. */
  residents: string[];

  // ── Governance ──
  /** Every proposal from the `governance` module, sorted by id. Re-queried per
   *  block like the roster; empty when the node exposes no governance surface. */
  proposals: ProposalView[];
  /** The current account-share registry; inactive preserves validator ballots. */
  governanceShares: SharesView;

  // ── Forge ──
  /** forge HEAD commit oid, or null on an unborn repo (no commits yet). */
  forgeHead: string | null;
  /** The repo whose tracker slices below are loaded. Repo SELECTION lives in
   *  the forge view (component-local); the loaders stamp this so a slow load
   *  for a repo the view has since left can never land (see loadForgeItems).
   *  Null until the first load. */
  forgeRepo: string | null;
  /** `forgeRepo`'s issues/PRs (ListItems). Per-screen loaded — the forge view
   *  calls loadForgeItems on open/repo switch; never in the per-block refresh. */
  forgeItems: ForgeItemSummary[];
  /** `forgeRepo`'s branch heads (ListRefs) — the PR forms' branch pickers and
   *  the branches rail. Per-screen loaded like forgeItems. */
  forgeBranches: ForgeRefHead[];

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
   *  Persisted per workspace/node scope and reconciled against its live
   *  enumeration. */
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
  runLease: Map<string, RunLease>;

  // ── Search (cross-module reads over the node's derived index) ──
  /** The last search's results, or null before any search ran. Query-driven —
   *  never part of the per-block snapshot. */
  search: SearchResults | null;
  /** A search round-trip is in flight (the module views fan out). */
  searchPending: boolean;
  /** The ⌘K command-palette search overlay is open. Global UI, not per-block. */
  searchOpen: boolean;

  // ── Files (duckfs) ──
  /** A flat index of file entries under the tree root (Find, prefix "/"),
   *  re-queried per block. Feeds the command palette's file filter; the files
   *  browser pages the tree live off the transport instead. */
  files: FileEntry[];

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

  /** The forge item the forge view should open on next render — a clicked
   *  desktop notification's hand-off (the explorerFocus idiom: the provider's
   *  navigate listener sets it, ForgeView consumes it, and the provider
   *  retires it when the user leaves the forge screen). `number` null means
   *  a repo-only focus. Null when nothing is pending. */
  forgeFocus: { repo: string; number: number | null } | null;

  /** The duckfs path the files browser should open on next render — the same
   *  one-shot hand-off idiom as forgeFocus, used by the agent form to point an
   *  operator at a skill document. Null when nothing is pending. */
  filesFocus: string | null;

  /** The agent the agent view should select on next render — a clicked @agent
   *  mention's hand-off (the explorerFocus idiom: the mention sets it, AgentView
   *  consumes it and clears it). Null when nothing is pending. */
  agentFocus: string | null;

  /** The person the members view should select on next render — a clicked @user
   *  mention's hand-off. An ACCOUNT id, not a node key: a mention mark carries
   *  the account, and the view maps it back to one of that account's node rows
   *  through `nodeUsers`. Null when nothing is pending. */
  memberFocus: string | null;

  /** Per-operation finalization ledger (entity key → newest op touching that
   *  row): pending while a write is in flight, then finalized with the
   *  inclusion height + addressable op hash from the submit receipt. Client
   *  bookkeeping, never committed state — node switches reset it. */
  ops: OpLedger;

  error: string | null;

  /** A managed node failed to start/connect — routes the console to the
   *  dedicated "Node failed to start" body (see BootError). Null on success. */
  bootError: BootError | null;

  /** The node went away mid-session — drives a persistent reconnecting banner
   *  (see ConnectionDown). Null while connected. */
  connectionDown: ConnectionDown | null;

  // ── Workspace / onboarding ──
  /** Every registered workspace, for the switcher. Empty on web. */
  workspaces: Workspace[];
  /** The active workspace whose node we talk to. Null on web / pre-onboarding. */
  workspace: Workspace | null;
  /** Desktop with no active workspace → show the onboarding gate. */
  needsOnboarding: boolean;
  /** The account-centric Home layer is showing (the workspace shell's routed
   *  screen is hidden). Not a disconnect — the node connection is kept alive
   *  underneath. */
  atHome: boolean;
  /** Where the session sits in the webview's history stack — drives the title
   *  bar's back/forward enablement. Owned by the provider's history effects
   *  (see nav-history.ts). */
  nav: NavStack;
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

export interface NotifyPrefs {
  enabled: boolean;
  mentions: boolean;
  replies: boolean;
  huddles: boolean;
  runs: boolean;
  forge: boolean;
  governance: boolean;
  mutedChannels: string[];
}

export const DEFAULT_NOTIFY_PREFS: NotifyPrefs = {
  enabled: true,
  mentions: true,
  replies: true,
  huddles: true,
  runs: true,
  forge: true,
  governance: true,
  mutedChannels: [],
};

/** The boot placeholder author. The provider replaces it with the chain's
 *  resolved name for our own node as soon as one hydrates — only a name the
 *  USER typed (≠ this placeholder) is ever kept over the chain's. */
export const DEFAULT_AUTHOR = "operator";

// ── Accent persistence ──────────────────────────────────
//
// The chosen accent survives restarts. Values are validated as #rrggbb on
// load so a corrupt/foreign string can never reach inline styles.
const ACCENT_KEY = "ducktape.accent";

export const loadAccent = (): string => {
  try {
    const raw = localStorage.getItem(ACCENT_KEY);
    return raw && /^#[0-9a-f]{6}$/i.test(raw) ? raw : DEFAULT_ACCENT;
  } catch {
    return DEFAULT_ACCENT; // storage unavailable (private mode / quota)
  }
};

export const saveAccent = (accent: string): void => {
  try {
    localStorage.setItem(ACCENT_KEY, accent);
  } catch {
    // persistence is best-effort; a failed write just doesn't survive restart.
  }
};

// ── Theme (light/dark) persistence ─────────────────────
//
// The chosen theme survives restarts. First run (no stored choice) follows the
// OS `prefers-color-scheme`; anything else falls back to light.
export type ThemeMode = "light" | "dark";
export const DEFAULT_THEME: ThemeMode = "light";
const THEME_KEY = "ducktape.theme";

export const loadTheme = (): ThemeMode => {
  try {
    const raw = localStorage.getItem(THEME_KEY);
    if (raw === "light" || raw === "dark") return raw;
  } catch {
    return DEFAULT_THEME; // storage unavailable (private mode / quota)
  }
  try {
    if (typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches) {
      return "dark";
    }
  } catch {
    // matchMedia unavailable (non-browser env) — fall through to the default.
  }
  return DEFAULT_THEME;
};

export const saveTheme = (theme: ThemeMode): void => {
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    // best-effort; a failed write just doesn't survive restart.
  }
};

// ── Notification prefs persistence ─────────────────────
//
// Desktop notification preferences survive restarts. Each field is validated
// independently so a partial or corrupt blob falls back only where needed.
const NOTIFY_PREFS_KEY = "ducktape.notifyPrefs";

const defaultNotifyPrefs = (): NotifyPrefs => ({
  ...DEFAULT_NOTIFY_PREFS,
  mutedChannels: [...DEFAULT_NOTIFY_PREFS.mutedChannels],
});

const loadNotifyPrefsFrom = (value: unknown): NotifyPrefs => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return defaultNotifyPrefs();
  }
  const prefs = value as Record<string, unknown>;
  return {
    enabled:
      typeof prefs.enabled === "boolean" ? prefs.enabled : DEFAULT_NOTIFY_PREFS.enabled,
    mentions:
      typeof prefs.mentions === "boolean" ? prefs.mentions : DEFAULT_NOTIFY_PREFS.mentions,
    replies:
      typeof prefs.replies === "boolean" ? prefs.replies : DEFAULT_NOTIFY_PREFS.replies,
    huddles:
      typeof prefs.huddles === "boolean" ? prefs.huddles : DEFAULT_NOTIFY_PREFS.huddles,
    runs: typeof prefs.runs === "boolean" ? prefs.runs : DEFAULT_NOTIFY_PREFS.runs,
    forge: typeof prefs.forge === "boolean" ? prefs.forge : DEFAULT_NOTIFY_PREFS.forge,
    governance:
      typeof prefs.governance === "boolean"
        ? prefs.governance
        : DEFAULT_NOTIFY_PREFS.governance,
    mutedChannels:
      Array.isArray(prefs.mutedChannels) &&
      prefs.mutedChannels.every((channel): channel is string => typeof channel === "string")
        ? [...prefs.mutedChannels]
        : [...DEFAULT_NOTIFY_PREFS.mutedChannels],
  };
};

export const loadNotifyPrefs = (): NotifyPrefs => {
  try {
    const raw = localStorage.getItem(NOTIFY_PREFS_KEY);
    return loadNotifyPrefsFrom(raw ? JSON.parse(raw) : null);
  } catch {
    return defaultNotifyPrefs();
  }
};

export const saveNotifyPrefs = (prefs: NotifyPrefs): void => {
  try {
    localStorage.setItem(NOTIFY_PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // persistence is best-effort; a failed write just doesn't survive restart.
  }
};

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
// Open Docs tabs survive restart, but page ids are only meaningful inside one
// workspace/node. Persist a map by connection scope so an empty new workspace
// cannot render another workspace's tabs while its enumeration is empty.
const DOC_TABS_KEY = "ducktape.docTabs";

export const docTabsScope = (
  workspaceId: string | null,
  nodeUrl: string | null,
): string =>
  workspaceId ? `workspace:${workspaceId}` : nodeUrl ? `remote:${nodeUrl}` : "session";

/** A workspace or remote node is in context — the shell has surfaces to show
 *  behind the Home layer. Nothing connected (smart boot, or after leaving a
 *  workspace) means Home owns the whole window: no sidebar, no search, no
 *  history traversal to fight it. */
export const hasNodeContext = (
  state: Pick<ConsoleState, "workspace" | "nodeUrl">,
): boolean => state.workspace !== null || state.nodeUrl !== null;

const parseDocTabStore = (raw: string | null): Record<string, string[]> => {
  if (!raw) return {};
  const parsed: unknown = JSON.parse(raw);
  // Pre-scoping builds stored one global array. It cannot safely be assigned
  // to whichever workspace happens to boot first, so discard it.
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
  return Object.fromEntries(
    Object.entries(parsed).filter(
      (entry): entry is [string, string[]] =>
        Array.isArray(entry[1]) && entry[1].every((id) => typeof id === "string"),
    ),
  );
};

export const loadDocTabs = (scope: string): string[] => {
  try {
    return [...(parseDocTabStore(localStorage.getItem(DOC_TABS_KEY))[scope] ?? [])];
  } catch {
    return [];
  }
};

export const saveDocTabs = (scope: string, tabs: string[]): void => {
  try {
    const store = parseDocTabStore(localStorage.getItem(DOC_TABS_KEY));
    store[scope] = [...tabs];
    localStorage.setItem(DOC_TABS_KEY, JSON.stringify(store));
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

// ── Onboarding hand-off persistence ─────────────────────
//
// Two first-run facts that outlive the onboarding screens. The display name
// chosen while creating the account can only land on-chain after the first
// node connects (names are chain-scoped), so it parks here until then. The
// "link this device to an EXISTING account" choice must stop auto-bind from
// founding a duplicate account until the other device's AddMemberKey lands —
// see auto-bind.ts's "deferred" branch.
const PENDING_NAME_KEY = "ducktape.pendingDisplayName";
const LINK_PENDING_KEY = "ducktape.accountLinkPending";

export const loadPendingDisplayName = (): string | null => {
  try {
    const raw = localStorage.getItem(PENDING_NAME_KEY);
    return raw && raw.trim().length > 0 ? raw : null;
  } catch {
    return null; // storage unavailable — no parked name
  }
};

export const savePendingDisplayName = (name: string): void => {
  try {
    localStorage.setItem(PENDING_NAME_KEY, name);
  } catch {
    // best-effort; a failed write just loses the parked name.
  }
};

export const clearPendingDisplayName = (): void => {
  try {
    localStorage.removeItem(PENDING_NAME_KEY);
  } catch {
    // best-effort; nothing to clean up if storage is unavailable.
  }
};

export const loadLinkPending = (): boolean => {
  try {
    return localStorage.getItem(LINK_PENDING_KEY) === "1";
  } catch {
    return false; // storage unavailable — treat as no pending link
  }
};

export const saveLinkPending = (): void => {
  try {
    localStorage.setItem(LINK_PENDING_KEY, "1");
  } catch {
    // best-effort; a failed write risks a duplicate account on next bind,
    // the same exposure a pre-link build had.
  }
};

export const clearLinkPending = (): void => {
  try {
    localStorage.removeItem(LINK_PENDING_KEY);
  } catch {
    // best-effort; nothing to clean up if storage is unavailable.
  }
};

export const createInitialState = (): ConsoleState => {
  const viewMode = loadViewMode();
  return {
    screen: viewMode === "operator" ? DEFAULT_OPERATOR_SCREEN : DEFAULT_USER_SCREEN,
    viewMode,
    accent: loadAccent(),
    theme: loadTheme(),
    notifyPrefs: loadNotifyPrefs(),
    author: DEFAULT_AUTHOR,
    connected: false,
    nodeUrl: null,
    managed: false,
    status: null,
    channels: [],
    activeChannel: null,
    messages: [],
    activeThread: null,
    tagFilter: null,
    tagHits: [],
    tagHitsPending: false,
    channelTags: [],
    authorNames: {},
    nodeUsers: {},
    accountKeys: {},
    accountHandles: {},
    voice: {
      channelId: null,
      muted: false,
      status: "idle",
      error: null,
      errorNote: null,
      mediaNote: null,
      popped: false,
      cameraOn: false,
      sharing: false,
      peers: {},
      sessionStartMs: null,
      speaking: false,
      level: 0,
    },
    videoCapability: { canEncode: false, canDecode: false, canScreenShare: false },
    devicePrefs: loadDevicePrefs(),
    deviceOptions: { mics: [], cameras: [], speakers: [] },
    members: [],
    residents: [],
    proposals: [],
    governanceShares: { active: false, allocations: [], total: 0 },
    forgeHead: null,
    forgeRepo: null,
    forgeItems: [],
    forgeBranches: [],
    pages: [],
    activePage: null,
    activePageBlocks: [],
    // The active workspace/node is resolved after mount; connectActive or
    // connectRemote loads that scope's persisted tabs.
    openTabs: [],
    pageThreads: [],
    agents: [],
    capabilities: [],
    watches: [],
    pendingRuns: [],
    capabilitiesByNode: new Map(),
    runLease: new Map(),
    search: null,
    searchPending: false,
    searchOpen: false,
    files: [],
    lastBlock: null,
    blocks: [],
    explorerFocus: null,
    forgeFocus: null,
    filesFocus: null,

    agentFocus: null,
    memberFocus: null,
    ops: {},
    error: null,
    bootError: null,
    connectionDown: null,
    workspaces: [],
    workspace: null,
    needsOnboarding: false,
    atHome: false,
    // the boot document is the stack's only entry until the sync effect stamps it
    nav: { index: 0, count: 1 },
    onboardingBusy: false,
    forgetNeedsForce: false,
    deleteNeedsForce: null,
    onboardingPhase: null,
    inviteBlob: null,
  };
};

/** Fresh values for every projection owned by the connected node. Workspace
 * switches keep global UI/preferences and the workspace registry, but no
 * committed, query-driven, or operation state may cross the node boundary.
 * Keep this single reset in lockstep with ConsoleState instead of maintaining
 * subtly different hand-written lists in each switch/delete path. */
export const resetNodeProjection = (): Partial<ConsoleState> => ({
  connected: false,
  status: null,
  channels: [],
  activeChannel: null,
  messages: [],
  activeThread: null,
  tagFilter: null,
  tagHits: [],
  tagHitsPending: false,
  channelTags: [],
  authorNames: {},
  nodeUsers: {},
  accountKeys: {},
  accountHandles: {},
  members: [],
  residents: [],
  proposals: [],
  governanceShares: { active: false, allocations: [], total: 0 },
  forgeHead: null,
  forgeRepo: null,
  forgeItems: [],
  forgeBranches: [],
  pages: [],
  activePage: null,
  activePageBlocks: [],
  openTabs: [],
  pageThreads: [],
  agents: [],
  capabilities: [],
  watches: [],
  pendingRuns: [],
  capabilitiesByNode: new Map(),
  runLease: new Map(),
  search: null,
  searchPending: false,
  files: [],
  lastBlock: null,
  blocks: [],
  explorerFocus: null,
  forgeFocus: null,
  filesFocus: null,

  agentFocus: null,
  memberFocus: null,
  ops: {},
  connectionDown: null,
});

export interface ConsoleSnapshot {
  connected: boolean;
  status: NodeStatus | null;
  channels: Channel[];
  members: string[];
  residents: string[];
  proposals: ProposalView[];
  governanceShares: SharesView;
  forgeHead: string | null;
  activeChannel: string | null;
  messages: MessageView[];
  authorNames: Record<string, string>;
  nodeUsers: Record<string, { accountId: string; name: string | null }>;
  accountKeys: Record<string, MemberKeyView[]>;
  accountHandles: Record<string, string>;
  pages: PageMeta[];
  activePageBlocks: PageBlock[];
  agents: AgentRecord[];
  capabilities: string[];
  watches: WatchView[];
  pendingRuns: PendingRun[];
  capabilitiesByNode: Map<string, string[]>;
  runLease: Map<string, RunLease>;
  files: FileEntry[];
  blocks: BlockRecord[];
}

/** Project a committed node snapshot onto store data fields. Global UI,
 *  workspace/onboarding, and error state are intentionally left untouched. */
export const applySnapshot = (snapshot: ConsoleSnapshot): Partial<ConsoleState> => ({
  connected: snapshot.connected,
  status: snapshot.status,
  channels: snapshot.channels,
  members: snapshot.members,
  residents: snapshot.residents,
  proposals: snapshot.proposals,
  governanceShares: snapshot.governanceShares,
  forgeHead: snapshot.forgeHead,
  activeChannel: snapshot.activeChannel,
  messages: snapshot.messages,
  authorNames: snapshot.authorNames,
  nodeUsers: snapshot.nodeUsers,
  accountKeys: snapshot.accountKeys,
  accountHandles: snapshot.accountHandles,
  pages: snapshot.pages,
  activePageBlocks: snapshot.activePageBlocks,
  agents: snapshot.agents,
  capabilities: snapshot.capabilities,
  watches: snapshot.watches,
  pendingRuns: snapshot.pendingRuns,
  capabilitiesByNode: snapshot.capabilitiesByNode,
  runLease: snapshot.runLease,
  files: snapshot.files,
  blocks: snapshot.blocks,
});

// ── Pure helpers ────────────────────────────────────────

/** The identity committed rows actually carry for OUR writes. The submit
 *  lane SIGNS frames on networked nodes: committed authorship is the NODE's
 *  pubkey and the client's origin string is deliberately ignored — so self
 *  is `status.publicKey` whenever the node reports one. The embedded daemon
 *  (empty publicKey) stores the origin string verbatim, so the author
 *  string remains its self there. This is the follow-the-head handoff's
 *  bug A: every self-comparison (optimistic rows, canModify,
 *  reaction-"mine", huddle roster) must use THESE bytes, never the literal
 *  author string. */
export const selfAuthorBytes = (
  status: NodeStatus | null,
  author: string,
): number[] => {
  const pk = (status?.publicKey ?? "").toLowerCase();
  if (pk.length >= 64 && pk.length % 2 === 0) {
    const bytes: number[] = [];
    for (let i = 0; i < pk.length; i += 2) {
      const byte = parseInt(pk.slice(i, i + 2), 16);
      if (Number.isNaN(byte)) return Array.from(new TextEncoder().encode(author));
      bytes.push(byte);
    }
    return bytes;
  }
  return Array.from(new TextEncoder().encode(author));
};

/** A channel id from a display name: lowercase, dash-separated, wire-safe. */
export const channelIdOf = (name: string): string =>
  name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "");
