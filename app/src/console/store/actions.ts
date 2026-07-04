import type { Dispatch } from "react";

import * as agentClient from "../../domain/agent-client";
import type { TurnPolicy } from "../../domain/agent-client";
import * as chatClient from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import * as documentClient from "../../domain/document-client";
import type { BlockKind } from "../../domain/document-client";
import * as forgeClient from "../../domain/forge-client";
import * as profilesClient from "../../domain/profiles-client";
import * as tasksClient from "../../domain/tasks-client";
import {
  connectWorkspace,
  shutdownNode,
  waitUntilUp,
} from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
import * as ws from "../../domain/workspace-client";
import type { Workspace } from "../../domain/workspace-client";
import { parseMessageInput } from "../views/chat/chat-input";
import type { Action } from "./reducer";
import { channelIdOf, docIdOf, nextTaskStatus } from "./state";
import type { ConsoleState } from "./state";

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
  /** Fetch the active workspace's invite blob into state for sharing. */
  revealInvite(): void;
  /** Admit a joiner by pubkey through the active (member) workspace. */
  admitMember(pubkey: string): void;
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

  // Connect the app to a workspace's node: select it (Rust spawns/adopts),
  // then either wait for a member's surface to answer, or poll a joiner's
  // park→promote phase until its promoted validator surface comes up.
  const connectActive = (target: Workspace): Promise<void> => {
    const gen = nextBootGeneration();
    const stale = () => isBootGenerationStale(gen);
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
        const transport = connectWorkspace(sel.httpUrl).transport;
        if (target.member) {
          // founder / already-admitted member: the surface comes up promptly.
          return waitUntilUp(transport).then(() => {
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
    setScreen: (screen) => patch({ screen }),
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

    stopNode: () => {
      const url = getState().nodeUrl;
      if (!url || !getState().managed) return;
      Promise.resolve()
        .then(() => shutdownNode(url))
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
        agents: [],
        watches: [],
        runs: [],
        onboardingPhase: null,
      });
      connectActive(target).catch(fail);
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

    newWorkspace: () => patch({ needsOnboarding: true, inviteBlob: null }),

    dismissOnboarding: () =>
      update((prev) => (prev.workspace ? { needsOnboarding: false } : {})),

    connectActive,
  };
}
