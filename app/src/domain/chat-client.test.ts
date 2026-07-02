// The chat client must encode the exact wire serde produces for ChatMsg /
// ChatQuery and unwrap ChatReply variants — a drift here corrupts blocks.

import { describe, expect, it, vi } from "vitest";

import { channels, createChannel, messages, replyInThread, sendMessage, thread } from "./chat-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  status: vi.fn(),
  onBlock: vi.fn(),
});

describe("chat msgs", () => {
  it("encodes CreateChannel with snake_case fields", async () => {
    const transport = stubTransport();
    await createChannel(transport, { channelId: "general", name: "General" });
    expect(transport.submit).toHaveBeenCalledWith("chat", {
      CreateChannel: { channel_id: "general", name: "General" },
    });
  });

  it("encodes SendMessage", async () => {
    const transport = stubTransport();
    await sendMessage(transport, {
      channelId: "general",
      messageId: "m1",
      author: "jess",
      body: "hello",
    });
    expect(transport.submit).toHaveBeenCalledWith("chat", {
      SendMessage: {
        channel_id: "general",
        message_id: "m1",
        author: "jess",
        body: "hello",
      },
    });
  });

  it("encodes ReplyInThread", async () => {
    const transport = stubTransport();
    await replyInThread(transport, {
      channelId: "general",
      threadId: "m1",
      messageId: "m2",
      author: "jess",
      body: "in thread",
    });
    expect(transport.submit).toHaveBeenCalledWith("chat", {
      ReplyInThread: {
        channel_id: "general",
        thread_id: "m1",
        message_id: "m2",
        author: "jess",
        body: "in thread",
      },
    });
  });
});

describe("chat queries", () => {
  it("decodes Channels", async () => {
    const wire = [{ id: "general", name: "General", created_at: 1 }];
    const transport = stubTransport({ Channels: wire });
    await expect(channels(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", "Channels");
  });

  it("decodes Messages for a channel", async () => {
    const wire = [
      {
        id: "m1",
        channel_id: "general",
        author: "jess",
        body: "hello",
        sequence: 1,
        sent_at: 10,
        thread_id: null,
        reply_count: 0,
        last_reply_at: null,
      },
    ];
    const transport = stubTransport({ Messages: wire });
    await expect(messages(transport, "general")).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("chat", {
      Messages: { channel_id: "general" },
    });
  });

  it("decodes a Thread and passes null through", async () => {
    const transport = stubTransport({ Thread: null });
    await expect(
      thread(transport, { channelId: "general", threadId: "m1" }),
    ).resolves.toBeNull();
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ Channels: [] });
    await expect(messages(transport, "general")).rejects.toThrow(
      "unexpected module reply: wanted Messages",
    );
  });
});
