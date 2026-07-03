// The console's one stateful component: resolves the node (a ~/.ducktape
// workspace on desktop, a dialed url on web), hydrates from it, re-queries
// committed state on every finalized block, and hands views a stable actions
// surface. Views stay render-only.
//
// Onboarding lives here too (desktop): with no active workspace the gate shows;
// founding connects immediately, joining parks and this provider polls the
// workspace phase (parked→admitted→promoted) until the promoted node's surface
// answers — see connectActive.
//
// Writes follow the node's model: submit one msg (one block), then re-query —
// there is no optimistic local state to reconcile. The post-submit refresh is
// deliberately kept even though block events also refresh: it covers a dead
// event stream, and a double refresh of cheap queries is harmless.
//
// Actions read live state through refs, never inside setState updaters —
// updaters must stay pure (StrictMode double-invokes them, which would
// double-submit blocks).

import {
  createContext,
  useCallback,
  useEffect,
  useRef,
  useState,
  useMemo,
} from "react";
import type { ReactNode } from "react";

import * as agentClient from "../../domain/agent-client";
import type { TurnPolicy } from "../../domain/agent-client";
import * as chatClient from "../../domain/chat-client";
import * as documentClient from "../../domain/document-client";
import type { Block, BlockKind } from "../../domain/document-client";
import * as forgeClient from "../../domain/forge-client";
import * as profilesClient from "../../domain/profiles-client";
import * as tasksClient from "../../domain/tasks-client";
import {
  connectWorkspace,
  isTauri,
  resolveNode,
  shutdownNode,
  waitUntilUp,
} from "../../domain/node-bootstrap";
import * as ws from "../../domain/workspace-client";
import type { Workspace } from "../../domain/workspace-client";
import type { NodeTransport } from "../../domain/transport";
import {
  channelIdOf,
  createInitialState,
  docIdOf,
  nextTaskStatus,
} from "./state";
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

// ── Per-node document registry ──────────────────────────
//
// The document module has NO "list docs" query — its store is keyed by
// sha256(doc_id) and cannot enumerate — so the set of known doc-ids is tracked
// CLIENT-SIDE and persisted per resolved node url (i.e. per workspace). This is
// a convenience registry, not a source of truth: a doc created on another
// client won't appear here until its id is opened by hand.
const docRegistryKey = (nodeUrl: string): string => `ducktape.docs.${nodeUrl}`;

const loadDocIds = (nodeUrl: string): string[] => {
  try {
    const raw = localStorage.getItem(docRegistryKey(nodeUrl));
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((id): id is string => typeof id === "string")
      : [];
  } catch {
    return []; // storage unavailable / corrupt entry — start from an empty list
  }
};

const saveDocIds = (nodeUrl: string, docIds: string[]): void => {
  try {
    localStorage.setItem(docRegistryKey(nodeUrl), JSON.stringify(docIds));
  } catch {
    // storage may be unavailable (private mode / quota); the registry is a
    // convenience, so a failed persist is non-fatal.
  }
};

// ── Context ─────────────────────────────────────────────

export interface ConsoleActions {
  setScreen(screen: string): void;
  setAccent(accent: string): void;
  setAuthor(author: string): void;
  /** Set our own display name in the `profiles` module (origin-gated SetName)
   *  and keep it as the local author identity, so it propagates to everyone. */
  setDisplayName(name: string): void;
  selectChannel(channelId: string): void;
  createChannel(name: string): void;
  sendMessage(body: string): void;
  openThread(rootSeq: number): void;
  closeThread(): void;
  replyInThread(body: string): void;
  /** Toggle our own reaction on a message: adds it if we haven't reacted with
   *  that emoji yet, removes it if we have. Refreshes the open thread panel
   *  too, since its replies are a separate snapshot from `state.messages`. */
  toggleReaction(seq: number, emoji: string): void;
  /** Which message (by seq) the hover action bar is anchored to, or null. */
  setHoverMsg(seq: number | null): void;
  /** Which message's "⋯" overflow menu is open, or null. */
  setMsgMenu(seq: number | null): void;
  addTask(title: string): void;
  advanceTask(taskId: string): void;
  commitForge(params: { path: string; content: string; message: string }): void;

