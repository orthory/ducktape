import { describe, expect, it } from "vitest";

import type { MessageView } from "../../domain/chat-client";
import {
  OP_STALE_MS,
  beginOp,
  failOp,
  finalizeOp,
  hasFreshPending,
  opForMessage,
  opKey,
  receiptOf,
} from "./finalization";
import type { OpLedger } from "./finalization";

describe("the op ledger", () => {
  it("begins pending, then finalizes with the receipt's inclusion facts", () => {
    let ops: OpLedger = {};
    ops = beginOp(ops, opKey.task("t1"), 1_000);
    expect(ops["task/t1"]).toMatchObject({ phase: "pending", startedAt: 1_000 });

    ops = finalizeOp(ops, opKey.task("t1"), { height: 42, opHash: "ab".repeat(32) });
    expect(ops["task/t1"]).toMatchObject({
      phase: "finalized",
      height: 42,
      opHash: "ab".repeat(32),
    });
  });

  it("finalizes without inclusion facts when the write resolved unshaped", () => {
    let ops = beginOp({}, opKey.task("t1"), 0);
    ops = finalizeOp(ops, opKey.task("t1"), receiptOf("not a receipt"));
    expect(ops["task/t1"].phase).toBe("finalized");
    expect(ops["task/t1"].height).toBeUndefined();
  });

  it("records the rejection on failure", () => {
    let ops = beginOp({}, opKey.task("t1"), 0);
    ops = failOp(ops, opKey.task("t1"), "chat: empty author");
    expect(ops["task/t1"]).toMatchObject({
      phase: "failed",
      error: "chat: empty author",
    });
  });

  it("a re-submit on the same entity key supersedes the settled record", () => {
    let ops = beginOp({}, opKey.task("t1"), 0);
    ops = finalizeOp(ops, opKey.task("t1"), { height: 7 });
    ops = beginOp(ops, opKey.task("t1"), 5_000);
    expect(ops["task/t1"]).toMatchObject({ phase: "pending", startedAt: 5_000 });
  });

  it("prunes oldest settled records past the cap, never pendings", () => {
    let ops: OpLedger = {};
    for (let i = 0; i < 512; i += 1) {
      ops = beginOp(ops, `task/settled-${i}`, i);
      ops = finalizeOp(ops, `task/settled-${i}`, { height: i });
    }
    ops = beginOp(ops, "task/in-flight", 999);
    expect(Object.keys(ops)).toHaveLength(512);
    expect(ops["task/settled-0"]).toBeUndefined();
    expect(ops["task/in-flight"].phase).toBe("pending");
  });

  it("gates refreshes only while a pending is fresh", () => {
    const ops = beginOp({}, opKey.task("t1"), 1_000);
    expect(hasFreshPending(ops, 1_000 + OP_STALE_MS - 1)).toBe(true);
    expect(hasFreshPending(ops, 1_000 + OP_STALE_MS)).toBe(false);
    expect(hasFreshPending(finalizeOp(ops, opKey.task("t1"), { height: 1 }), 1_001)).toBe(
      false,
    );
  });
});

describe("receiptOf", () => {
  it("accepts the submit receipt shape, opHash optional", () => {
    expect(receiptOf({ height: 3, appHash: "aa", opHash: "bb" })).toEqual({
      height: 3,
      opHash: "bb",
    });
    expect(receiptOf({ height: 3, appHash: "aa" })).toEqual({
      height: 3,
      opHash: undefined,
    });
  });

  it("rejects anything unshaped", () => {
    expect(receiptOf(undefined)).toBeNull();
    expect(receiptOf("ok")).toBeNull();
    expect(receiptOf({ opHash: "bb" })).toBeNull();
  });
});

describe("opForMessage", () => {
  const message = (channelId: string, seq: number, messageId: string): MessageView => ({
    channel_id: channelId,
    seq,
    head: {
      message_id: messageId,
      author: "system",
      blocks: [],
      created_at: 0,
      rev: 0,
      edited_at: null,
      base_rev: null,
      deleted: false,
      thread: null,
      reply_count: 0,
      last_reply_seq: null,
    },
    reactions: [],
    channel_head_seq: seq,
  });

  it("matches a new post by its minted message id", () => {
    const ops = beginOp({}, opKey.message("general", "m-uuid"), 0);
    expect(opForMessage(ops, message("general", 9, "m-uuid"))).toBeDefined();
    expect(opForMessage(ops, message("general", 9, "other"))).toBeUndefined();
  });

  it("prefers the newer record when both id and seq keys exist", () => {
    let ops = beginOp({}, opKey.message("general", "m-uuid"), 0);
    ops = finalizeOp(ops, opKey.message("general", "m-uuid"), { height: 1 });
    ops = beginOp(ops, opKey.messageSeq("general", 9), 10);
    expect(opForMessage(ops, message("general", 9, "m-uuid"))?.phase).toBe("pending");
  });
});
