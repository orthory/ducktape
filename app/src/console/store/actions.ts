import type { Dispatch } from "react";

import * as agentClient from "../../domain/agent-client";
import * as chatClient from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import * as filesClient from "../../domain/files-client";
import type { Manifest } from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import * as pagesClient from "../../domain/pages-client";
import type { BlockKind as PageBlockKind } from "../../domain/pages-client";
import * as profilesClient from "../../domain/profiles-client";
import * as runsClient from "../../domain/runs-client";
import type { TurnPolicy } from "../../domain/runs-client";
import * as bootstrap from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
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
  channelIdOf,
  clearRemoteUrl,
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
  commitForge(params: { path: string; content: string; message: string }): void;

  // ── Docs (block-tree notebook over the `pages` module) ──
  /** Re-query the page enumeration into `state.pages`. */
  listPages(): void;
  /** Create a page (root block id minted here) and open it. */
  createPage(title: string): void;
  /** Open a page, loading its preorder block tree. */
  openPage(pageId: string): void;
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

  // the single entry point into a page: make it active and load its preorder
  // block tree — every path into a page (new-page, a rail click) goes here.
  const enterPage = (pageId: string) => {
    const live = getNode();
    if (!live || !pageId) return;
    patch({
      activePage: pageId,
      activePageBlocks: [],
    });
    Promise.resolve()
      .then(() => pagesClient.getPage(live, pageId))
      .then((blocks) => patch({ activePageBlocks: blocks ?? [] }))
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
    // Tear the old node down BEFORE clearing its projections, so its block /
    // telemetry subscriptions can't fire a straggler frame into the just-cleared
    // arrays during the async selectWorkspace/waitUntilUp window that follows
    // (mirrors connectRemote). Without this teardown-first order, an old-node
    // frame lands in the cleared telemetry and the backfill's retain-prev keeps
    // it atop the NEW node's timeline — the exact staleness this clear prevents.
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
      // node effect re-hydrates blocks and re-backfills telemetry once the new
      // node is set below.
      telemetry: [],
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

    createPage: (title) => {
      const clean = title.trim();
      if (!clean) return;
      // the page root's block id — minted here like task/job ids; the refresh
      // re-enumerates ListPages so the rail shows it, then open it.
      const pageId = crypto.randomUUID();
      submitTracked(
        opKey.page(pageId),
        (live) => pagesClient.createPage(live, { pageId, title: clean }),
        (prev) => optimistic.pageCreated(prev, { pageId, title: clean }),
      ).then(() => enterPage(pageId));
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
        // per-node observability: the live telemetry stream and the node's own
        // durable block history. clear them on a node switch so the new node's
        // timeline/explorer never shows the previous node's rows (the telemetry
        // backfill retains prior frames when the new node returns none).
        telemetry: [],
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
