import type { Dispatch } from "react";

import * as agentClient from "../../domain/agent-client";
import type { TurnPolicy } from "../../domain/agent-client";
import * as automationsClient from "../../domain/automations-client";
import type { Action as RuleAction, Trigger } from "../../domain/automations-client";
import * as chatClient from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import * as documentClient from "../../domain/document-client";
import type { BlockKind } from "../../domain/document-client";
import * as filesClient from "../../domain/files-client";
import type { Manifest } from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import * as inboxClient from "../../domain/inbox-client";
import * as jobsClient from "../../domain/jobs-client";
import * as memoryClient from "../../domain/memory-client";
import type { Meta } from "../../domain/memory-client";
import * as pagesClient from "../../domain/pages-client";
import type { BlockKind as PageBlockKind } from "../../domain/pages-client";
import * as profilesClient from "../../domain/profiles-client";
import * as tasksClient from "../../domain/tasks-client";
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
import {
  channelIdOf,
  clearRemoteUrl,
  docIdOf,
  nextTaskStatus,
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
  addTask(title: string): void;
  advanceTask(taskId: string): void;
  commitForge(params: { path: string; content: string; message: string }): void;

  // ── Documents (block store over the `document` module) ──
  /** Re-query the module's enumeration index into `state.docIds` (the tree). */
  listDocs(): void;
  /** Create a doc at a "/"-delimited path (CreateDoc, idempotent) and open it.
   *  The refresh after the write re-enumerates the index, so the new path
   *  appears in the tree. */
  createDoc(docId: string): void;
  /** Open a doc by path, loading its blocks (like selectChannel). */
  openDoc(docId: string): void;
  /** Append/insert a fresh block into the active doc (id generated here). */
  insertBlock(params: { after: string | null; kind: BlockKind; text: string }): void;
  /** Replace a block's text in the active doc. */
  updateBlock(params: { blockId: string; text: string }): void;
  /** Remove a block from the active doc. */
  removeBlock(blockId: string): void;
  /** Move a block within the active doc (see the `after` rule). */
  moveBlock(params: { blockId: string; after: string | null }): void;

  // ── Pages (block-tree notebook over the `pages` module) ──
  /** Re-query the page enumeration into `state.pages`. */
  listPages(): void;
  /** Create a page (root block id minted here) and open it. */
  createPage(title: string): void;
  /** Open a page, loading its preorder block tree (like openDoc). */
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
    modelRef: string;
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
    modelRef?: string;
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

  // ── Inbox (per-member notification queue over the `inbox` module) ──
  /** Mark every notification in the local member's queue read (idempotent). */
  markInboxRead(): void;
  /** Mark every item up to and including `seq` read. */
  markInboxReadTo(seq: number): void;
  /** Delete every notification in the local member's queue (up to the latest). */
  clearInbox(): void;
  /** Enqueue a notification (module follow-ups are the primary writers, but the
   *  console can self-deliver or notify another member). */
  deliverNotification(params: { member: string; kind: string; body: string }): void;

  // ── Jobs (consensus work board over the `jobs` module) ──
  /** Post a new job (id generated here). */
  submitJob(params: { kind: string; spec: string }): void;
  /** Claim a Pending job under a view-count lease. */
  claimJob(params: { jobId: string; leaseViews: number }): void;
  /** Report a result on a job this node is processing. */
  finalizeJob(params: { jobId: string; ok: boolean; payload: string }): void;
  /** Hand a Processing job back to Pending. */
  releaseJob(jobId: string): void;
  /** Permissionless requeue of a Processing job whose lease expired. */
  reclaimJob(jobId: string): void;
  /** Cancel a still-Pending job (submitter only). */
  cancelJob(jobId: string): void;
  /** Remove a terminal job's record entirely (submitter only). */
  pruneJob(jobId: string): void;

  // ── Automations (event-triggered rules over the `automations` module) ──
  /** Create a rule pairing a trigger with an action. */
  createRule(params: { ruleId: string; trigger: Trigger; action: RuleAction }): void;
  /** Enable or disable a rule without deleting it. */
  setRuleEnabled(ruleId: string, enabled: boolean): void;
  /** Delete a rule. */
  deleteRule(ruleId: string): void;

  // ── Memory (agent filesystem over the `memory` module) ──
  /** Browse a directory: list its entries and make it the active path. */
  browseMemory(path: string): void;
  /** Open a file into the viewer, loading its latest (or a specific) generation. */
  openMemoryFile(params: { path: string; generation?: number | null }): void;
  /** Close the open file. */
  closeMemoryFile(): void;
  /** Write-once publish of an inline document at `path`, then refresh the tree. */
  publishMemory(params: { path: string; text: string; meta?: Meta }): void;
  /** Delete a memory file (all live generations). */
  deleteMemory(path: string): void;
  /** Run a case-sensitive substring search under `prefix`; results land in
   *  `state.memoryMatches`. */
  searchMemory(params: { prefix: string; pattern: string }): void;
  /** Clear the active search. */
  clearMemorySearch(): void;

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
  /** Admit a joiner by pubkey through the active (member) workspace. */
  admitMember(pubkey: string): void;
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
   *  set of two-or-more (that would halt quorum). */
  forgetWorkspace(): void;
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

  const submitThenRefresh = (submit: (live: NodeTransport) => Promise<unknown>) => {
    const live = getNode();
    if (!live) return Promise.resolve();
    return Promise.resolve()
      .then(() => submit(live))
      .then(() => refresh())
      .catch(fail);
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
  // have touched the root or a reply. `submitThenRefresh` already refreshed the
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

  // the single entry point into a doc: make it active and load its blocks.
  // Every path into a doc (new-doc, a tree click) goes here — like
  // enterChannel. The known-doc set is the node's index (state.docIds), not a
  // local registry, so entering a doc no longer writes any list — it just
  // focuses the reader on `docId`.
  const enterDoc = (rawId: string) => {
    const live = getNode();
    const docId = docIdOf(rawId);
    if (!live || !docId) return;
    patch({
      activeDoc: docId,
      activeDocBlocks: [],
    });
    Promise.resolve()
      .then(() => documentClient.getDoc(live, docId))
      .then((blocks) => patch({ activeDocBlocks: blocks ?? [] }))
      .catch(fail);
  };

  // the single entry point into a page: make it active and load its preorder
  // block tree — every path into a page (new-page, a rail click) goes here,
  // mirroring enterDoc.
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
    patch({
      workspace: target,
      needsOnboarding: false,
      onboardingBusy: false,
      inviteBlob: null,
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
      patch({ author: name });
      submitThenRefresh((live) =>
        profilesClient.setName(live, {
          displayName: name,
          origin: getState().author,
        }),
      );
    },

    selectChannel: enterChannel,

    createChannel: (name, postPolicy) => {
      const channelId = channelIdOf(name);
      if (!channelId) return;
      submitThenRefresh((live) =>
        chatClient.createChannel(live, {
          channelId,
          name,
          postPolicy,
          origin: getState().author,
        }),
      ).then(() => enterChannel(channelId));
    },

    sendMessage: (body) => {
      const channelId = getState().activeChannel;
      if (!channelId || !body.trim()) return;
      submitThenRefresh((live) =>
        chatClient.postMessage(live, {
          channelId,
          messageId: crypto.randomUUID(),
          blocks: parseMessageInput(body),
          origin: getState().author,
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
    // included) via `submitThenRefresh`, but the thread panel reads its own
    // `ChatThread` snapshot, so that one extra cheap query keeps the panel in
    // sync without repeating the old heavy-refresh-twice pattern.
    replyInThread: (body) => {
      const channelId = getState().activeChannel;
      const root = getState().activeThread?.root;
      if (!channelId || !root || !body.trim()) return;
      submitThenRefresh((live) =>
        chatClient.postMessage(live, {
          channelId,
          messageId: crypto.randomUUID(),
          blocks: parseMessageInput(body),
          origin: getState().author,
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
      submitThenRefresh((live) =>
        chatClient.editMessage(live, {
          channelId,
          seq,
          blocks: parseMessageInput(body),
          baseRev: target?.head.rev ?? null,
          origin: getState().author,
        }),
      ).then(resyncOpenThread);
    },

    deleteMessage: (seq) => {
      const channelId = getState().activeChannel;
      if (!channelId) return;
      submitThenRefresh((live) =>
        chatClient.deleteMessage(live, { channelId, seq, origin: getState().author }),
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
            "User" in author &&
            author.User.length === selfBytes.length &&
            author.User.every((byte, i) => byte === selfBytes[i]),
        );
      submitThenRefresh((live) =>
        mine
          ? chatClient.removeReaction(live, { channelId, seq, emoji, origin })
          : chatClient.addReaction(live, { channelId, seq, emoji, origin }),
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

    addTask: (title) => {
      if (!title.trim()) return;
      submitThenRefresh((live) =>
        tasksClient.createTask(live, {
          taskId: crypto.randomUUID(),
          title: title.trim(),
        }),
      );
    },

    advanceTask: (taskId) => {
      const task = getState().tasks.find((t) => t.id === taskId);
      if (!task || task.status === "Done") return;
      submitThenRefresh((live) =>
        tasksClient.updateStatus(live, {
          taskId,
          status: nextTaskStatus(task.status),
        }),
      );
    },

    commitForge: (params) => {
      if (!params.path.trim() || params.content.length === 0) return;
      submitThenRefresh((live) =>
        forgeClient.commit(live, {
          path: params.path.trim(),
          content: params.content,
          message: params.message.trim() || `commit ${params.path.trim()}`,
          origin: getState().author,
        }),
      );
    },

    // ── Documents ──
    listDocs: () => {
      const live = getNode();
      if (!live) return;
      Promise.resolve()
        .then(() => documentClient.listDocs(live))
        .then((docIds) => patch({ docIds }))
        .catch(fail);
    },

    openDoc: enterDoc,

    createDoc: (rawId) => {
      const docId = docIdOf(rawId);
      if (!docId) return;
      // CreateDoc is idempotent and REQUIRED before any block op; the refresh
      // re-enumerates the index so the new path shows in the tree, then open
      // it (loads blocks), mirroring createChannel.
      submitThenRefresh((live) => documentClient.createDoc(live, { docId })).then(
        () => enterDoc(docId),
      );
    },

    insertBlock: ({ after, kind, text }) => {
      const docId = getState().activeDoc;
      if (!docId) return;
      submitThenRefresh((live) =>
        documentClient.insertBlock(live, {
          docId,
          after,
          block: { id: crypto.randomUUID(), kind, text },
        }),
      );
    },

    updateBlock: ({ blockId, text }) => {
      const docId = getState().activeDoc;
      if (!docId) return;
      submitThenRefresh((live) =>
        documentClient.updateBlock(live, { docId, blockId, text }),
      );
    },

    removeBlock: (blockId) => {
      const docId = getState().activeDoc;
      if (!docId) return;
      submitThenRefresh((live) =>
        documentClient.removeBlock(live, { docId, blockId }),
      );
    },

    moveBlock: ({ blockId, after }) => {
      const docId = getState().activeDoc;
      if (!docId) return;
      submitThenRefresh((live) =>
        documentClient.moveBlock(live, { docId, blockId, after }),
      );
    },

    // ── Pages ──
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
      submitThenRefresh((live) =>
        pagesClient.createPage(live, { pageId, title: clean }),
      ).then(() => enterPage(pageId));
    },

    insertPageBlock: ({ blockId, parent, after, kind, text }) => {
      if (!getState().activePage) return;
      submitThenRefresh((live) =>
        pagesClient.insertBlock(live, {
          parent,
          after,
          block: { id: blockId, kind, text },
        }),
      );
    },

    updatePageBlockText: ({ blockId, text }) => {
      submitThenRefresh((live) =>
        pagesClient.updateText(live, { blockId, text }),
      );
    },

    setPageBlockKind: ({ blockId, kind }) => {
      submitThenRefresh((live) => pagesClient.setKind(live, { blockId, kind }));
    },

    setPageBlockChecked: ({ blockId, checked }) => {
      submitThenRefresh((live) =>
        pagesClient.setChecked(live, { blockId, checked }),
      );
    },

    movePageBlock: ({ blockId, parent, after }) => {
      submitThenRefresh((live) =>
        pagesClient.moveBlock(live, { blockId, parent, after }),
      );
    },

    removePageBlock: (blockId) => {
      if (!blockId) return;
      submitThenRefresh((live) => pagesClient.removeBlock(live, blockId));
    },

    // ── Agents ──
    registerAgent: ({ displayName, agentId, modelRef, prompt, allowedActions }) => {
      const id = agentId.trim();
      const name = displayName.trim();
      const model = modelRef.trim();
      if (!id || !name || !model) return;
      submitThenRefresh((live) =>
        // stage the prompt in the node's blob store, then register with its
        // digest as prompt_hash — the blob is keyed by sha256(bytes), which
        // IS the hash the oracle worker fetches the prompt by.
        Promise.resolve()
          .then(() => live.putBlob(new TextEncoder().encode(prompt)))
          .then((digest) =>
            agentClient.registerAgent(live, {
              agentId: id,
              displayName: name,
              modelRef: model,
              promptHash: agentClient.hexToBytes(digest),
              allowedActions,
              origin: getState().author,
            }),
          ),
      );
    },

    pauseAgent: (agentId) => {
      if (!agentId) return;
      submitThenRefresh((live) =>
        agentClient.pauseAgent(live, { agentId, origin: getState().author }),
      );
    },

    resumeAgent: (agentId) => {
      if (!agentId) return;
      submitThenRefresh((live) =>
        agentClient.resumeAgent(live, { agentId, origin: getState().author }),
      );
    },

    watchChannel: ({ channelId, policy }) => {
      if (!channelId) return;
      submitThenRefresh((live) =>
        agentClient.watchChannel(live, {
          channelId,
          policy,
          origin: getState().author,
        }),
      );
    },

    unwatchChannel: (channelId) => {
      if (!channelId) return;
      submitThenRefresh((live) =>
        agentClient.unwatchChannel(live, {
          channelId,
          origin: getState().author,
        }),
      );
    },

    requestRun: ({ agentId, channelId, anchorSeq }) => {
      if (!agentId || !channelId) return;
      submitThenRefresh((live) =>
        agentClient.requestRun(live, {
          agentId,
          channelId,
          anchorSeq,
          origin: getState().author,
        }),
      );
    },

    cancelRun: (runId) => {
      if (!runId) return;
      submitThenRefresh((live) =>
        agentClient.cancelRun(live, { runId, origin: getState().author }),
      );
    },

    updateAgent: ({ agentId, displayName, modelRef, prompt, allowedActions }) => {
      const id = agentId.trim();
      if (!id) return;
      submitThenRefresh((live) =>
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
              modelRef: modelRef?.trim() || null,
              promptHash,
              allowedActions: allowedActions ?? null,
              origin: getState().author,
            }),
          ),
      );
    },

    enableJobWorker: (enabled) => {
      submitThenRefresh((live) =>
        agentClient.enableJobWorker(live, { enabled, origin: getState().author }),
      );
    },

    // ── Governance ──
    // Every submit is signed by THIS node's validator key (the daemon ignores the
    // claimed origin), so these carry no origin. `refresh()` re-reads the proposal
    // set after each write.
    proposeSignal: (text) => {
      const body = text.trim();
      if (!body) return;
      submitThenRefresh((live) =>
        governanceClient.propose(live, {
          proposalId: crypto.randomUUID(),
          action: { Signal: { text: body } },
        }),
      );
    },

    voteProposal: (proposalId, approve) => {
      if (!proposalId) return;
      submitThenRefresh((live) =>
        governanceClient.vote(live, { proposalId, approve }),
      );
    },

    executeProposal: (proposalId) => {
      if (!proposalId) return;
      submitThenRefresh((live) =>
        governanceClient.execute(live, { proposalId }),
      );
    },

    // ── Inbox ──
    // The local member's queue is keyed by the author identity; mark/clear act on
    // the highest seq currently loaded, so "mark all read" needs no per-item loop.
    markInboxRead: () => {
      const items = getState().inbox;
      if (items.length === 0) return;
      const upToSeq = items[items.length - 1].seq;
      submitThenRefresh((live) =>
        inboxClient.markRead(live, { member: getState().author, upToSeq }),
      );
    },

    markInboxReadTo: (seq) => {
      submitThenRefresh((live) =>
        inboxClient.markRead(live, { member: getState().author, upToSeq: seq }),
      );
    },

    clearInbox: () => {
      const items = getState().inbox;
      if (items.length === 0) return;
      const upToSeq = items[items.length - 1].seq;
      submitThenRefresh((live) =>
        inboxClient.clear(live, { member: getState().author, upToSeq }),
      );
    },

    deliverNotification: ({ member, kind, body }) => {
      if (!member.trim() || !kind.trim()) return;
      submitThenRefresh((live) =>
        inboxClient.deliver(live, { member: member.trim(), kind: kind.trim(), body }),
      );
    },

    // ── Jobs ──
    // Identity-gated ops (cancel/prune by submitter; finalize/release by
    // claimant) all ride the daemon's default identity — origin is omitted — so
    // submitter and claimant stay consistent for this node's own jobs.
    submitJob: ({ kind, spec }) => {
      if (!kind.trim()) return;
      submitThenRefresh((live) =>
        jobsClient.submitJob(live, { jobId: crypto.randomUUID(), kind: kind.trim(), spec }),
      );
    },

    claimJob: ({ jobId, leaseViews }) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.claimJob(live, { jobId, leaseViews }));
    },

    finalizeJob: ({ jobId, ok, payload }) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.finalizeJob(live, { jobId, ok, payload }));
    },

    releaseJob: (jobId) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.releaseJob(live, { jobId }));
    },

    reclaimJob: (jobId) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.reclaimJob(live, { jobId }));
    },

    cancelJob: (jobId) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.cancelJob(live, { jobId }));
    },

    pruneJob: (jobId) => {
      if (!jobId) return;
      submitThenRefresh((live) => jobsClient.pruneJob(live, { jobId }));
    },

    // ── Automations ──
    createRule: ({ ruleId, trigger, action }) => {
      const id = ruleId.trim();
      if (!id) return;
      submitThenRefresh((live) =>
        automationsClient.createRule(live, { ruleId: id, trigger, action }),
      );
    },

    setRuleEnabled: (ruleId, enabled) => {
      if (!ruleId) return;
      submitThenRefresh((live) =>
        automationsClient.setEnabled(live, { ruleId, enabled }),
      );
    },

    deleteRule: (ruleId) => {
      if (!ruleId) return;
      submitThenRefresh((live) => automationsClient.deleteRule(live, ruleId));
    },

    // ── Memory ──
    browseMemory: (path) => {
      const live = getNode();
      const dir = path || "/";
      if (!live) return;
      // set the active dir immediately (so refresh re-lists it), clear the open
      // file + any search, then list eagerly for a snappy transition.
      patch({ memoryPath: dir, memoryOpen: null, memoryMatches: null });
      Promise.resolve()
        .then(() => memoryClient.ls(live, { path: dir }))
        .then((memoryEntries) => patch({ memoryEntries }))
        .catch(fail);
    },

    openMemoryFile: ({ path, generation }) => {
      const live = getNode();
      if (!live || !path) return;
      Promise.resolve()
        .then(() =>
          Promise.all([
            memoryClient.stat(live, path),
            memoryClient.read(live, { path, generation: generation ?? null }),
          ]),
        )
        .then(([stat, gen]) =>
          patch({ memoryOpen: stat && gen ? { stat, generation: gen } : null }),
        )
        .catch(fail);
    },

    closeMemoryFile: () => patch({ memoryOpen: null }),

    publishMemory: ({ path, text, meta }) => {
      const p = path.trim();
      if (!p) return;
      submitThenRefresh((live) =>
        memoryClient.publish(live, { path: p, body: memoryClient.inlineBody(text), meta }),
      ).then(() => {
        const live = getNode();
        if (!live) return;
        // reflect the new generation in the open viewer if it is this file.
        if (getState().memoryOpen?.stat.path === p) {
          return Promise.all([
            memoryClient.stat(live, p),
            memoryClient.read(live, { path: p, generation: null }),
          ])
            .then(([stat, gen]) =>
              patch({ memoryOpen: stat && gen ? { stat, generation: gen } : null }),
            )
            .catch(fail);
        }
      });
    },

    deleteMemory: (path) => {
      if (!path) return;
      submitThenRefresh((live) => memoryClient.remove(live, path)).then(() => {
        if (getState().memoryOpen?.stat.path === path) patch({ memoryOpen: null });
      });
    },

    searchMemory: ({ prefix, pattern }) => {
      const live = getNode();
      if (!live || !pattern) return;
      Promise.resolve()
        .then(() => memoryClient.grep(live, { prefix: prefix || "/", pattern }))
        .then((memoryMatches) => patch({ memoryMatches }))
        .catch(fail);
    },

    clearMemorySearch: () => patch({ memoryMatches: null }),

    // ── Files ──
    uploadFile: ({ name, mime, bytes }) => {
      const cleanName = name.trim();
      if (!cleanName) return;
      submitThenRefresh((live) =>
        filesClient.uploadFile(live, {
          fileId: crypto.randomUUID(),
          name: cleanName,
          mime: mime || "application/octet-stream",
          bytes,
        }),
      );
    },

    removeFile: (fileId) => {
      if (!fileId) return;
      submitThenRefresh((live) => filesClient.removeManifest(live, fileId));
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
        tasks: [],
        docIds: [],
        activeDoc: null,
        activeDocBlocks: [],
        pages: [],
        activePage: null,
        activePageBlocks: [],
        agents: [],
        watches: [],
        runs: [],
        inbox: [],
        inboxUnread: 0,
        jobs: [],
        jobCounts: null,
        rules: [],
        memoryPath: "/",
        memoryEntries: [],
        memoryOpen: null,
        memoryMatches: null,
        files: [],
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
        channels: [],
        messages: [],
        activeChannel: null,
        activeThread: null,
        authorNames: {},
        tasks: [],
        members: [],
        proposals: [],
        forgeHead: null,
        docIds: [],
        activeDoc: null,
        activeDocBlocks: [],
        pages: [],
        activePage: null,
        activePageBlocks: [],
        agents: [],
        watches: [],
        runs: [],
        inbox: [],
        inboxUnread: 0,
        jobs: [],
        jobCounts: null,
        rules: [],
        memoryPath: "/",
        memoryEntries: [],
        memoryOpen: null,
        memoryMatches: null,
        files: [],
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

    forgetWorkspace: () => {
      const target = getState().workspace;
      if (!target) return;
      patch({ onboardingBusy: true, error: null });
      // Call the GUARDED backend FIRST — it refuses while this node is still a
      // current validator of a set of two-or-more. Only tear down the local
      // node + projections once the backend has actually forgotten it, so a
      // refused forget leaves the live UI intact (still connected, error shown).
      Promise.resolve()
        .then(() => ws.forgetWorkspace(target.id))
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
            tasks: [],
            docIds: [],
            activeDoc: null,
            activeDocBlocks: [],
            pages: [],
            activePage: null,
            activePageBlocks: [],
            agents: [],
            watches: [],
            runs: [],
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
          patch({ onboardingBusy: false });
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
