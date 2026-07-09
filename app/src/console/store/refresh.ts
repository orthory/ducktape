import * as agentClient from "../../domain/agent-client";
import * as capabilityClient from "../../domain/capability-client";
import * as chatClient from "../../domain/chat-client";
import * as dispatchClient from "../../domain/dispatch-client";
import * as filesClient from "../../domain/files-client";
import type { FileEntry } from "../../domain/files-client";
import * as forgeClient from "../../domain/forge-client";
import * as governanceClient from "../../domain/governance-client";
import type { ProposalView } from "../../domain/governance-client";
import * as identityClient from "../../domain/identity-client";
import * as pagesClient from "../../domain/pages-client";
import type { PageBlock, PageMeta } from "../../domain/pages-client";
import * as profilesClient from "../../domain/profiles-client";
import * as runsClient from "../../domain/runs-client";
import * as valsetClient from "../../domain/valset-client";
import type { BlockRecord, NodeTransport } from "../../domain/transport";
import { hasFreshPending, pageSnapshotSuperseded } from "./finalization";
import type { Action } from "./reducer";
import type { ConsoleState } from "./state";
import { saveDocTabs } from "./state";

/** How many recent non-empty blocks the explorer pulls per refresh. */
export const BLOCKS_KEEP = 200;

export interface RefreshEnv {
  live: NodeTransport;
  getState: () => ConsoleState;
  dispatch: (action: Action) => void;
  fail: (err: unknown) => void;
}

type RefreshPatch = Partial<ConsoleState>;
type RefreshFetcher = (env: RefreshEnv) => Promise<RefreshPatch>;

export const fetchStatus: RefreshFetcher = async ({ live }) => ({
  status: await live.status(),
});

export const fetchChat: RefreshFetcher = async ({ live, getState }) => {
  const channels = await chatClient.channels(live);
  const current = getState().activeChannel;
  // Default-channel selection skips module-reserved channels (forge's hidden
  // `forge:<repo>:<n>` discussion threads) — the chat surface must never land
  // on one by default. A deliberately-selected id survives as long as it exists,
  // whatever its shape (future module deep-links).
  const active =
    current && channels.some((c) => c.id === current)
      ? current
      : (channels.find((c) => !chatClient.isModuleChannel(c.id))?.id ?? null);
  const messages = active ? await chatClient.latestMessages(live, active) : [];
  return { channels, activeChannel: active, messages };
};

export const fetchRoster: RefreshFetcher = async ({ live }) => {
  const [validators, residentKeys] = await Promise.all([
    // valset only exists on the NETWORKED node (the local daemon has no
    // validator set) — best-effort, so a local node reads as "no members".
    valsetClient.validators(live).catch((): number[][] => []),
    // the resident tier (staged admission) — same best-effort contract:
    // a pre-resident node (protocol < 3) reads as "no residents".
    valsetClient.residents(live).catch((): number[][] => []),
  ]);
  return {
    members: validators.map(valsetClient.validatorHex),
    residents: residentKeys.map(valsetClient.validatorHex),
  };
};

export const fetchGovernance: RefreshFetcher = async ({ live }) => ({
  // governance is a first-class operator surface but best-effort in the
  // snapshot: a node/build without it just reads as "no proposals".
  proposals: await governanceClient
    .proposals(live)
    .catch((): ProposalView[] => []),
});

export const fetchForgeHead: RefreshFetcher = async ({ live }) => ({
  forgeHead: await forgeClient.head(live),
});

export const fetchPages: RefreshFetcher = async ({ live, getState }) => {
  // the pages (docs) slice refreshes by enumeration + the open page's tree.
  const fetchedPage = getState().activePage;
  // ops submitted at or after this instant cannot be in the snapshot the
  // queries below return — pageSnapshotSuperseded keys off it at apply time.
  const fetchStartedAt = Date.now();
  const [pages, pageBlocks] = await Promise.all([
    // pages is newer than some reachable nodes: best-effort, so a node without
    // it reads as "no docs", never a failed refresh.
    pagesClient.listPages(live).catch((): PageMeta[] => []),
    fetchedPage
      ? pagesClient
          .getPage(live, fetchedPage)
          .catch((): PageBlock[] | null => null)
      : Promise.resolve<PageBlock[] | null>(null),
  ]);
  // A pages snapshot that predates a page op must not be applied: it would
  // clobber the op's preconfirmed projection. A snapshot for a page we've since
  // navigated away from is equally stale.
  const current = getState();
  const holdPages =
    current.activePage !== fetchedPage ||
    pageSnapshotSuperseded(current.ops, fetchStartedAt, Date.now());
  // reconcile doc tabs against the live enumeration: a tab whose page no
  // longer exists drops, and a now-dead active page falls back to the first
  // surviving tab. CRITICAL: only reconcile when we actually got an
  // enumeration — an empty result may be a transient failure.
  const prevTabs = current.openTabs;
  const prevActive = current.activePage;
  let openTabs = prevTabs;
  let activePage = prevActive;
  if (!holdPages && pages.length > 0) {
    const liveIds = new Set(pages.map((p) => p.id));
    openTabs = prevTabs.filter((id) => liveIds.has(id));
    if (openTabs.length !== prevTabs.length) saveDocTabs(openTabs);
    activePage =
      prevActive && liveIds.has(prevActive) ? prevActive : (openTabs[0] ?? null);
  }
  return {
    pages: holdPages ? current.pages : pages,
    activePageBlocks: holdPages
      ? current.activePageBlocks
      : (pageBlocks ?? []),
    openTabs,
    activePage,
  };
};

