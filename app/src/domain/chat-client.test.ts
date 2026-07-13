// The chat client must encode the exact wire serde produces for the
// block-based ChatMsg / ChatQuery, thread the submit origin through, and
// decode ChatReply variants — a drift here corrupts blocks.

import { describe, expect, it, vi } from "vitest";

import {
  authorName,
  blocksText,
  channelMembers,
  channels,
  createChannel,
  isModuleChannel,
  joinHuddle,
  keyBytes,
  keyHex,
  latestMessages,
  leaveHuddle,
  postMessage,
  renameChannel,
  searchMessages,
  setChannelArchived,
  setMembership,
  sweepHuddle,
  tagSearch,
  tags,
  thread,
} from "./chat-client";
import type { MessageView } from "./chat-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

const wireMessage = (over: Partial<MessageView["head"]> = {}): MessageView => ({
  channel_id: "general",
  seq: 1,
  head: {
    message_id: "m1",
    author: { user: Array.from(new TextEncoder().encode("jess")) },
    blocks: [{ paragraph: [{ text: "hello", marks: [] }] }],
    created_at: 10,
    rev: 0,
    edited_at: null,
    base_rev: null,
    deleted: false,
    thread: null,
    reply_count: 0,
    last_reply_seq: null,
    ...over,
  },
  reactions: [],
  channel_head_seq: 1,
});

