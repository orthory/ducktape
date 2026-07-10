import type { Dispatch } from "react";

import * as agentClient from "../../domain/agent-client";
import * as chatClient from "../../domain/chat-client";
import * as duckdnsClient from "../../domain/duckdns-client";
import type { ChatBlock, PostPolicy } from "../../domain/chat-client";
import * as forgeClient from "../../domain/forge-client";
import type {
  ForgeItemDetail,
  ForgeReviewComment,
  ForgeReviewVerdict,
} from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import * as identityClient from "../../domain/identity-client";
import { normalizeKey } from "../../domain/names";
import * as pagesClient from "../../domain/pages-client";
import type { BlockKind as PageBlockKind, PageBlock } from "../../domain/pages-client";
import * as runsClient from "../../domain/runs-client";
import type { TurnPolicy } from "../../domain/runs-client";
import { parseMetrics, type NodeMetrics } from "../../domain/metrics";
import * as bootstrap from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
import { callSocketUrl } from "../../domain/transport";
// Task 7 moved the huddle session to call-session (typed /v1/call/ws + audio +
// camera video + control on one socket); this store drives it via CallEvent.
import {
  createCallSession,
  MAX_VIDEO_PARTICIPANTS,
} from "../../domain/call-session";
import type { CallSession, CallEvent } from "../../domain/call-session";
import { probeVideoCapability } from "../../domain/video-capability";
import { enumerateHuddleDevices, saveDevicePrefs } from "../../domain/media-devices";
import type { DevicePrefs } from "../../domain/media-devices";
import { huddleRecipients } from "../../domain/voice-session";
import { keyBytes, keyHex } from "../../domain/chat-client";
import * as valsetClient from "../../domain/valset-client";
import * as ws from "../../domain/workspace-client";
import type { Workspace } from "../../domain/workspace-client";
import { parseMessageInput } from "../views/chat/chat-input";
import {
  hasAgentMention,
  mentionableUsers,
  mentionResolverOf,
} from "../views/chat/mention";
import {
  defaultScreenForSection,
  sectionForScreen,
} from "../modules/registry";
import type { Action } from "./reducer";
import {
  addMemberFromResponse,
  approvePhoneEnrollment,
  mintLinkChallenge,
  removeMemberKey,
  startPhoneEnrollment,
  unbindNode,
} from "./account-ops";
import type { AccountOpsDeps, PhoneEnrollment } from "./account-ops";
import type { LinkChallenge } from "../views/account/link-device";
import { autoBindUserIdentity } from "./auto-bind";
import { beginOp, failOp, finalizeOp, opKey, receiptOf } from "./finalization";
import * as optimistic from "./optimistic";
import { closeHuddleWindow, openHuddleWindow } from "./huddle-window";
import {
  addTab,
  channelIdOf,
  clearPendingDisplayName,
  clearRemoteUrl,
  loadPendingDisplayName,
  removeTab,
  saveAccent,
  saveTheme,
  saveDocTabs,
  saveNotifyPrefs,
  saveRemoteUrl,
  saveViewMode,
  selfAuthorBytes,
} from "./state";
import type { ConsoleState, NotifyPrefs, ViewMode } from "./state";

/** How often a parked joiner's phase is polled while it promotes. */
const JOIN_POLL_MS = 1500;

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const hasValsetStanding = (transport: NodeTransport, pubkey: string): Promise<boolean> =>
  Promise.all([
    valsetClient.validators(transport).catch((): number[][] => []),
    valsetClient.residents(transport).catch((): number[][] => []),
  ]).then(([validators, residents]) => {
    const wanted = pubkey.toLowerCase();
    return validators
      .concat(residents)
      .some((key) => keyHex(key).toLowerCase() === wanted);
  });

/** Replace a workspace by id, else append — keeps the registry list current. */
const mergeWorkspace = (list: Workspace[], next: Workspace): Workspace[] =>
  list.some((w) => w.id === next.id)
    ? list.map((w) => (w.id === next.id ? next : w))
    : [...list, next];

export interface ConsoleActions {
  setScreen(screen: string): void;
  /** Jump to the explorer opened on the block at `height` — the finalization
   *  mark's cross-link. Best-effort: if the ring no longer holds that height
   *  the explorer just lands on the list. */
  openExplorerAt(height: number): void;
  /** The explorer calls this once it has consumed (or given up on) a pending
   *  focus hand-off. */
  clearExplorerFocus(): void;
  /** Switch the sidebar rail (user apps vs operator surfaces) and persist it.
   *  Jumps to the target rail's default surface when the current screen belongs
   *  to the other rail, so the body always matches the rail. */
  setViewMode(mode: ViewMode): void;
  setAccent(accent: string): void;
  /** Flip the light/dark color theme and persist the choice. */
  toggleTheme(): void;
  setNotifyPrefs(prefs: NotifyPrefs): void;
  toggleChannelMute(channelId: string): void;
  setAuthor(author: string): void;
  /** Set this node's bound identity account display name. */
  setDisplayName(name: string): void;
  /** Declaratively set or clear the bound account's optional `.duck` name. */
  setDuckHandle(handle: string | null): void;

  // ── Account (the person: member keys, bound nodes) ──
  /** Mint a fresh device-link challenge for this node's account — the Account
   *  view encodes it for display and holds it to approve the response against
   *  (the possession proof is pinned to the challenge's nonce). */
  accountLinkChallenge(): Promise<LinkChallenge>;
  /** Approve a pasted link response against `challenge` — submits
   *  AddMemberKey. Rejects with an actionable error on nonce drift. */
  accountAddMember(challenge: LinkChallenge, responseBlob: string): Promise<void>;
  /** Drop a member key from this account (the module refuses the last one). */
  accountRemoveMember(targetKeyHex: string): Promise<void>;
  /** Evict a (lost) node from this account — its first UI consumer. */
  accountUnbindNode(targetNodeHex: string): Promise<void>;
  /** Stand up the LAN QR-enrollment server for this account (fresh nonce);
   *  the card polls enroll-client directly and cancels on unmount. */
  accountPhoneEnrollStart(): Promise<PhoneEnrollment>;
  /** Approve the phone's candidate P-256 key — submits AddMemberKey. Rejects
   *  with an actionable error on nonce drift. */
  accountPhoneEnrollApprove(
    enrollment: PhoneEnrollment,
    newKeyHex: string,
    sigHex: string,
    label: string | null,
  ): Promise<void>;
  selectChannel(channelId: string): void;
  createChannel(name: string, postPolicy: PostPolicy): void;
  sendMessage(body: string): void;
  /** Post `body` into ANY channel (not just the active one) with the same
   *  mention parsing + first-agent-mention watch arming as `sendMessage` —
   *  the forge item Discussion's post path into its hidden channel. Resolves
   *  when the tracked submit settles (errors surface via the ops ledger). */
  postInChannel(channelId: string, body: string): Promise<void>;
  openThread(rootSeq: number): void;
  closeThread(): void;
  replyInThread(body: string): void;
  /** Replace our own message's text (EditMessage). The module rejects edits
   *  from a non-author, so this is only surfaced on own messages. */
  editMessage(seq: number, body: string): void;
  /** Tombstone our own message (DeleteMessage). Author-gated by the module. */
  deleteMessage(seq: number): void;
  /** Toggle our own reaction on a message: adds it if we haven't reacted with
   *  that emoji yet, removes it if we have. Refreshes the open thread panel
   *  too, since its replies are a separate snapshot from `state.messages`. */
  toggleReaction(seq: number, emoji: string): void;

  // ── Huddle (voice over the chat channel's roster) ──
  /** Join a channel's voice huddle: leave any current huddle first (leave op +
   *  stop the old session), submit join_huddle carrying this node's key, start
   *  the audio session, and push the current roster as the fan-out set. No-op
   *  when the daemon can't do voice (no status.publicKey) or we're already in
   *  this channel's huddle — except an errored session, where re-join retries. */
  joinHuddle(channelId: string): void;
  /** Leave the active huddle: stop the audio session, clear the voice slice,
   *  and submit leave_huddle for the channel. */
  leaveHuddle(): void;
  /** Mute / unmute the mic — stops forwarding captured frames without dropping
   *  the track, so unmute is instant. */
  setHuddleMuted(muted: boolean): void;
  /** Recompute the live session's fan-out set from the active channel's roster
   *  and push it. Called by the provider whenever a refresh lands a new
   *  snapshot while a huddle is active; a no-op when not huddling. */
  syncHuddleRecipients(): void;
  /** Turn the local camera on / off in the active huddle — acquires + encodes
   *  (or tears down) camera video on the live session and beacons the change to
   *  peers. Guarded: no-op with no session, on a runtime that can't do video,
   *  or when the roster already EXCEEDS the video cap (audio-only past it). */
  setCamera(on: boolean): void;
  /** Toggle screen share on the single video lane (camera XOR screen). Gated on
   *  `videoCapability.canScreenShare` (VP8 encode + getDisplayMedia) + the video
   *  cap. Enabling swaps the camera off; beacons `sharing` to peers. */
  setScreenShare(on: boolean): void;
  /** Re-enumerate the mic/camera/speaker options into `deviceOptions` (labels
   *  appear only after a media grant). Called when the devices menu opens. */
  refreshDevices(): void;
  /** Choose input/output devices: persist, apply to the live session, and store
   *  on `devicePrefs`. A leave/rejoin keeps the selection. */
  setDevicePrefs(prefs: DevicePrefs): void;
  /** Evict a stale huddle member (one whose beacons went silent) from the
   *  channel roster on consensus — the cleanup for a client that died without
   *  leaving. Keyed by the target's submitter identity bytes, not its node. */
  sweepHuddle(channelId: string, user: number[]): void;
  /** The live call session (audio graph + camera + ws), or null when not
   *  huddling — so video tiles can bind their canvas / preview element to it.
   *  Ephemeral and per-client, exactly like the session itself. */
  getCallSession(): CallSession | null;
  /** Pop the huddle out into its own desktop window (Tauri only). The media
   *  session HANDS OFF to that window: main releases its session (WS/mic/camera)
   *  — consensus membership untouched — and the window runs its own full video
   *  session. No-op when not in a huddle. */
  popOutHuddle(): void;
  /** Return the huddle to the in-app card: close the window and re-take the
   *  media session in the main window. Also invoked when Rust reports the window
   *  destroyed (any way it dies), so a dead float always falls back to the dock. */
  popInHuddle(): void;
  /** Re-establish the main-window media session for the huddle we are still a
   *  consensus member of (after the popped window released it). Idempotent — a
   *  no-op when a main session already exists or we are not in a huddle. */
  retakeHuddleMedia(): void;
  /** Record the popped window's current mute (it owns mute locally) so a re-take
   *  reconnects with the same mute — no consensus, no session touch (main has
   *  none while popped). */
  noteHuddleMuted(muted: boolean): void;

  commitForge(params: { path: string; content: string; message: string }): void;

  // ── Forge tracker (issues / PRs / reviews over the `forge` module) ──
  //
  // Per-screen data: the forge view owns repo selection (component-local), so
  // it calls these imperatively — loaders on open/repo switch, tracked writes
  // from its forms. The loaders land in `state.forgeItems`/`forgeBranches`
  // stamped with `state.forgeRepo`; nothing here rides the per-block refresh.

