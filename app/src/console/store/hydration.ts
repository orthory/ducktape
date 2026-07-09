// Module-root-scoped hydration — which console slices a block actually moved.
//
// Every module's state root rides `status().modules[]`, so diffing roots
// across a head advance names exactly the modules a block changed — and the
// console re-queries ONLY the slice groups that read them, instead of the
// wholesale ~17-query refresh on every block event. The full refresh()
// remains the boot/reconnect/completion path; this module owns the PURE
// pieces (root diffing, the module → slice-group map, and the per-group
// fetchers both paths compose) so they test without React.
//
// The replica pipeline (unified-node design, phases 2-4) is what makes this
// exact: every node — validator or resident — folds per block and emits a ws
// Block event per height, so a root diff between consecutive statuses is a
// complete account of what that block touched. Under the old
// boundary-reinstall model residents jumped N heights at a time; the diff
// was still sound then, just coarser.
//
// Group boundaries follow the DERIVED maps, not just the queries: profiles
// and identity fetch together because `authorNames` overlays one on the
// other; runs/dispatch/saga share a group because the runs timeline reads
// dispatch assignments per pending run.

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
import type { NodeStatus, NodeTransport } from "../../domain/transport";

// ── Types ───────────────────────────────────────────────

/** One independently-refreshable cluster of console slices. */
export type SliceGroup =
  | "chat"
  | "valset"
  | "governance"
  | "forge"
  | "pages"
  | "agents"
  | "capability"
  | "runs"
  | "people"
  | "files";

// ── Root diffing ────────────────────────────────────────

/** The module ids whose state roots moved between two statuses. A first
 *  sighting (no previous status) reads as "everything changed" — the caller
 *  degrades to a full hydrate, which is exactly right at boot/reconnect. */
export const changedModules = (
  prev: NodeStatus | null,
  next: NodeStatus,
): Set<string> => {
  if (!prev) return new Set(next.modules.map((m) => m.id));
  const before = new Map(prev.modules.map((m) => [m.id, m.root]));
  return new Set(
    next.modules.filter((m) => before.get(m.id) !== m.root).map((m) => m.id),
  );
};

/** Module id → the slice groups that read it. A module absent here moves no
 *  console slice (kv, blobstore, tagging, upgrade, ... have no console
 *  projection of their own — their effects surface through the modules that
 *  do, e.g. a tagging mention lands as a runs assignment). */
const GROUPS_BY_MODULE: Record<string, readonly SliceGroup[]> = {
  chat: ["chat"],
  valset: ["valset"],
  governance: ["governance"],
  forge: ["forge"],
  pages: ["pages"],
  agent: ["agents"],
  capability: ["capability"],
  runs: ["runs"],
  dispatch: ["runs"],
  saga: ["runs"],
  profiles: ["people"],
  identity: ["people"],
  files: ["files"],
};

/** The slice groups a changed-module set touches. */
export const scopeFor = (changed: Set<string>): Set<SliceGroup> => {
  const scope = new Set<SliceGroup>();
  for (const id of changed) {
    for (const group of GROUPS_BY_MODULE[id] ?? []) scope.add(group);
  }
  return scope;
};

// ── Group fetchers ──────────────────────────────────────
//
// Each mirrors one cluster of the wholesale refresh's queries and returns
// exactly the state slices it owns, so a scoped patch is a plain subset of
// the snapshot fields. Best-effort contracts (`.catch(() => empty)`) are
// preserved per query — a node missing a module degrades, never fails.

export interface ChatSlices {
  channels: chatClient.Channel[];
  activeChannel: string | null;
  messages: chatClient.MessageView[];
}

/** Channels + the active channel's latest messages. Default-channel
 *  selection skips module-reserved channels (forge's hidden discussion
 *  threads); a deliberately-selected id survives as long as it exists. */
export const fetchChatSlices = (
  live: NodeTransport,
  currentActive: string | null,
): Promise<ChatSlices> =>
  Promise.resolve()
    .then(() => chatClient.channels(live))
    .then((channels) => {
      const active =
        currentActive && channels.some((c) => c.id === currentActive)
          ? currentActive
          : (channels.find((c) => !chatClient.isModuleChannel(c.id))?.id ?? null);
      return Promise.resolve()
        .then(() => (active ? chatClient.latestMessages(live, active) : []))
        .then((messages) => ({ channels, activeChannel: active, messages }));
    });

export interface ValsetSlices {
  members: string[];
  residents: string[];
}

export const fetchValsetSlices = (live: NodeTransport): Promise<ValsetSlices> =>
  Promise.resolve()
    .then(() =>
      Promise.all([
        valsetClient.validators(live).catch((): number[][] => []),
        valsetClient.residents(live).catch((): number[][] => []),
      ]),
    )
    .then(([validators, residentKeys]) => ({
      members: validators.map(valsetClient.validatorHex),
      residents: residentKeys.map(valsetClient.validatorHex),
    }));

export const fetchGovernanceSlices = (
  live: NodeTransport,
): Promise<{ proposals: ProposalView[] }> =>
  Promise.resolve()
    .then(() => governanceClient.proposals(live).catch((): ProposalView[] => []))
    .then((proposals) => ({ proposals }));

