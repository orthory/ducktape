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

import { parseItemChannelId } from "../../domain/forge-client";
import {
  isTauri,
  resolveNode,
} from "../../domain/node-bootstrap";
import * as notifyClient from "../../domain/notify-client";
import type { NotifyConfigPayload } from "../../domain/notify-client";
import type { PageMeta } from "../../domain/pages-client";
import { moduleTopic } from "../../domain/stream";
import {
  NodeError,
  type BlockRecord,
  type NodeStatus,
  type NodeTransport,
} from "../../domain/transport";
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
import { normalizeKey } from "../../domain/names";
import {
  applySnapshot,
  createInitialState,
  DEFAULT_AUTHOR,
  docTabsScope,
  loadRemoteUrl,
  saveDocTabs,
} from "./state";

export type { ConsoleActions } from "./actions";
export type { ConsoleContextValue } from "./context";

/** How many recent non-empty blocks the explorer pulls per refresh. */
const BLOCKS_KEEP = 200;

// One shared dynamic import of the Tauri event API for every effect below.
// Several effects need it on the same mount tick, and concurrent dynamic
// imports of one specifier both waste work and race vitest's module mocker
// (the loser of the race resolves the REAL module past the test's mock).
let tauriEventModule: Promise<typeof import("@tauri-apps/api/event")> | null = null;
const tauriEventApi = () => {
  if (!tauriEventModule) {
    tauriEventModule = import("@tauri-apps/api/event");
    // Never cache a rejection: one failed load would otherwise leave the
    // navigate listener and both huddle bridges dead for the whole session,
    // with every consumer swallowing the same cached error silently.
    tauriEventModule.catch((err) => {
      tauriEventModule = null;
      console.warn("[console] @tauri-apps/api/event failed to load; retrying on next use", err);
    });
  }
  return tauriEventModule;
};

/** The structured deep-link a desktop notification navigates with. A plain
 *  string payload remains the tray popover's bare screen switch. Mirrored by
 *  the Rust notifier — extend both sides together. */
export interface NavigateTarget {
  screen: string;
  channelId?: string;
  threadRoot?: number;
  repo?: string;
  number?: number;
}

/** Defensive parse of an untrusted navigate payload: anything without a
 *  non-empty string `screen` is ignored, and each optional field is dropped
 *  unless it carries the expected primitive type. */
