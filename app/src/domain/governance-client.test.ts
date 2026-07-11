import { describe, expect, it } from "vitest";

import {
  canSettleEarly,
  decisionThreshold,
  proposals,
  tally,
  type ProposalView,
} from "./governance-client";

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

describe("weighted governance", () => {
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
