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
import type { PageMeta } from "../../domain/pages-client";
import type { BlockRecord, NodeTransport } from "../../domain/transport";
import * as ws from "../../domain/workspace-client";
import { createActions } from "./actions";
import { ConsoleContext, type ConsoleContextValue } from "./context";
import {
  hasFreshPending,
  pageSnapshotSuperseded,
  receiptFloor,
} from "./finalization";
import {
  changedModules,
  fetchAgentsSlices,
  fetchCapabilitySlices,
  fetchChatSlices,
  fetchFilesSlices,
  fetchForgeSlices,
  fetchGovernanceSlices,
  fetchPagesSlices,
  fetchPeopleSlices,
  fetchRunsSlices,
  fetchValsetSlices,
  scopeFor,
} from "./hydration";
import {
  HUDDLE_CLOSED_EVENT,
  HUDDLE_CMD_EVENT,
  HUDDLE_CONTEXT_EVENT,
  applyHuddleWindowCmd,
  buildHuddleContext,
} from "./huddle-window";
import type { HuddleWindowCmd } from "./huddle-window";
import { reducer } from "./reducer";
import {
  applySnapshot,
  createInitialState,
  loadRemoteUrl,
  saveDocTabs,
} from "./state";

export type { ConsoleActions } from "./actions";
export type { ConsoleContextValue } from "./context";

/** How many recent non-empty blocks the explorer pulls per refresh. */
const BLOCKS_KEEP = 200;