  /** Load `repo`'s issue/PR summaries into `state.forgeItems` (and stamp
   *  `state.forgeRepo`). Awaitable; a load for a repo the view has since left
   *  is dropped. */
  loadForgeItems(repo: string): Promise<void>;
  /** Load `repo`'s branch heads into `state.forgeBranches` — same stamping and
   *  staleness contract as loadForgeItems. */
  loadForgeBranches(repo: string): Promise<void>;
  /** One item in full (body, reviews, PR branches), resolved to the caller —
   *  detail is view-local (like the files browser's reads), never store state. */
  getForgeItem(repo: string, number: number): Promise<ForgeItemDetail | null>;
  /** Open an issue on `repo`; reloads the repo's item list once committed. */
  openForgeIssue(params: { repo: string; title: string; body: string }): Promise<void>;
  /** Open a PR from `sourceBranch` into `targetBranch` ("" → the repo's main). */
  openForgePr(params: {
    repo: string;
    title: string;
    body: string;
    sourceBranch: string;
    targetBranch: string;
  }): Promise<void>;
  /** Retitle/rebody an item; null leaves that field untouched. */
  editForgeItem(params: {
    repo: string;
    number: number;
    title: string | null;
    body: string | null;
  }): Promise<void>;
  /** Close (open: false) or reopen (open: true) an issue or PR. */
  setForgeItemState(params: { repo: string; number: number; open: boolean }): Promise<void>;
  /** Merge a PR: CAS against both heads, referencing a pack already staged via
   *  forge-client's uploadMergePack (`packDigest`). */
  mergeForgePr(params: {
    repo: string;
    number: number;
    prevTargetOid: string;
    expectedSourceOid: string;
    mergeOid: string;
    packDigest: string;
  }): Promise<void>;
  /** Submit a review on a PR, pinned to the source head it looked at. */
  submitForgeReview(params: {
    repo: string;
    number: number;
    verdict: ForgeReviewVerdict;
    body: string;
    commitOid: string;
    comments: ForgeReviewComment[];
  }): Promise<void>;

  // ── Docs (block-tree notebook over the `pages` module) ──
  /** Re-query the page enumeration into `state.pages`. */
  listPages(): void;
  /** Create a page (root block id minted here) and open it. */
  createPage(title: string): void;
  /** Create an untitled page (optionally nested under `parent`) and open it —
   *  the instant Notion-style new-page flow. `parent` null == top level. */
  createChildPage(parent: string | null): void;
  /** Re-nest a page under a (possibly new) parent, or to top level with null. */
  setPageParent(params: { pageId: string; parent: string | null }): void;
  /** Delete a page (root + subtree; child pages promote up). */
  deletePage(pageId: string): void;
  /** Open a page, loading its preorder block tree and comment threads. */
  openPage(pageId: string): void;
  /** Close a document tab; activates a neighbor if it was active. */
  closeTab(pageId: string): void;
  /** Insert a block into the active page. The VIEW mints the id (it drives
   *  focus to the new block, so it must know the id before the round-trip). */
  insertPageBlock(params: {
    blockId: string;
    parent: string;
    after: string | null;
    kind: PageBlockKind;
    text: string;
  }): void;
  /** Replace a block's text; on the page root this renames the page. */
  updatePageBlockText(params: { blockId: string; text: string }): void;
  /** Convert a block to another kind (markdown shortcuts, slash menu). */
  setPageBlockKind(params: { blockId: string; kind: PageBlockKind }): void;
  /** Flip a Todo block's checked state. */
  setPageBlockChecked(params: { blockId: string; checked: boolean }): void;
  /** Move a block under a (possibly new) parent in the active page. */
  movePageBlock(params: {
    blockId: string;
    parent: string;
    after: string | null;
  }): void;
  /** Remove a block and its whole subtree. */
  removePageBlock(blockId: string): void;

  // ── Comments (threads over the `comments` module) ──
  /** Load the open page's comment threads (page + every visible block). */
  loadPageThreads(): void;
  /** Add a comment: opens a new thread when `threadId` is omitted (a fresh id
   *  is minted), else appends to that thread. `target` is a block or page id. */
  addComment(params: { threadId?: string; target: string; text: string }): void;
  /** Edit own comment text. */
  editComment(params: { commentId: string; text: string }): void;
  /** Tombstone own comment (removes the thread if it was the last live one). */
  deleteComment(commentId: string): void;
  /** Toggle a thread's resolved state. */
  resolveThread(params: { threadId: string; resolved: boolean }): void;

  // ── Agents (collaboration loop over the `agent` module) ──
  /** Upload the prompt text to the blob store, then RegisterAgent with the
   *  resulting 32-byte digest as its prompt_hash. */
  registerAgent(params: {
    displayName: string;
    agentId: string;
    capability: string;
    prompt: string;
    allowedActions: string[];
    caps?: agentClient.ResourceCaps;
  }): void;
  /** Pause / resume an agent (owner-gated). */
  pauseAgent(agentId: string): void;
  resumeAgent(agentId: string): void;
  /** Watch a channel under a turn policy / drop the watch. */
  watchChannel(params: { channelId: string; policy: TurnPolicy }): void;
  unwatchChannel(channelId: string): void;
  /** Explicitly run an agent against a channel anchor. */
  requestRun(params: { agentId: string; channelId: string; anchorSeq: number }): void;
  /** Cancel an awaiting run (run-creator or owner only). */
  cancelRun(runId: string): void;
  /** Owner-gated edit of a registered agent. A provided `prompt` is staged in
   *  the blob store and its digest committed as the new prompt_hash; every
   *  omitted field keeps its current value. */
  updateAgent(params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    prompt?: string;
    allowedActions?: string[];
    /** REPLACES the whole caps record when present; omit to keep it. */
    caps?: agentClient.ResourceCaps;
  }): void;
  /** Opt the agent MODULE into (or out of) jobs-board work notifications, so it
   *  can process job-backed runs. */
  enableJobWorker(enabled: boolean): void;

  // ── Governance (proposals + votes over the `governance` module) ──
  /** Open a binding Signal proposal (no on-chain effect beyond its outcome).
   *  Membership-gated by the module: only a current validator can propose. */
  proposeSignal(text: string): void;
  /** Cast (or change) this node's ballot on an open proposal. */
  voteProposal(proposalId: string, approve: boolean): void;
  /** Tally and settle a decidable proposal (anyone may trigger it). */
  executeProposal(proposalId: string): void;

  // ── Search (cross-module, over the node's derived-index views) ──
  /** Search chat + docs with one text: the two modules' materialized views
   *  fan out concurrently and land grouped in `state.search`. A node without
   *  the index tier contributes empty groups rather than failing the search. */
  runSearch(text: string): void;
  /** Drop the last search's results. */
  clearSearch(): void;
  /** Open / close the ⌘K command-palette search overlay. */
  openSearch(): void;
  closeSearch(): void;

  // ── Chat tags (the chat index's derived #tag view) ──
  /** Filter the active channel by a #tag (leading `#` optional — the display
   *  form as clicked): runs the chat index's tagSearch and shows its hits
   *  instead of the live message slice until cleared. Channel-scoped, so a
   *  channel switch clears it. */
  setTagFilter(tag: string): void;
  /** Drop the tag filter and return to the live view. */
  clearTagFilter(): void;
  /** Load the active channel's tag catalog into `state.channelTags` for the
   *  header's tag dropdown. Best-effort: a node without the index tier just
   *  leaves the list empty. */
  loadChannelTags(): void;

  // ── Files (duckfs) ──
  // The files browser drives duckfs reads/writes directly off the live
  // transport (context.transport) via domain/files-client — no store action,
  // like the forge browser. The per-block Find projection into `state.files`
  // (DucktapeProvider) is all the store keeps, feeding the command palette.

  /** Ask the managed daemon to exit (desktop only). */
  stopNode(): void;
  /** Re-spawn / re-adopt the managed daemon after a stop (desktop only). */
  startNode(): void;
  /** Retry connecting the SAME workspace after a boot failure (from the
   *  "Node failed to start" surface). Idempotent — re-runs connectActive
   *  against the existing workspace, never re-minting one. */
  retryConnect(): void;
  /** Scrape + parse the node's `/metrics`. Null when no node is resolved or the
   *  scrape fails — best-effort, for the poll-driven Metrics view. */
  readMetrics(): Promise<NodeMetrics | null>;
  /** The active workspace's `daemon.log` path + last 64 KB — polled by the
   *  Node → Logs tab. Null when there is no managed workspace or the read
   *  fails (node stopped mid-view); the viewer keeps its last good frame. */
  readDaemonLog(): Promise<ws.LogTail | null>;
  /** The active managed node's runtime facts (pid, uptime, binary, paths) for
   *  the Logs tab's facts row. Null when unmanaged or the read fails. */
  readRuntimeFacts(): Promise<ws.RuntimeFacts | null>;
  dismissError(): void;

  // ── Onboarding / workspaces (desktop only) ──
  /** Found a new network and connect to it. */
  createWorkspace(name: string): void;
  /** Join an existing network from an invite blob, then park until admitted. */
  joinWorkspace(name: string, blob: string): void;
  /** Switch the active workspace (spawns/adopts its node). */
  selectWorkspace(id: string): void;
  /** Connect to a node running on another device over http/https. Unmanaged —
   *  we only dial it; the url is remembered and reconnected on next launch. */
  connectRemote(url: string): void;
  /** Fetch the active workspace's invite blob into state for sharing. */
  revealInvite(): void;
  /** Admit a joiner by pubkey through the active (member) workspace — grants
   *  RESIDENT standing (staged admission's first step); promote seats it. */
  admitMember(pubkey: string): void;
  /** Promote a resident into the consensus quorum by pubkey — staged
   *  admission's second step, once the resident's node is warm. */
  promoteMember(pubkey: string): void;
  /** Revoke a key's resident standing — the undo of admitMember; its node
   *  parks again and another admit re-grants. */
  removeResident(pubkey: string): void;
  /** Open a removal proposal for a validator by pubkey and cast this node's
   *  yes-ballot; the removal takes effect only once a strict majority approve. */
  demoteMember(pubkey: string): void;
  /** Request to leave the active network: drive this node's on-chain
   *  self-removal (pending remaining-member approval) and KEEP THE NODE RUNNING.
   *  The node must stay up through its own pending removal or quorum can't
   *  finalize it. On success the roster re-query shows the removal is pending;
   *  once approved and this node drops out of the valset, use `forgetWorkspace`. */
  requestLeaveWorkspace(): void;
  /** Forget the active workspace: stop its node, delete its directory + registry
   *  entry, then switch to another workspace or open the onboarding gate. Guarded
   *  in the backend — refused while this node is still a current validator of a
   *  set of two-or-more (that would halt quorum). When the guarded attempt can't
   *  confirm the node left (it's down/bricked), `state.forgetNeedsForce` flips on;
   *  call again with `force` to override that uncertainty (the backend still
   *  refuses to force-tear-down a reachable, provably-live multi-member node). */
  forgetWorkspace(force?: boolean): void;
  /** Delete a workspace BY ID from the picker: stop its node and remove its
   *  directory + registry entry (the same guarded backend as forgetWorkspace,
   *  so a live multi-member validator is refused). Deleting the active
   *  workspace tears down and falls back like forgetWorkspace; deleting any
   *  other only drops it from the list. A refused delete that couldn't confirm
   *  the node left its valset flags `state.deleteNeedsForce` with this id —
   *  call again with `force` to override that uncertainty. */
  deleteWorkspace(id: string, force?: boolean): void;
  /** Open the onboarding gate to add or switch workspaces (keeps the active
   *  one running underneath). */
  newWorkspace(): void;
  /** Close the gate without changing workspaces (only if one is active). */
  dismissOnboarding(): void;
}

interface InternalActions {
  connectActive(target: Workspace): Promise<void>;
}

interface CreateActionsDeps {
  dispatch: Dispatch<Action>;
  getState: () => ConsoleState;
  getNode: () => NodeTransport | null;
  setNode(node: NodeTransport | null): void;
  refresh(): Promise<void>;
  fail(err: unknown): void;
  nextBootGeneration(): number;
  isBootGenerationStale(generation: number): boolean;
}