const parseNavigateTarget = (payload: unknown): NavigateTarget | null => {
  if (typeof payload !== "object" || payload === null) return null;
  const raw = payload as Record<string, unknown>;
  if (typeof raw.screen !== "string" || raw.screen.length === 0) return null;
  return {
    screen: raw.screen,
    channelId: typeof raw.channelId === "string" ? raw.channelId : undefined,
    threadRoot: typeof raw.threadRoot === "number" ? raw.threadRoot : undefined,
    repo: typeof raw.repo === "string" && raw.repo.length > 0 ? raw.repo : undefined,
    number: typeof raw.number === "number" ? raw.number : undefined,
  };
};

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

  // Every status consumer for the CURRENT transport shares the same bounded
  // probe (including its one timeout-only retry). A node switch may start a
  // different flight before the old one settles, so completion clears the slot
  // only while it still owns it.
  // Server follow-up (deliberately outside this UI fix): serve status from
  // freshness-bounded committed data, and decouple Forge full-pack state sync
  // from frequent local recovery checkpoints.
  const statusFlightRef = useRef<{
    transport: NodeTransport;
    promise: Promise<NodeStatus>;
  } | null>(null);
  const recoveryFlightRef = useRef<{
    transport: NodeTransport;
    promise: Promise<void>;
  } | null>(null);
  // Identity rejection belongs to the transport that produced it. Keeping the
  // latch outside global UI state lets a newly selected transport prove itself
  // while the rejected transport stays permanently unable to recover.
  const rejectedTransportsRef = useRef(new WeakSet<NodeTransport>());
  // HTTP success cannot overrule an authoritative stream-down edge. Keep the
  // fence transport-scoped so refreshes queued (or submitted) after the edge
  // may still land safe data without claiming the stream recovered.
  const streamDownTransportsRef = useRef(new WeakSet<NodeTransport>());
  // A newer hydrate against the same transport owns the UI. This prevents a
  // slower, older slice failure from overwriting a snapshot that already won.
  const hydrateGenRef = useRef(0);

  const fail = useCallback(
    (err: unknown) =>
      dispatch({ type: "patch", patch: { error: String(err) } }),
    [],
  );

  const probeStatus = useCallback((live: NodeTransport): Promise<NodeStatus> => {
    const existing = statusFlightRef.current;
    if (existing?.transport === live) return existing.promise;

    const promise = Promise.resolve()
      .then(() => live.status())
      .catch((err: unknown) => {
        if (err instanceof NodeError && err.kind === "timeout") {
          return live.status();
        }
        throw err;
      });
    const flight = { transport: live, promise };
    statusFlightRef.current = flight;
    void promise.then(
      () => {
        if (statusFlightRef.current === flight) statusFlightRef.current = null;
      },
      () => {
        if (statusFlightRef.current === flight) statusFlightRef.current = null;
      },
    );
    return promise;
  }, []);

  // Status is also the recovery identity proof. In particular, a successful
  // timeout retry must not skip the managed-workspace impostor guard.
  const acceptsStatus = useCallback((live: NodeTransport, status: NodeStatus): boolean => {
    if (nodeRef.current !== live) return false;
    // An impostor finding is sticky for this transport. Stream noise from the
    // foreign process must never promote it back into an established session.
    if (rejectedTransportsRef.current.has(live)) return false;
    const expected = stateRef.current.workspace?.pubkey;
    const got = status.publicKey;
    if (!expected || !got || got.toLowerCase() === expected.toLowerCase()) return true;
    rejectedTransportsRef.current.add(live);
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
    return false;
  }, []);

  const handleHydrateFailure = useCallback(
    (err: unknown, hadSnapshot: boolean) => {
      // A slice query failing after status succeeded says nothing about stream
      // liveness. Preserve an established session and keep the query failure
      // loud; a later scoped refresh or guarded stream recovery can converge.
      if (!hadSnapshot) {
        dispatch({ type: "patch", patch: { connected: false } });
      }
      fail(err);
    },
    [fail],
  );

  const handleStatusFailure = useCallback(
    (err: unknown, hadSnapshot: boolean) => {
      if (hadSnapshot) {
        if (err instanceof NodeError && err.kind === "timeout") {
          // A status deadline is not a stream-down signal. Preserve the last
          // good snapshot and connectivity; the live stream triggers recovery.
          dispatch({
            type: "update",
            fn: (prev) =>
              prev.connected && !prev.connectionDown?.impostor
                ? { connectionDown: { reason: "Node is busy — retrying…" } }
                : {},
          });
          return;
        }
        // An HTTP/malformed response is actionable but still is not proof the
        // established stream died. Keep its error loud without a false drop.
        fail(err);
        return;
      }
      // Cold boot has no safe snapshot to retain, so every failure remains loud.
      dispatch({ type: "patch", patch: { connected: false } });
      fail(err);
    },
    [fail],
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
    if (openTabs.length !== prevTabs.length) {
      saveDocTabs(
        docTabsScope(
          stateRef.current.workspace?.id ?? null,
          stateRef.current.nodeUrl,
        ),
        openTabs,
      );
    }
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
    // A node switch cannot cancel the transport's already-issued requests.
    // Bind this hydrate to the exact transport that started it so a late
    // response from the previous workspace cannot repopulate cleared slices.
    const generation = (hydrateGenRef.current += 1);
    const stale = () =>
      nodeRef.current !== live || hydrateGenRef.current !== generation;
    const hadSnapshot = stateRef.current.status !== null;
    // the pages (docs) slice refreshes by enumeration + the open page's tree.
    const fetchedPage = stateRef.current.activePage;
    // ops submitted at or after this instant cannot be in the snapshot the
    // queries below return — pageSnapshotSuperseded keys off it at apply time.
    const fetchStartedAt = Date.now();
    const hydrate = (status: NodeStatus) => {
      if (stale() || !acceptsStatus(live, status)) return Promise.resolve();
      return Promise.resolve()
        .then(() =>
          Promise.all([
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
            if (stale()) return;
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
            const streamDown = streamDownTransportsRef.current.has(live);
            return dispatch({
              type: "patch",
              patch: {
                ...applySnapshot({
                  connected: !streamDown,
                  status,
                  channels: chat.channels,
                  members: valset.members,
                  residents: valset.residents,
                  proposals: governance.proposals,
                  governanceShares: governance.governanceShares,
                  forgeHead: forge.forgeHead,
                  activeChannel: chat.activeChannel,
                  messages: chat.messages,
                  authorNames: people.authorNames,
                  nodeUsers: people.nodeUsers,
                  accountKeys: people.accountKeys,
                  accountHandles: people.accountHandles,
                  pages: holdPages ? stateRef.current.pages : pagesSlices.pages,
                  activePageBlocks: holdPages
                    ? stateRef.current.activePageBlocks
                    : (pagesSlices.pageBlocks ?? []),
                  agents: agents.agents,
                  capabilities: capability.capabilities,
                  capabilitiesByNode: capability.capabilitiesByNode,
                  watches: runs.watches,
                  pendingRuns: runs.pendingRuns,
                  runLease: runs.runLease,
                  files: files.files,
                  blocks,
                }),
                ...(streamDown ? {} : { connectionDown: null }),
                openTabs,
                activePage,
              },
            });
          },
        )
        .catch((err) => {
          if (stale()) return;
          handleHydrateFailure(err, hadSnapshot);
        });
    };
    return probeStatus(live).then(
      hydrate,
      (err: unknown) => {
        if (!stale()) handleStatusFailure(err, hadSnapshot);
      },
    );
  }, [
    acceptsStatus,
    fail,
    handleHydrateFailure,
    handleStatusFailure,
    probeStatus,
    reconcileDocTabs,
    shouldHoldPages,
  ]);

  // Scoped hydration for block events: ONE status read names the modules the
  // block changed (their state roots ride status().modules[]), and only the
  // slice groups that read those modules re-query — the wholesale refresh
  // stays the boot / reconnect / never-hydrated path. The replica pipeline
  // makes the diff exact: every node folds per block, so consecutive statuses
  // differ by exactly what the block touched.
  const refreshScoped = useCallback(() => {
    const live = nodeRef.current;
    if (!live) return Promise.resolve();
    const generation = (hydrateGenRef.current += 1);
    const stale = () =>
      nodeRef.current !== live || hydrateGenRef.current !== generation;
    const hadSnapshot = stateRef.current.status !== null;
    const fetchStartedAt = Date.now();
    const hydrate = (status: NodeStatus) => {
      if (stale() || !acceptsStatus(live, status)) return Promise.resolve();
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
            if (stale()) return;
            const holdPages =
              !pagesSlices || shouldHoldPages(fetchedPage, fetchStartedAt);
            const tabs =
              !holdPages && pagesSlices
                ? reconcileDocTabs(pagesSlices.pages)
                : null;
            return dispatch({
              type: "patch",
              patch: {
                ...(streamDownTransportsRef.current.has(live)
                  ? {}
                  : { connected: true, connectionDown: null }),
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
    };
    return probeStatus(live)
      .then(hydrate, (err: unknown) => {
        if (!stale()) handleStatusFailure(err, hadSnapshot);
      })
      .catch((err) => {
        if (stale()) return;
        handleHydrateFailure(err, hadSnapshot);
      });
  }, [
    acceptsStatus,
    fail,
    handleHydrateFailure,
    handleStatusFailure,
    probeStatus,
    refresh,
    reconcileDocTabs,
    shouldHoldPages,
  ]);

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

  // 2a. Follow the module stream topics: every finalized op streams as an
  //     event, the chain tip patches UNGATED, and a trailing debounce
  //     coalesces a block burst into ONE scoped hydrate — refreshScoped's
  //     root diff fetches exactly the slice groups the blocks touched, so
  //     the events are the trigger and the roots are the scope. The
  //     fresh-pending gate carries over from the block-frame era; a lagged
  //     topic hydrates immediately (the root diff covers whatever the gap
  //     contained).
  useEffect(() => {
    if (!node || !streamModuleKey) return;
    const modules = streamModuleKey.split("\0").filter(Boolean);
    let timer: ReturnType<typeof setTimeout> | null = null;
    const clearFlush = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    };
    const flush = () => {
      timer = null;
      void refreshScoped();
    };
    const schedule = () => {
      // An op of OURS in flight: its own completion refresh follows, and a
      // scoped re-query now would clobber the preconfirmed projection. Stale
      // pendings (a hung submit) stop gating so the stream can't be starved.
      if (hasFreshPending(stateRef.current.ops, Date.now())) return;
      clearFlush();
      timer = setTimeout(flush, 100);
    };

    const off = node.subscribe(modules.map(moduleTopic), {
      onEvent: (frame) => {
        // The live chain tip, UNGATED — recorded before the pending gate so
        // the console always knows the chain moved even while an op of ours
        // is in flight. An event at height N also proves the node's tip is
        // ≥ N, so status.height advances here too instead of waiting up to a
        // heartbeat interval (appHash refreshes on the next beat).
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
      onLagged: () => {
        clearFlush();
        void refreshScoped();
      },
    });
    return () => {
      clearFlush();
      off();
    };
  }, [node, streamModuleKey, refreshScoped]);

  // 2b. Liveness heartbeat — connection banner, tip patching, and one-shot
  //     recovery on the up edge. The transport's watchdog turns silent stream
  //     death into a down signal; there is no separate reconnect poll here.
  useEffect(() => {
    if (!node) return;
    if (state.needsOnboarding || state.onboardingPhase) return;
    const recover = () => {
      const down = stateRef.current.connectionDown;
      if (down?.impostor || rejectedTransportsRef.current.has(node)) return;
      if (stateRef.current.connected && !down) return;
      const existing = recoveryFlightRef.current;
      if (existing?.transport === node) return;
      const promise = refresh();
      const flight = { transport: node, promise };
      recoveryFlightRef.current = flight;
      void promise.then(
        () => {
          if (recoveryFlightRef.current === flight) recoveryFlightRef.current = null;
        },
        () => {
          if (recoveryFlightRef.current === flight) recoveryFlightRef.current = null;
        },
      );
    };
    const off = node.onStream((signal) => {
      if (signal.kind === "heartbeat") {
        // A heartbeat is transport-level proof the stream is live again. It
        // may report height zero before the node has a tip, but it still opens
        // the liveness gate so guarded recovery can prove identity via status.
        streamDownTransportsRef.current.delete(node);
        const { height, appHash } = signal.frame;
        if (height > 0) {
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
        }
        if (!stateRef.current.connected || stateRef.current.connectionDown) {
          recover();
        }
        return;
      }
      if (signal.kind === "down") {
        streamDownTransportsRef.current.add(node);
        // Stream down is authoritative. Supersede every hydrate already past
        // status so its late slice completion cannot clear this down edge.
        hydrateGenRef.current += 1;
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
      streamDownTransportsRef.current.delete(node);
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

  // 2c². Adopt the chain's name for our own node while the author is still the
  //      boot placeholder — a returning user must read as themselves (Account
  //      avatar initials, composer identity), not as "operator". A name the
  //      user typed this session (≠ placeholder) always wins; the resolved
  //      name is identity's canonical account display name.
  useEffect(() => {
    if (state.author !== DEFAULT_AUTHOR) return;
    const self = state.status?.publicKey;
    if (!self) return;
    const resolved = state.authorNames[normalizeKey(self)];
    if (resolved) dispatch({ type: "patch", patch: { author: resolved } });
  }, [state.author, state.status?.publicKey, state.authorNames]);

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
    void tauriEventApi()
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
    void tauriEventApi()
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

  // 3b. Reflect the light/dark choice onto <html data-theme>; the --c-* palette
  //     in global.css keys off it, re-skinning every token-driven surface.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", state.theme);
  }, [state.theme]);

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

  // Deep-link thread hand-off parked by the navigate listener (see 5/5b), the
  // window-focus half of the notify config, and the config push fingerprint.
  const pendingThreadRef = useRef<{ channelId: string; root: number } | null>(null);
  const [windowFocused, setWindowFocused] = useState(
    () => typeof document === "undefined" || document.hasFocus(),
  );
  const notifyConfigFp = useRef("");

  // 5. Navigation events from OUTSIDE this webview (desktop): the tray popover
  //    asks the console to switch screens with a plain string (kept byte-for-
  //    byte), and a clicked notification deep-links a structured NavigateTarget
  //    — screen plus an optional channel/thread and forge repo/item. Inert on
  //    web, and a malformed object payload is ignored.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void tauriEventApi()
      .then(({ listen }) =>
        listen<string | NavigateTarget>("ducktape://navigate", (event) => {
          const payload = event.payload;
          if (typeof payload === "string") {
            if (payload) dispatch({ type: "patch", patch: { screen: payload } });
            return;
          }
          let target = parseNavigateTarget(payload);
          if (!target) return;
          // A chat target into a forge item's HIDDEN discussion channel
          // (`forge:<repo>:<n>`) is unroutable on the chat surface (those
          // channels are filtered out) — the reply landed in the item's
          // Discussion, so route to the forge item view instead.
          const forgeItem =
            target.screen === "chat" && target.channelId
              ? parseItemChannelId(target.channelId)
              : null;
          if (forgeItem) target = { screen: "forge", ...forgeItem };
          const { channelId, threadRoot } = target;
          // Every structured navigate owns the parked-thread slot: a stale
          // hand-off from an earlier navigate (channel never entered, or a
          // no-thread target arriving next) must not open a thread later.
          pendingThreadRef.current = null;
          // Park the thread hand-off BEFORE the channel switch dispatches:
          // openThread reads the ACTIVE channel, and the switch only lands on
          // the next render — effect 5b opens it once it has (an already-
          // active channel opens immediately below instead).
          if (channelId && threadRoot != null && stateRef.current.activeChannel !== channelId) {
            pendingThreadRef.current = { channelId, root: threadRoot };
          }
          actions.setScreen(target.screen);
          if (channelId) actions.selectChannel(channelId);
          if (
            threadRoot != null &&
            (!channelId || stateRef.current.activeChannel === channelId)
          ) {
            actions.openThread(threadRoot);
          }
          // A forge target with no repo is unroutable — there is nothing to
          // select — so `number` alone never sets a focus.
          if (target.repo) {
            dispatch({
              type: "patch",
              patch: { forgeFocus: { repo: target.repo, number: target.number ?? null } },
            });
          }
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
  }, [actions]);

  // 5b. Deep-link thread hand-off: a selectChannel dispatched by the navigate
  //     listener is not visible to openThread (which reads the ACTIVE channel)
  //     until React re-renders, so the listener parks the target here and this
  //     effect opens the thread once the channel switch has landed.
  useEffect(() => {
    const pending = pendingThreadRef.current;
    if (!pending || state.activeChannel !== pending.channelId) return;
    pendingThreadRef.current = null;
    actions.openThread(pending.root);
  }, [state.activeChannel, actions]);

  // 5c. forgeFocus is a one-shot hand-off (the explorerFocus idiom), but the
  //     forge view has no store action to clear it with — so the provider
  //     retires it when the user leaves the forge screen, which is what keeps
  //     a later remount of ForgeView from replaying the jump.
  useEffect(() => {
    if (state.screen !== "forge" && state.forgeFocus) {
      dispatch({ type: "patch", patch: { forgeFocus: null } });
    }
  }, [state.screen, state.forgeFocus]);

  // 6. Desktop notifier config push: the Rust notifier learns who "me" is (the
  //    identity account behind our node key and EVERY node bound to it), what
  //    is focused, and which categories are enabled — all from this webview.
  //    Re-pushed whenever an input changes; the JSON fingerprint (the huddle-
  //    context idiom) swallows the per-block identity churn of re-fetched but
  //    unchanged projections. The window focus edge also marks everything seen
  //    — the webview-side complement of the notifier's native focus backstop.
  useEffect(() => {
    if (!isTauri()) return;
    const onFocus = () => {
      setWindowFocused(true);
      void notifyClient.markSeen();
    };
    const onBlur = () => setWindowFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    // An empty publicKey (legacy daemon) reads as unknown, not as self "".
    const selfNodeKeyHex = state.status?.publicKey?.toLowerCase() || null;
    const selfUserKeyHex = selfNodeKeyHex
      ? state.nodeUsers[selfNodeKeyHex]?.accountId ?? null
      : null;
    // Every node of our account — mentions of "me" on ANY of my devices
    // notify here. Unbound (or unknown) identity falls back to just our node.
    const accountNodes = selfUserKeyHex
      ? Object.keys(state.nodeUsers)
          .filter((nodeHex) => state.nodeUsers[nodeHex].accountId === selfUserKeyHex)
          .map((nodeHex) => nodeHex.toLowerCase())
      : [];
    if (selfNodeKeyHex && !accountNodes.includes(selfNodeKeyHex)) {
      accountNodes.push(selfNodeKeyHex);
    }
    accountNodes.sort(); // key-order independent fingerprint
    const payload: NotifyConfigPayload = {
      nodeUrl: state.nodeUrl ?? null,
      selfUserKeyHex,
      selfNodeKeysHex: accountNodes,
      focusedChannel: state.screen === "chat" ? state.activeChannel : null,
      mainWindowFocused: windowFocused,
      authorNames: state.authorNames,
      prefs: state.notifyPrefs,
    };
    const fp = JSON.stringify(payload);
    if (fp === notifyConfigFp.current) return;
    notifyConfigFp.current = fp;
    void notifyClient.configure(payload);
  }, [
    state.status?.publicKey,
    state.nodeUsers,
    state.nodeUrl,
    state.screen,
    state.activeChannel,
    state.authorNames,
    state.notifyPrefs,
    windowFocused,
  ]);

  const value = useMemo<ConsoleContextValue>(
    () => ({ state, actions, transport: node }),
    [state, actions, node],
  );
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