/** How often to re-poll a resolved-but-unanswering node until it comes back. */
const RECONNECT_POLL_MS = 3_000;

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

  // Reconcile doc tabs against a live enumeration: a tab whose page no longer
  // exists (deleted here or elsewhere) drops, and a now-dead active page falls
  // back to the first surviving tab. CRITICAL: only called when the caller
  // actually got an enumeration AND isn't holding the pages slices —
  // `listPages` is best-effort, and evicting open tabs on a transient empty
  // result would blank the editor mid-edit.
  const reconcileDocTabs = useCallback((pages: PageMeta[]) => {
    const prevTabs = stateRef.current.openTabs;
    const prevActive = stateRef.current.activePage;
    if (pages.length === 0) return { openTabs: prevTabs, activePage: prevActive };
    const liveIds = new Set(pages.map((p) => p.id));
    const openTabs = prevTabs.filter((id) => liveIds.has(id));
    if (openTabs.length !== prevTabs.length) saveDocTabs(openTabs);
    const activePage =
      prevActive && liveIds.has(prevActive) ? prevActive : (openTabs[0] ?? null);
    return { openTabs, activePage };
  }, []);

  // A pages snapshot that predates a page op must not be applied: it would
  // clobber the op's preconfirmed projection — an optimistically inserted
  // block unmounts (dropping the focused textarea with it) and just-committed
  // text reverts until the op finalizes. The block stream's hasFreshPending
  // gate covers stream refreshes; this covers the completion refresh of an
  // EARLIER op that settles while a later one is still in flight, a snapshot
  // raced by an op submitted mid-fetch, and a page we've navigated away from.
  // Held slices converge on the last op's own refresh.
  const shouldHoldPages = useCallback(
    (fetchedPage: string | null, fetchStartedAt: number) =>
      stateRef.current.activePage !== fetchedPage ||
      pageSnapshotSuperseded(stateRef.current.ops, fetchStartedAt, Date.now()),
    [],
  );

  const refresh = useCallback(() => {
    const live = nodeRef.current;
    if (!live) return Promise.resolve();
    // the pages (docs) slice refreshes by enumeration + the open page's tree.
    const fetchedPage = stateRef.current.activePage;
    // ops submitted at or after this instant cannot be in the snapshot the
    // queries below return — pageSnapshotSuperseded keys off it at apply time.
    const fetchStartedAt = Date.now();
    return Promise.resolve()
      .then(() =>
        Promise.all([
          live.status(),
          fetchChatSlices(live, stateRef.current.activeChannel),
          fetchValsetSlices(live),
          fetchGovernanceSlices(live),
          fetchForgeSlices(live),
          fetchPagesSlices(live, fetchedPage),
          fetchAgentsSlices(live),
          fetchCapabilitySlices(live),
          fetchRunsSlices(live),
          fetchPeopleSlices(live),
          fetchFilesSlices(live),
          // the explorer's ring pull — best-effort, so a node without
          // /v1/blocks reads as "no blocks yet".
          live.blocks(BLOCKS_KEEP).catch((): BlockRecord[] => []),
        ]),
      )
      .then(
        ([
          status,
          chat,
          valset,
          governance,
          forge,
          pagesSlices,
          agents,
          capability,
          runs,
          people,
          files,
          blocks,
        ]) => {
          // read-your-writes floor (the follow-the-head handoff's bug B): a
          // snapshot below a height this console holds a receipt for would
          // un-render the confirmed write until a later refresh — skip; the
          // next block's hydrate carries a taller status.
          if (status.height < receiptFloor(stateRef.current.ops)) return;
          const holdPages = shouldHoldPages(fetchedPage, fetchStartedAt);
          const { openTabs, activePage } = holdPages
            ? {
                openTabs: stateRef.current.openTabs,
                activePage: stateRef.current.activePage,
              }
            : reconcileDocTabs(pagesSlices.pages);
          return dispatch({
            type: "patch",
            patch: {
              ...applySnapshot({
                connected: true,
                status,
                channels: chat.channels,
                members: valset.members,
                residents: valset.residents,
                proposals: governance.proposals,
                forgeHead: forge.forgeHead,
                activeChannel: chat.activeChannel,
                messages: chat.messages,
                authorNames: people.authorNames,
                nodeUsers: people.nodeUsers,
                accountKeys: people.accountKeys,
                pages: holdPages ? stateRef.current.pages : pagesSlices.pages,
                activePageBlocks: holdPages
                  ? stateRef.current.activePageBlocks
                  : (pagesSlices.pageBlocks ?? []),
                agents: agents.agents,
                capabilities: capability.capabilities,
                capabilitiesByNode: capability.capabilitiesByNode,
                watches: runs.watches,
                pendingRuns: runs.pendingRuns,
                runAssignee: runs.runAssignee,
                files: files.files,
                blocks,
              }),
              openTabs,
              activePage,
            },
          });
        },
      )
      .catch((err) => {
        dispatch({ type: "patch", patch: { connected: false } });
        fail(err);
      });
  }, [fail, reconcileDocTabs, shouldHoldPages]);

  // Scoped hydration for block events: ONE status read names the modules the
  // block changed (their state roots ride status().modules[]), and only the
  // slice groups that read those modules re-query — the wholesale refresh
  // stays the boot / reconnect / never-hydrated path. The replica pipeline
  // makes the diff exact: every node folds per block, so consecutive statuses
  // differ by exactly what the block touched.
  const refreshScoped = useCallback(() => {
    const live = nodeRef.current;
    if (!live) return Promise.resolve();
    const fetchStartedAt = Date.now();
    return Promise.resolve()
      .then(() => live.status())
      .then((status) => {
        // read-your-writes floor, checked BEFORE fanning out: a lagging
        // status buys nothing — the next block event retries.
        if (status.height < receiptFloor(stateRef.current.ops)) return;
        const prev = stateRef.current.status;
        if (!prev) return refresh();
        const scope = scopeFor(changedModules(prev, status));
        const fetchedPage = stateRef.current.activePage;
        return Promise.resolve()
          .then(() =>
            Promise.all([
              scope.has("chat")
                ? fetchChatSlices(live, stateRef.current.activeChannel)
                : null,
              scope.has("valset") ? fetchValsetSlices(live) : null,
              scope.has("governance") ? fetchGovernanceSlices(live) : null,
              scope.has("forge") ? fetchForgeSlices(live) : null,
              scope.has("pages") ? fetchPagesSlices(live, fetchedPage) : null,
              scope.has("agents") ? fetchAgentsSlices(live) : null,
              scope.has("capability") ? fetchCapabilitySlices(live) : null,
              scope.has("runs") ? fetchRunsSlices(live) : null,
              scope.has("people") ? fetchPeopleSlices(live) : null,
              scope.has("files") ? fetchFilesSlices(live) : null,
              // the explorer ring follows every block regardless of scope.
              live.blocks(BLOCKS_KEEP).catch((): BlockRecord[] => []),
            ]),
          )
          .then(
            ([
              chat,
              valset,
              governance,
              forge,
              pagesSlices,
              agents,
              capability,
              runs,
              people,
              files,
              blocks,
            ]) => {
              const holdPages =
                !pagesSlices || shouldHoldPages(fetchedPage, fetchStartedAt);
              const tabs =
                !holdPages && pagesSlices
                  ? reconcileDocTabs(pagesSlices.pages)
                  : null;
              return dispatch({
                type: "patch",
                patch: {
                  connected: true,
                  status,
                  blocks,
                  ...(chat ?? {}),
                  ...(valset ?? {}),
                  ...(governance ?? {}),
                  ...(forge ?? {}),
                  ...(agents ?? {}),
                  ...(capability ?? {}),
                  ...(runs ?? {}),
                  ...(people ?? {}),
                  ...(files ?? {}),
                  ...(!holdPages && pagesSlices
                    ? {
                        pages: pagesSlices.pages,
                        activePageBlocks: pagesSlices.pageBlocks ?? [],
                      }
                    : {}),
                  ...(tabs ?? {}),
                },
              });
            },
          );
      })
      .catch((err) => {
        dispatch({ type: "patch", patch: { connected: false } });
        fail(err);
      });
  }, [fail, refresh, reconcileDocTabs, shouldHoldPages]);

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

  // 2. Hydrate once the node is resolved, then follow the block stream. The
  //    lastBlock updater stays pure (StrictMode double-invokes it): it only
  //    moves forward on the strictly-increasing block height.
  useEffect(() => {
    if (!node) return;
    refresh();

    const offBlock = node.onBlock((block) => {
      // The live chain tip, UNGATED — recorded before the pending gate below,
      // so the console always knows the chain moved even while an op of ours
      // is in flight (a seam duplicate or reconnect replay never moves it back).
      dispatch({
        type: "update",
        fn: (prev) =>
          block.height > (prev.lastBlock ?? -1) ? { lastBlock: block.height } : {},
      });
      // A block landing while one of OUR ops is still in flight would re-query
      // state that predates the op and clobber its preconfirmed projection —
      // and the op's own completion refresh follows immediately anyway. Stale
      // pendings (a hung submit) stop gating so the stream can't be starved.
      if (hasFreshPending(stateRef.current.ops, Date.now())) return;
      refreshScoped();
    });
    return offBlock;
  }, [node, refresh, refreshScoped]);

  // 2b. Liveness heartbeat — the "no running node" detection AND recovery. The
  //     block stream can't do this alone: a node that silently goes away (crash,
  //     stop, remote endpoint unplugged) just stops sending blocks, with no error
  //     to flip `connected` off — and a healthy but idle node also sends none, so
  //     silence isn't a reliable signal. Instead, ping `status()` on an interval
  //     whenever a node is resolved: a failure marks it down (so the UI reflects
  //     it and this same loop keeps retrying), and the first success after a drop
  //     re-hydrates via refresh(). Reads live `connected` through the ref so the
  //     interval isn't torn down and rebuilt on every block. Skipped during
  //     onboarding / a joiner's park phase (which has its own poll).
  useEffect(() => {
    if (!node) return;
    if (state.needsOnboarding || state.onboardingPhase) return;
    const beat = () =>
      node.status().then(
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
          // Healthy: on the down→up edge clear the banner and re-hydrate; while
          // already connected the block stream keeps projections fresh.
          if (!stateRef.current.connected || stateRef.current.connectionDown) {
            dispatch({ type: "patch", patch: { connectionDown: null } });
            refresh();
          }
        },
        (err: unknown) => {
          // Unreachable: surface the REAL reason in a persistent reconnecting
          // banner and keep this loop trying to recover. Patch only on the
          // down-edge to avoid a re-render every beat.
          if (stateRef.current.connected || !stateRef.current.connectionDown) {
            const reason = err instanceof Error ? err.message : String(err);
            dispatch({
              type: "patch",
              patch: { connected: false, connectionDown: { reason } },
            });
          }
        },
      );
    const timer = setInterval(beat, RECONNECT_POLL_MS);
    return () => clearInterval(timer);
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
