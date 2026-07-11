// Typed client for the node's `governance` system module — the TS mirror of
// `crates/system/governance-interface`. Governance is member-gated decision
// making over the validator set: a CURRENT valset member opens a proposal,
// members vote before a consensus-time deadline, and anyone may execute once
// its frozen rule is irreversible or the deadline passes. A validator majority
// may make a one-way transition to explicit,
// non-transferable Identity-account shares; each later proposal freezes its
// account-power electorate and decision rule.
//
// The embedded daemon signs every submit with THIS node's authenticated key and
// ignores the claimed `origin` (see bin/node/src/main.rs), so a vote/propose/
// execute from the console is authored by this node's identity — the
// authenticated authorship governance relies on. Callers therefore pass no
// origin here.

import { keyHex } from "./chat-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "governance";

// ── Wire types (GovReply / ProposalView payloads, verbatim) ─────────────────

export type ProposalStatus = "open" | "passed" | "rejected";

export interface ShareAllocation {
  account_id: number[];
  shares: number;
}

export interface SharesView {
  active: boolean;
  allocations: ShareAllocation[];
  total: number;
}

export type VoterKind = "validator_node" | "account";

export type VotingRule =
  | "dynamic_validator_majority"
  | { threshold: { required_yes: number } }
  | { participating_majority: { quorum: number } };

/** What a passing proposal DOES. Serializes as a single-variant object. */
export type GovAction =
  | { add_validator: { key: number[] } }
  | { remove_validator: { key: number[] } }
  | { signal: { text: string } }
  | { add_resident: { key: number[] } }
  | { remove_resident: { key: number[] } }
  | { schedule_upgrade: { name: string; activation_height: number; to_version: number } }
  | { cancel_upgrade: { name: string } }
  | { adopt_shares: { allocations: ShareAllocation[] } }
  | { set_shares: { account_id: number[]; shares: number } };

export interface ProposalView {
  proposal_id: string;
  action: GovAction;
  proposer: number[];
  created_at: number;
  deadline: number;
  status: ProposalStatus;
  /** Ballots by member key: [key, approve]. */
  votes: [number[], boolean][];
  voter_kind: VoterKind;
  /** Frozen [principal, power] rows; empty only for restored legacy proposals. */
  electorate: [number[], number][];
  voting_rule: VotingRule;
}

/** The default voting window (consensus-time units), matching the node's own
 *  admission flow in bin/node/src/main.rs. */
export const DEFAULT_VOTING_PERIOD = 1_000_000;

// ── Queries (reads) ─────────────────────────────────────

const withVotingSnapshot = (proposal: ProposalView): ProposalView => ({
  ...proposal,
  voter_kind: proposal.voter_kind ?? "validator_node",
  electorate: proposal.electorate ?? [],
  voting_rule: proposal.voting_rule ?? "dynamic_validator_majority",
});

export const proposals = (transport: NodeTransport): Promise<ProposalView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "proposals"))
    .then((reply) => replyVariant<ProposalView[]>(reply, "proposals"))
    .then((rows) => rows.map(withVotingSnapshot));

export const shares = (transport: NodeTransport): Promise<SharesView> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "shares"))
    .then((reply) => replyVariant<SharesView>(reply, "shares"));

// ── Msgs (writes) ───────────────────────────────────────

export const propose = (
  transport: NodeTransport,
  params: { proposalId: string; action: GovAction; votingPeriod?: number },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    propose: {
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
    vote: { proposal_id: params.proposalId, approve: params.approve },
  });

export const execute = (
  transport: NodeTransport,
  params: { proposalId: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { execute: { proposal_id: params.proposalId } });

// ── Pure helpers ────────────────────────────────────────

export const proposerHex = (key: number[]): string => keyHex(key);

/** A human summary of what a proposal would do. */
export const actionLabel = (action: GovAction): string => {
  if ("add_validator" in action) return "Add validator";
  if ("remove_validator" in action) return "Remove validator";
  if ("add_resident" in action) return "Add resident";
  if ("remove_resident" in action) return "Remove resident";
  if ("schedule_upgrade" in action) return "Schedule upgrade";
  if ("cancel_upgrade" in action) return "Cancel upgrade";
  if ("adopt_shares" in action) return "Adopt governance shares";
  if ("set_shares" in action) return "Set account shares";
  return "Signal";
};

/** The subject key of a membership action, hex — null for a signal. */
export const actionKeyHex = (action: GovAction): string | null => {
  if ("add_validator" in action) return keyHex(action.add_validator.key);
  if ("remove_validator" in action) return keyHex(action.remove_validator.key);
  if ("add_resident" in action) return keyHex(action.add_resident.key);
  if ("remove_resident" in action) return keyHex(action.remove_resident.key);
  if ("set_shares" in action) return keyHex(action.set_shares.account_id);
  return null;
};

/** The free text of a signal action — null for a membership action. */
export const actionText = (action: GovAction): string | null =>
  "signal" in action ? action.signal.text : null;

export interface Tally {
  yes: number;
  no: number;
  total: number;
}

export const tally = (proposal: ProposalView): Tally => {
  const powers = new Map(
    proposal.electorate.map(([principal, power]) => [keyHex(principal), power]),
  );
  let yes = 0;
  let no = 0;
  for (const [principal, approve] of proposal.votes) {
    const power = proposal.electorate.length === 0 ? 1 : (powers.get(keyHex(principal)) ?? 0);
    if (approve) yes += power;
    else no += power;
  }
  return { yes, no, total: yes + no };
};

/** Strict majority of the current member count: members / 2 + 1. */
export const majorityOf = (memberCount: number): number =>
  Math.floor(memberCount / 2) + 1;

export const totalPower = (proposal: ProposalView, memberCount: number): number =>
  proposal.electorate.length > 0
    ? proposal.electorate.reduce((sum, [, power]) => sum + power, 0)
    : memberCount;

export const decisionThreshold = (
  proposal: ProposalView,
  memberCount: number,
): number => {
  const rule = proposal.voting_rule;
  if (rule === "dynamic_validator_majority") return majorityOf(memberCount);
  if ("threshold" in rule) return rule.threshold.required_yes;
  return rule.participating_majority.quorum;
};

export const canSettleEarly = (
  proposal: ProposalView,
  memberCount: number,
): boolean => {
  const counts = tally(proposal);
  const rule = proposal.voting_rule;
  if (rule === "dynamic_validator_majority") {
    return counts.yes >= majorityOf(memberCount);
  }
  if ("threshold" in rule) return counts.yes >= rule.threshold.required_yes;
  return (
    counts.total >= rule.participating_majority.quorum &&
    counts.yes > totalPower(proposal, memberCount) - counts.yes
  );
};