export const fetchAgents: RefreshFetcher = async ({ live }) => ({
  agents: await agentClient.agents(live),
});

export const fetchCapabilities: RefreshFetcher = async ({ live }) => {
  const [capabilities, capabilitiesByNode] = await Promise.all([
    // the executor registry — best-effort, so a node without the capability
    // module reads as "no executors".
    capabilityClient.capabilities(live).catch((): string[] => []),
    capabilityClient
      .capabilitiesByNode(live)
      .catch((): Map<string, string[]> => new Map()),
  ]);
  return { capabilities, capabilitiesByNode };
};

export const fetchRuns: RefreshFetcher = async ({ live }) => {
  const [watches, pendingRuns] = await Promise.all([
    runsClient.watches(live),
    // newest-first for the timeline; the wire orders by dispatch id.
    runsClient
      .pendingRuns(live)
      .then((list) => [...list].sort((a, b) => b.created_at - a.created_at)),
  ]);
  const assigneePairs = await Promise.all(
    pendingRuns.map((run) =>
      dispatchClient
        .dispatch(live, { dispatchId: run.dispatch_id })
        .then((view) => [run.run_id, dispatchClient.assigneeHex(view)] as const)
        .catch(() => [run.run_id, null] as const),
    ),
  );
  const runAssignee = new Map<string, string>();
  for (const [runId, hex] of assigneePairs) if (hex) runAssignee.set(runId, hex);
  return { watches, pendingRuns, runAssignee };
};

export const fetchNames: RefreshFetcher = async ({ live }) => {
  const [profiles, users] = await Promise.all([
    profilesClient.allProfiles(live, { from: 0, limit: 256 }),
    // identity is newer than some reachable nodes — best-effort like pages.
    identityClient
      .allUsers(live, { from: 0, limit: 256 })
      .catch((): identityClient.UserView[] => []),
  ]);
  // Profile.key is the origin bytes — the same bytes AuthorRef::User carries —
  // so hex(key) is exactly authorName's AuthorNames key. identity overlays
  // profiles for every node that user binds.
  const authorNames: Record<string, string> = Object.fromEntries(
    profiles.map((p) => [chatClient.keyHex(p.key), p.display_name]),
  );
  const nodeUsers: Record<string, { userKey: string; name: string | null }> = {};
  for (const u of users) {
    const userKey = chatClient.keyHex(u.user_key);
    for (const node of u.nodes) {
      const nodeHex = chatClient.keyHex(node);
      nodeUsers[nodeHex] = { userKey, name: u.display_name };
      if (u.display_name) authorNames[nodeHex] = u.display_name;
    }
  }
  return { authorNames, nodeUsers };
};

export const fetchFilesIndex: RefreshFetcher = async ({ live }) => ({
  // files is best-effort so a node that does not register the module reads as
  // "empty", never a failed refresh. Find under the tree root gives a flat file
  // index for the command palette; the files browser pages the tree itself.
  files: await filesClient
    .find(live, { prefix: "/" })
    .then((page) => page.entries.filter((e) => e.kind === "file"))
    .catch((): FileEntry[] => []),
});

export const fetchBlocks: RefreshFetcher = async ({ live }) => ({
  // the explorer's ring pull — best-effort, so a node without /v1/blocks reads
  // as "no blocks yet".
  blocks: await live.blocks(BLOCKS_KEEP).catch((): BlockRecord[] => []),
});

export const moduleRefreshers: Record<string, RefreshFetcher> = {
  chat: fetchChat,
  valset: fetchRoster,
  governance: fetchGovernance,
  forge: fetchForgeHead,
  pages: fetchPages,
  agent: fetchAgents,
  capability: fetchCapabilities,
  runs: fetchRuns,
  dispatch: fetchRuns,
  profiles: fetchNames,
  identity: fetchNames,
  files: fetchFilesIndex,
};

const mergePatches = (patches: RefreshPatch[]): RefreshPatch =>
  Object.assign({}, ...patches);

export const refreshAll = (env: RefreshEnv): Promise<void> =>
  Promise.all([
    fetchStatus(env),
    fetchChat(env),
    fetchRoster(env),
    fetchGovernance(env),
    fetchForgeHead(env),
    fetchPages(env),
    fetchAgents(env),
    fetchCapabilities(env),
    fetchRuns(env),
    fetchNames(env),
    fetchFilesIndex(env),
    fetchBlocks(env),
  ])
    .then((patches) => {
      env.dispatch({
        type: "patch",
        patch: { connected: true, ...mergePatches(patches) },
      });
    })
    .catch((err) => {
      env.dispatch({ type: "patch", patch: { connected: false } });
      env.fail(err);
    });

export const refreshModules = (
  env: RefreshEnv,
  modules: Iterable<string>,
  options: { includeBlocks?: boolean; respectFreshPending?: boolean } = {},
): Promise<void> => {
  if (options.respectFreshPending && hasFreshPending(env.getState().ops, Date.now())) {
    return Promise.resolve();
  }
  const fetchers = new Set<RefreshFetcher>();
  for (const module of modules) {
    const fetcher = moduleRefreshers[module];
    if (fetcher) fetchers.add(fetcher);
  }
  if (options.includeBlocks) fetchers.add(fetchBlocks);
  if (fetchers.size === 0) return Promise.resolve();
  return Promise.all([...fetchers].map((fetcher) => fetcher(env)))
    .then((patches) => {
      env.dispatch({ type: "patch", patch: mergePatches(patches) });
    })
    .catch(env.fail);
};
