// Store wiring for the console: resolves/adopts the node transport, hydrates
// committed projections on boot and block events, and provides the stable
// actions facade. Business logic lives in actions.ts; state projection lives in
// state.ts.

import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";

import {
  isTauri,
  resolveNode,
} from "../../domain/node-bootstrap";
import { moduleTopic } from "../../domain/stream";
import type { NodeTransport } from "../../domain/transport";
import * as ws from "../../domain/workspace-client";
import { createActions } from "./actions";
import { ConsoleContext, type ConsoleContextValue } from "./context";
import {
  HUDDLE_CLOSED_EVENT,
  HUDDLE_CMD_EVENT,
  HUDDLE_CONTEXT_EVENT,
  applyHuddleWindowCmd,
  buildHuddleContext,
} from "./huddle-window";
import type { HuddleWindowCmd } from "./huddle-window";
import { refreshAll, refreshModules, type RefreshEnv } from "./refresh";
import { reducer } from "./reducer";
import {
  createInitialState,
  loadRemoteUrl,
} from "./state";

export type { ConsoleActions } from "./actions";
export type { ConsoleContextValue } from "./context";

export function DucktapeProvider({
  transport,
  children,
}: {
  /** Injected in tests; production resolves the node via node-bootstrap. */
  transport?: NodeTransport;
  children: ReactNode;
}) {
  const [state, dispatch] = useReducer(reducer, undefined, createInitialState);
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
    (err: unknown) =>
      dispatch({ type: "patch", patch: { error: String(err) } }),
    [],
  );

  const refresh = useCallback(() => {
    const live = nodeRef.current;
    if (!live) return Promise.resolve();
    return refreshAll({
      live,
      getState: () => stateRef.current,
      dispatch,
      fail,
    });
  }, [fail]);

  const actions = useMemo(
    () =>
      createActions({
        dispatch,
        getState: () => stateRef.current,
        getNode: () => nodeRef.current,
        setNode,
        refresh,
        fail,
        nextBootGeneration: () => (bootGenRef.current += 1),
        isBootGenerationStale: (generation) => bootGenRef.current !== generation,
      }),
    [],
  );

  const streamModuleKey = useMemo(
    () =>
      (state.status?.modules ?? [])
        .map((m) => m.id)
        .sort()
        .join("\0"),
    [state.status?.modules],
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
      dispatch({
        type: "patch",
        patch: {
          nodeUrl: resolution.url,
          managed: false,
          needsOnboarding: false,
        },
      });
      setNode(resolution.transport);
      return;
    }

    let cancelled = false;
    Promise.resolve()
      .then(() => Promise.all([ws.listWorkspaces(), ws.activeWorkspace()]))
      .then(([all, active]) => {
        if (cancelled) return;
        dispatch({ type: "patch", patch: { workspaces: all } });
        // A remembered remote node supersedes the local active workspace — it
        // was the user's last choice, so reconnect to it. An unreachable one
        // just reads as disconnected rather than blocking boot.
        const savedRemote = loadRemoteUrl();
        if (savedRemote) {
          actions.connectRemote(savedRemote);
          return;
        }
        if (!active) {
          dispatch({ type: "patch", patch: { needsOnboarding: true } });
          return;
        }
        return actions.connectActive(active);
      })
      .catch((err) => {
        if (cancelled) return;
        // A failed boot resolution (corrupt/locked ~/.ducktape registry,
        // permissions, a concurrent instance holding a lock) used to drop to a
        // hollow, disconnected shell with a toast that then vanished — no
        // workspace list, no way forward. Land on the onboarding gate, the
        // actionable front door, with the error shown, never an empty console.
        dispatch({ type: "patch", patch: { needsOnboarding: true } });
        fail(err);
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
  }, [transport, actions, fail]);

  // 2. Hydrate once the node is resolved.
  useEffect(() => {
    if (!node) return;
    refresh();
  }, [node, refresh]);

  // 2a. Follow module stream topics. The chain tip update is ungated; slice
  //     fetches are trailing-debounced and still honor the fresh-pending gate.
  useEffect(() => {
    if (!node || !streamModuleKey) return;
    const modules = streamModuleKey.split("\0").filter(Boolean);
    const dirty = new Set<string>();
    let timer: ReturnType<typeof setTimeout> | null = null;
    const env = (): RefreshEnv => ({
      live: node,
      getState: () => stateRef.current,
      dispatch,
      fail,
    });
    const clearFlush = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    };
    const flush = () => {
      timer = null;
      const modulesToRefresh = [...dirty];
      dirty.clear();
      void refreshModules(env(), modulesToRefresh, {
        includeBlocks: true,
        respectFreshPending: true,
      });
    };
    const schedule = () => {
      clearFlush();
      timer = setTimeout(flush, 100);
    };

    const off = node.subscribe(
      modules.map(moduleTopic),
      {
        onEvent: (frame) => {
          const module = frame.topic.startsWith("module:")
            ? frame.topic.slice("module:".length)
            : null;
          if (!module) return;
          dirty.add(module);
          // The live chain tip, UNGATED — recorded before the pending gate
          // below, so the console always knows the chain moved even while an op
          // of ours is in flight. An event at height N also proves the node's
          // tip is ≥ N, so status.height advances here too instead of waiting
          // up to a heartbeat interval (appHash refreshes on the next beat).
          dispatch({
            type: "update",
            fn: (prev) => {
              const patch: { lastBlock?: number; status?: typeof prev.status } = {};
              if (frame.op.height > (prev.lastBlock ?? -1)) {
                patch.lastBlock = frame.op.height;
              }
              if (prev.status && frame.op.height > prev.status.height) {
                patch.status = { ...prev.status, height: frame.op.height };
              }
              return patch;
            },
          });
          schedule();
        },
        onLagged: (topic) => {
          const module = topic.startsWith("module:")
            ? topic.slice("module:".length)
            : null;
          if (!module) return;
          dirty.delete(module);
          void refreshModules(env(), [module], { includeBlocks: true });
        },
      },
    );
    return () => {
      clearFlush();
      off();
    };
  }, [node, streamModuleKey, fail]);

  // 2b. Liveness heartbeat — connection banner, tip patching, and one-shot
  //     recovery on the up edge. The transport's watchdog turns silent stream
  //     death into a down signal; there is no separate reconnect poll here.
  useEffect(() => {
    if (!node) return;
    if (state.needsOnboarding || state.onboardingPhase) return;
    const recover = () => {
      void node.status().then(
        (s) => {
          // Recovery identity re-check: a foreign / different-build node could
          // have grabbed the reused port while ours was down (only the INITIAL
          // adopt used to verify this). Adopting it would silently show another
          // node's state. Guarded to when we know both keys (managed workspace).
          const expected = stateRef.current.workspace?.pubkey;
          const got = s.publicKey;
          if (expected && got && got.toLowerCase() !== expected.toLowerCase()) {
            if (stateRef.current.connected || !stateRef.current.connectionDown?.impostor) {
              dispatch({
                type: "patch",
                patch: {
                  connected: false,
                  connectionDown: {
                    reason:
                      "a different node is now answering this workspace's port — not reconnecting",
                    impostor: true,
                  },
                },
              });
            }
            return;
          }
          dispatch({ type: "patch", patch: { connectionDown: null } });
          refresh();
        },
        (err: unknown) => {
          if (stateRef.current.connected || !stateRef.current.connectionDown) {
            const reason = err instanceof Error ? err.message : String(err);
            dispatch({
              type: "patch",
              patch: { connected: false, connectionDown: { reason } },
            });
          }
        },
      );
    };
    const off = node.onStream((signal) => {
      if (signal.kind === "heartbeat") {
        const { height, appHash } = signal.frame;
        if (height <= 0) return;
        dispatch({
          type: "update",
          fn: (prev) => {
            const patch: Partial<typeof prev> = {};
            if (height > (prev.lastBlock ?? -1)) patch.lastBlock = height;
            // Patch only on real movement: an idle chain heartbeats every 3s
            // and an unconditional new status object would re-render each beat.
            if (
              prev.status &&
              (prev.status.height !== height || prev.status.appHash !== appHash)
            ) {
              patch.status = { ...prev.status, height, appHash };
            }
            return patch;
          },
        });
        return;
      }
      if (signal.kind === "down") {
        if (stateRef.current.connected || !stateRef.current.connectionDown) {
          dispatch({
            type: "patch",
            patch: {
              connected: false,
              connectionDown: { reason: signal.reason },
            },
          });
        }
        return;
      }
      if (!stateRef.current.connected || stateRef.current.connectionDown) {
        recover();
      }
    });
    return off;
  }, [node, state.needsOnboarding, state.onboardingPhase, refresh]);

  // 2c. Keep a live huddle's voice fan-out in step with the consensus roster:
  //     every refresh that lands a new channel snapshot may add/remove members,
  //     so re-derive the recipient set and push it into the audio session. A
  //     no-op when not huddling; the push itself dedupes by value (refresh
  //     patches a fresh channels array every block).
  useEffect(() => {
    actions.syncHuddleRecipients();
  }, [state.channels, state.voice.channelId, actions]);

  // 2d. Best-effort roster reconciliation on the way out: quitting or
  //     reloading mid-huddle can't run the normal leave path, so fire a
  //     keepalive leave_huddle beacon — otherwise peers keep showing a
  //     participant whose client is gone (the roster has no TTL).
  useEffect(() => {
    const channelId = state.voice.channelId;
    const url = state.nodeUrl;
    if (!channelId || !url) return;
    const origin = state.author;
    const leaveOnHide = () => {
      const body = new Blob(
        [
          JSON.stringify({
            target: "chat",
            payload: { leave_huddle: { channel_id: channelId } },
            origin,
          }),
        ],
        { type: "application/json" },
      );
      navigator.sendBeacon(`${url.replace(/\/$/, "")}/v1/submit`, body);
    };
    window.addEventListener("pagehide", leaveOnHide);
    return () => window.removeEventListener("pagehide", leaveOnHide);
  }, [state.voice.channelId, state.nodeUrl, state.author]);

  // 2e. Huddle pop-out bridge, sender half (desktop): while the window is open,
  //     push it the CONTEXT it needs to run its own media session (node url +
  //     channel + raw roster + capability). A fingerprint dedupes the per-block
  //     channels churn. Staleness now lives in the window (it owns the beacons),
  //     so there is no re-push tick here. Protocol: store/huddle-window.ts.
  const huddleCtxFp = useRef("");
  useEffect(() => {
    if (!state.voice.popped || !isTauri()) return;
    const ctx = buildHuddleContext(
      state.voice,
      state.channels,
      state.authorNames,
      state.nodeUrl,
      state.status?.publicKey ?? "",
      state.videoCapability,
    );
    if (!ctx) return;
    const fp = JSON.stringify(ctx);
    if (fp === huddleCtxFp.current) return;
    huddleCtxFp.current = fp;
    void import("@tauri-apps/api/event")
      .then(({ emit }) => emit(HUDDLE_CONTEXT_EVENT, ctx))
      // A dropped push must not strand the window on a stale roster (it has no
      // other source) — clear the fingerprint so the next render re-emits.
      .catch(() => {
        huddleCtxFp.current = "";
      });
  }, [state.voice, state.channels, state.authorNames, state.nodeUrl, state.status?.publicKey, state.videoCapability]);

  // 2f. ...and the receiver half: apply the window's commands (leave / sweep) to
  //     the store, replay the context on its ready handshake, and re-take the
  //     media session when Rust reports the window destroyed (any way it dies,
  //     incl. the window closing itself on a media failure). The mount-time
  //     popInHuddle also re-takes any session left dangling by a reload.
  useEffect(() => {
    if (!isTauri()) return;
    actions.popInHuddle();
    const unlisteners: Array<() => void> = [];
    let cancelled = false;
    const hold = (un: () => void) => {
      if (cancelled) un();
      else unlisteners.push(un);
    };
    void import("@tauri-apps/api/event")
      .then(({ listen, emit }) =>
        Promise.all([
          listen(HUDDLE_CMD_EVENT, (event) => {
            const cmd = event.payload as HuddleWindowCmd;
            const current = stateRef.current;
            if (cmd.op === "ready") {
              const ctx = buildHuddleContext(
                current.voice,
                current.channels,
                current.authorNames,
                current.nodeUrl,
                current.status?.publicKey ?? "",
                current.videoCapability,
              );
              if (ctx) void emit(HUDDLE_CONTEXT_EVENT, ctx);
              return;
            }
            applyHuddleWindowCmd(cmd, actions, current.voice.channelId);
          }),
          listen(HUDDLE_CLOSED_EVENT, () => actions.popInHuddle()),
        ]),
      )
      .then((uns) => uns.forEach(hold))
      .catch(() => {
        // event API unavailable (non-tauri / test stub) — the bridge no-ops.
      });
    return () => {
      cancelled = true;
      unlisteners.forEach((un) => un());
    };
  }, [actions]);

  // 3. Reflect the accent into the css var the theme reads.
  useEffect(() => {
    document.documentElement.style.setProperty("--accent", state.accent);
  }, [state.accent]);

  // 4. Drop any open page (doc) when the node url resolves or changes — a
  //    different node has different docs. The page enumeration is re-queried
  //    from the new node's index by `refresh`, so it isn't seeded here.
  useEffect(() => {
    const url = state.nodeUrl;
    if (!url) return;
    dispatch({
      type: "patch",
      patch: {
        pages: [],
        activePage: null,
        activePageBlocks: [],
      },
    });
  }, [state.nodeUrl]);

  // 5. Menu-bar popover navigation (desktop/macOS): the tray popover is a
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
          if (screen) dispatch({ type: "patch", patch: { screen } });
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

  const value = useMemo<ConsoleContextValue>(
    () => ({ state, actions, transport: node }),
    [state, actions, node],
  );
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
