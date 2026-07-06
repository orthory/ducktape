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

import * as agentClient from "../../domain/agent-client";
import * as capabilityClient from "../../domain/capability-client";
import * as chatClient from "../../domain/chat-client";
import * as filesClient from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import type { ProposalView } from "../../domain/governance-client";
import * as pagesClient from "../../domain/pages-client";
import type { PageBlock, PageMeta } from "../../domain/pages-client";
import {
  isTauri,
  resolveNode,
} from "../../domain/node-bootstrap";
import * as profilesClient from "../../domain/profiles-client";
import * as runsClient from "../../domain/runs-client";
import * as valsetClient from "../../domain/valset-client";
import type { BlockRecord, NodeTransport } from "../../domain/transport";
import * as ws from "../../domain/workspace-client";
import { createActions } from "./actions";
import { ConsoleContext, type ConsoleContextValue } from "./context";
import { hasFreshPending } from "./finalization";
import { reducer } from "./reducer";
import {
  applySnapshot,
  createInitialState,
  loadRemoteUrl,
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

  const refresh = useCallback(() => {
    const live = nodeRef.current;
    if (!live) return Promise.resolve();
    // the pages (docs) slice refreshes by enumeration + the open page's tree.
    const activePage = stateRef.current.activePage;
    return Promise.resolve()
      .then(() =>
        Promise.all([
          live.status(),
          chatClient.channels(live),
          // valset only exists on the NETWORKED node (the local daemon has no
          // validator set) — best-effort like governance below, so a local
          // node reads as "no members" instead of never connecting.
          valsetClient.validators(live).catch((): number[][] => []),
          // the observer tier (staged admission) — same best-effort contract:
          // a pre-observer node (protocol < 3) reads as "no observers".
          valsetClient.observers(live).catch((): number[][] => []),
          // governance is a first-class operator surface but best-effort in the
          // snapshot: a node/build without it just reads as "no proposals"
          // rather than failing the whole refresh.
          governanceClient.proposals(live).catch((): ProposalView[] => []),
          forgeClient.head(live),
          // pages (the docs surface) is newer than some reachable nodes:
          // best-effort, so a node without it reads as "no docs", never a
          // failed refresh.
          pagesClient.listPages(live).catch((): PageMeta[] => []),
          activePage
            ? pagesClient
                .getPage(live, activePage)
                .catch((): PageBlock[] | null => null)
            : Promise.resolve<PageBlock[] | null>(null),
          agentClient.agents(live),
          // the executor registry — best-effort like governance/files above, so
          // a node without the capability module reads as "no executors" (the
          // "Runs on" picker degrades to a text field) rather than a failed
          // refresh.
          capabilityClient.capabilities(live).catch((): string[] => []),
          runsClient.watches(live),
          // newest-first for the timeline; the wire orders by dispatch id.
          runsClient
            .pendingRuns(live)
            .then((list) => [...list].sort((a, b) => b.created_at - a.created_at)),
          profilesClient.allProfiles(live, { from: 0, limit: 256 }),
          // files is best-effort so a node that does not register the module
          // reads as "empty", never a failed refresh (same contract as
          // governance above).
          filesClient.list(live, {}).catch(() => []),
          // the explorer's ring pull — best-effort, so a node without
          // /v1/blocks reads as "no blocks yet".
          live.blocks(BLOCKS_KEEP).catch((): BlockRecord[] => []),
        ]),
      )
      .then(([
        status,
        channels,
        validators,
        observerKeys,
        proposals,
        forgeHead,
        pages,
        pageBlocks,
        agents,
        capabilities,
        watches,
        pendingRuns,
        profiles,
        files,
        blocks,
      ]) => {
        // Profile.key is the origin bytes — the same bytes AuthorRef::User
        // carries — so hex(key) is exactly authorName's AuthorNames key.
        const authorNames = Object.fromEntries(
          profiles.map((p) => [chatClient.keyHex(p.key), p.display_name]),
        );
        const members = validators.map(valsetClient.validatorHex);
        const observers = observerKeys.map(valsetClient.validatorHex);
        const current = stateRef.current.activeChannel;
        const active =
          current && channels.some((c) => c.id === current)
            ? current
            : (channels[0]?.id ?? null);
        return Promise.resolve()
          .then(() => (active ? chatClient.latestMessages(live, active) : []))
          .then((messages) =>
            dispatch({
              type: "patch",
              patch: applySnapshot({
                connected: true,
                status,
                channels,
                members,
                observers,
                proposals,
                forgeHead,
                activeChannel: active,
                messages,
                authorNames,
                pages,
                activePageBlocks: pageBlocks ?? [],
                agents,
                capabilities,
                watches,
                pendingRuns,
                files,
                blocks,
              }),
            }),
          );
      })
      .catch((err) => {
        dispatch({ type: "patch", patch: { connected: false } });
        fail(err);
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
      refresh();
    });
    return offBlock;
  }, [node, refresh]);

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
        () => {
          // up: only pay for a full refresh on the down→up edge; while already
          //     connected the block stream keeps projections fresh.
          if (!stateRef.current.connected) refresh();
        },
        () => {
          // unreachable: surface it once; the next beats keep trying to recover.
          if (stateRef.current.connected) {
            dispatch({ type: "patch", patch: { connected: false } });
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
    () => ({ state, actions }),
    [state, actions],
  );
  return <ConsoleContext.Provider value={value}>{children}</ConsoleContext.Provider>;
}