  // ── Documents (block store over the `document` module) ──
  /** Create a doc (CreateDoc, idempotent), register it, and open it. */
  createDoc(docId: string): void;
  /** Register + open a doc by id, loading its blocks (like selectChannel). */
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

export interface ConsoleContextValue {
  state: ConsoleState;
  actions: ConsoleActions;
}

export const ConsoleContext = createContext<ConsoleContextValue | null>(null);

// ── Provider ────────────────────────────────────────────

export function DucktapeProvider({
  transport,
  children,
}: {
  /** Injected in tests; production resolves the node via node-bootstrap. */
  transport?: NodeTransport;
  children: ReactNode;
}) {
  const [state, setState] = useState<ConsoleState>(createInitialState);
  const [node, setNode] = useState<NodeTransport | null>(transport ?? null);

  // actions and block-event callbacks read CURRENT values here, not the
  // snapshots captured when they were created
  const stateRef = useRef(state);
  stateRef.current = state;
  const nodeRef = useRef(node);
  nodeRef.current = node;

  // stale-guards async boot/connect loops: each connectActive bumps the
  // generation, so a superseded loop (workspace switch, re-select) sees its gen
  // change and stops touching state.
  const bootGenRef = useRef(0);
  const bootStartedRef = useRef(false);

  const fail = useCallback(
    (err: unknown) => setState((prev) => ({ ...prev, error: String(err) })),
    [],
  );

  // Connect the app to a workspace's node: select it (Rust spawns/adopts),
  // then either wait for a member's surface to answer, or poll a joiner's
  // park→promote phase until its promoted validator surface comes up.
  const connectActive = useCallback(
    (target: Workspace): Promise<void> => {
      const gen = (bootGenRef.current += 1);
      const stale = () => bootGenRef.current !== gen;
      setState((prev) => ({
        ...prev,
        workspace: target,
        needsOnboarding: false,
        onboardingBusy: false,
        inviteBlob: null,
      }));
      return Promise.resolve()
        .then(() => ws.selectWorkspace(target.id))
        .then((sel) => {
          if (stale()) return;
          setState((prev) => ({ ...prev, nodeUrl: sel.httpUrl, managed: true }));
          const transport = connectWorkspace(sel.httpUrl).transport;
          if (target.member) {
            // founder / already-admitted member: the surface comes up promptly.
            return waitUntilUp(transport).then(() => {
              if (stale()) return;
              setState((prev) => ({ ...prev, onboardingPhase: null }));
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
                setState((prev) => ({ ...prev, onboardingPhase: null }));
                setNode(transport);
              },
              () =>
                ws.workspacePhase(target.id).then((report) => {
                  if (stale()) return;
                  setState((prev) => ({ ...prev, onboardingPhase: report }));
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
            setState((prev) => ({ ...prev, onboardingBusy: false }));
            fail(err);
          }
        });
    },
    [fail],
  );

  // 1. Resolve the node once. Web: dial the configured url. Desktop: resolve
  //    via the ~/.ducktape registry — connect the active workspace, or raise
  //    the onboarding gate when there is none. Injected transports (tests) and
  //    a re-run under StrictMode are both skipped.
  useEffect(() => {
    if (transport || bootStartedRef.current) return;
    bootStartedRef.current = true;

    if (!isTauri()) {
      const resolution = resolveNode();
      setState((prev) => ({
        ...prev,
        nodeUrl: resolution.url,
        managed: false,
        needsOnboarding: false,
      }));
      setNode(resolution.transport);
      return;
    }

    let cancelled = false;
    Promise.resolve()
      .then(() => Promise.all([ws.listWorkspaces(), ws.activeWorkspace()]))
      .then(([all, active]) => {
        if (cancelled) return;
        setState((prev) => ({ ...prev, workspaces: all }));
        if (!active) {
          setState((prev) => ({ ...prev, needsOnboarding: true }));
          return;
        }
        return connectActive(active);
      })
      .catch((err) => {
        if (!cancelled) fail(err);
      });
    // Reset the guard on cleanup so StrictMode's mount→unmount→remount re-runs
    // the boot: without this the first mount's async resolve is cancelled while
    // the guard blocks the remount, so connectActive never fires and the app is
    // stuck unmanaged. (The remount's connectActive is idempotent — it adopts an
    // already-listening node rather than double-spawning.)
    return () => {
      cancelled = true;
      bootStartedRef.current = false;
    };
  }, [transport, fail, connectActive]);

  // 2. Pull every committed projection; adopt the first channel when none is
  //    active yet (or the active one vanished from a fresh node).
  const refresh = useCallback(
    () => {
      const live = nodeRef.current;
      if (!live) return Promise.resolve();
      // the document module has no bulk read, so re-query only the open doc
      // (null when none is open) alongside the other projections.
      const activeDoc = stateRef.current.activeDoc;
      return Promise.resolve()
        .then(() =>
          Promise.all([
            live.status(),
            chatClient.channels(live),
            tasksClient.listTasks(live),
            forgeClient.head(live),
            activeDoc
              ? documentClient.getDoc(live, activeDoc)
              : Promise.resolve<Block[] | null>(null),
            agentClient.agents(live),
            agentClient.watches(live),
            // newest-first for the timeline; Runs is ascending on the wire.
            agentClient
              .runs(live, { channelId: null, limit: 50 })
              .then((list) => [...list].reverse()),
            profilesClient.allProfiles(live, { from: 0, limit: 256 }),
          ]),
        )
        .then(([status, channels, tasks, forgeHead, docBlocks, agents, watches, runs, profiles]) => {
          // Profile.key is the origin bytes — the same bytes AuthorRef::User
          // carries — so hex(key) is exactly authorName's AuthorNames key.
          const authorNames = Object.fromEntries(
            profiles.map((p) => [chatClient.keyHex(p.key), p.display_name]),
          );
          const current = stateRef.current.activeChannel;
          const active =
            current && channels.some((c) => c.id === current)
              ? current
              : (channels[0]?.id ?? null);
          return Promise.resolve()
            .then(() => (active ? chatClient.latestMessages(live, active) : []))
            .then((messages) =>
              setState((prev) => ({
                ...prev,
                connected: true,
                status,
                channels,
                tasks,
                forgeHead,
                activeChannel: active,
                messages,
                authorNames,
                activeDocBlocks: docBlocks ?? [],
                agents,
                watches,
                runs,
              })),
            );
        })
        .catch((err) => {
          setState((prev) => ({ ...prev, connected: false }));
          fail(err);
        });
    },
    [fail],
  );

  // 3. Hydrate once the node is resolved, then follow the block stream.
  useEffect(() => {
    if (!node) return;
    refresh();
    return node.onBlock(() => {
      refresh();
    });
  }, [node, refresh]);

  // 4. Reflect the accent into the css var the theme reads.
  useEffect(() => {
    document.documentElement.style.setProperty("--accent", state.accent);
  }, [state.accent]);

  // 5. Load the per-node doc registry when the node url resolves or changes,
  //    and drop any open doc — a different node has different documents.
  //    Writes go the other way through openDoc (the only place docIds grows).
  useEffect(() => {
    const url = state.nodeUrl;
    if (!url) return;
    setState((prev) => ({
      ...prev,
      docIds: loadDocIds(url),
      activeDoc: null,
      activeDocBlocks: [],
    }));
  }, [state.nodeUrl]);

  // 6. Menu-bar popover navigation (desktop/macOS): the tray popover is a
  //    separate webview, so it asks the console to switch screens by having Rust
  //    emit `ducktape://navigate` after showing this window. Inert on web.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<string>("ducktape://navigate", (event) => {
          const screen = event.payload;
          if (screen) setState((prev) => ({ ...prev, screen }));
        }),
      )
      .then((un) => {
        if (cancelled) un();
        else unlisten = un;
      })
      .catch(() => {
        // event API unavailable (non-tauri / permission) — navigation just no-ops.
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const actions = useMemo<ConsoleActions>(() => {
    const submitThenRefresh = (submit: (live: NodeTransport) => Promise<unknown>) => {
      const live = nodeRef.current;
      if (!live) return Promise.resolve();
      return Promise.resolve()
        .then(() => submit(live))
        .then(() => refresh())
        .catch(fail);
    };

    // switching channels means: new active channel, thread panel closed, and
    // THAT channel's messages loaded — every path into a channel goes here
    const enterChannel = (channelId: string) => {
      const live = nodeRef.current;
      if (!live) return;
      setState((prev) => ({
        ...prev,
        activeChannel: channelId,
        activeThread: null,
        hoverMsg: null,
        msgMenuId: null,
      }));
      Promise.resolve()
        .then(() => chatClient.latestMessages(live, channelId))
        .then((messages) => setState((prev) => ({ ...prev, messages })))
        .catch(fail);
    };

    // the single entry point into a doc: record it in the per-node registry
    // (persist), make it active, and load its blocks. Every path into a doc
    // (new-doc, open-by-id, a registry click) goes here — like enterChannel.
    const enterDoc = (rawId: string) => {
      const live = nodeRef.current;
      const docId = docIdOf(rawId);
      if (!live || !docId) return;
      const known = stateRef.current.docIds;
      const docIds = known.includes(docId) ? known : [...known, docId];
      const url = stateRef.current.nodeUrl;
      if (url) saveDocIds(url, docIds);
      setState((prev) => ({
        ...prev,
        docIds,
        activeDoc: docId,
        activeDocBlocks: [],
      }));
      Promise.resolve()
        .then(() => documentClient.getDoc(live, docId))
        .then((blocks) =>
          setState((prev) => ({ ...prev, activeDocBlocks: blocks ?? [] })),
        )
        .catch(fail);
    };

    return {
      setScreen: (screen) => setState((prev) => ({ ...prev, screen })),
      setAccent: (accent) => setState((prev) => ({ ...prev, accent })),
      setAuthor: (author) => setState((prev) => ({ ...prev, author })),

      // Keep the local author identity (still the web-origin string) AND submit
      // SetName so the chosen name propagates: it's origin-gated, so passing our
      // origin sets our OWN profile only. Refresh re-reads authorNames.
      setDisplayName: (name) => {
        setState((prev) => ({ ...prev, author: name }));
        submitThenRefresh((live) =>
          profilesClient.setName(live, {
            displayName: name,
            origin: stateRef.current.author,
          }),
        );
      },

      selectChannel: enterChannel,

      createChannel: (name) => {
        const channelId = channelIdOf(name);
        if (!channelId) return;
        submitThenRefresh((live) =>
          chatClient.createChannel(live, {
            channelId,
            name,
            origin: stateRef.current.author,
          }),
        ).then(() => enterChannel(channelId));
      },

      sendMessage: (body) => {
        const channelId = stateRef.current.activeChannel;
        if (!channelId || !body.trim()) return;
        submitThenRefresh((live) =>
          chatClient.postMessage(live, {
            channelId,
            messageId: crypto.randomUUID(),
            text: body.trim(),
            origin: stateRef.current.author,
          }),
        );
      },

      openThread: (rootSeq) => {
        const live = nodeRef.current;
        const channelId = stateRef.current.activeChannel;
        if (!live || !channelId) return;
        Promise.resolve()
          .then(() => chatClient.thread(live, { channelId, rootSeq }))
          .then((activeThread) =>
            setState((prev) => ({ ...prev, activeThread })),
          )
          .catch(fail);
      },

      closeThread: () =>
        setState((prev) => ({ ...prev, activeThread: null })),

      // Re-queries just the open thread's replies after the write: `refresh()`
      // already re-pulls `state.messages` (which carries every sequence,
      // replies included) via `submitThenRefresh`, but the thread panel reads
      // its own `ChatThread` snapshot, so that one extra cheap query keeps the
      // panel in sync without repeating the old heavy-refresh-twice pattern.
      replyInThread: (body) => {
        const channelId = stateRef.current.activeChannel;
        const root = stateRef.current.activeThread?.root;
        if (!channelId || !root || !body.trim()) return;
        submitThenRefresh((live) =>
          chatClient.postMessage(live, {
            channelId,
            messageId: crypto.randomUUID(),
            text: body.trim(),
            origin: stateRef.current.author,
            thread: root.seq,
          }),
        ).then(() => {
          const live = nodeRef.current;
          if (!live) return;
          return chatClient
            .thread(live, { channelId, rootSeq: root.seq })
            .then((activeThread) =>
              setState((prev) =>
                prev.activeThread?.root.seq === root.seq
                  ? { ...prev, activeThread }
                  : prev,
              ),
            )
            .catch(fail);
        });
      },

      toggleReaction: (seq, emoji) => {
        const channelId = stateRef.current.activeChannel;
        if (!channelId) return;
        const target =
          stateRef.current.messages.find((m) => m.seq === seq) ??
          (stateRef.current.activeThread?.root.seq === seq
            ? stateRef.current.activeThread.root
            : stateRef.current.activeThread?.replies.find((m) => m.seq === seq));
        if (!target) return;
        const origin = stateRef.current.author;
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
          const live = nodeRef.current;
          const root = stateRef.current.activeThread?.root;
          if (!live || !root) return;
          return chatClient
            .thread(live, { channelId, rootSeq: root.seq })
            .then((activeThread) =>
              setState((prev) =>
                prev.activeThread?.root.seq === root.seq
                  ? { ...prev, activeThread }
                  : prev,
              ),
            )
            .catch(fail);
        });
      },

      setHoverMsg: (seq) => setState((prev) => ({ ...prev, hoverMsg: seq })),
      setMsgMenu: (seq) => setState((prev) => ({ ...prev, msgMenuId: seq })),

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
        const task = stateRef.current.tasks.find((t) => t.id === taskId);
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
            origin: stateRef.current.author,
          }),
        );
      },

