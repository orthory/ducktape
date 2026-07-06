import type { Dispatch } from "react";

import * as agentClient from "../../domain/agent-client";
import * as chatClient from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import * as filesClient from "../../domain/files-client";
import type { Manifest } from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import * as pagesClient from "../../domain/pages-client";
import type { BlockKind as PageBlockKind, PageBlock } from "../../domain/pages-client";
import * as profilesClient from "../../domain/profiles-client";
import * as runsClient from "../../domain/runs-client";
import type { TurnPolicy } from "../../domain/runs-client";
import { parseMetrics, type NodeMetrics } from "../../domain/metrics";
import * as bootstrap from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
import { voiceSocketUrl } from "../../domain/transport";
import { createVoiceSession, huddleRecipients } from "../../domain/voice-session";
import type { VoiceError, VoiceSession, VoiceStatus } from "../../domain/voice-session";
import { keyBytes } from "../../domain/chat-client";
import * as ws from "../../domain/workspace-client";
import type { Workspace } from "../../domain/workspace-client";
import { parseMessageInput } from "../views/chat/chat-input";
import {
  defaultScreenForSection,
  sectionForScreen,
} from "../modules/registry";
import type { Action } from "./reducer";
import { beginOp, failOp, finalizeOp, opKey, receiptOf } from "./finalization";
import * as optimistic from "./optimistic";
import {
  addTab,
  channelIdOf,
  clearRemoteUrl,
  removeTab,
  saveDocTabs,
  saveRemoteUrl,
  saveViewMode,
} from "./state";
import type { ConsoleState, ViewMode } from "./state";