export function createActions({
  dispatch,
  getState,
  getNode,
  setNode,
  refresh,
  fail,
  nextBootGeneration,
  isBootGenerationStale,
}: CreateActionsDeps): ConsoleActions & InternalActions {
  const patch = (p: Partial<ConsoleState>) => dispatch({ type: "patch", patch: p });
  const update = (fn: (state: ConsoleState) => Partial<ConsoleState>) =>
    dispatch({ type: "update", fn });

  /** The live transport + active workspace the account writes sign against.
   *  Throws when disconnected — callers wrap it in a promise chain, so the
   *  rejection lands in the Account view's inline error slot. */
  const accountDeps = (): AccountOpsDeps => {
    const live = getNode();
    const { workspace } = getState();
    if (!live || !workspace) throw new Error("not connected to a workspace node");
    return { transport: live, chainId: workspace.chainId, nodePub: workspace.pubkey };
  };

  // Monotonic token gating the async search fan-out: each runSearch/clearSearch
  // bumps it, and a resolving fan-out only writes results if its token is still
  // current — so a slow or out-of-order response can never clobber a newer
  // query's results (or repopulate a cleared palette).
  let searchToken = 0;

  // The tag filter's own token, same discipline: setTagFilter/clearTagFilter
  // (and a channel switch) bump it so a stale tagSearch can't land.
  let tagToken = 0;

  // The live call session (the browser audio graph + camera + ws), or null when
  // not in a huddle. Ephemeral and per-client — it lives here, not in state;
  // the `voice` slice mirrors only its status + camera/peer beacons for the ui.
  let voice: CallSession | null = null;

  // Resolve real VP8 encode/decode support ONCE (isConfigSupported is async, and
  // API presence lies on WebKitGTK). Until it lands, videoCapability stays
  // {false,false} so the camera toggle never appears as a dead control.
  void probeVideoCapability()
    .then((capability) => patch({ videoCapability: capability }))
    .catch(() => {}); // stay at {false,false} — no camera — on any probe failure

  /** Our own node key hex — the fan-out set excludes it. Empty on a daemon
   *  that can't do voice. */
  const selfNodeHex = (): string => getState().status?.publicKey ?? "";

  const setNotifyPrefs = (prefs: NotifyPrefs): void => {
    saveNotifyPrefs(prefs);
    patch({ notifyPrefs: prefs });
  };

  // The last fan-out set pushed into the live session — refresh() lands a new
  // channels array every block, so pushes are deduped by value here rather
  // than by effect identity upstream.
  let lastRecipients: string | null = null;
  // Membership reconciliation bookkeeping: whether the FINALIZED roster has
  // carried our node at least once this membership — only then does a roster
  // without us mean "we were removed" rather than "the join hasn't landed yet".
  let huddleSelfSeen = false;
  // Auto-reconnect damping: one re-establish per window. A second unexpected
  // close inside the window (flapping network, or another client of this node
  // taking the session — a fight we must not enter) fails honestly instead.
  const RECONNECT_DAMP_MS = 30_000;
  let lastReconnectAtMs = 0;
  // The transient media-failure note auto-clears; keep the timer so a newer
  // note supersedes an older one cleanly.
  let mediaNoteTimer: ReturnType<typeof setTimeout> | null = null;

  /** Recompute + push the fan-out set for `channelId` (default: the active
   *  huddle) into the live session. No-op when not huddling or unchanged. */
  const pushRecipients = (channelId = getState().voice.channelId): void => {
    if (!voice || !channelId) return;
    const channel = getState().channels.find((c) => c.id === channelId);
    const recipients = huddleRecipients(channel?.huddle ?? [], selfNodeHex());
    const fingerprint = recipients.join(",");
    if (fingerprint === lastRecipients) return;
    lastRecipients = fingerprint;
    voice.setRecipients(recipients);
  };

  /** Stop + drop the live audio session (no consensus write). Idempotent. */
  const stopVoice = (): void => {
    voice?.stop();
    voice = null;
    lastRecipients = null;
  };

  /** Reconcile our session against the FINALIZED roster: once the roster has
   *  carried our node and then drops it while the session runs (a sweep by
   *  another member, or another client of this identity leaving), end the media
   *  and say so — the server keeps mixing audio for any authenticated member
   *  regardless of roster, so without this a removed participant keeps hearing
   *  the huddle behind a card that shows nobody in it. Runs on every channel
   *  refresh (beside the recipients push). */
  const reconcileHuddleMembership = (): void => {
    const state = getState();
    const { channelId, status } = state.voice;
    const active = status === "live" || status === "connecting" || status === "reconnecting";
    if (!channelId || !active) {
      huddleSelfSeen = false;
      return;
    }
    const channel = state.channels.find((c) => c.id === channelId);
    if (!channel) return; // channel list mid-refresh — never treat as removal.
    const self = selfNodeHex();
    const inRoster = (channel.huddle ?? []).some((m) => keyHex(m.node) === self);
    if (inRoster) {
      huddleSelfSeen = true;
      return;
    }
    // Not in the roster: before the join lands that's expected (the optimistic
    // projection usually covers even that); after we've been seen, it's removal.
    if (!huddleSelfSeen) return;
    huddleSelfSeen = false;
    stopVoice();
    closeHuddleWindow();
    update((prev) => ({
      voice: {
        ...prev.voice,
        popped: false,
        status: "error",
        error: "removed",
        mediaNote: null,
        cameraOn: false,
        sharing: false,
        peers: {},
        speaking: false,
      },
    }));
  };

  /** Build + start a fresh media session for `channelId` (consensus membership
   *  untouched) and put the slice in `status`. The shared core of the pop-in
   *  retake and the auto-reconnect — a CallSession can never restart, so any
   *  re-establish is a new instance. Camera resets off (a stream cannot survive
   *  its session); mute is carried for call continuity. */
  const startHuddleMedia = (
    channelId: string,
    seedMuted: boolean,
    status: "connecting" | "reconnecting",
  ): void => {
    const nodeUrl = getState().nodeUrl;
    if (voice || !nodeUrl) return;
    voice = createCallSession(onCallEvent);
    voice.setMuted(seedMuted);
    voice.setDevices(getState().devicePrefs); // start() reads these at acquire
    update((prev) => ({
      voice: {
        ...prev.voice,
        popped: false,
        muted: seedMuted,
        status,
        error: null,
        mediaNote: null,
        cameraOn: false,
        sharing: false,
        peers: {},
        sessionStartMs: Date.now(),
        speaking: false,
      },
    }));
    voice.start(callSocketUrl(nodeUrl, channelId));
    pushRecipients(channelId);
  };

  // Session events → the voice slice. A `peerBeacon` merges that peer's latest
  // ephemeral call state (keyed by its already-lowercase node hex) into the
  // slice. A `status` event drives lifecycle:
  //   - 'closed' on a LIVE session (socket drop, node restart, session replaced)
  //     gets ONE automatic media re-establish — membership is kept, the slice
  //     shows "reconnecting". Damped by RECONNECT_DAMP_MS so a flapping link (or
  //     another client of this node repeatedly taking the session) converges on
  //     a visible failure instead of a silent steal loop.
  //   - any other terminal end ('error', or 'closed' when not live / inside the
  //     damp window) reconciles the consensus roster (submit leave) so peers
  //     never keep showing a dead participant, and keeps the dock up in its
  //     error state — a huddle must end visibly, never vanish.
  // Leave dismisses the error card.
  const onCallEvent = (event: CallEvent): void => {
    if (event.kind === "peerBeacon") {
      update((prev) => ({
        voice: {
          ...prev.voice,
          peers: {
            ...prev.voice.peers,
            [event.peer]: { muted: event.muted, cameraOn: event.cameraOn, sharing: event.sharing, atMs: event.atMs },
          },
        },
      }));
      return;
    }
    if (event.kind === "selfVideo") {
      // Authoritative lane state — corrects a failed acquire / encoder death /
      // the browser's native "Stop sharing" that the optimistic action missed.
      update((prev) => ({ voice: { ...prev.voice, cameraOn: event.cameraOn, sharing: event.sharing } }));
      return;
    }
    if (event.kind === "selfSpeaking") {
      update((prev) => ({ voice: { ...prev.voice, speaking: event.speaking } }));
      return;
    }
    if (event.kind === "selfLevel") {
      update((prev) => ({ voice: { ...prev.voice, level: event.level } }));
      return;
    }
    if (event.kind === "mediaNote") {
      if (mediaNoteTimer !== null) clearTimeout(mediaNoteTimer);
      update((prev) => ({ voice: { ...prev.voice, mediaNote: event.note } }));
      mediaNoteTimer = setTimeout(() => {
        mediaNoteTimer = null;
        update((prev) => ({ voice: { ...prev.voice, mediaNote: null } }));
      }, 5_000);
      return;
    }
    const status = event.status;
    const error = event.error;
    if (status === "closed" || status === "error") {
      const prevVoice = getState().voice;
      const channelId = prevVoice.channelId;
      stopVoice();
      if (
        status === "closed" &&
        channelId &&
        prevVoice.status === "live" &&
        Date.now() - lastReconnectAtMs > RECONNECT_DAMP_MS
      ) {
        lastReconnectAtMs = Date.now();
        startHuddleMedia(channelId, prevVoice.muted, "reconnecting");
        return;
      }
      if (channelId) submitLeaveHuddle(channelId);
      closeHuddleWindow();
      update((prev) => ({
        voice: {
          ...prev.voice,
          popped: false,
          status: "error",
          error: error ?? "connection",
          mediaNote: null,
          cameraOn: false,
          sharing: false,
          peers: {},
          speaking: false,
        },
      }));
      return;
    }
    update((prev) => ({
      voice: {
        ...prev.voice,
        // A re-establish's own session reports 'connecting' — keep the visible
        // "reconnecting" until it actually lands ('live' promotes both).
        status:
          prev.voice.status === "reconnecting" && status === "connecting"
            ? "reconnecting"
            : status,
        error: null,
      },
    }));
  };

  /** Submit a leave_huddle for `channelId` with the optimistic roster prune. */
  const submitLeaveHuddle = (channelId: string) =>
    submitTracked(
      opKey.huddle(channelId),
      (live) => chatClient.leaveHuddle(live, { channelId, origin: getState().author }),
      (prev) => optimistic.huddleLeft(prev, channelId, selfNodeHex()),
    );

  // Re-establish the main-window media session for a huddle we are still a
  // consensus member of, after the popped window released it. Idempotent — a
  // no-op (just clears `popped`) when a session already exists or we are not in
  // a huddle. A fresh session (never restart a stopped one). This is a media
  // reconnect, NOT a re-join, so it PRESERVES the current mute (the window
  // reported its mute via `noteHuddleMuted`) for call continuity; camera resets
  // off (the stream can't transfer between webviews). Consensus is intact.
  const retakeHuddleMedia = (): void => {
    const state = getState();
    const channelId = state.voice.channelId;
    if (voice || !channelId || !state.nodeUrl) {
      update((prev) => ({ voice: { ...prev.voice, popped: false } }));
      return;
    }
    startHuddleMedia(channelId, state.voice.muted, "connecting");
  };

  // The one write path: apply the op's PRECONFIRMED render immediately (the
  // optimistic projection plus a pending ledger record under the entity's
  // key), submit, then settle the record from the node's receipt — finalized
  // with the inclusion height + addressable op hash, or failed. Committed
  // truth replaces the projection on the refresh that follows either way (a
  // failed submit's refresh is the rollback).
  const submitTracked = (
    key: string,
    submit: (live: NodeTransport) => Promise<unknown>,
    preconfirm?: (prev: ConsoleState) => Partial<ConsoleState>,
  ) => {
    const live = getNode();
    if (!live) return Promise.resolve();
    const startedAt = Date.now();
    update((prev) => ({
      ...(preconfirm ? preconfirm(prev) : {}),
      ops: beginOp(prev.ops, key, startedAt),
    }));
    return Promise.resolve()
      .then(() => submit(live))
      .then((result) => {
        update((prev) => ({ ops: finalizeOp(prev.ops, key, receiptOf(result)) }));
        return refresh();
      })
      .catch((err) => {
        update((prev) => ({ ops: failOp(prev.ops, key, String(err)) }));
        fail(err);
        return refresh();
      });
  };

  // switching channels means: new active channel, thread panel closed, any
  // channel-scoped tag filter/catalog dropped, and THAT channel's messages
  // loaded — every path into a channel goes here
  const enterChannel = (channelId: string) => {
    const live = getNode();
    if (!live) return;
    tagToken += 1; // supersede any in-flight tagSearch so it can't repopulate
    patch({
      activeChannel: channelId,
      activeThread: null,
      tagFilter: null,
      tagHits: [],
      tagHitsPending: false,
      channelTags: [],
    });
    Promise.resolve()
      .then(() => chatClient.latestMessages(live, channelId))
      .then((messages) => patch({ messages }))
      .catch(fail);
  };

  // A first agent mention in an UNWATCHED channel creates the runs watch the
  // engagement pipeline requires (policy "mention") and awaits its ack BEFORE
  // the post — otherwise the mention commits with nothing routing it to the
  // agent. An existing watch of ANY policy is respected, never overwritten.
  const ensureMentionWatch = (channelId: string, blocks: ChatBlock[]): Promise<unknown> => {
    if (!hasAgentMention(blocks)) return Promise.resolve();
    if (getState().watches.some((watch) => watch.channel_id === channelId))
      return Promise.resolve();
    return submitTracked(
      opKey.watch(channelId),
      (live) =>
        runsClient.watchChannel(live, {
          channelId,
          policy: "mention",
          origin: getState().author,
        }),
      (prev) => optimistic.watchSet(prev, { channelId, policy: "mention" }),
    );
  };

  const mentionResolver = () => {
    const state = getState();
    return mentionResolverOf(
      state.agents,
      mentionableUsers(state.nodeUsers, state.agents),
    );
  };

  // THE channel-parameterized post every composer shares: parse mentions with
  // the live resolver, arm the first-mention watch, then submit the tracked
  // post. `sendMessage`/`replyInThread` route the ACTIVE channel through it;
  // the forge item Discussion posts into its hidden channel via
  // `postInChannel`. The optimistic patch self-guards on the active channel
  // (`optimistic.postedMessage` returns {} for a background channel).
  const postToChannel = (
    channelId: string,
    body: string,
    thread: number | null,
  ): Promise<unknown> => {
    const messageId = crypto.randomUUID();
    const blocks = parseMessageInput(body, mentionResolver());
    const author = getState().author;
    return ensureMentionWatch(channelId, blocks).then(() =>
      submitTracked(
        opKey.message(channelId, messageId),
        (live) =>
          chatClient.postMessage(live, {
            channelId,
            messageId,
            blocks,
            origin: author,
            ...(thread !== null ? { thread } : {}),
          }),
        (prev) =>
          optimistic.postedMessage(prev, {
            channelId,
            messageId,
            blocks,
            authorBytes: selfAuthorBytes(prev.status, prev.author),
            at: Date.now(),
            thread,
          }),
      ),
    );
  };

  // Re-pull the open thread's own ChatThread snapshot after a write that may
  // have touched the root or a reply. `submitTracked` already refreshed the
  // flat `state.messages` (every sequence, replies included), but the thread
  // panel reads a separate snapshot, so it needs this extra cheap re-query.
  const resyncOpenThread = (): Promise<void> => {
    const live = getNode();
    const channelId = getState().activeChannel;
    const root = getState().activeThread?.root;
    if (!live || !channelId || !root) return Promise.resolve();
    return chatClient
      .thread(live, { channelId, rootSeq: root.seq })
      .then((activeThread) =>
        update((prev) => (prev.activeThread?.root.seq === root.seq ? { activeThread } : {})),
      )
      .catch(fail);
  };

  // load the comment threads for the open page (the page id + every visible
  // block id) in one batch; refreshed on open and after any comment op.
  const loadPageThreads = (blocksOverride?: PageBlock[]): Promise<void> => {
    // `blocks` is passed by callers that JUST fetched the tree, because
    // getState().activePageBlocks lags a dispatch (stateRef updates on render);
    // reading it here would ship only the page target and miss every block.
    const live = getNode();
    const page = getState().activePage;
    if (!live || !page) {
      patch({ pageThreads: [] });
      return Promise.resolve();
    }
    const blocks = blocksOverride ?? getState().activePageBlocks;
    // dedupe: the tree's root block carries the page's own id, so naively
    // prepending `page` requests it twice — and the module answers with one
    // group PER REQUESTED target, duplicating every page-level thread in the
    // panel.
    const targets = [...new Set([page, ...blocks.map((b) => b.id)])];
    // the module rejects a ThreadsForTargets over MAX_QUERY_TARGETS (512), so a
    // large page must chunk its targets across several queries.
    const CHUNK = 512;
    const batches: string[][] = [];
    for (let i = 0; i < targets.length; i += CHUNK) batches.push(targets.slice(i, i + CHUNK));
    return Promise.all(batches.map((b) => pagesClient.threadsForTargets(live, { targets: b })))
      .then((results) => patch({ pageThreads: results.flat() }))
      .catch(fail);
  };

  // load the active page's block tree + its comment threads (threads keyed off
  // the freshly-fetched blocks, not the lagging store copy). shared by every
  // activation path; does NOT touch the tab list.
  const loadActivePage = (pageId: string) => {
    const live = getNode();
    if (!live) return;
    Promise.resolve()
      .then(() => pagesClient.getPage(live, pageId))
      .then((blocks) => {
        patch({ activePageBlocks: blocks ?? [] });
        return loadPageThreads(blocks ?? []);
      })
      .catch(fail);
  };

  // the single entry point into a page: make it active (opening a tab), then
  // load its tree + threads — every path into a page (new-page, a rail click,
  // a tab click) goes here.
  const enterPage = (pageId: string) => {
    const live = getNode();
    if (!live || !pageId) return;
    const tabs = addTab(getState().openTabs, pageId);
    saveDocTabs(tabs);
    patch({
      activePage: pageId,
      activePageBlocks: [],
      openTabs: tabs,
      pageThreads: [],
    });
    loadActivePage(pageId);
  };

  // close a document tab; if it was active, activate a neighbor (loading its
  // tree) so the editor never lands on a closed page. Activation here patches
  // the already-reduced tab list directly — it must NOT go through enterPage,
  // whose addTab(getState().openTabs, …) reads the stale pre-close list and
  // would re-stage the just-closed id.
  const closeTabLocal = (pageId: string) => {
    const { tabs, active } = removeTab(getState().openTabs, getState().activePage, pageId);
    saveDocTabs(tabs);
    if (active && active !== getState().activePage) {
      patch({ openTabs: tabs, activePage: active, activePageBlocks: [], pageThreads: [] });
      loadActivePage(active);
      return;
    }
    patch({
      openTabs: tabs,
      activePage: active,
      ...(active ? {} : { activePageBlocks: [], pageThreads: [] }),
    });
  };

  // ── Forge tracker loaders ──
  // Per-screen data (never in the per-block refresh): the forge view calls
  // these on open/repo switch. Stamp the repo synchronously — clearing the
  // previous repo's slices on a switch — then land the fetch ONLY while the
  // stamp still matches, so a slow load for a repo the view has since left
  // can never clobber the current one (loadChannelTags' guard, repo-keyed).
  const enterForgeRepo = (repo: string): void =>
    update((prev) =>
      prev.forgeRepo === repo
        ? {}
        : { forgeRepo: repo, forgeItems: [], forgeBranches: [] },
    );

  const loadForgeItems = (repo: string): Promise<void> => {
    const live = getNode();
    if (!live || !repo) return Promise.resolve();
    enterForgeRepo(repo);
    return forgeClient
      .listItems(live, repo)
      .then((items) =>
        update((prev) => (prev.forgeRepo === repo ? { forgeItems: items } : {})),
      )
      .catch(fail);
  };

  const loadForgeBranches = (repo: string): Promise<void> => {
    const live = getNode();
    if (!live || !repo) return Promise.resolve();
    enterForgeRepo(repo);
    return forgeClient
      .listRefs(live, repo)
      .then((refs) =>
        update((prev) => (prev.forgeRepo === repo ? { forgeBranches: refs } : {})),
      )
      .catch(fail);
  };

  // Connect the app to a workspace's node: select it (Rust spawns/adopts),
  // then either wait for a member's surface to answer, or poll a joiner's
  // park→promote phase until its promoted validator surface comes up.
  const connectActive = (target: Workspace): Promise<void> => {
    const gen = nextBootGeneration();
    const stale = () => isBootGenerationStale(gen);
    // Choosing a local workspace supersedes any remembered remote — it becomes
    // what we reconnect to on next launch.
    clearRemoteUrl();
    // Tear the old node down BEFORE clearing its projections, so its block
    // subscription can't fire a straggler frame into the just-cleared state
    // during the async selectWorkspace/waitUntilUp window that follows
    // (mirrors connectRemote). Without this teardown-first order, an old-node
    // block would re-set lastBlock right after the clear — the exact
    // staleness this clear prevents.
    setNode(null);
    patch({
      workspace: target,
      needsOnboarding: false,
      onboardingBusy: false,
      // a force-forget/delete offer is scoped to the workspace it was raised
      // for; switching targets clears it so it can never fire on the wrong one.
      forgetNeedsForce: false,
      deleteNeedsForce: null,
      inviteBlob: null,
      // a fresh connect/retry starts from a clean slate — clear any prior boot
      // failure, mid-session-down banner, and error so a stale reason can't
      // linger over the new attempt.
      bootError: null,
      connectionDown: null,
      error: null,
      // per-node observability belonging to the workspace we're leaving; the
      // node effect re-hydrates blocks and re-follows the block stream once
      // the new node is set below.
      lastBlock: null,
      blocks: [],
      // a non-member target parks first: seed the waiting-room phase NOW so
      // the console shell (still holding the previous workspace's residual
      // projections) can never flash during the async select/poll below.
      onboardingPhase: target.member ? null : { phase: "starting", detail: null },
    });
    // Adopt the answering node ONLY once it proves it is THIS workspace's
    // node: /v1/status carries the node's identity key and the registry
    // records the workspace's. A recycled port can be held by something else
    // (say, a zombie node of a forgotten workspace) — adopting that would
    // silently open another workspace's data. An absent key on either side
    // (an older node build) trusts the port, as before.
    const identityMatches = (got: string | undefined): boolean =>
      !got || !target.pubkey || got.toLowerCase() === target.pubkey.toLowerCase();
    const rejectImpostor = (): void => {
      patch({
        workspace: null,
        nodeUrl: null,
        managed: false,
        needsOnboarding: true,
        onboardingPhase: null,
        onboardingBusy: false,
      });
      fail(
        `the process answering on "${target.name}"'s node port reports a ` +
          `different node identity — not connecting. Another node is likely ` +
          `still running on this port; quit it and try again.`,
      );
    };
    // Adopt a node that just proved it's this workspace's own: clear the
    // onboarding phase, hand it to the store, and — best-effort, desktop-only
    // — offer this machine's user key to bind it (Task 8). Fire-and-forget:
    // a failed bind is invisible here by design (auto-bind.ts never throws)
    // and the provider's per-block refresh already re-reads the identity
    // module, so a successful bind surfaces on its own on the next block.
    const adopt = (transport: NodeTransport): void => {
      patch({ onboardingPhase: null });
      setNode(transport);
      autoBindUserIdentity(transport, target)
        .then((outcome) => {
          // First-run hand-off: the name chosen while creating the account
          // parks in localStorage (names are chain-scoped) and lands here, on
          // the first adopted node. When the bind landed, the name belongs on
          // the ACCOUNT (identity SetAccountName) so it travels with the
          // person across devices. An unbound outcome leaves it parked for the
          // next connect; there is no second per-node name registry.
          const pending = loadPendingDisplayName();
          if (!pending) return;
          patch({ author: pending });
          if (outcome !== "bound" && outcome !== "already") return;
          return identityClient
            .setAccountName(transport, {
              displayName: pending,
              origin: pending,
            })
            .then(() => clearPendingDisplayName())
            .catch(() => {});
        })
        .catch(() => {});
    };
    return Promise.resolve()
      .then(() => ws.selectWorkspace(target.id))
      .then((sel) => {
        if (stale()) return;
        patch({ nodeUrl: sel.httpUrl, managed: true });
        const transport = bootstrap.connectWorkspace(sel.httpUrl).transport;
        if (target.member) {
          // founder / already-admitted member: the surface comes up promptly.
          return bootstrap.waitUntilUp(transport).then(() => {
            if (stale()) return;
            return transport.status().then((s) => {
              if (stale()) return;
              if (!identityMatches(s.publicKey)) return rejectImpostor();
              adopt(transport);
            });
          });
        }
        // joiner: the node parks until a member admits it and the epoch cuts
        // over. It may stop at resident standing (mesh + statesync, no quorum
        // seat) or later promote into the validator set. NOTE a parked joiner
        // may serve its http surface — a mere status answer is NOT admission,
        // so adoption additionally requires OUR key in either committed valset
        // tier.
        const park = (): Promise<void> =>
          ws.workspacePhase(target.id).then((report) => {
            if (stale()) return;
            patch({ onboardingPhase: report });
            if (report.phase === "fatal") {
              fail(report.detail ?? "the node failed to join");
              return;
            }
            return wait(JOIN_POLL_MS).then(tick);
          });
        const tick = (): Promise<void> => {
          if (stale()) return Promise.resolve();
          return transport.status().then(
            (s) => {
              if (stale()) return;
              if (!identityMatches(s.publicKey)) return rejectImpostor();
              return hasValsetStanding(transport, target.pubkey).then((seated) => {
                if (stale()) return;
                if (!seated) return park();
                adopt(transport);
              });
            },
            () => park(),
          );
        };
        return tick();
      })
      .catch((err) => {
        if (stale()) return;
        patch({ onboardingBusy: false });
        const reason =
          typeof err === "object" && err && "message" in err
            ? String((err as { message?: unknown }).message)
            : String(err);
        // Best-effort: pull the node's daemon.log so even a plain "did not come
        // up" boot timeout carries the real reason the node wrote to disk (bind
        // conflict, bad config, panic) — the file nothing in the UI used to read.
        Promise.resolve()
          .then(() => ws.workspaceLogTail(target.id))
          .then(
            (log): { path: string | null; tail: string } => ({ path: log.path, tail: log.tail }),
            () => ({ path: null, tail: "" }),
          )
          .then((log) => {
            if (stale()) return;
            if (target.member) {
              // member/founder: route to the dedicated "Node failed to start"
              // body with the reason, the log, and an idempotent Retry — never
              // a hollow disconnected console whose toast then vanishes.
              patch({
                bootError: {
                  workspaceId: target.id,
                  reason,
                  logPath: log.path,
                  logTail: log.tail,
                },
              });
            } else {
              // joiner: surface it IN the waiting room as a fatal phase instead
              // of leaving the "ask a member to approve" spinner up over a node
              // that never started.
              patch({ onboardingPhase: { phase: "fatal", detail: reason } });
            }
            fail(reason);
          });
      });
  };

  return {
    setScreen: (screen) => {
      // Navigating adopts the target surface's rail, so the sidebar highlight and
      // the body never disagree. Shell screens (settings → null section) leave the
      // current rail untouched.
      const section = sectionForScreen(screen);
      if (section) {
        saveViewMode(section);
        patch({ screen, viewMode: section });
      } else {
        patch({ screen });
      }
    },

    openExplorerAt: (height) => {
      // Same rail-adoption contract as setScreen — the explorer lives on the
      // operator rail, so the jump must move the sidebar with it.
      const section = sectionForScreen("explorer");
      if (section) {
        saveViewMode(section);
        patch({ screen: "explorer", viewMode: section, explorerFocus: height });
      } else {
        patch({ screen: "explorer", explorerFocus: height });
      }
    },

    clearExplorerFocus: () => {
      patch({ explorerFocus: null });
    },

    setViewMode: (mode) => {
      saveViewMode(mode);
      update((prev) => {
        // Keep the body on the chosen rail: if the current screen belongs to the
        // other rail (or is a shell screen), land on this rail's default surface.
        const screen =
          sectionForScreen(prev.screen) === mode
            ? prev.screen
            : defaultScreenForSection(mode);
        return { viewMode: mode, screen };
      });
    },

    setAccent: (accent) => {
      saveAccent(accent);
      patch({ accent });
    },

    toggleTheme: () => {
      const next = getState().theme === "dark" ? "light" : "dark";
      saveTheme(next);
      patch({ theme: next });
    },
    setNotifyPrefs,
    toggleChannelMute: (channelId) => {
      const prefs = getState().notifyPrefs;
      setNotifyPrefs({
        ...prefs,
        mutedChannels: prefs.mutedChannels.includes(channelId)
          ? prefs.mutedChannels.filter((id) => id !== channelId)
          : [...prefs.mutedChannels, channelId],
      });
    },
    setAuthor: (author) => patch({ author }),

    // ── Account writes (see account-ops.ts) ──
    // Each resolves the live transport + active workspace up front; callers
    // (the Account view) surface the rejection inline. The refresh() after a
    // landed submit re-reads the identity projections promptly instead of
    // waiting for the next block tick.
    accountLinkChallenge: () =>
      Promise.resolve().then(() => mintLinkChallenge(accountDeps())),
    accountAddMember: (challenge, responseBlob) =>
      Promise.resolve()
        .then(() => addMemberFromResponse(accountDeps(), challenge, responseBlob))
        .then(() => refresh()),
    accountRemoveMember: (targetKeyHex) =>
      Promise.resolve()
        .then(() => removeMemberKey(accountDeps(), targetKeyHex))
        .then(() => refresh()),
    accountUnbindNode: (targetNodeHex) =>
      Promise.resolve()
        .then(() => unbindNode(accountDeps(), targetNodeHex))
        .then(() => refresh()),
    accountPhoneEnrollStart: () =>
      Promise.resolve().then(() => startPhoneEnrollment(accountDeps())),
    accountPhoneEnrollApprove: (enrollment, newKeyHex, sigHex, label) =>
      Promise.resolve()
        .then(() =>
          approvePhoneEnrollment(accountDeps(), enrollment, newKeyHex, sigHex, label),
        )
        .then(() => refresh()),

    readMetrics: () => {
      const live = getNode();
      return live
        ? live.metrics().then(parseMetrics).catch(() => null)
        : Promise.resolve(null);
    },

    // The Logs tab surfaces the LOCAL daemon.log — a per-workspace file only the
    // desktop shell that spawned the node can read. Both reads are best-effort:
    // a stopped/forgotten node just yields null and the tab keeps its last frame
    // (or shows its managed-only empty state). Keyed on the active workspace id.
    readDaemonLog: () => {
      const { managed, workspace } = getState();
      if (!managed || !workspace) return Promise.resolve(null);
      return ws.workspaceLogTail(workspace.id).catch(() => null);
    },

    readRuntimeFacts: () => {
      const { managed, workspace } = getState();
      if (!managed || !workspace) return Promise.resolve(null);
      return ws.workspaceRuntimeFacts(workspace.id).catch(() => null);
    },

    // Identity is the only durable display-name authority. The module derives
    // the account from the authenticated node and rejects an unbound origin.
    setDisplayName: (name) => {
      const current = getState();
      const origin = current.author;
      submitTracked(
        opKey.accountName(),
        (live) => identityClient.setAccountName(live, { displayName: name, origin }),
        () => ({ author: name }),
      );
    },

    setDuckHandle: (handle) => {
      const current = getState();
      const nodeKey = normalizeKey(current.status?.publicKey || current.workspace?.pubkey);
      const accountId = nodeKey
        ? current.nodeUsers[nodeKey]?.accountId
        : undefined;
      if (!accountId) {
        fail("bind this node to an identity account before registering a .duck name");
        return;
      }
      submitTracked(
        opKey.duckHandle(),
        (live) => duckdnsClient.setHandle(live, { handle, origin: current.author }),
        (prev) => {
          const accountHandles = { ...prev.accountHandles };
          if (handle) accountHandles[accountId] = handle;
          else delete accountHandles[accountId];
          return { accountHandles };
        },
      );
    },

    selectChannel: enterChannel,

    createChannel: (name, postPolicy) => {
      const channelId = channelIdOf(name);
      if (!channelId) return;
      submitTracked(
        opKey.channel(channelId),
        (live) =>
          chatClient.createChannel(live, {
            channelId,
            name,
            postPolicy,
            origin: getState().author,
          }),
        (prev) =>
          optimistic.channelCreated(prev, {
            channelId,
            name,
            postPolicy,
            at: Date.now(),
          }),
      ).then(() => enterChannel(channelId));
    },

    sendMessage: (body) => {
      const channelId = getState().activeChannel;
      if (!channelId || !body.trim()) return;
      void postToChannel(channelId, body, null);
    },

    postInChannel: (channelId, body) => {
      if (!channelId || !body.trim()) return Promise.resolve();
      return postToChannel(channelId, body, null).then(() => undefined);
    },

    openThread: (rootSeq) => {
      const live = getNode();
      const channelId = getState().activeChannel;
      if (!live || !channelId) return;
      Promise.resolve()
        .then(() => chatClient.thread(live, { channelId, rootSeq }))
        .then((activeThread) => patch({ activeThread }))
        .catch(fail);
    },

    closeThread: () => patch({ activeThread: null }),

    // Re-queries just the open thread's replies after the write: `refresh()`
    // already re-pulls `state.messages` (which carries every sequence, replies
    // included) via `submitTracked`, but the thread panel reads its own
    // `ChatThread` snapshot, so that one extra cheap query keeps the panel in
    // sync without repeating the old heavy-refresh-twice pattern.
    replyInThread: (body) => {
      const channelId = getState().activeChannel;
      const root = getState().activeThread?.root;
      if (!channelId || !root || !body.trim()) return;
      void postToChannel(channelId, body, root.seq).then(() => resyncOpenThread());
    },

    editMessage: (seq, body) => {
      const channelId = getState().activeChannel;
      if (!channelId || !body.trim()) return;
      const activeThread = getState().activeThread;
      const target =
        getState().messages.find((m) => m.seq === seq) ??
        (activeThread?.root.seq === seq
          ? activeThread.root
          : activeThread?.replies.find((m) => m.seq === seq));
      // The resolver keeps an edited mention's mark intact: blocksToInput
      // seeded the editor with "@agent_id", so re-parsing must resolve it
      // back or the edit silently strips the mention. No auto-watch here —
      // engagement is a post-time concern.
      const blocks = parseMessageInput(body, mentionResolver());
      submitTracked(
        opKey.messageSeq(channelId, seq),
        (live) =>
          chatClient.editMessage(live, {
            channelId,
            seq,
            blocks,
            baseRev: target?.head.rev ?? null,
            origin: getState().author,
          }),
        (prev) => optimistic.editedMessage(prev, channelId, seq, blocks, Date.now()),
      ).then(resyncOpenThread);
    },

    deleteMessage: (seq) => {
      const channelId = getState().activeChannel;
      if (!channelId) return;
      submitTracked(
        opKey.messageSeq(channelId, seq),
        (live) =>
          chatClient.deleteMessage(live, { channelId, seq, origin: getState().author }),
        (prev) => optimistic.deletedMessage(prev, channelId, seq),
      ).then(resyncOpenThread);
    },

    toggleReaction: (seq, emoji) => {
      const channelId = getState().activeChannel;
      if (!channelId) return;
      const activeThread = getState().activeThread;
      const target =
        getState().messages.find((m) => m.seq === seq) ??
        (activeThread?.root.seq === seq
          ? activeThread.root
          : activeThread?.replies.find((m) => m.seq === seq));
      if (!target) return;
      const origin = getState().author;
      const selfBytes = Array.from(new TextEncoder().encode(origin));
      const mine = target.reactions
        .find((r) => r.emoji === emoji)
        ?.reactors.some(
          (author) =>
            typeof author === "object" &&
            "user" in author &&
            author.user.length === selfBytes.length &&
            author.user.every((byte, i) => byte === selfBytes[i]),
        );
      submitTracked(
        opKey.reaction(channelId, seq, emoji),
        (live) =>
          mine
            ? chatClient.removeReaction(live, { channelId, seq, emoji, origin })
            : chatClient.addReaction(live, { channelId, seq, emoji, origin }),
        (prev) =>
          optimistic.reactionToggled(prev, channelId, seq, emoji, selfBytes, Boolean(mine)),
      ).then(() => {
        const live = getNode();
        const root = getState().activeThread?.root;
        if (!live || !root) return;
        return chatClient
          .thread(live, { channelId, rootSeq: root.seq })
          .then((activeThread) =>
            update((prev) =>
              prev.activeThread?.root.seq === root.seq ? { activeThread } : {},
            ),
          )
          .catch(fail);
      });
    },

    // ── Huddle ──
    joinHuddle: (channelId) => {
      const state = getState();
      const publicKey = state.status?.publicKey;
      const nodeUrl = state.nodeUrl;
      // no voice identity (legacy daemon) or no resolved node → nothing to do.
      if (!publicKey || !nodeUrl || !channelId) return;
      const active = state.voice.channelId;
      // already in this huddle — unless it errored, where re-join is the retry.
      if (active === channelId && state.voice.status !== "error") return;
      // switching huddles: the server replaces the session, so leave the old on
      // consensus and stop its audio before starting the new one. An errored
      // session already left (onCallEvent reconciled the roster) — skip it.
      if (active && active !== channelId) submitLeaveHuddle(active);
      stopVoice();
      // submit the join carrying our node key bytes; optimistically add us to
      // the roster so the pill/dock react instantly.
      const node = keyBytes(publicKey);
      void submitTracked(
        opKey.huddle(channelId),
        (live) => chatClient.joinHuddle(live, { channelId, node, origin: getState().author }),
        (prev) =>
          optimistic.huddleJoined(prev, {
            channelId,
            node,
            authorBytes: selfAuthorBytes(prev.status, prev.author),
            at: Math.floor(Date.now() / 1000),
          }),
      ).then(() => {
        // consensus refused the join (members-only, roster full): the audio
        // session must not keep streaming into a huddle we are not in.
        const settled = getState();
        if (
          settled.ops[opKey.huddle(channelId)]?.phase === "failed" &&
          settled.voice.channelId === channelId
        ) {
          stopVoice();
          // the session is gone — camera/beacon state must not outlive it.
          update((prev) => ({
            voice: { ...prev.voice, status: "error", error: "refused", cameraOn: false, sharing: false, peers: {} },
          }));
        }
      });
      // start the audio session and reflect "connecting"; push whatever roster
      // we already know (others may be huddling), self excluded. joins start
      // MUTED — joining a room must never be a hot-mic moment; unmuting is the
      // deliberate act. Deliberately NOT startHuddleMedia: a join differs from
      // a media re-establish (it sets channelId, forces muted, and must keep
      // `popped` for a retry from the popped window) — folding them would break
      // one or the other.
      // A fresh membership: reset the reconcile + reconnect bookkeeping so an
      // old session's history never bleeds into this one.
      huddleSelfSeen = false;
      lastReconnectAtMs = 0;
      voice = createCallSession(onCallEvent);
      voice.setMuted(true);
      voice.setDevices(getState().devicePrefs); // start() reads these at acquire
      // a retry from the popped window must keep it popped — spread, don't reset;
      // camera/peer state resets since this is a fresh session.
      update((prev) => ({
        voice: {
          ...prev.voice,
          channelId,
          muted: true,
          status: "connecting",
          error: null,
          mediaNote: null,
          cameraOn: false,
          sharing: false,
          peers: {},
          // Fresh session → fresh staleness baseline (a retry replaces the session).
          sessionStartMs: Date.now(),
          speaking: false,
        },
      }));
      voice.start(callSocketUrl(nodeUrl, channelId));
      pushRecipients(channelId);
    },

    leaveHuddle: () => {
      const channelId = getState().voice.channelId;
      huddleSelfSeen = false;
      stopVoice();
      closeHuddleWindow();
      patch({
        voice: {
          channelId: null,
          muted: false,
          status: "idle",
          error: null,
          mediaNote: null,
          popped: false,
          cameraOn: false,
          sharing: false,
          peers: {},
          sessionStartMs: null,
          speaking: false,
          level: 0,
        },
      });
      if (channelId) submitLeaveHuddle(channelId);
    },

    setHuddleMuted: (muted) => {
      voice?.setMuted(muted);
      update((prev) => ({ voice: { ...prev.voice, muted } }));
    },

    syncHuddleRecipients: () => {
      reconcileHuddleMembership();
      pushRecipients();
    },

    setCamera: (on) => {
      if (!voice) return;
      if (on && !getState().videoCapability.canEncode) return; // capability-gated UI should prevent this
      const channel = getState().channels.find((c) => c.id === getState().voice.channelId);
      // block turning the camera on once the roster EXCEEDS the video cap — the
      // grid can't render more tiles, so those huddles stay audio-only.
      if (on && (channel?.huddle?.length ?? 0) > MAX_VIDEO_PARTICIPANTS) return;
      voice.setCamera(on);
      // Camera XOR screen: enabling the camera swaps any screen share off.
      update((prev) => ({ voice: { ...prev.voice, cameraOn: on, ...(on ? { sharing: false } : {}) } }));
    },

    setScreenShare: (on) => {
      if (!voice) return;
      if (on && !getState().videoCapability.canScreenShare) return; // capability-gated UI should prevent this
      const channel = getState().channels.find((c) => c.id === getState().voice.channelId);
      if (on && (channel?.huddle?.length ?? 0) > MAX_VIDEO_PARTICIPANTS) return; // same tile cap as the camera
      voice.setScreenShare(on);
      // Camera XOR screen: enabling the share swaps the camera off.
      update((prev) => ({ voice: { ...prev.voice, sharing: on, ...(on ? { cameraOn: false } : {}) } }));
    },

    refreshDevices: () => {
      void enumerateHuddleDevices()
        .then((deviceOptions) => patch({ deviceOptions }))
        .catch(() => {});
    },

    setDevicePrefs: (prefs) => {
      saveDevicePrefs(prefs);
      voice?.setDevices(prefs);
      patch({ devicePrefs: prefs });
    },

    sweepHuddle: (channelId, user) => {
      submitTracked(
        opKey.huddle(channelId),
        (live) => chatClient.sweepHuddle(live, { channelId, user, origin: getState().author }),
        (prev) => optimistic.huddleSwept(prev, channelId, keyHex(user)),
      );
    },

    getCallSession: () => voice,

    popOutHuddle: () => {
      if (!getState().voice.channelId) return;
      // Hand the media session to the window: open it, then release ours FIRST
      // (the hub is one-session-per-node — main must close before the window
      // dials). Consensus membership + channelId stay; only the media goes.
      openHuddleWindow();
      stopVoice();
      update((prev) => ({
        voice: { ...prev.voice, popped: true, cameraOn: false, peers: {}, speaking: false },
      }));
    },

    popInHuddle: () => {
      closeHuddleWindow();
      retakeHuddleMedia();
    },

    retakeHuddleMedia,

    noteHuddleMuted: (muted) => update((prev) => ({ voice: { ...prev.voice, muted } })),

    commitForge: (params) => {
      if (!params.path.trim() || params.content.length === 0) return;
      submitTracked(opKey.forgeHead(), (live) =>
        forgeClient.commit(live, {
          path: params.path.trim(),
          content: params.content,
          message: params.message.trim() || `commit ${params.path.trim()}`,
          origin: getState().author,
        }),
      );
    },

    // ── Forge tracker ──
    // Writes follow the one tracked path (preconfirm-less — the tracker has no
    // optimistic projection yet), then re-load the repo's per-screen item list,
    // since the global refresh deliberately excludes it. Detail (getForgeItem)
    // resolves to the caller: the item panel re-fetches after its own writes.
    loadForgeItems,
    loadForgeBranches,

    getForgeItem: (repo, number) => {
      const live = getNode();
      if (!live || !repo) return Promise.resolve(null);
      return forgeClient.getItem(live, { repo, number }).catch((err) => {
        fail(err);
        return null;
      });
    },

    openForgeIssue: ({ repo, title, body }) => {
      if (!repo || !title.trim()) return Promise.resolve();
      return submitTracked(opKey.forgeItemOpen(repo), (live) =>
        forgeClient.openIssue(live, {
          repo,
          title: title.trim(),
          body,
          origin: getState().author,
        }),
      ).then(() => loadForgeItems(repo));
    },

    openForgePr: ({ repo, title, body, sourceBranch, targetBranch }) => {
      if (!repo || !title.trim() || !sourceBranch) return Promise.resolve();
      return submitTracked(opKey.forgeItemOpen(repo), (live) =>
        forgeClient.openPr(live, {
          repo,
          title: title.trim(),
          body,
          sourceBranch,
          targetBranch,
          origin: getState().author,
        }),
      ).then(() => loadForgeItems(repo));
    },

    editForgeItem: ({ repo, number, title, body }) => {
      // an all-null edit is a wire no-op — don't spend a block on it.
      if (!repo || (title === null && body === null)) return Promise.resolve();
      return submitTracked(opKey.forgeItem(repo, number), (live) =>
        forgeClient.editItem(live, { repo, number, title, body, origin: getState().author }),
      ).then(() => loadForgeItems(repo));
    },

    setForgeItemState: ({ repo, number, open }) => {
      if (!repo) return Promise.resolve();
      return submitTracked(opKey.forgeItem(repo, number), (live) =>
        forgeClient.setItemState(live, { repo, number, open, origin: getState().author }),
      ).then(() => loadForgeItems(repo));
    },

    mergeForgePr: ({ repo, number, prevTargetOid, expectedSourceOid, mergeOid, packDigest }) => {
      if (!repo) return Promise.resolve();
      return submitTracked(opKey.forgeItem(repo, number), (live) =>
        forgeClient.mergePr(live, {
          repo,
          number,
          prevTargetOid,
          expectedSourceOid,
          mergeOid,
          packDigest,
          origin: getState().author,
        }),
        // a merge moves the target branch head too — reload both slices.
      ).then(() => Promise.all([loadForgeItems(repo), loadForgeBranches(repo)])).then(() => {});
    },

    submitForgeReview: ({ repo, number, verdict, body, commitOid, comments }) => {
      if (!repo) return Promise.resolve();
      return submitTracked(opKey.forgeItem(repo, number), (live) =>
        forgeClient.submitReview(live, {
          repo,
          number,
          verdict,
          body,
          commitOid,
          comments,
          origin: getState().author,
        }),
      ).then(() => loadForgeItems(repo));
    },

    // ── Docs ──
    listPages: () => {
      const live = getNode();
      if (!live) return;
      Promise.resolve()
        .then(() => pagesClient.listPages(live))
        .then((pages) => patch({ pages }))
        .catch(fail);
    },

    openPage: enterPage,
    closeTab: closeTabLocal,

    // create a page (optionally nested under `parent`) with an EMPTY title and
    // open it — the doc title input is where naming happens (Notion-style
    // instant page). `parent` null == top level.
    createChildPage: (parent: string | null) => {
      const pageId = crypto.randomUUID();
      submitTracked(
        opKey.page(pageId),
        (live) => pagesClient.createPage(live, { pageId, title: "", parent }),
        (prev) => optimistic.pageCreated(prev, { pageId, title: "", parent }),
      ).then(() => enterPage(pageId));
    },

    // kept for programmatic/test callers that pass a title.
    createPage: (title) => {
      const pageId = crypto.randomUUID();
      submitTracked(
        opKey.page(pageId),
        (live) => pagesClient.createPage(live, { pageId, title: title.trim() }),
        (prev) => optimistic.pageCreated(prev, { pageId, title: title.trim() }),
      ).then(() => enterPage(pageId));
    },

    setPageParent: ({ pageId, parent }) => {
      submitTracked(opKey.page(pageId), (live) =>
        pagesClient.setPageParent(live, { pageId, parent }),
      );
    },

    deletePage: (pageId) => {
      if (!pageId) return;
      submitTracked(opKey.page(pageId), (live) => pagesClient.deletePage(live, pageId))
        .then(() => {
          const live = getNode();
          if (live) pagesClient.listPages(live).then((pages) => patch({ pages })).catch(fail);
        })
        .catch(fail);
      // close its tab immediately (optimistic UX).
      closeTabLocal(pageId);
    },

    // ── Comments ──
    loadPageThreads: () => {
      void loadPageThreads();
    },

    addComment: ({ threadId, target, text }) => {
      const clean = text.trim();
      if (!clean) return;
      const tid = threadId ?? crypto.randomUUID();
      const commentId = crypto.randomUUID();
      submitTracked(opKey.commentThread(tid), (live) =>
        pagesClient.addComment(live, { threadId: tid, commentId, target, text: clean }),
      ).then(() => loadPageThreads());
    },

    editComment: ({ commentId, text }) => {
      const clean = text.trim();
      if (!clean) return;
      submitTracked(opKey.comment(commentId), (live) =>
        pagesClient.editComment(live, { commentId, text: clean }),
      ).then(() => loadPageThreads());
    },

    deleteComment: (commentId) => {
      submitTracked(opKey.comment(commentId), (live) =>
        pagesClient.deleteComment(live, commentId),
      ).then(() => loadPageThreads());
    },

    resolveThread: ({ threadId, resolved }) => {
      submitTracked(opKey.commentThread(threadId), (live) =>
        pagesClient.resolveThread(live, { threadId, resolved }),
      ).then(() => loadPageThreads());
    },

    insertPageBlock: ({ blockId, parent, after, kind, text }) => {
      const page = getState().activePage;
      if (!page) return;
      submitTracked(
        opKey.pageBlock(blockId),
        (live) =>
          pagesClient.insertBlock(live, {
            parent,
            after,
            block: { id: blockId, kind, text },
          }),
        (prev) =>
          optimistic.pageBlockInserted(prev, {
            parent,
            after,
            block: { id: blockId, parent, page, kind, text, checked: false, children: [] },
          }),
      );
    },

    updatePageBlockText: ({ blockId, text }) => {
      submitTracked(
        opKey.pageBlock(blockId),
        (live) => pagesClient.updateText(live, { blockId, text }),
        (prev) => optimistic.pageBlockPatched(prev, blockId, { text }),
      );
    },

    setPageBlockKind: ({ blockId, kind }) => {
      submitTracked(
        opKey.pageBlock(blockId),
        (live) => pagesClient.setKind(live, { blockId, kind }),
        (prev) => optimistic.pageBlockPatched(prev, blockId, { kind }),
      );
    },

    setPageBlockChecked: ({ blockId, checked }) => {
      submitTracked(
        opKey.pageBlock(blockId),
        (live) => pagesClient.setChecked(live, { blockId, checked }),
        (prev) => optimistic.pageBlockPatched(prev, blockId, { checked }),
      );
    },

    movePageBlock: ({ blockId, parent, after }) => {
      submitTracked(opKey.pageBlock(blockId), (live) =>
        pagesClient.moveBlock(live, { blockId, parent, after }),
      );
    },

    removePageBlock: (blockId) => {
      if (!blockId) return;
      submitTracked(
        opKey.pageBlock(blockId),
        (live) => pagesClient.removeBlock(live, blockId),
        (prev) => optimistic.pageBlockRemoved(prev, blockId),
      );
    },

    // ── Agents ──
    registerAgent: ({ displayName, agentId, capability, prompt, allowedActions, caps }) => {
      const id = agentId.trim();
      const name = displayName.trim();
      const tag = capability.trim();
      if (!id || !name || !tag) return;
      submitTracked(opKey.agent(id), (live) =>
        // stage the prompt in the node's blob store, then register with its
        // digest as prompt_hash — the blob is keyed by sha256(bytes), which
        // IS the hash the oracle worker fetches the prompt by.
        Promise.resolve()
          .then(() => live.putBlob(new TextEncoder().encode(prompt)))
          .then((digest) =>
            agentClient.registerAgent(live, {
              agentId: id,
              displayName: name,
              capability: tag,
              promptHash: agentClient.hexToBytes(digest),
              allowedActions,
              caps,
              origin: getState().author,
            }),
          ),
      );
    },

    pauseAgent: (agentId) => {
      if (!agentId) return;
      submitTracked(
        opKey.agent(agentId),
        (live) => agentClient.pauseAgent(live, { agentId, origin: getState().author }),
        (prev) => optimistic.agentPatched(prev, agentId, { status: "paused" }),
      );
    },

    resumeAgent: (agentId) => {
      if (!agentId) return;
      submitTracked(
        opKey.agent(agentId),
        (live) => agentClient.resumeAgent(live, { agentId, origin: getState().author }),
        (prev) => optimistic.agentPatched(prev, agentId, { status: "active" }),
      );
    },

    watchChannel: ({ channelId, policy }) => {
      if (!channelId) return;
      submitTracked(
        opKey.watch(channelId),
        (live) =>
          runsClient.watchChannel(live, {
            channelId,
            policy,
            origin: getState().author,
          }),
        (prev) => optimistic.watchSet(prev, { channelId, policy }),
      );
    },

    unwatchChannel: (channelId) => {
      if (!channelId) return;
      submitTracked(
        opKey.watch(channelId),
        (live) =>
          runsClient.unwatchChannel(live, {
            channelId,
            origin: getState().author,
          }),
        (prev) => optimistic.watchRemoved(prev, channelId),
      );
    },

    requestRun: ({ agentId, channelId, anchorSeq }) => {
      if (!agentId || !channelId) return;
      submitTracked(opKey.runRequest(agentId), (live) =>
        runsClient.requestRun(live, {
          agentId,
          channelId,
          anchorSeq,
          origin: getState().author,
        }),
      );
    },

    cancelRun: (runId) => {
      if (!runId) return;
      submitTracked(
        opKey.run(runId),
        (live) => runsClient.cancelRun(live, { runId, origin: getState().author }),
        (prev) => optimistic.runCancelled(prev, runId),
      );
    },

    updateAgent: ({ agentId, displayName, capability, prompt, allowedActions, caps }) => {
      const id = agentId.trim();
      if (!id) return;
      submitTracked(
        opKey.agent(id),
        (live) =>
        Promise.resolve()
          // a provided prompt is re-staged in the blob store; its digest becomes
          // the new prompt_hash. An absent prompt leaves the hash untouched.
          .then(() =>
            prompt !== undefined && prompt.length > 0
              ? live
                  .putBlob(new TextEncoder().encode(prompt))
                  .then((digest) => agentClient.hexToBytes(digest))
              : null,
          )
          .then((promptHash) =>
            agentClient.updateAgent(live, {
              agentId: id,
              displayName: displayName?.trim() || null,
              capability: capability?.trim() || null,
              promptHash,
              allowedActions: allowedActions ?? null,
              caps: caps ?? null,
              origin: getState().author,
            }),
          ),
        (prev) =>
          optimistic.agentPatched(prev, id, {
            ...(displayName?.trim() ? { display_name: displayName.trim() } : {}),
            ...(capability?.trim() ? { capability: capability.trim() } : {}),
          }),
      );
    },

    enableJobWorker: (enabled) => {
      submitTracked(opKey.jobWorker(), (live) =>
        runsClient.enableJobWorker(live, { enabled, origin: getState().author }),
      );
    },

    // ── Governance ──
    // Every submit is signed by THIS node's validator key (the daemon ignores the
    // claimed origin), so these carry no origin. `refresh()` re-reads the proposal
    // set after each write.
    proposeSignal: (text) => {
      const body = text.trim();
      if (!body) return;
      const proposalId = crypto.randomUUID();
      submitTracked(opKey.proposal(proposalId), (live) =>
        governanceClient.propose(live, {
          proposalId,
          action: { signal: { text: body } },
        }),
      );
    },

    voteProposal: (proposalId, approve) => {
      if (!proposalId) return;
      submitTracked(opKey.proposal(proposalId), (live) =>
        governanceClient.vote(live, { proposalId, approve }),
      );
    },

    executeProposal: (proposalId) => {
      if (!proposalId) return;
      submitTracked(opKey.proposal(proposalId), (live) =>
        governanceClient.execute(live, { proposalId }),
      );
    },

    // ── Search (derived-index views) ──
    runSearch: (text) => {
      const live = getNode();
      const query = text.trim();
      if (!live || !query) return;
      const token = ++searchToken;
      patch({ searchPending: true });
      // per-module tolerance (deliberate granular catches): an older node
      // without the index tier 404s a view; that module contributes an empty
      // group instead of sinking the whole search.
      const tolerant = <T,>(read: Promise<T[]>): Promise<T[]> => read.catch(() => []);
      // `docs` == the pages module's block hits — pages is the docs surface.
      Promise.resolve()
        .then(() =>
          Promise.all([
            tolerant(chatClient.searchMessages(live, { text: query })),
            tolerant(pagesClient.searchPageBlocks(live, { text: query })),
          ]),
        )
        .then(([chat, docs]) => {
          if (token !== searchToken) return; // a newer query superseded this one
          patch({ search: { query, chat, docs }, searchPending: false });
        })
        .catch((err) => {
          if (token !== searchToken) return;
          patch({ searchPending: false });
          fail(err);
        });
    },

    clearSearch: () => {
      searchToken += 1; // supersede any in-flight fan-out so it can't repopulate
      patch({ search: null, searchPending: false });
    },

    openSearch: () => patch({ searchOpen: true }),

    closeSearch: () => patch({ searchOpen: false }),

    // ── Chat tags (derived-index view) ──
    setTagFilter: (tag) => {
      const live = getNode();
      // keep the as-typed display form for the bar; the node normalizes.
      const clean = tag.trim().replace(/^#+/, "");
      const channelId = getState().activeChannel;
      if (!live || !clean) return;
      const token = ++tagToken;
      patch({ tagFilter: { tag: clean, channelId }, tagHits: [], tagHitsPending: true });
      chatClient
        .tagSearch(live, { tag: clean, channelId: channelId ?? undefined, limit: 100 })
        .then((hits) => {
          if (token !== tagToken) return; // superseded by a newer filter/clear
          patch({ tagHits: hits, tagHitsPending: false });
        })
        .catch((err) => {
          if (token !== tagToken) return;
          patch({ tagHitsPending: false });
          fail(err);
        });
    },

    clearTagFilter: () => {
      tagToken += 1; // supersede any in-flight tagSearch so it can't repopulate
      patch({ tagFilter: null, tagHits: [], tagHitsPending: false });
    },

    loadChannelTags: () => {
      const live = getNode();
      const channelId = getState().activeChannel;
      if (!live || !channelId) return;
      chatClient
        .tags(live, { channelId, limit: 20 })
        .then((rows) => {
          // only land on the channel the load was asked for.
          if (getState().activeChannel === channelId) patch({ channelTags: rows });
        })
        // best-effort: an older node without the index tier 404s the view.
        .catch(() => {});
    },

    stopNode: () => {
      const url = getState().nodeUrl;
      if (!url || !getState().managed) return;
      Promise.resolve()
        .then(() => bootstrap.shutdownNode(url))
        .then(() => patch({ connected: false }))
        .catch(fail);
    },

    startNode: () => {
      const target = getState().workspace;
      if (!getState().managed || !target) return;
      // re-select the active workspace: Rust adopts a live node or respawns
      // one, then connectActive reconnects and re-hydrates.
      connectActive(target).catch(fail);
    },

    dismissError: () => patch({ error: null }),

    retryConnect: () => {
      const st = getState();
      const id = st.bootError?.workspaceId ?? st.workspace?.id ?? null;
      const target =
        (id ? st.workspaces.find((w) => w.id === id) : undefined) ?? st.workspace ?? null;
      if (!target) {
        // nothing to reconnect to — fall back to the front door.
        patch({ bootError: null, needsOnboarding: true });
        return;
      }
      // connectActive clears bootError/error at the start; re-drive the SAME
      // workspace (idempotent — never mints a new one).
      connectActive(target).catch(fail);
    },

    // ── Onboarding / workspaces ──
    createWorkspace: (name) => {
      if (!name.trim()) return;
      patch({ onboardingBusy: true, error: null });
      Promise.resolve()
        .then(() => ws.createWorkspace(name.trim()))
        .then((created) => {
          update((prev) => ({
            workspaces: mergeWorkspace(prev.workspaces, created),
          }));
          return connectActive(created);
        })
        .catch((err) => {
          patch({ onboardingBusy: false });
          fail(err);
        });
    },

    joinWorkspace: (name, blob) => {
      if (!name.trim() || !blob.trim()) return;
      patch({ onboardingBusy: true, error: null });
      Promise.resolve()
        .then(() => ws.joinWorkspace(name.trim(), blob.trim()))
        .then((joined) => {
          update((prev) => ({
            workspaces: mergeWorkspace(prev.workspaces, joined),
          }));
          return connectActive(joined);
        })
        .catch((err) => {
          patch({ onboardingBusy: false });
          fail(err);
        });
    },

    selectWorkspace: (id) => {
      const target = getState().workspaces.find((w) => w.id === id);
      if (!target) return;
      // re-clicking the current MEMBER workspace is a no-op; a current
      // NON-member one falls through to the admission check below — its honest
      // "not admitted yet" error beats a silent nothing (and a genuinely
      // progressing one just re-runs the idempotent connect).
      if (target.id === getState().workspace?.id && target.member) return;
      const enter = (): void => {
        // drop the old node + its projections so the switch shows no stale state.
        setNode(null);
        patch({
          connected: false,
          status: null,
          channels: [],
          messages: [],
          activeChannel: null,
          activeThread: null,
          authorNames: {},
          nodeUsers: {},
          accountKeys: {},
          accountHandles: {},
          pages: [],
          activePage: null,
          activePageBlocks: [],
          agents: [],
          watches: [],
          pendingRuns: [],
          files: [],
          ops: {},
          onboardingPhase: null,
        });
        connectActive(target).catch(fail);
      };
      if (target.member) {
        enter();
        return;
      }
      // A non-member workspace can't serve the console — its node parks until a
      // member admits it. Entering it from the picker would only strand the
      // user in the waiting room, so refuse a parked/never-started/fatal one
      // with the honest status and STAY PUT (no registry repoint, no spawn).
      // Admission that is actually progressing (admitted/synced/promoted — the
      // node was seen mid-onboarding) proceeds: promoted connects straight, the
      // rest resume the waiting room the join flow opened.
      Promise.resolve()
        .then(() => ws.workspacePhase(target.id))
        .then((report) => {
          if (report.phase === "fatal") {
            fail(report.detail ?? `"${target.name}" failed to join its network`);
            return;
          }
          if (report.phase === "parked" || report.phase === "starting") {
            fail(
              `"${target.name}" hasn't been admitted to its network yet — its ` +
                `node parks until a member approves it. Ask a member to admit ` +
                `you (rejoin with a fresh invite), or delete this workspace.`,
            );
            return;
          }
          enter();
        })
        .catch(fail);
    },

    connectRemote: (rawUrl) => {
      const url = bootstrap.normalizeNodeUrl(rawUrl);
      if (!url) return;
      // Supersede any in-flight workspace connect/poll loop (joiner tick).
      nextBootGeneration();
      // Drop the old node + its projections so the switch shows no stale state
      // (mirrors selectWorkspace's reset).
      setNode(null);
      patch({
        workspace: null,
        connected: false,
        status: null,
        // per-node observability: the live chain tip and the node's own
        // durable block history. clear them on a node switch so the new
        // node's explorer never shows the previous node's rows.
        lastBlock: null,
        blocks: [],
        channels: [],
        messages: [],
        activeChannel: null,
        activeThread: null,
        authorNames: {},
        nodeUsers: {},
        accountKeys: {},
        accountHandles: {},
        members: [],
        proposals: [],
        forgeHead: null,
        pages: [],
        activePage: null,
        activePageBlocks: [],
        agents: [],
        watches: [],
        pendingRuns: [],
        files: [],
        ops: {},
        onboardingPhase: null,
        onboardingBusy: false,
        inviteBlob: null,
        // A remote node is unmanaged — dialed directly, never spawned here.
        nodeUrl: url,
        managed: false,
        needsOnboarding: false,
        error: null,
      });
      // Remember it for next launch, then dial. The hydrate effect (keyed on the
      // node) runs refresh(); an unreachable remote simply reads as disconnected
      // (the "no running node" surface) instead of throwing.
      saveRemoteUrl(url);
      setNode(bootstrap.connectRemote(url).transport);
    },

    revealInvite: () => {
      const target = getState().workspace;
      if (!target) return;
      Promise.resolve()
        .then(() => ws.inviteBlob(target.id))
        .then((blob) => patch({ inviteBlob: blob }))
        .catch(fail);
    },

    admitMember: (pubkey) => {
      const target = getState().workspace;
      if (!target || !pubkey.trim()) return;
      Promise.resolve()
        .then(() => ws.admitMember(target.id, pubkey.trim()))
        .then(() => refresh())
        .catch(fail);
    },

    promoteMember: (pubkey) => {
      const target = getState().workspace;
      if (!target || !pubkey.trim()) return;
      Promise.resolve()
        .then(() => ws.promoteMember(target.id, pubkey.trim()))
        .then(() => refresh())
        .catch(fail);
    },

    removeResident: (pubkey) => {
      const target = getState().workspace;
      if (!target || !pubkey.trim()) return;
      Promise.resolve()
        .then(() => ws.removeResident(target.id, pubkey.trim()))
        .then(() => refresh())
        .catch(fail);
    },

    demoteMember: (pubkey) => {
      const target = getState().workspace;
      if (!target || !pubkey.trim()) return;
      Promise.resolve()
        .then(() => ws.demoteMember(target.id, pubkey.trim()))
        .then(() => refresh())
        .catch(fail);
    },

    requestLeaveWorkspace: () => {
      const target = getState().workspace;
      if (!target) return;
      patch({ error: null });
      // Submit the on-chain self-removal but KEEP the node running — it must
      // stay up through its own pending removal or quorum can't finalize it.
      // The per-block roster re-query surfaces the pending removal; nothing is
      // torn down here.
      Promise.resolve()
        .then(() => ws.requestLeaveWorkspace(target.id))
        .then(() => refresh())
        .catch(fail);
    },

    forgetWorkspace: (force = false) => {
      const target = getState().workspace;
      if (!target) return;
      patch({ onboardingBusy: true, error: null, forgetNeedsForce: false });
      // Call the GUARDED backend FIRST — it refuses while this node is still a
      // current validator of a set of two-or-more. Only tear down the local
      // node + projections once the backend has actually forgotten it, so a
      // refused forget leaves the live UI intact (still connected, error shown).
      Promise.resolve()
        .then(() => ws.forgetWorkspace(target.id, force))
        .then((next) => {
          // Forgotten: drop the live node + its projections (mirrors
          // selectWorkspace's reset), then repoint the switcher.
          setNode(null);
          patch({
            connected: false,
            status: null,
            channels: [],
            messages: [],
            activeChannel: null,
            activeThread: null,
            authorNames: {},
            nodeUsers: {},
            accountKeys: {},
            accountHandles: {},
            pages: [],
            activePage: null,
            activePageBlocks: [],
            agents: [],
            watches: [],
            pendingRuns: [],
            ops: {},
            onboardingPhase: null,
            inviteBlob: null,
          });
          update((prev) => ({
            workspaces: prev.workspaces.filter((w) => w.id !== target.id),
          }));
          if (next) {
            // The registry repointed to another workspace — connect to it.
            return connectActive(next);
          }
          // None remain — fall back to the onboarding gate.
          patch({
            workspace: null,
            needsOnboarding: true,
            onboardingBusy: false,
            managed: false,
            nodeUrl: null,
          });
        })
        .catch((err) => {
          // A GUARDED forget that couldn't confirm the node left (it's
          // down/bricked) is exactly the case a force override exists for —
          // reveal it so a workspace whose node can never start isn't stranded.
          // A force attempt that still fails does NOT re-reveal (no loop): the
          // backend only refuses force for a reachable, provably-live node.
          patch({ onboardingBusy: false, forgetNeedsForce: !force });
          fail(err);
        });
    },

    deleteWorkspace: (id, force = false) => {
      const target = getState().workspaces.find((w) => w.id === id);
      if (!target) return;
      patch({ error: null, deleteNeedsForce: null });
      // Same guarded backend as forgetWorkspace — refused while the node is
      // still a current validator of a set of two-or-more. Only touch local
      // state once the backend has actually forgotten it.
      Promise.resolve()
        .then(() => ws.forgetWorkspace(target.id, force))
        .then((next) => {
          const wasActive = getState().workspace?.id === target.id;
          update((prev) => ({
            workspaces: prev.workspaces.filter((w) => w.id !== target.id),
          }));
          // Deleting a workspace we're not connected to only drops its row —
          // the registry's active pointer and the live connection are untouched.
          if (!wasActive) return;
          // Deleted the active one: drop the live node + its projections
          // (mirrors forgetWorkspace), then repoint or fall back to the gate.
          setNode(null);
          patch({
            connected: false,
            status: null,
            channels: [],
            messages: [],
            activeChannel: null,
            activeThread: null,
            authorNames: {},
            nodeUsers: {},
            accountKeys: {},
            accountHandles: {},
            pages: [],
            activePage: null,
            activePageBlocks: [],
            agents: [],
            watches: [],
            pendingRuns: [],
            ops: {},
            onboardingPhase: null,
            inviteBlob: null,
          });
          if (next) return connectActive(next);
          patch({
            workspace: null,
            needsOnboarding: true,
            onboardingBusy: false,
            managed: false,
            nodeUrl: null,
          });
        })
        .catch((err) => {
          // The forgetWorkspace escalation contract, scoped to this row: an
          // unconfirmable node reveals the force override for exactly this
          // workspace; a force attempt that still fails does not re-reveal.
          patch({ deleteNeedsForce: force ? null : target.id });
          fail(err);
        });
    },

    newWorkspace: () => patch({ needsOnboarding: true, inviteBlob: null, bootError: null }),

    dismissOnboarding: () =>
      // Closable when there's a connection to return to — a local workspace or a
      // remote node (nodeUrl set). Nothing to go back to on a cold first boot.
      update((prev) =>
        prev.workspace || prev.nodeUrl ? { needsOnboarding: false } : {},
      ),

    connectActive,
  };
}
