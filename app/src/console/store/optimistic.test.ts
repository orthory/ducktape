import { describe, expect, it } from "vitest";

import type { PageBlock } from "../../domain/pages-client";
import * as optimistic from "./optimistic";
import { createInitialState } from "./state";
import type { ConsoleState } from "./state";

const base = (patch: Partial<ConsoleState> = {}): ConsoleState => ({
  ...createInitialState(),
  ...patch,
});

describe("postedMessage", () => {
  it("appends a preconfirmed view with the next likely seq", () => {
    const prev = base({ activeChannel: "general", messages: [] });
    const out = optimistic.postedMessage(prev, {
      channelId: "general",
      messageId: "m-1",
      blocks: [{ paragraph: [{ text: "hi", marks: [] }] }],
      author: "jess",
      at: 123,
      thread: null,
    });
    expect(out.messages).toHaveLength(1);
    const view = out.messages![0];
    expect(view.seq).toBe(1);
    expect(view.head.message_id).toBe("m-1");
    expect(view.head.author).toEqual({
      user: Array.from(new TextEncoder().encode("jess")),
    });
  });

  it("is a no-op when the channel is no longer active", () => {
    const prev = base({ activeChannel: "other" });
    expect(
      optimistic.postedMessage(prev, {
        channelId: "general",
        messageId: "m-1",
        blocks: [],
        author: "jess",
        at: 0,
        thread: null,
      }),
    ).toEqual({});
  });

  it("threads a reply into the open panel and bumps the root's counts", () => {
    const root = optimistic.postedMessage(
      base({ activeChannel: "general" }),
      { channelId: "general", messageId: "r", blocks: [], author: "a", at: 0, thread: null },
    ).messages![0];
    const prev = base({
      activeChannel: "general",
      messages: [root],
      activeThread: { root, replies: [] },
    });
    const out = optimistic.postedMessage(prev, {
      channelId: "general",
      messageId: "m-2",
      blocks: [],
      author: "jess",
      at: 5,
      thread: root.seq,
    });
    expect(out.activeThread!.replies).toHaveLength(1);
    expect(out.activeThread!.root.head.reply_count).toBe(1);
  });
});

describe("page block projections", () => {
  // root ─ a ─ a1, then b: preorder [root, a, a1, b]
  const block = (
    id: string,
    parent: string | null,
    children: string[] = [],
  ): PageBlock => ({
    id,
    parent,
    page: "root",
    kind: id === "root" ? "page" : "paragraph",
    text: id,
    checked: false,
    children,
  });
  const tree = [
    block("root", null, ["a", "b"]),
    block("a", "root", ["a1"]),
    block("a1", "a"),
    block("b", "root"),
  ];

  it("inserts a sibling AFTER the anchor's whole subtree in preorder", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockInserted(prev, {
      parent: "root",
      after: "a",
      block: block("c", "root"),
    });
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual([
      "root",
      "a",
      "a1",
      "c",
      "b",
    ]);
    expect(out.activePageBlocks![0].children).toEqual(["a", "c", "b"]);
  });

  it("inserts a first child right after the parent row", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockInserted(prev, {
      parent: "a",
      after: null,
      block: block("a0", "a"),
    });
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual([
      "root",
      "a",
      "a0",
      "a1",
      "b",
    ]);
  });

  it("removes a block with its whole subtree and unlinks the parent", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockRemoved(prev, "a");
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual(["root", "b"]);
    expect(out.activePageBlocks![0].children).toEqual(["b"]);
  });

  it("renaming the page ROOT renames the rail entry too", () => {
    const prev = base({
      activePage: "root",
      activePageBlocks: tree,
      pages: [{ id: "root", title: "root" }],
    });
    const out = optimistic.pageBlockPatched(prev, "root", { text: "Launch Plan" });
    expect(out.pages).toEqual([{ id: "root", title: "Launch Plan" }]);
  });
});

describe("doc block projections", () => {
  const doc = [
    { id: "x", kind: "paragraph" as const, text: "x" },
    { id: "y", kind: "paragraph" as const, text: "y" },
  ];

  it("inserts at the front on a null anchor (the module's `after` rule)", () => {
    const prev = base({ activeDocBlocks: doc });
    const out = optimistic.docBlockInserted(prev, {
      after: null,
      block: { id: "n", kind: "paragraph", text: "n" },
    });
    expect(out.activeDocBlocks!.map((b) => b.id)).toEqual(["n", "x", "y"]);
  });

  it("moves a block immediately after its anchor", () => {
    const prev = base({ activeDocBlocks: doc });
    const out = optimistic.docBlockMoved(prev, { blockId: "x", after: "y" });
    expect(out.activeDocBlocks!.map((b) => b.id)).toEqual(["y", "x"]);
  });
});

describe("inbox projections", () => {
  const item = (seq: number, read = false) => ({
    seq,
    member: "jess",
    kind: "note",
    body: "",
    source: "system",
    created_at: 0,
    read,
  });

  it("marks read up to a seq and recounts unread", () => {
    const prev = base({ inbox: [item(1), item(2), item(3)], inboxUnread: 3 });
    const out = optimistic.inboxReadTo(prev, 2);
    expect(out.inbox!.map((n) => n.read)).toEqual([true, true, false]);
    expect(out.inboxUnread).toBe(1);
  });

  it("clears up to a seq, keeping later arrivals", () => {
    const prev = base({ inbox: [item(1), item(2), item(3)], inboxUnread: 3 });
    const out = optimistic.inboxCleared(prev, 2);
    expect(out.inbox!.map((n) => n.seq)).toEqual([3]);
    expect(out.inboxUnread).toBe(1);
  });
});

describe("reaction projections", () => {
  it("adds then removes the local member's reaction", () => {
    const self = Array.from(new TextEncoder().encode("jess"));
    const seeded = optimistic.postedMessage(
      base({ activeChannel: "general" }),
      { channelId: "general", messageId: "m", blocks: [], author: "a", at: 0, thread: null },
    ).messages![0];
    const prev = base({ activeChannel: "general", messages: [seeded] });

    const added = optimistic.reactionToggled(prev, "general", seeded.seq, "🦆", self, false);
    expect(added.messages![0].reactions).toEqual([
      { emoji: "🦆", reactors: [{ user: self }] },
    ]);

    const removed = optimistic.reactionToggled(
      base({ activeChannel: "general", messages: added.messages! }),
      "general",
      seeded.seq,
      "🦆",
      self,
      true,
    );
    expect(removed.messages![0].reactions).toEqual([]);
  });
});
