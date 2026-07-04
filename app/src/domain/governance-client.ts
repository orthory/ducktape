// Typed client for the node's `governance` system module — the TS mirror of
// `crates/system/governance-interface`. Governance is member-gated decision
// making over the validator set: a CURRENT valset member opens a proposal,
// members vote before a consensus-time deadline, and anyone may execute once
// the outcome is decidable (deadline passed, or a strict majority already
// reached). Membership is pure majority governance — one member, one vote;
// there is NO founder/genesis privilege.
//
// The embedded daemon signs every submit with THIS node's validator key and
// ignores the claimed `origin` (see bin/node/src/main.rs), so a vote/propose/
// execute from the console is authored by this node's validator identity — the
// authenticated authorship governance relies on. Callers therefore pass no
// origin here.

import { keyHex } from "./chat-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "governance";

// ── Wire types (GovReply / ProposalView payloads, verbatim) ─────────────────

export type ProposalStatus = "Open" | "Passed" | "Rejected";

/** What a passing proposal DOES. Serializes as a single-variant object. */
export type GovAction =
  | { AddValidator: { key: number[] } }
  | { RemoveValidator: { key: number[] } }
  | { Signal: { text: string } };

export interface ProposalView {
  proposal_id: string;
  action: GovAction;
  proposer: number[];
  created_at: number;
  deadline: number;
  status: ProposalStatus;
  /** Ballots by member key: [key, approve]. */
  votes: [number[], boolean][];
}

/** The default voting window (consensus-time units), matching the node's own
 *  admission flow in bin/node/src/main.rs. */
export const DEFAULT_VOTING_PERIOD = 1_000_000;

// ── Queries (reads) ─────────────────────────────────────

export const proposals = (transport: NodeTransport): Promise<ProposalView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Proposals"))
    .then((reply) => replyVariant<ProposalView[]>(reply, "Proposals"));

// ── Msgs (writes) ───────────────────────────────────────

export const propose = (
  transport: NodeTransport,
  params: { proposalId: string; action: GovAction; votingPeriod?: number },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    Propose: {
      proposal_id: params.proposalId,
      action: params.action,
      voting_period: params.votingPeriod ?? DEFAULT_VOTING_PERIOD,
    },
  });

export const vote = (
  transport: NodeTransport,
  params: { proposalId: string; approve: boolean },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    Vote: { proposal_id: params.proposalId, approve: params.approve },
  });

export const execute = (
  transport: NodeTransport,
  params: { proposalId: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { Execute: { proposal_id: params.proposalId } });

// ── Pure helpers ────────────────────────────────────────

export const proposerHex = (key: number[]): string => keyHex(key);

/** A human summary of what a proposal would do. */
export const actionLabel = (action: GovAction): string => {
  if ("AddValidator" in action) return "Add validator";
  if ("RemoveValidator" in action) return "Remove validator";
  return "Signal";
};

/** The subject key of a membership action, hex — null for a signal. */
export const actionKeyHex = (action: GovAction): string | null => {
  if ("AddValidator" in action) return keyHex(action.AddValidator.key);
  if ("RemoveValidator" in action) return keyHex(action.RemoveValidator.key);
  return null;
};

/** The free text of a signal action — null for a membership action. */
export const actionText = (action: GovAction): string | null =>
  "Signal" in action ? action.Signal.text : null;

export interface Tally {
  yes: number;
  no: number;
  total: number;
}

export const tally = (proposal: ProposalView): Tally => {
  let yes = 0;
  let no = 0;
  for (const [, approve] of proposal.votes) {
    if (approve) yes += 1;
    else no += 1;
  }
  return { yes, no, total: proposal.votes.length };
};

/** Strict majority of the current member count: members / 2 + 1. */
export const majorityOf = (memberCount: number): number =>
  Math.floor(memberCount / 2) + 1;