      // ── Documents ──
      openDoc: enterDoc,

      createDoc: (rawId) => {
        const docId = docIdOf(rawId);
        if (!docId) return;
        // CreateDoc is idempotent and REQUIRED before any block op; then open
        // it (registers the id + loads blocks), mirroring createChannel.
        submitThenRefresh((live) => documentClient.createDoc(live, { docId })).then(
          () => enterDoc(docId),
        );
      },

      insertBlock: ({ after, kind, text }) => {
        const docId = stateRef.current.activeDoc;
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
        const docId = stateRef.current.activeDoc;
        if (!docId) return;
        submitThenRefresh((live) =>
          documentClient.updateBlock(live, { docId, blockId, text }),
        );
      },

      removeBlock: (blockId) => {
        const docId = stateRef.current.activeDoc;
        if (!docId) return;
        submitThenRefresh((live) =>
          documentClient.removeBlock(live, { docId, blockId }),
        );
      },

      moveBlock: ({ blockId, after }) => {
        const docId = stateRef.current.activeDoc;
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
                origin: stateRef.current.author,
              }),
            ),
        );
      },

      pauseAgent: (agentId) => {
        if (!agentId) return;
        submitThenRefresh((live) =>
          agentClient.pauseAgent(live, { agentId, origin: stateRef.current.author }),
        );
      },

      resumeAgent: (agentId) => {
        if (!agentId) return;
        submitThenRefresh((live) =>
          agentClient.resumeAgent(live, { agentId, origin: stateRef.current.author }),
        );
      },

      watchChannel: ({ channelId, policy }) => {
        if (!channelId) return;
        submitThenRefresh((live) =>
          agentClient.watchChannel(live, {
            channelId,
            policy,
            origin: stateRef.current.author,
          }),
        );
      },

      unwatchChannel: (channelId) => {
        if (!channelId) return;
        submitThenRefresh((live) =>
          agentClient.unwatchChannel(live, {
            channelId,
            origin: stateRef.current.author,
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
            origin: stateRef.current.author,
          }),
        );
      },

      cancelRun: (runId) => {
        if (!runId) return;
        submitThenRefresh((live) =>
          agentClient.cancelRun(live, { runId, origin: stateRef.current.author }),
        );
      },

      stopNode: () => {
        const url = stateRef.current.nodeUrl;
        if (!url || !stateRef.current.managed) return;
        Promise.resolve()
          .then(() => shutdownNode(url))
          .then(() => setState((prev) => ({ ...prev, connected: false })))
          .catch(fail);
      },

      startNode: () => {
        const target = stateRef.current.workspace;
        if (!stateRef.current.managed || !target) return;
        // re-select the active workspace: Rust adopts a live node or respawns
        // one, then connectActive reconnects and re-hydrates.
        connectActive(target).catch(fail);
      },

      dismissError: () => setState((prev) => ({ ...prev, error: null })),

      // ── Onboarding / workspaces ──
      createWorkspace: (name) => {
        if (!name.trim()) return;
        setState((prev) => ({ ...prev, onboardingBusy: true, error: null }));
        Promise.resolve()
          .then(() => ws.createWorkspace(name.trim()))
          .then((created) => {
            setState((prev) => ({
              ...prev,
              workspaces: mergeWorkspace(prev.workspaces, created),
            }));
            return connectActive(created);
          })
          .catch((err) => {
            setState((prev) => ({ ...prev, onboardingBusy: false }));
            fail(err);
          });
      },

      joinWorkspace: (name, blob) => {
        if (!name.trim() || !blob.trim()) return;
        setState((prev) => ({ ...prev, onboardingBusy: true, error: null }));
        Promise.resolve()
          .then(() => ws.joinWorkspace(name.trim(), blob.trim()))
          .then((joined) => {
            setState((prev) => ({
              ...prev,
              workspaces: mergeWorkspace(prev.workspaces, joined),
            }));
            return connectActive(joined);
          })
          .catch((err) => {
            setState((prev) => ({ ...prev, onboardingBusy: false }));
            fail(err);
          });
      },

      selectWorkspace: (id) => {
        const target = stateRef.current.workspaces.find((w) => w.id === id);
        if (!target || target.id === stateRef.current.workspace?.id) return;
        // drop the old node + its projections so the switch shows no stale state.
        setNode(null);
        setState((prev) => ({
          ...prev,
          connected: false,
          status: null,
          channels: [],
          messages: [],
          activeChannel: null,
          activeThread: null,
          hoverMsg: null,
          msgMenuId: null,
          authorNames: {},
          tasks: [],
          docIds: [],
          activeDoc: null,
          activeDocBlocks: [],
          agents: [],
          watches: [],
          runs: [],
          onboardingPhase: null,
        }));
        connectActive(target).catch(fail);
      },

      revealInvite: () => {
        const target = stateRef.current.workspace;
        if (!target) return;
        Promise.resolve()
          .then(() => ws.inviteBlob(target.id))
          .then((blob) => setState((prev) => ({ ...prev, inviteBlob: blob })))
          .catch(fail);
      },

      admitMember: (pubkey) => {
        const target = stateRef.current.workspace;
        if (!target || !pubkey.trim()) return;
        Promise.resolve()
          .then(() => ws.admitMember(target.id, pubkey.trim()))
          .then(() => refresh())
          .catch(fail);
      },

      newWorkspace: () =>
        setState((prev) => ({ ...prev, needsOnboarding: true, inviteBlob: null })),

      dismissOnboarding: () =>
        setState((prev) =>
          prev.workspace ? { ...prev, needsOnboarding: false } : prev,
        ),
    };
  }, [refresh, fail, connectActive]);

  const value = useMemo(() => ({ state, actions }), [state, actions]);
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