/** How often a parked joiner's phase is polled while it promotes. */
const JOIN_POLL_MS = 1500;

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

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
  setAuthor(author: string): void;
  /** Set our own display name in the `profiles` module (origin-gated SetName)
   *  and keep it as the local author identity, so it propagates to everyone. */
  setDisplayName(name: string): void;
  selectChannel(channelId: string): void;
  createChannel(name: string, postPolicy: PostPolicy): void;
  sendMessage(body: string): void;
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

  commitForge(params: { path: string; content: string; message: string }): void;

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

  // ── Files (content-addressed manifests over the `files` module) ──
  /** Chunk + stage a file's bytes into the blob store, then commit its manifest. */
  uploadFile(params: { name: string; mime: string; bytes: Uint8Array<ArrayBuffer> }): void;
  /** Remove a manifest (owner-gated; rides the daemon identity that added it). */
  removeFile(fileId: string): void;
  /** Reassemble a file's bytes, verifying every chunk against the manifest.
   *  Returns the manifest + bytes for the view to hand to a browser download,
   *  or null when the file/node is unavailable. */
  downloadFile(
    fileId: string,
  ): Promise<{ manifest: Manifest; bytes: Uint8Array<ArrayBuffer> } | null>;

  /** Ask the managed daemon to exit (desktop only). */
  stopNode(): void;
  /** Re-spawn / re-adopt the managed daemon after a stop (desktop only). */
  startNode(): void;
  /** Scrape + parse the node's `/metrics`. Null when no node is resolved or the
   *  scrape fails — best-effort, for the poll-driven Metrics view. */
  readMetrics(): Promise<NodeMetrics | null>;
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
   *  OBSERVER standing (staged admission's first step); promote seats it. */
  admitMember(pubkey: string): void;
  /** Promote an observer into the consensus quorum by pubkey — staged
   *  admission's second step, once the observer's node is warm. */
  promoteMember(pubkey: string): void;
  /** Revoke a key's observer standing — the undo of admitMember; its node
   *  parks again and another admit re-grants. */
  removeObserver(pubkey: string): void;
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

  // Monotonic token gating the async search fan-out: each runSearch/clearSearch
  // bumps it, and a resolving fan-out only writes results if its token is still
  // current — so a slow or out-of-order response can never clobber a newer
  // query's results (or repopulate a cleared palette).
  let searchToken = 0;

  // The live voice session (the browser audio graph + ws), or null when not in
  // a huddle. Ephemeral and per-client — it lives here, not in state; the
  // `voice` slice mirrors only its status for the ui.
  let voice: VoiceSession | null = null;

  /** Our own node key hex — the fan-out set excludes it. Empty on a daemon
   *  that can't do voice. */
  const selfNodeHex = (): string => getState().status?.publicKey ?? "";

  // The last fan-out set pushed into the live session — refresh() lands a new
  // channels array every block, so pushes are deduped by value here rather
  // than by effect identity upstream.
  let lastRecipients: string | null = null;

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

  // Session lifecycle → the voice slice. Any terminal end reconciles the
  // consensus roster (submit leave) so peers never keep showing a dead
  // participant: 'closed' (the session was replaced) clears the slice
  // entirely; 'error' (hub refusal, socket failure, mic denial) keeps the dock
  // up in its error state so the failure is visible — Leave dismisses it.
  const onVoiceStatus = (status: VoiceStatus, error?: VoiceError): void => {
    if (status === "closed" || status === "error") {
      const channelId = getState().voice.channelId;
      stopVoice();
      if (channelId) submitLeaveHuddle(channelId);
      if (status === "closed") {
        patch({ voice: { channelId: null, muted: false, status: "idle", error: null } });
      } else {
        update((prev) => ({
          voice: { ...prev.voice, status: "error", error: error ?? "connection" },
        }));
      }
      return;
    }
    update((prev) => ({ voice: { ...prev.voice, status, error: null } }));
  };

  /** Submit a leave_huddle for `channelId` with the optimistic roster prune. */
  const submitLeaveHuddle = (channelId: string) =>
    submitTracked(
      opKey.huddle(channelId),
      (live) => chatClient.leaveHuddle(live, { channelId, origin: getState().author }),
      (prev) => optimistic.huddleLeft(prev, channelId, selfNodeHex()),
    );

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

  // switching channels means: new active channel, thread panel closed, and
  // THAT channel's messages loaded — every path into a channel goes here
  const enterChannel = (channelId: string) => {
    const live = getNode();
    if (!live) return;
    patch({
      activeChannel: channelId,
      activeThread: null,
    });
    Promise.resolve()
      .then(() => chatClient.latestMessages(live, channelId))
      .then((messages) => patch({ messages }))
      .catch(fail);
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
    const targets = [page, ...blocks.map((b) => b.id)];
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
      // a force-forget offer is scoped to the workspace it was raised for;
      // switching targets clears it so it can never fire on the wrong one.
      forgetNeedsForce: false,
      inviteBlob: null,
      // per-node observability belonging to the workspace we're leaving; the
      // node effect re-hydrates blocks and re-follows the block stream once
      // the new node is set below.
      lastBlock: null,
      blocks: [],
    });
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
            patch({ onboardingPhase: null });
            setNode(transport);
          });
        }
        // joiner: the node parks (no surface) until a member admits it and
        // the epoch cuts over; it then promotes, reboots as a validator, and
        // its surface starts answering. Poll the phase until that happens.
        const tick = (): Promise<void> => {
          if (stale()) return Promise.resolve();
          return transport.status().then(
            () => {
              if (stale()) return;
              patch({ onboardingPhase: null });
              setNode(transport);
            },
            () =>
              ws.workspacePhase(target.id).then((report) => {
                if (stale()) return;
                patch({ onboardingPhase: report });
                if (report.phase === "fatal") {
                  fail(report.detail ?? "the node failed to join");
                  return;
                }
                return wait(JOIN_POLL_MS).then(tick);
              }),
          );
        };
        return tick();
      })
      .catch((err) => {
        if (!stale()) {
          patch({ onboardingBusy: false });
          fail(err);
        }
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

    setAccent: (accent) => patch({ accent }),
    setAuthor: (author) => patch({ author }),

    readMetrics: () => {
      const live = getNode();
      return live
        ? live.metrics().then(parseMetrics).catch(() => null)
        : Promise.resolve(null);
    },

    // Keep the local author identity (still the web-origin string) AND submit
    // SetName so the chosen name propagates: it's origin-gated, so passing our
    // origin sets our OWN profile only. Refresh re-reads authorNames.
    setDisplayName: (name) => {
      const origin = getState().author;
      submitTracked(
        opKey.profile(),
        (live) => profilesClient.setName(live, { displayName: name, origin }),
        () => ({ author: name }),
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
      const messageId = crypto.randomUUID();
      const blocks = parseMessageInput(body);
      const author = getState().author;
      submitTracked(
        opKey.message(channelId, messageId),
        (live) =>
          chatClient.postMessage(live, { channelId, messageId, blocks, origin: author }),
        (prev) =>
          optimistic.postedMessage(prev, {
            channelId,
            messageId,
            blocks,
            author,
            at: Date.now(),
            thread: null,
          }),
      );
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
      const messageId = crypto.randomUUID();
      const blocks = parseMessageInput(body);
      const author = getState().author;
      submitTracked(
        opKey.message(channelId, messageId),
        (live) =>
          chatClient.postMessage(live, {
            channelId,
            messageId,
            blocks,
            origin: author,
            thread: root.seq,
          }),
        (prev) =>
          optimistic.postedMessage(prev, {
            channelId,
            messageId,
            blocks,
            author,
            at: Date.now(),
            thread: root.seq,
          }),
      ).then(() => {
        const live = getNode();
        if (!live) return;
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

    editMessage: (seq, body) => {
      const channelId = getState().activeChannel;
      if (!channelId || !body.trim()) return;
      const activeThread = getState().activeThread;
      const target =
        getState().messages.find((m) => m.seq === seq) ??
        (activeThread?.root.seq === seq
          ? activeThread.root
          : activeThread?.replies.find((m) => m.seq === seq));
      const blocks = parseMessageInput(body);
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
      // session already left (onVoiceStatus reconciled the roster) — skip it.
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
            author: prev.author,
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
          update((prev) => ({ voice: { ...prev.voice, status: "error", error: "refused" } }));
        }
      });
      // start the audio session and reflect "connecting"; push whatever roster
      // we already know (others may be huddling), self excluded. joins start
      // MUTED — joining a room must never be a hot-mic moment; unmuting is the
      // deliberate act.
      voice = createVoiceSession(onVoiceStatus);
      voice.setMuted(true);
      patch({ voice: { channelId, muted: true, status: "connecting", error: null } });
      voice.start(voiceSocketUrl(nodeUrl, channelId));
      pushRecipients(channelId);
    },

    leaveHuddle: () => {
      const channelId = getState().voice.channelId;
      stopVoice();
      patch({ voice: { channelId: null, muted: false, status: "idle", error: null } });
      if (channelId) submitLeaveHuddle(channelId);
    },

    setHuddleMuted: (muted) => {
      voice?.setMuted(muted);
      update((prev) => ({ voice: { ...prev.voice, muted } }));
    },

    syncHuddleRecipients: () => pushRecipients(),

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
    registerAgent: ({ displayName, agentId, capability, prompt, allowedActions }) => {
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

    updateAgent: ({ agentId, displayName, capability, prompt, allowedActions }) => {
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

    // ── Files ──
    uploadFile: ({ name, mime, bytes }) => {
      const cleanName = name.trim();
      if (!cleanName) return;
      const fileId = crypto.randomUUID();
      submitTracked(opKey.file(fileId), (live) =>
        filesClient.uploadFile(live, {
          fileId,
          name: cleanName,
          mime: mime || "application/octet-stream",
          bytes,
        }),
      );
    },

    removeFile: (fileId) => {
      if (!fileId) return;
      submitTracked(
        opKey.file(fileId),
        (live) => filesClient.removeManifest(live, fileId),
        (prev) => optimistic.fileRemoved(prev, fileId),
      );
    },

    downloadFile: (fileId) => {
      const live = getNode();
      if (!live || !fileId) return Promise.resolve(null);
      return Promise.resolve()
        .then(() => filesClient.stat(live, fileId))
        .then((manifest) =>
          manifest
            ? filesClient
                .downloadFile(live, manifest)
                .then((bytes) => ({ manifest, bytes }))
            : null,
        )
        .catch((err) => {
          fail(err);
          return null;
        });
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
      if (!target || target.id === getState().workspace?.id) return;
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

    removeObserver: (pubkey) => {
      const target = getState().workspace;
      if (!target || !pubkey.trim()) return;
      Promise.resolve()
        .then(() => ws.removeObserver(target.id, pubkey.trim()))
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

    newWorkspace: () => patch({ needsOnboarding: true, inviteBlob: null }),

    dismissOnboarding: () =>
      // Closable when there's a connection to return to — a local workspace or a
      // remote node (nodeUrl set). Nothing to go back to on a cold first boot.
      update((prev) =>
        prev.workspace || prev.nodeUrl ? { needsOnboarding: false } : {},
      ),

    connectActive,
  };
}
