// The chat client must encode the exact wire serde produces for the
// block-based ChatMsg / ChatQuery, thread the submit origin through, and
// decode ChatReply variants — a drift here corrupts blocks.

import { describe, expect, it, vi } from "vitest";

import {
  authorName,
  blocksText,
  channels,
  createChannel,
  latestMessages,
  postMessage,
  thread,
} from "./chat-client";
import type { MessageView } from "./chat-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  putBlob: vi.fn(),
  status: vi.fn(),
  onBlock: vi.fn(),
});

const wireMessage = (over: Partial<MessageView["head"]> = {}): MessageView => ({
  channel_id: "general",
  seq: 1,
  head: {
    message_id: "m1",
    author: { User: Array.from(new TextEncoder().encode("jess")) },
    blocks: [{ Paragraph: [{ text: "hello", marks: [] }] }],
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
      postPolicy: "MembersOnly",
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      {
        CreateChannel: {
          channel_id: "general",
          name: "General",
          post_policy: "MembersOnly",
        },
      },
      "jess",
    );
  });

  it("encodes PostMessage with the given blocks and no author field", async () => {
    const transport = stubTransport();
    await postMessage(transport, {
      channelId: "general",
      messageId: "m1",
      blocks: [{ Paragraph: [{ text: "hello", marks: [] }] }],
      origin: "jess",
    });
    expect(transport.submit).toHaveBeenCalledWith(
      "chat",
      {
        PostMessage: {
          channel_id: "general",
          message_id: "m1",
          blocks: [{ Paragraph: [{ text: "hello", marks: [] }] }],
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
      blocks: [{ Paragraph: [{ text: "in thread", marks: [] }] }],
      origin: "jess",
      thread: 7,
    });
    const [, payload] = vi.mocked(transport.submit).mock.calls[0];
    expect((payload as { PostMessage: { thread: number } }).PostMessage.thread).toBe(7);
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
        post_policy: "Open",
        hooks: [],
        pinned: [],
      },
    ];
    const transport = stubTransport({ Channels: wire });
    await expect(channels(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", "Channels");
  });

  it("queries MessagesLatest and decodes Messages", async () => {
    const wire = [wireMessage()];
    const transport = stubTransport({ Messages: wire });
    await expect(latestMessages(transport, "general", 50)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", {
      MessagesLatest: { channel_id: "general", limit: 50 },
    });
  });

  it("queries a Thread by root seq and passes null through", async () => {
    const transport = stubTransport({ Thread: null });
    await expect(
      thread(transport, { channelId: "general", rootSeq: 7 }),
    ).resolves.toBeNull();
    expect(transport.query).toHaveBeenCalledWith("chat", {
      Thread: { channel_id: "general", root_seq: 7, from: 0, limit: 256 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ Channels: [] });
    await expect(latestMessages(transport, "general")).rejects.toThrow(
      "unexpected module reply: wanted Messages",
    );
  });
});

describe("rendering helpers", () => {
  it("decodes every AuthorRef variant to a display name", () => {
    expect(authorName({ User: Array.from(new TextEncoder().encode("jess")) })).toBe("jess");
    expect(authorName({ Agent: { module: "chat", agent_id: "duck" } })).toBe("chat/duck");
    expect(authorName({ Module: "forge" })).toBe("forge");
    expect(authorName("System")).toBe("system");
  });

  it("flattens block bodies to text", () => {
    expect(
      blocksText([
        { Paragraph: [{ text: "one ", marks: [] }, { text: "two", marks: ["Bold"] }] },
        "Divider",
        { Code: { lang: null, text: "let x = 1;" } },
        { Quote: [{ text: "said", marks: [] }] },
      ]),
    ).toBe("one two\n———\nlet x = 1;\n> said");
  });
});
