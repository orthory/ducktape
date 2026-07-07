// Pure derivations for the Node view's connection + health surfaces. Everything
// here composes signals THIS node already commits — the valset roster
// (validators/residents), the recent finalized-block ring, and the capability
// registry — into an operational read of the mesh. No signal is invented: peer
// liveness is derived strictly from which validator VERIFIABLY proposed recent
// blocks (the frame's signer), and residents — which hold no quorum seat and so
// never propose — are reported as statesync standing, not as "offline".

import { displayNameForKey, normalizeKey, sameKey, shortKey } from "../../../domain/names";
import type { BlockRecord } from "../../../domain/transport";

// ── Proposal window (liveness from the block ring) ──────────

/** What one proposer did across the observed block window. */
export interface ProposerActivity {
  /** blocks in the window this key VERIFIABLY proposed (the frame's signer). */
  count: number;
  /** highest block height it proposed — the "last seen leading" height. */
  lastHeight: number;
}

/** The recent-proposal tally over a slice of the block ring — the denominator
 *  and per-proposer counts a liveness read is built from. */
export interface ProposalWindow {
  /** keyNorm → activity, for every key that proposed in the window. */
  byProposer: Map<string, ProposerActivity>;
  /** blocks counted (the share denominator). */
  total: number;
  /** lowest / highest block height in the window, null when empty. */
  low: number | null;
  high: number | null;
}

/** Tally proposals across the given blocks (any order; heights compared
 *  numerically). Blocks with a blank proposer are counted toward `total` but
 *  attributed to no key. */
export function proposalWindow(blocks: readonly BlockRecord[]): ProposalWindow {
  const byProposer = new Map<string, ProposerActivity>();
  let low: number | null = null;
  let high: number | null = null;
  for (const block of blocks) {
    const key = normalizeKey(block.proposer);
    if (key) {
      const cur = byProposer.get(key);
      if (cur) {
        cur.count += 1;
        if (block.height > cur.lastHeight) cur.lastHeight = block.height;
      } else {
        byProposer.set(key, { count: 1, lastHeight: block.height });
      }
    }
    if (low === null || block.height < low) low = block.height;
    if (high === null || block.height > high) high = block.height;
  }
  return { byProposer, total: blocks.length, low, high };
}

// ── Peer roster (the node's connection list) ────────────────

export type PeerTier = "validator" | "resident";

/** One row of the node's connection list — a valset member seen through this
 *  node's operational lens (identity + derived liveness + announced work). */
export interface PeerVM {
  key: string;
  keyNorm: string;
  displayName: string;
  shortKey: string;
  initials: string;
  /** Which valset tier: seated quorum (`validator`) or warming statesync
   *  standing (`resident`). The tiers never overlap. */
  tier: PeerTier;
  /** This is the local node's own validator identity. */
  isSelf: boolean;
  /** The local node founded the network at genesis — provenance only, and only
   *  ever knowable for `isSelf` (a remote peer's genesis role isn't committed
   *  in a form this client reads). */
  isFounder: boolean;
  /** Executor tags this node announced to the capability registry. */
  capabilities: string[];
  /** Recent-proposal activity, or null when it proposed nothing in the window
   *  (every resident, plus any validator that hasn't led lately). */
  activity: ProposerActivity | null;
  /** Fraction of window proposals this peer led, 0..1 (0 without activity). */
  share: number;
}

export interface BuildPeersInput {
  members: readonly string[];
  residents: readonly string[];
  authorNames: Record<string, string>;
  workspace: { pubkey: string; founder: boolean } | null;
  capabilitiesByNode: Map<string, string[]>;
  window: ProposalWindow;
}

/** First two alphanumeric initials of a display name; falls back to the first
 *  two alnum chars of a hex key ("4c…" → "4C"). Mirrors the Members avatar. */
export function initialsOf(name: string): string {
  const trimmed = name.replace(/\s*\([^)]*\)\s*/g, " ").trim();
  if (!trimmed) return "?";
  const words = trimmed.split(/\s+/).filter((w) => /^[\p{L}\p{N}]/u.test(w));
  if (words.length >= 2) return `${words[0][0]}${words[1][0]}`.toUpperCase();
  const alnum = (words[0] ?? trimmed).replace(/[^\p{L}\p{N}]/gu, "");
  return alnum.slice(0, 2).toUpperCase() || "?";
}

