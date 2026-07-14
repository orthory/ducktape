import { describe, expect, it } from "vitest";

import {
  CeremonyIncomplete,
  actionLabel,
  canSettleEarly,
  decisionThreshold,
  driveMembership,
  proposals,
  tally,
  type ProposalView,
} from "./governance-client";
import type { NodeTransport } from "./transport";

const accountProposal = (overrides: Partial<ProposalView> = {}): ProposalView => ({
  proposal_id: "p",
  action: { signal: { text: "ship" } },
  proposer: [1],
  created_at: 1,
  deadline: 10,
  status: "open",
  votes: [],
  voter_kind: "account",
  electorate: [
    [[1], 60],
    [[2], 30],
    [[3], 10],
  ],
  voting_rule: { participating_majority: { quorum: 50 } },
  ...overrides,
});

/** A validator-mode membership proposal whose action matches the ceremony under
 *  test. Electorate: three nodes, one vote each. */
const validatorProposal = (overrides: Partial<ProposalView> = {}): ProposalView => ({
  proposal_id: "resident:09:0",
  action: { add_resident: { key: [9] } },
  proposer: [1],
  created_at: 1,
  deadline: 1_000_000,
  status: "open",
  votes: [],
  voter_kind: "validator_node",
  electorate: [
    [[1], 1],
    [[2], 1],
    [[3], 1],
  ],
  voting_rule: { threshold: { required_yes: 2 } },
  ...overrides,
});

/** A transport whose `proposals` reads walk `states` (one snapshot per read)
 *  and whose submits always land — enough to script a whole ceremony. The
 *  proposal id in `states` must match what the ceremony mints/adopts. */
const ceremonyTransport = (states: ProposalView[][]): NodeTransport => {
  let reads = 0;
  return {
    query: async () => ({
      proposals: states[Math.min(reads++, states.length - 1)],
    }),
    submitControl: async () => ({ height: 1, appHash: "aa".repeat(32) }),
  } as unknown as NodeTransport;
};

describe("weighted governance", () => {
  it("labels both governance-selected voting modes", () => {
    expect(actionLabel({ set_share_mode: { enabled: true } })).toBe("Use account shares");
    expect(actionLabel({ set_share_mode: { enabled: false } })).toBe("Use validator votes");
  });

  it("tallies frozen account power and applies the snapshotted rule", () => {
    const signal = accountProposal({
      votes: [
        [[1], false],
        [[2], true],
        [[3], true],
      ],
    });
    expect(tally(signal)).toEqual({ yes: 40, no: 60, total: 100 });
    expect(decisionThreshold(signal, 99)).toBe(50);
    expect(canSettleEarly(signal, 99)).toBe(false);

    const structural = accountProposal({
      action: { set_shares: { account_id: [3], shares: 20 } },
      votes: [
        [[1], true],
        [[2], true],
      ],
      voting_rule: { threshold: { required_yes: 67 } },
    });
    expect(tally(structural).yes).toBe(90);
    expect(canSettleEarly(structural, 99)).toBe(true);

    expect(
      tally({ ...structural, votes: [...structural.votes, [[9], true]] }).yes,
    ).toBe(90);
  });

  it("surfaces a rejected ceremony as a failure, not a success", async () => {
    // The module rejects the proposal at execute time (e.g. removing the last
    // validator): driveMembership must REJECT with the outcome in the message —
    // an op tracker that treats resolution as success would otherwise paint a
    // rejected membership change green.
    const decidedByOne = { threshold: { required_yes: 1 } } as const;
    const states: ProposalView[][] = [
      [], // initial read: nothing open → the ceremony mints "resident:09:0"
      // after propose+vote: our sole ballot decides, settle early…
      [validatorProposal({ status: "open", votes: [[[1], true]], voting_rule: decidedByOne })],
      // …but execute settled it REJECTED (e.g. a competing change won).
      [validatorProposal({ status: "rejected", votes: [[[1], true]], voting_rule: decidedByOne })],
    ];
    const transport = ceremonyTransport(states);
    const err = await driveMembership(
      transport,
      { add_resident: { key: [9] } },
      "resident:",
      "09",
      1,
    ).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(CeremonyIncomplete);
    expect((err as CeremonyIncomplete).status).toBe("rejected");
    expect((err as CeremonyIncomplete).message).toContain("rejected");
  });

  it("surfaces a ballot shortfall as awaiting, and a passed ceremony resolves", async () => {
    // Shortfall: 1 yes of 2 required (3 members) → rejects with "awaiting".
    const shortfall = ceremonyTransport([
      [],
      [validatorProposal({ status: "open", votes: [[[1], true]] })],
    ]);
    const err = await driveMembership(
      shortfall,
      { add_resident: { key: [9] } },
      "resident:",
      "09",
      3,
    ).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(CeremonyIncomplete);
    expect((err as CeremonyIncomplete).status).toBe("open");
    expect((err as CeremonyIncomplete).message).toContain("1 of 2");

    // Passed: the deciding ballot lands and execute settles it.
    const decidedByOne = { threshold: { required_yes: 1 } } as const;
    const passed = ceremonyTransport([
      [],
      [validatorProposal({ status: "open", votes: [[[1], true]], voting_rule: decidedByOne })],
      [validatorProposal({ status: "passed", votes: [[[1], true]], voting_rule: decidedByOne })],
    ]);
    await expect(
      driveMembership(passed, { add_resident: { key: [9] } }, "resident:", "09", 1),
    ).resolves.toMatchObject({ status: "passed" });
  });

  it("defaults proposal-time fields from a pre-share node", async () => {
    const legacy = accountProposal();
    const { voter_kind: _voterKind, electorate: _electorate, voting_rule: _rule, ...wire } = legacy;
    const transport = {
      query: async () => ({ proposals: [wire] }),
    } as unknown as Parameters<typeof proposals>[0];

    await expect(proposals(transport)).resolves.toMatchObject([
      {
        voter_kind: "validator_node",
        electorate: [],
        voting_rule: "dynamic_validator_majority",
      },
    ]);
  });
});