describe("chat msgs", () => {
  it("encodes CreateChannel with post_policy and stamps the origin", async () => {
    const transport = stubTransport();
    await createChannel(transport, {
      channelId: "general",
      name: "General",
      postPolicy: "members_only",
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      {
        create_channel: {
          channel_id: "general",
          name: "General",
          post_policy: "members_only",
        },
      },
      "jess",
    );
  });

  it("encodes RenameChannel and stamps the origin", async () => {
    const transport = stubTransport();
    await renameChannel(transport, { channelId: "general", name: "General v2", origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { rename_channel: { channel_id: "general", name: "General v2" } },
      "jess",
    );
  });

  it("encodes SetChannelArchived and stamps the origin", async () => {
    const transport = stubTransport();
    await setChannelArchived(transport, { channelId: "general", archived: true, origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { set_channel_archived: { channel_id: "general", archived: true } },
      "jess",
    );
  });

  it("encodes PostMessage with the given blocks and no author field", async () => {
    const transport = stubTransport();
    await postMessage(transport, {
      channelId: "general",
      messageId: "m1",
      blocks: [{ paragraph: [{ text: "hello", marks: [] }] }],
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      {
        post_message: {
          channel_id: "general",
          message_id: "m1",
          blocks: [{ paragraph: [{ text: "hello", marks: [] }] }],
          thread: null,
          as_agent: null,
        },
      },
      "jess",
    );
  });

  it("encodes a thread reply as PostMessage with the root seq", async () => {
    const transport = stubTransport();
    await postMessage(transport, {
      channelId: "general",
      messageId: "m2",
      blocks: [{ paragraph: [{ text: "in thread", marks: [] }] }],
      origin: "jess",
      thread: 7,
    });
    const [, payload] = vi.mocked(transport.submit).mock.calls[0];
    expect((payload as { post_message: { thread: number } }).post_message.thread).toBe(7);
  });
});

describe("huddle msgs", () => {
  it("encodes JoinHuddle with the node key bytes and stamps the origin", async () => {
    const transport = stubTransport();
    const node = keyBytes("ab".repeat(32));
    await joinHuddle(transport, { channelId: "general", node, origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { join_huddle: { channel_id: "general", node } },
      "jess",
    );
  });

  it("encodes LeaveHuddle for the channel and stamps the origin", async () => {
    const transport = stubTransport();
    await leaveHuddle(transport, { channelId: "general", origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { leave_huddle: { channel_id: "general" } },
      "jess",
    );
  });

  it("encodes SweepHuddle with the target user bytes and stamps the origin", async () => {
    const transport = stubTransport();
    const user = Array.from(new TextEncoder().encode("stale"));
    await sweepHuddle(transport, { channelId: "general", user, origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { sweep_huddle: { channel_id: "general", user } },
      "jess",
    );
  });

  it("keyBytes inverts keyHex for a mesh key", () => {
    const bytes = Array.from({ length: 32 }, (_, i) => i * 7 % 256);
    expect(keyBytes(keyHex(bytes))).toEqual(bytes);
  });
});

describe("membership", () => {
  it("encodes SetMembership with the user bytes + flag and stamps the origin", async () => {
    const transport = stubTransport();
    const user = Array.from(new TextEncoder().encode("jess"));
    await setMembership(transport, { channelId: "general", user, member: true, origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      { set_membership: { channel_id: "general", user, member: true } },
      "jess",
    );
  });

  it("queries Members by channel and decodes the byte-key list", async () => {
    const wire = [Array.from(new TextEncoder().encode("jess")), keyBytes("ab".repeat(32))];
    const transport = stubTransport({ members: wire });
    await expect(channelMembers(transport, "general")).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", { members: { channel_id: "general" } });
  });
});

describe("chat queries", () => {
  it("decodes Channels", async () => {
    const wire = [
      {
        id: "general",
        name: "General",
        created_at: 1,
        head_seq: 3,
        post_policy: "open",
        hooks: [],
        pinned: [],
      },
    ];
    const transport = stubTransport({ channels: wire });
    await expect(channels(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", "channels");
  });

  it("queries MessagesLatest and decodes Messages", async () => {
    const wire = [wireMessage()];
    const transport = stubTransport({ messages: wire });
    await expect(latestMessages(transport, "general", 50)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", {
      messages_latest: { channel_id: "general", limit: 50 },
    });
  });

  it("queries a Thread by root seq and passes null through", async () => {
    const transport = stubTransport({ thread: null });
    await expect(
      thread(transport, { channelId: "general", rootSeq: 7 }),
    ).resolves.toBeNull();
    expect(transport.query).toHaveBeenCalledWith("chat", {
      thread: { channel_id: "general", root_seq: 7, from: 0, limit: 256 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ channels: [] });
    await expect(latestMessages(transport, "general")).rejects.toThrow(
      "unexpected module reply: wanted messages",
    );
  });
});

describe("isModuleChannel", () => {
  it("reserves ':' ids for modules and passes user ids through", () => {
    expect(isModuleChannel("forge:ducktape:1")).toBe(true);
    expect(isModuleChannel("forge:my-repo:42")).toBe(true);
    expect(isModuleChannel("general")).toBe(false);
    expect(isModuleChannel("release-2026")).toBe(false);
  });
});

describe("rendering helpers", () => {
  it("decodes every AuthorRef variant to a display name", () => {
    expect(authorName({ user: Array.from(new TextEncoder().encode("jess")) })).toBe("jess");
    expect(authorName({ agent: { module: "chat", agent_id: "duck" } })).toBe("chat/duck");
    expect(authorName({ module: "forge" })).toBe("forge");
    expect(authorName("system")).toBe("system");
  });

  it("flattens block bodies to text", () => {
    expect(
      blocksText([
        { paragraph: [{ text: "one ", marks: [] }, { text: "two", marks: ["bold"] }] },
        "divider",
        { code: { lang: null, text: "let x = 1;" } },
        { quote: [{ text: "said", marks: [] }] },
      ]),
    ).toBe("one two\n———\nlet x = 1;\n> said");
  });
});

describe("materialized view (search)", () => {
  it("posts the search request to chat's view endpoint and unwraps hits", async () => {
    const hit = {
      channelId: "general",
      seq: 1,
      messageId: "m1",
      author: "user:jess",
      height: 4,
      time: 1_000,
      text: "fluent index demo",
      deleted: false,
      edited: false,
    };
    const transport = stubTransport();
    (transport.view as ReturnType<typeof vi.fn>).mockResolvedValue({ hits: [hit] });

    const hits = await searchMessages(transport, { text: "fluent", channelId: "general" });
    expect(transport.view).toHaveBeenCalledWith("chat", {
      search: { text: "fluent", channelId: "general", limit: undefined },
    });
    expect(hits).toEqual([hit]);
  });

  it("posts the tag-catalog request and unwraps tag rows", async () => {
    const row = { tag: "rust", count: 3, lastSeq: 7 };
    const transport = stubTransport();
    (transport.view as ReturnType<typeof vi.fn>).mockResolvedValue({ tags: [row] });

    const rows = await tags(transport, { channelId: "general" });
    expect(transport.view).toHaveBeenCalledWith("chat", {
      tags: { channelId: "general", limit: undefined },
    });
    expect(rows).toEqual([row]);
  });

  it("posts the tagSearch request (display form passes through) and unwraps hits", async () => {
    const hit = {
      channelId: "general",
      seq: 2,
      messageId: "m2",
      author: "user:jess",
      height: 5,
      time: 1_001,
      text: "shipping #Rust today",
      deleted: false,
      edited: false,
      tags: ["rust"],
    };
    const transport = stubTransport();
    (transport.view as ReturnType<typeof vi.fn>).mockResolvedValue({ hits: [hit] });

    const hits = await tagSearch(transport, { tag: "Rust", channelId: "general", limit: 100 });
    expect(transport.view).toHaveBeenCalledWith("chat", {
      tagSearch: { tag: "Rust", channelId: "general", limit: 100 },
    });
    expect(hits).toEqual([hit]);
  });
});