/** Build the peer roster: validators first (self pinned, then busiest
 *  proposers, then by name), then residents (self pinned, then by name). */
export function buildPeers(input: BuildPeersInput): PeerVM[] {
  const localKey = input.workspace?.pubkey ?? null;
  const total = input.window.total;
  const toVM = (key: string, tier: PeerTier): PeerVM => {
    const keyNorm = normalizeKey(key);
    const isSelf = sameKey(key, localKey);
    const activity = tier === "validator" ? (input.window.byProposer.get(keyNorm) ?? null) : null;
    const displayName = displayNameForKey(key, input.authorNames) ?? shortKey(key);
    return {
      key,
      keyNorm,
      displayName,
      shortKey: shortKey(key),
      initials: initialsOf(displayName),
      tier,
      isSelf,
      isFounder: Boolean(isSelf && input.workspace?.founder),
      capabilities: input.capabilitiesByNode.get(keyNorm) ?? [],
      activity,
      share: activity && total > 0 ? activity.count / total : 0,
    };
  };

  const rank = (a: PeerVM, b: PeerVM): number => {
    if (a.isSelf !== b.isSelf) return a.isSelf ? -1 : 1;
    if (b.share !== a.share) return b.share - a.share;
    return a.displayName.localeCompare(b.displayName);
  };

  return [
    ...input.members.map((key) => toVM(key, "validator")).sort(rank),
    ...input.residents.map((key) => toVM(key, "resident")).sort(rank),
  ];
}

// ── Health strip (recent commit outcomes) ───────────────────

/** One tick of the status-page health bar — a finalized block and how its op
 *  settled (an `applied` op mutated state; a `rejected` op finalized but rolled
 *  back — a failed tx, normal texture rather than a node fault). */
export interface HealthSeg {
  height: number;
  disposition: BlockRecord["disposition"];
}

/**
 * The last `slots` finalized blocks as health ticks, oldest-first so a
 * left→right render puts the newest commit on the right. `state.blocks` is
 * already oldest-first, so this is a tail slice.
 */
export function healthSegments(blocks: readonly BlockRecord[], slots: number): HealthSeg[] {
  const start = Math.max(0, blocks.length - slots);
  return blocks.slice(start).map((block) => ({
    height: block.height,
    disposition: block.disposition,
  }));
}

/** Applied/rejected split of a health strip — the bar's caption numbers. */
export interface CommitHealth {
  applied: number;
  rejected: number;
  total: number;
}

export function commitHealth(segments: readonly HealthSeg[]): CommitHealth {
  let rejected = 0;
  for (const seg of segments) if (seg.disposition === "rejected") rejected += 1;
  return { applied: segments.length - rejected, rejected, total: segments.length };
}

// ── Node liveness headline ──────────────────────────────────

export type LivenessTone = "live" | "idle" | "stopped" | "offline";

/** The one-line liveness read for the Node header/health card. Distinct from
 *  the commit strip: this is whether the CHAIN is advancing for this node, not
 *  whether individual ops applied. `tip` advancing (the ungated ws height) is
 *  the positive signal; a connected node with no tip yet is `idle`, not down. */
export interface NodeLiveness {
  tone: LivenessTone;
  label: string;
  detail: string;
}

export function nodeLiveness(input: {
  connected: boolean;
  managed: boolean;
  tip: number | null;
}): NodeLiveness {
  if (!input.connected) {
    return input.managed
      ? { tone: "stopped", label: "Stopped", detail: "local daemon is not running" }
      : { tone: "offline", label: "Offline", detail: "node is unreachable" };
  }
  if (input.tip === null) {
    return { tone: "idle", label: "Connecting", detail: "waiting for the block stream" };
  }
  return {
    tone: "live",
    label: "Live",
    detail: `following the chain at height ${input.tip.toLocaleString()}`,
  };
}
