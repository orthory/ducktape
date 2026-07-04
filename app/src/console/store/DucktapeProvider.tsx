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
import * as automationsClient from "../../domain/automations-client";
import * as chatClient from "../../domain/chat-client";
import * as documentClient from "../../domain/document-client";
import type { Block } from "../../domain/document-client";
import * as filesClient from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import type { ProposalView } from "../../domain/governance-client";
import * as inboxClient from "../../domain/inbox-client";
import * as jobsClient from "../../domain/jobs-client";
import type { BoardCounts } from "../../domain/jobs-client";
import * as memoryClient from "../../domain/memory-client";
import {
  isTauri,
  resolveNode,
} from "../../domain/node-bootstrap";
import * as profilesClient from "../../domain/profiles-client";
import * as tasksClient from "../../domain/tasks-client";
import * as valsetClient from "../../domain/valset-client";
import type { NodeTransport } from "../../domain/transport";
import * as ws from "../../domain/workspace-client";
import { createActions } from "./actions";
import { ConsoleContext, type ConsoleContextValue } from "./context";
import { reducer } from "./reducer";
import {
  applySnapshot,
  createInitialState,
} from "./state";

export type { ConsoleActions } from "./actions";
export type { ConsoleContextValue } from "./context";

/** How many recent telemetry frames the console keeps in memory (the node's
 *  ring holds more; this bounds the live view's buffer). */
const TELEMETRY_KEEP = 200;

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
    // enumerate the doc index (the browse tree) and re-query the open doc's
    // blocks (null when none is open) alongside the other projections.
    const activeDoc = stateRef.current.activeDoc;
    // the inbox is per-member; the console keys "my" queue by the local author
    // identity, and memory browsing re-lists whatever dir is open.
    const member = stateRef.current.author;
    const memoryPath = stateRef.current.memoryPath;
    return Promise.resolve()
      .then(() =>
        Promise.all([
          live.status(),
          chatClient.channels(live),
          tasksClient.listTasks(live),
          valsetClient.validators(live),
          // governance is a first-class operator surface but best-effort in the
          // snapshot: a node/build without it just reads as "no proposals"
          // rather than failing the whole refresh.
          governanceClient.proposals(live).catch((): ProposalView[] => []),
          forgeClient.head(live),
          documentClient.listDocs(live),
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
          // ── unexposed-until-now modules — every one best-effort so a node
          //    that does not register the module reads as "empty", never a
          //    failed refresh (same contract as governance above). ──
          inboxClient.list(live, { member }).catch(() => []),
          inboxClient.unread(live, member).catch(() => 0),
          jobsClient.listJobs(live, {}).catch(() => []),
          jobsClient.counts(live).catch((): BoardCounts | null => null),
          automationsClient.listRules(live).catch(() => []),
          memoryClient.ls(live, { path: memoryPath }).catch(() => []),
          filesClient.list(live, {}).catch(() => []),
        ]),
      )
      .then(([
        status,
        channels,
        tasks,
        validators,
        proposals,
        forgeHead,
        docIds,
        docBlocks,
        agents,
        watches,
        runs,
        profiles,
        inbox,
        inboxUnread,
        jobs,
        jobCounts,
        rules,
        memoryEntries,
        files,
      ]) => {
        // Profile.key is the origin bytes — the same bytes AuthorRef::User
        // carries — so hex(key) is exactly authorName's AuthorNames key.
        const authorNames = Object.fromEntries(
          profiles.map((p) => [chatClient.keyHex(p.key), p.display_name]),
        );
        const members = validators.map(valsetClient.validatorHex);
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
                tasks,
                members,
                proposals,
                forgeHead,
                activeChannel: active,
                messages,
                authorNames,
                docIds,
                activeDocBlocks: docBlocks ?? [],
                agents,
                watches,
                runs,
                inbox,
                inboxUnread,
                jobs,
                jobCounts,
                rules,
                memoryEntries,
                files,
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

  // 2. Hydrate once the node is resolved, then follow the block stream and the
  //    node-local telemetry stream (backfilled from the ring, then live). The
  //    telemetry updaters stay pure (StrictMode double-invokes them): they append
  //    idempotently and dedupe on the strictly-increasing block height.
  useEffect(() => {
    if (!node) return;
    refresh();

    // Backfill recent telemetry, then keep any newer live frames layered on top.
    node
      .telemetry(TELEMETRY_KEEP)
      .then((frames) =>
        dispatch({
          type: "update",
          fn: (prev) => {
            const cutoff = frames.length ? frames[frames.length - 1].height : -1;
            const newer = prev.telemetry.filter((f) => f.height > cutoff);
            return { telemetry: [...frames, ...newer].slice(-TELEMETRY_KEEP) };
          },
        }),
      )
      .catch(() => {
        /* telemetry is best-effort observability; a miss just leaves it empty */
      });

    const offBlock = node.onBlock(() => {
      refresh();
    });
    const offTelemetry = node.onTelemetry((frame) => {
      dispatch({
        type: "update",
        fn: (prev) => {
          const last = prev.telemetry[prev.telemetry.length - 1];
          // Heights strictly increase; drop a seam duplicate or a reconnect replay.
          if (last && frame.height <= last.height) return {};
          return { telemetry: [...prev.telemetry, frame].slice(-TELEMETRY_KEEP) };
        },
      });
    });
    return () => {
      offBlock();
      offTelemetry();
    };
  }, [node, refresh]);

  // 3. Reflect the accent into the css var the theme reads.
  useEffect(() => {
    document.documentElement.style.setProperty("--accent", state.accent);
  }, [state.accent]);

  // 4. Drop any open doc when the node url resolves or changes — a different
  //    node has different documents. `docIds` (the browse tree) is re-enumerated
  //    from the new node's index by `refresh`, so it isn't seeded here.
  useEffect(() => {
    const url = state.nodeUrl;
    if (!url) return;
    dispatch({
      type: "patch",
      patch: {
        docIds: [],
        activeDoc: null,
        activeDocBlocks: [],
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
