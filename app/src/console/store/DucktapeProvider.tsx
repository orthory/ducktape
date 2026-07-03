// The console's one stateful component: resolves the node (adopt-or-spawn on
// desktop, dial on web), hydrates from it, re-queries committed state on every
// finalized block, and hands views a stable actions surface. Views stay
// render-only.
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

import * as chatClient from "../../domain/chat-client";
import * as forgeClient from "../../domain/forge-client";
import * as tasksClient from "../../domain/tasks-client";
import { ensureDaemon, resolveNode, shutdownNode } from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
import {
  channelIdOf,
  createInitialState,
  nextTaskStatus,
} from "./state";
import type { ConsoleState } from "./state";

// ── Context ─────────────────────────────────────────────

export interface ConsoleActions {
  setScreen(screen: string): void;
  setAccent(accent: string): void;
  setAuthor(author: string): void;
  selectChannel(channelId: string): void;
  createChannel(name: string): void;
  sendMessage(body: string): void;
  openThread(rootSeq: number): void;
  closeThread(): void;
  replyInThread(body: string): void;
  addTask(title: string): void;
  advanceTask(taskId: string): void;
  commitForge(params: { path: string; content: string; message: string }): void;
  /** Ask the managed daemon to exit (desktop only). */
  stopNode(): void;
  /** Re-spawn / re-adopt the managed daemon after a stop (desktop only). */
  startNode(): void;
  dismissError(): void;
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

  const fail = useCallback(
    (err: unknown) => setState((prev) => ({ ...prev, error: String(err) })),
    [],
  );

  // 1. Resolve the node once: adopt-or-spawn the daemon on desktop, dial the
  //    configured url on web. Injected transports (tests) skip this.
  useEffect(() => {
    if (node) return;
    let cancelled = false;
    resolveNode()
      .then((resolution) => {
        if (cancelled) return;
        setState((prev) => ({
          ...prev,
          nodeUrl: resolution.url,
          managed: resolution.managed,
        }));
        setNode(resolution.transport);
      })
      .catch((err) => {
        if (!cancelled) fail(err);
      });
    return () => {
      cancelled = true;
    };
  }, [node, fail]);

  // 2. Pull every committed projection; adopt the first channel when none is
  //    active yet (or the active one vanished from a fresh node).
  const refresh = useCallback(
    () => {
      const live = nodeRef.current;
      if (!live) return Promise.resolve();
      return Promise.resolve()
        .then(() =>
          Promise.all([
            live.status(),
            chatClient.channels(live),
            tasksClient.listTasks(live),
            forgeClient.head(live),
          ]),
        )
        .then(([status, channels, tasks, forgeHead]) => {
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
      }));
      Promise.resolve()
        .then(() => chatClient.latestMessages(live, channelId))
        .then((messages) => setState((prev) => ({ ...prev, messages })))
        .catch(fail);
    };

    return {
      setScreen: (screen) => setState((prev) => ({ ...prev, screen })),
      setAccent: (accent) => setState((prev) => ({ ...prev, accent })),
      setAuthor: (author) => setState((prev) => ({ ...prev, author })),

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

      closeThread: () => setState((prev) => ({ ...prev, activeThread: null })),

      replyInThread: (body) => {
        const live = nodeRef.current;
        const channelId = stateRef.current.activeChannel;
        const root = stateRef.current.activeThread?.root;
        if (!live || !channelId || !root || !body.trim()) return;
        Promise.resolve()
          .then(() =>
            chatClient.postMessage(live, {
              channelId,
              messageId: crypto.randomUUID(),
              text: body.trim(),
              origin: stateRef.current.author,
              thread: root.seq,
            }),
          )
          .then(() =>
            chatClient.thread(live, { channelId, rootSeq: root.seq }),
          )
          .then((activeThread) => {
            setState((prev) => ({ ...prev, activeThread }));
            return refresh();
          })
          .catch(fail);
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

      stopNode: () => {
        const url = stateRef.current.nodeUrl;
        if (!url || !stateRef.current.managed) return;
        Promise.resolve()
          .then(() => shutdownNode(url))
          .then(() => setState((prev) => ({ ...prev, connected: false })))
          .catch(fail);
      },

      startNode: () => {
        const live = nodeRef.current;
        if (!live || !stateRef.current.managed) return;
        Promise.resolve()
          .then(() => ensureDaemon(live))
          .then(() => refresh())
          .catch(fail);
      },

      dismissError: () => setState((prev) => ({ ...prev, error: null })),
    };
  }, [refresh, fail]);

  const value = useMemo(() => ({ state, actions }), [state, actions]);
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
