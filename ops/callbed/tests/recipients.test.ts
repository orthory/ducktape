// L1 unit test for the REAL huddleRecipients fan-out helper.
// Imports the actual app function (no reimplementation) and asserts it returns
// exactly the OTHER members' node keys, excludes self, and dedupes repeats.
import { expect, test } from "bun:test";
import { huddleRecipients } from "../../../app/src/domain/voice-session";
import type { HuddleMember } from "../../../app/src/domain/chat-client";

// Build a HuddleMember in the exact shape the function consumes: `node` is the
// 32-byte ed25519 mesh key as a byte array, `user` the origin bytes.
const member = (nodeHex: string, userTag = 0): HuddleMember => {
  const bytes = nodeHex.match(/../g)!.map((h) => parseInt(h, 16));
  return { user: [userTag], node: bytes, joined_at: 0 };
};

// Distinct 32-byte keys as 64-hex.
const SELF = "aa".repeat(32);
const PEER1 = "01".repeat(32);
const PEER2 = "02".repeat(32);

test("returns the OTHER members' node keys and excludes self", () => {
  const roster = [member(SELF), member(PEER1), member(PEER2)];
  expect(huddleRecipients(roster, SELF).sort()).toEqual([PEER1, PEER2].sort());
});

test("empty roster -> []", () => {
  expect(huddleRecipients([], SELF)).toEqual([]);
});

test("only-self roster -> []", () => {
  expect(huddleRecipients([member(SELF)], SELF)).toEqual([]);
});

test("duplicate member.node is deduped", () => {
  const roster = [member(SELF), member(PEER1), member(PEER1), member(PEER1, 9)];
  // PEER1 appears three times (incl. a second user sharing the same node) yet
  // must be emitted once; self still excluded.
  expect(huddleRecipients(roster, SELF)).toEqual([PEER1]);
});

test("two users sharing one node collapse to a single recipient", () => {
  // Real-world case from the doc comment: two users huddling from one daemon.
  const roster = [member(SELF, 1), member(PEER1, 2), member(PEER1, 3)];
  expect(huddleRecipients(roster, SELF)).toEqual([PEER1]);
});

test("self match is case-insensitive on the supplied self hex", () => {
  // keyHex emits lowercase; the function lowercases selfNodeHex before compare.
  const roster = [member(SELF), member(PEER1)];
  expect(huddleRecipients(roster, SELF.toUpperCase())).toEqual([PEER1]);
});
