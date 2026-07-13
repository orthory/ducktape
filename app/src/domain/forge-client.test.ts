// The forge client must encode the exact wire serde produces for the tracker's
// ForgeMsg / ForgeQuery (snake_case variants + fields), thread the submit
// origin through, and decode ForgeReply variants — a drift here corrupts blocks.

import { describe, expect, it, vi } from "vitest";

import {
  editItem,
  forgeItemTarget,
  getItem,
  listItems,
  listRefs,
  mergePr,
  openIssue,
  openPr,
  setItemState,
  submitReview,
  uploadMergePack,
} from "./forge-client";
import type { ForgeItemDetail, ForgeItemSummary } from "./forge-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

const OID_A = "a".repeat(40);
const OID_B = "b".repeat(40);
const OID_C = "c".repeat(40);
const PACK = "d".repeat(64);

describe("forgeItemTarget", () => {
  it("turns a hidden discussion channel into a public anchored item route", () => {
    expect(
      forgeItemTarget("forge:team:ducktape:58", { messageId: "m-4", messageSeq: 4 }),
    ).toEqual({ repo: "team:ducktape", number: 58, messageId: "m-4", messageSeq: 4 });
    expect(forgeItemTarget("forge:ducktape:58")).not.toHaveProperty("channelId");
  });

  it("rejects malformed channels and drops malformed anchors", () => {
    expect(forgeItemTarget("general", { messageSeq: 4 })).toBeNull();
    expect(forgeItemTarget("forge:ducktape:0")).toBeNull();
    expect(forgeItemTarget("forge:ducktape:58", { messageId: " ", messageSeq: -1 })).toEqual({
      repo: "ducktape",
      number: 58,
    });
  });
});

const summary: ForgeItemSummary = {
  number: 1,
  kind: "issue",
  title: "flaky test",
  state: "open",
  author: { user: Array.from(new TextEncoder().encode("jess")) },
  created_at: 10,
  updated_at: 10,
};

describe("forge item msgs", () => {
  it("encodes OpenIssue and stamps the origin", async () => {
    const transport = stubTransport();
    await openIssue(transport, {
      repo: "ducktape",
      title: "flaky test",
      body: "the capability-host test races",
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      {
        open_issue: {
          repo: "ducktape",
          title: "flaky test",
          body: "the capability-host test races",
        },
      },
      "jess",
    );
  });

  it("encodes OpenPr with snake_case branches ('' targets main)", async () => {
    const transport = stubTransport();
    await openPr(transport, {
      repo: "ducktape",
      title: "fix race",
      body: "serializes the host boot",
      sourceBranch: "fix/race",
      targetBranch: "",
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      {
        open_pr: {
          repo: "ducktape",
          title: "fix race",
          body: "serializes the host boot",
          source_branch: "fix/race",
          target_branch: "",
        },
      },
      "jess",
    );
  });

  it("encodes EditItem passing nulls through for untouched fields", async () => {
    const transport = stubTransport();
    await editItem(transport, {
      repo: "ducktape",
      number: 4,
      title: "retitled",
      body: null,
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      { edit_item: { repo: "ducktape", number: 4, title: "retitled", body: null } },
      "jess",
    );
  });

  it("encodes SetItemState with the open flag", async () => {
    const transport = stubTransport();
    await setItemState(transport, { repo: "ducktape", number: 4, open: false, origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      { set_item_state: { repo: "ducktape", number: 4, open: false } },
      "jess",
    );
  });

  it("encodes MergePr with snake_case oids and the pack digest", async () => {
    const transport = stubTransport();
    await mergePr(transport, {
      repo: "ducktape",
      number: 7,
      prevTargetOid: OID_A,
      expectedSourceOid: OID_B,
      mergeOid: OID_C,
      packDigest: PACK,
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      {
        merge_pr: {
          repo: "ducktape",
          number: 7,
          prev_target_oid: OID_A,
          expected_source_oid: OID_B,
          merge_oid: OID_C,
          pack_digest: PACK,
        },
      },
      "jess",
    );
  });

  it("encodes SubmitReview with inline comments verbatim", async () => {
    const transport = stubTransport();
    await submitReview(transport, {
      repo: "ducktape",
      number: 7,
      verdict: "request_changes",
      body: "the guard is inverted",
      commitOid: OID_B,
      comments: [{ path: "src/lib.rs", line: 12, side: "new", body: "flip this" }],
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "forge",
      {
        submit_review: {
          repo: "ducktape",
          number: 7,
          verdict: "request_changes",
          body: "the guard is inverted",
          commit_oid: OID_B,
          comments: [{ path: "src/lib.rs", line: 12, side: "new", body: "flip this" }],
        },
      },
      "jess",
    );
  });
});

describe("uploadMergePack", () => {
  it("stages the bytes via putBlob and lowercases the digest", async () => {
    const transport = stubTransport();
    vi.mocked(transport.putBlob).mockResolvedValue("D".repeat(64));
    const bytes = new Uint8Array([1, 2, 3]);
    await expect(uploadMergePack(transport, bytes)).resolves.toBe("d".repeat(64));
    const staged = vi.mocked(transport.putBlob).mock.calls[0][0];
    // a fresh plain-ArrayBuffer-backed copy rides the wire (putBlob's contract)
    expect(Array.from(staged)).toEqual([1, 2, 3]);
    expect(staged).not.toBe(bytes);
  });
});

describe("forge queries", () => {
  it("queries ListRefs and decodes refs", async () => {
    const refs = [{ name: "main", head: OID_A }, { name: "fix/race", head: OID_B }];
    const transport = stubTransport({ refs });
    await expect(listRefs(transport, "ducktape")).resolves.toEqual(refs);
    expect(transport.query).toHaveBeenCalledWith("forge", { list_refs: { repo: "ducktape" } });
  });

  it("queries ListItems and decodes items", async () => {
    const transport = stubTransport({ items: [summary] });
    await expect(listItems(transport, "ducktape")).resolves.toEqual([summary]);
    expect(transport.query).toHaveBeenCalledWith("forge", { list_items: { repo: "ducktape" } });
  });

  it("queries GetItem, decoding the flattened detail", async () => {
    const detail: ForgeItemDetail = {
      ...summary,
      kind: "pr",
      body: "serializes the host boot",
      channel_id: "forge:ducktape:1",
      source_branch: "fix/race",
      target_branch: "main",
      merge_oid: null,
      reviews: [
        {
          author: { user: Array.from(new TextEncoder().encode("sam")) },
          verdict: "approve",
          body: "lgtm",
          commit_oid: OID_B,
          comments: [],
          created_at: 11,
        },
      ],
    };
    const transport = stubTransport({ item: detail });
    await expect(getItem(transport, { repo: "ducktape", number: 1 })).resolves.toEqual(detail);
    expect(transport.query).toHaveBeenCalledWith("forge", {
      get_item: { repo: "ducktape", number: 1 },
    });
  });

  it("passes an absent item through as null", async () => {
    const transport = stubTransport({ item: null });
    await expect(getItem(transport, { repo: "ducktape", number: 99 })).resolves.toBeNull();
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ refs: [] });
    await expect(listItems(transport, "ducktape")).rejects.toThrow(
      "unexpected module reply: wanted items",
    );
  });
});
