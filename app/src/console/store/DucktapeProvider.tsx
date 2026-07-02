// The console's one stateful component: owns the injected NodeTransport,
// hydrates from the node, re-queries committed state on every finalized block
// (ws frames on web, window events on desktop), and hands views a stable
// actions surface. Views stay render-only.
//
// Writes follow the node's model: submit one msg (one block), then re-query —
// there is no optimistic local state to reconcile. The post-submit refresh is
// deliberately kept even though block events also refresh: it covers a dead
// event stream, and a double refresh of cheap queries is harmless.
//
// Actions read live state through stateRef, never inside setState updaters —
// updaters must stay pure (StrictMode double-invokes them, which would
// double-submit blocks).

import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";

import * as chatClient from "../../domain/chat-client";
import * as tasksClient from "../../domain/tasks-client";
import { getTransport } from "../../domain/transport";
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
  openThread(rootId: string): void;
  closeThread(): void;
  replyInThread(body: string): void;
  addTask(title: string): void;
  advanceTask(taskId: string): void;
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
  /** Injected in tests; production resolves the variant via getTransport(). */
  transport?: NodeTransport;
  children: ReactNode;
}) {
  const [state, setState] = useState<ConsoleState>(createInitialState);
  const node = useMemo(() => transport ?? getTransport(), [transport]);

  // actions and block-event callbacks read CURRENT state here, not the
  // snapshot captured when they were created
  const stateRef = useRef(state);
  stateRef.current = state;

  const fail = useCallback(
    (err: unknown) => setState((prev) => ({ ...prev, error: String(err) })),
    [],
  );

  // 1. Pull every committed projection; adopt the first channel when none is
  //    active yet (or the active one vanished from a fresh node).
  const refresh = useCallback(
    () =>
      Promise.resolve()
        .then(() =>
          Promise.all([
            node.status(),
            chatClient.channels(node),
            tasksClient.listTasks(node),
          ]),
        )
        .then(([status, channels, tasks]) => {
          const current = stateRef.current.activeChannel;
          const active =
            current && channels.some((c) => c.id === current)
              ? current
              : (channels[0]?.id ?? null);
          return Promise.resolve()
            .then(() => (active ? chatClient.messages(node, active) : []))
            .then((messages) =>
              setState((prev) => ({
                ...prev,
                connected: true,
                status,
                channels,
                tasks,
                activeChannel: active,
                messages,
              })),
            );
        })
        .catch((err) => {
          setState((prev) => ({ ...prev, connected: false }));
          fail(err);
        }),
    [node, fail],
  );

  // 2. Hydrate once, then follow the block stream.
  useEffect(() => {
    refresh();
    return node.onBlock(() => {
      refresh();
    });
  }, [node, refresh]);

  // 3. Reflect the accent into the css var the theme reads.
  useEffect(() => {
    document.documentElement.style.setProperty("--accent", state.accent);
  }, [state.accent]);

  const actions = useMemo<ConsoleActions>(() => {
    const submitThenRefresh = (submit: () => Promise<unknown>) =>
      Promise.resolve()
        .then(submit)
        .then(() => refresh())
        .catch(fail);

    return {
      setScreen: (screen) => setState((prev) => ({ ...prev, screen })),
      setAccent: (accent) => setState((prev) => ({ ...prev, accent })),
      setAuthor: (author) => setState((prev) => ({ ...prev, author })),

      selectChannel: (channelId) => {
        setState((prev) => ({
          ...prev,
          activeChannel: channelId,
          activeThread: null,
        }));
        Promise.resolve()
          .then(() => chatClient.messages(node, channelId))
          .then((messages) => setState((prev) => ({ ...prev, messages })))
          .catch(fail);
      },

      createChannel: (name) => {
        const channelId = channelIdOf(name);
        if (!channelId) return;
        submitThenRefresh(() =>
          chatClient.createChannel(node, { channelId, name }),
        ).then(() =>
          setState((prev) => ({ ...prev, activeChannel: channelId })),
        );
      },

      sendMessage: (body) => {
        const channelId = stateRef.current.activeChannel;
        if (!channelId || !body.trim()) return;
        submitThenRefresh(() =>
          chatClient.sendMessage(node, {
            channelId,
            messageId: crypto.randomUUID(),
            author: stateRef.current.author,
            body: body.trim(),
          }),
        );
      },

      openThread: (rootId) => {
        const channelId = stateRef.current.activeChannel;
        if (!channelId) return;
        Promise.resolve()
          .then(() => chatClient.thread(node, { channelId, threadId: rootId }))
          .then((activeThread) =>
            setState((prev) => ({ ...prev, activeThread })),
          )
          .catch(fail);
      },

      closeThread: () => setState((prev) => ({ ...prev, activeThread: null })),

      replyInThread: (body) => {
        const channelId = stateRef.current.activeChannel;
        const root = stateRef.current.activeThread?.root;
        if (!channelId || !root || !body.trim()) return;
        Promise.resolve()
          .then(() =>
            chatClient.replyInThread(node, {
              channelId,
              threadId: root.id,
              messageId: crypto.randomUUID(),
              author: stateRef.current.author,
              body: body.trim(),
            }),
          )
          .then(() => chatClient.thread(node, { channelId, threadId: root.id }))
          .then((activeThread) => {
            setState((prev) => ({ ...prev, activeThread }));
            return refresh();
          })
          .catch(fail);
      },

      addTask: (title) => {
        if (!title.trim()) return;
        submitThenRefresh(() =>
          tasksClient.createTask(node, {
            taskId: crypto.randomUUID(),
            title: title.trim(),
          }),
        );
      },

      advanceTask: (taskId) => {
        const task = stateRef.current.tasks.find((t) => t.id === taskId);
        if (!task || task.status === "Done") return;
        submitThenRefresh(() =>
          tasksClient.updateStatus(node, {
            taskId,
            status: nextTaskStatus(task.status),
          }),
        );
      },

      dismissError: () => setState((prev) => ({ ...prev, error: null })),
    };
  }, [node, refresh, fail]);

  const value = useMemo(() => ({ state, actions }), [state, actions]);
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