export const fetchForgeSlices = (
  live: NodeTransport,
): Promise<{ forgeHead: Awaited<ReturnType<typeof forgeClient.head>> }> =>
  Promise.resolve()
    .then(() => forgeClient.head(live))
    .then((forgeHead) => ({ forgeHead }));

export interface PagesSlices {
  pages: PageMeta[];
  pageBlocks: PageBlock[] | null;
  /** The page whose tree `pageBlocks` reflects — the provider's hold logic
   *  compares it against the CURRENT active page at apply time. */
  fetchedPage: string | null;
}

export const fetchPagesSlices = (
  live: NodeTransport,
  activePage: string | null,
): Promise<PagesSlices> =>
  Promise.resolve()
    .then(() =>
      Promise.all([
        pagesClient.listPages(live).catch((): PageMeta[] => []),
        activePage
          ? pagesClient.getPage(live, activePage).catch((): PageBlock[] | null => null)
          : Promise.resolve<PageBlock[] | null>(null),
      ]),
    )
    .then(([pages, pageBlocks]) => ({ pages, pageBlocks, fetchedPage: activePage }));

export const fetchAgentsSlices = (
  live: NodeTransport,
): Promise<{ agents: Awaited<ReturnType<typeof agentClient.agents>> }> =>
  Promise.resolve()
    .then(() => agentClient.agents(live))
    .then((agents) => ({ agents }));

export interface CapabilitySlices {
  capabilities: string[];
  capabilitiesByNode: Map<string, string[]>;
}

export const fetchCapabilitySlices = (
  live: NodeTransport,
): Promise<CapabilitySlices> =>
  Promise.resolve()
    .then(() =>
      Promise.all([
        capabilityClient.capabilities(live).catch((): string[] => []),
        capabilityClient
          .capabilitiesByNode(live)
          .catch((): Map<string, string[]> => new Map()),
      ]),
    )
    .then(([capabilities, capabilitiesByNode]) => ({
      capabilities,
      capabilitiesByNode,
    }));

export interface RunsSlices {
  watches: Awaited<ReturnType<typeof runsClient.watches>>;
  pendingRuns: Awaited<ReturnType<typeof runsClient.pendingRuns>>;
  runAssignee: Map<string, string>;
}

/** Watches + the pending-run timeline + one dispatch read per in-flight run
 *  (its executor node) — bounded by pendingRuns.length, each best-effort. */
export const fetchRunsSlices = (live: NodeTransport): Promise<RunsSlices> =>
  Promise.resolve()
    .then(() =>
      Promise.all([
        runsClient.watches(live),
        runsClient
          .pendingRuns(live)
          .then((list) => [...list].sort((a, b) => b.created_at - a.created_at)),
      ]),
    )
    .then(([watches, pendingRuns]) =>
      Promise.all(
        pendingRuns.map((run) =>
          dispatchClient
            .dispatch(live, { dispatchId: run.dispatch_id })
            .then((view) => [run.run_id, dispatchClient.assigneeHex(view)] as const)
            .catch(() => [run.run_id, null] as const),
        ),
      ).then((assigneePairs) => {
        const runAssignee = new Map<string, string>();
        for (const [runId, hex] of assigneePairs) if (hex) runAssignee.set(runId, hex);
        return { watches, pendingRuns, runAssignee };
      }),
    );

export interface PeopleSlices {
  authorNames: Record<string, string>;
  nodeUsers: Record<string, { userKey: string; name: string | null }>;
  accountKeys: Record<string, identityClient.MemberKeyView[]>;
}

/** Profiles + identity accounts, derived together: identity's per-user
 *  display name OVERLAYS profiles for every node that user binds — the user
 *  is the durable identity, the node just its hardware. */
export const fetchPeopleSlices = (live: NodeTransport): Promise<PeopleSlices> =>
  Promise.resolve()
    .then(() =>
      Promise.all([
        profilesClient.allProfiles(live, { from: 0, limit: 256 }),
        identityClient
          .allAccounts(live, { from: 0, limit: 256 })
          .catch((): identityClient.AccountView[] => []),
      ]),
    )
    .then(([profiles, users]) => {
      const authorNames: Record<string, string> = Object.fromEntries(
        profiles.map((p) => [chatClient.keyHex(p.key), p.display_name]),
      );
      const nodeUsers: Record<string, { userKey: string; name: string | null }> = {};
      const accountKeys: Record<string, identityClient.MemberKeyView[]> = {};
      for (const u of users) {
        const userKey = chatClient.keyHex(u.account_id);
        accountKeys[userKey] = u.member_keys;
        for (const node of u.nodes) {
          const nodeHex = chatClient.keyHex(node);
          nodeUsers[nodeHex] = { userKey, name: u.display_name };
          if (u.display_name) authorNames[nodeHex] = u.display_name;
        }
      }
      return { authorNames, nodeUsers, accountKeys };
    });

export const fetchFilesSlices = (
  live: NodeTransport,
): Promise<{ files: FileEntry[] }> =>
  Promise.resolve()
    .then(() =>
      filesClient
        .find(live, { prefix: "/" })
        .then((page) => page.entries.filter((e) => e.kind === "file"))
        .catch((): FileEntry[] => []),
    )
    .then((files) => ({ files }));
