import { describe, expect, it } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { Channel } from "../../domain/chat-client";
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
      authorBytes: Array.from(new TextEncoder().encode("jess")),
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
        authorBytes: Array.from(new TextEncoder().encode("jess")),
        at: 0,
        thread: null,
      }),
    ).toEqual({});
  });

  it("threads a reply into the open panel and bumps the root's counts", () => {
    const root = optimistic.postedMessage(
      base({ activeChannel: "general" }),
      { channelId: "general", messageId: "r", blocks: [], authorBytes: Array.from(new TextEncoder().encode("a")), at: 0, thread: null },
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
      authorBytes: Array.from(new TextEncoder().encode("jess")),
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

  it("moves a block under its previous sibling — the Tab indent", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockMoved(prev, {
      blockId: "b",
      parent: "a",
      after: "a1",
    });
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual(["root", "a", "a1", "b"]);
    expect(out.activePageBlocks![0].children).toEqual(["a"]);
    expect(out.activePageBlocks![1].children).toEqual(["a1", "b"]);
    expect(out.activePageBlocks![3].parent).toBe("a");
  });

  it("moves a block up to its grandparent — the Shift+Tab outdent", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockMoved(prev, {
      blockId: "a1",
      parent: "root",
      after: "a",
    });
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual(["root", "a", "a1", "b"]);
    expect(out.activePageBlocks![0].children).toEqual(["a", "a1", "b"]);
    expect(out.activePageBlocks![1].children).toEqual([]);
    expect(out.activePageBlocks![2].parent).toBe("root");
  });

  it("carries the whole subtree on a same-parent reorder", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    const out = optimistic.pageBlockMoved(prev, {
      blockId: "a",
      parent: "root",
      after: "b",
    });
    expect(out.activePageBlocks!.map((b) => b.id)).toEqual(["root", "b", "a", "a1"]);
    expect(out.activePageBlocks![0].children).toEqual(["b", "a"]);
  });

  it("refuses to render a move into the block's own subtree", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    expect(
      optimistic.pageBlockMoved(prev, { blockId: "a", parent: "a1", after: null }),
    ).toEqual({});
  });

  it("defers a torn move snapshot (missing parent or anchor) to the refresh", () => {
    const prev = base({ activePage: "root", activePageBlocks: tree });
    expect(
      optimistic.pageBlockMoved(prev, { blockId: "b", parent: "ghost", after: null }),
    ).toEqual({});
    expect(
      optimistic.pageBlockMoved(prev, { blockId: "b", parent: "a", after: "ghost" }),
    ).toEqual({});
    expect(
      optimistic.pageBlockMoved(prev, { blockId: "ghost", parent: "a", after: null }),
    ).toEqual({});
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
      pages: [{ id: "root", title: "root", parent: null }],
    });
    const out = optimistic.pageBlockPatched(prev, "root", { text: "Launch Plan" });
    expect(out.pages).toEqual([{ id: "root", title: "Launch Plan", parent: null }]);
  });
});

describe("huddle projections", () => {
  const channel = (huddle?: Channel["huddle"]): Channel => ({
    id: "general",
    name: "General",
    created_at: 0,
    head_seq: 0,
    post_policy: "open",
    hooks: [],
    pinned: [],
    huddle,
  });
  const selfNode = [1, 2, 3, 4];

  it("adds our node to the roster on join, and is idempotent", () => {
    const prev = base({ channels: [channel()] });
    const out = optimistic.huddleJoined(prev, {
      channelId: "general",
      node: selfNode,
      authorBytes: Array.from(new TextEncoder().encode("jess")),
      at: 42,
    });
    expect(out.channels![0].huddle).toEqual([
      { user: Array.from(new TextEncoder().encode("jess")), node: selfNode, joined_at: 42 },
    ]);

    // re-joining with the same node key doesn't duplicate us.
    const again = optimistic.huddleJoined(base({ channels: out.channels! }), {
      channelId: "general",
      node: selfNode,
      authorBytes: Array.from(new TextEncoder().encode("jess")),
      at: 99,
    });
    expect(again).toEqual({});
  });

  it("drops our node from the roster on leave, keeping others", () => {
    const other = [9, 9, 9];
    const prev = base({
      channels: [
        channel([
          { user: [], node: selfNode, joined_at: 1 },
          { user: [], node: other, joined_at: 2 },
        ]),
      ],
    });
    const out = optimistic.huddleLeft(prev, "general", keyHex(selfNode));
    expect(out.channels![0].huddle).toEqual([{ user: [], node: other, joined_at: 2 }]);
  });

  it("prunes exactly the swept user from the roster, keeping others", () => {
    const staleUser = Array.from(new TextEncoder().encode("stale"));
    const liveUser = Array.from(new TextEncoder().encode("live"));
    const prev = base({
      channels: [
        channel([
          { user: staleUser, node: [1, 1, 1], joined_at: 1 },
          { user: liveUser, node: [2, 2, 2], joined_at: 2 },
        ]),
      ],
    });
    const out = optimistic.huddleSwept(prev, "general", keyHex(staleUser));
    expect(out.channels![0].huddle).toEqual([
      { user: liveUser, node: [2, 2, 2], joined_at: 2 },
    ]);
  });
});

describe("reaction projections", () => {
  it("adds then removes the local member's reaction", () => {
    const self = Array.from(new TextEncoder().encode("jess"));
    const seeded = optimistic.postedMessage(
      base({ activeChannel: "general" }),
      { channelId: "general", messageId: "m", blocks: [], authorBytes: Array.from(new TextEncoder().encode("a")), at: 0, thread: null },
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
