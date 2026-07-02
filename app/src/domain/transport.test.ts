// The transport must map the daemon's http/ws wire honestly: request shapes,
// error bodies, and the shared block stream.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { remoteTransport } from "./transport";
import type { BlockEvent } from "./transport";

// ── Fake websocket (records instances, scriptable) ──────

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  url: string;
  closed = false;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  close() {
    this.closed = true;
    this.onclose?.();
  }
}

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("remoteTransport", () => {
  it("submits over POST /v1/submit and returns the block", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      jsonResponse(200, { height: 4, appHash: "ab".repeat(32) }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844/");
    const block = await transport.submit("chat", {
      CreateChannel: { channel_id: "general", name: "General" },
    });

    expect(block).toEqual({ height: 4, appHash: "ab".repeat(32) });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://node.example:8844/v1/submit");
    expect(JSON.parse(String(init.body))).toEqual({
      target: "chat",
      payload: { CreateChannel: { channel_id: "general", name: "General" } },
    });
  });

  it("surfaces the daemon's error body as the thrown message", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(400, { error: "Module(channel already exists: general)" }),
      ),
    );

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.submit("chat", {})).rejects.toThrow(
      "Module(channel already exists: general)",
    );
  });

  it("queries over POST /v1/query and returns the raw reply", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      jsonResponse(200, { Tasks: [] }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.query("tasks", "List")).resolves.toEqual({
      Tasks: [],
    });
  });

  it("fetches GET /v1/status including the daemon version", async () => {
    const status = {
      version: "0.1.0",
      appHash: "cd".repeat(32),
      height: 2,
      modules: [],
    };
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, status));
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.status()).resolves.toEqual(status);
    expect(fetchMock.mock.calls[0][0]).toBe("http://node.example:8844/v1/status");
  });

  it("streams block frames to subscribers over one shared ws", () => {
    const transport = remoteTransport("http://node.example:8844");
    const seen: BlockEvent[] = [];
    const unsubscribe = transport.onBlock((block) => seen.push(block));

    expect(FakeWebSocket.instances).toHaveLength(1);
    const ws = FakeWebSocket.instances[0];
    expect(ws.url).toBe("ws://node.example:8844/v1/ws");

    ws.onmessage?.({
      data: JSON.stringify({ type: "block", height: 9, appHash: "ee".repeat(32) }),
    });
    expect(seen).toEqual([{ height: 9, appHash: "ee".repeat(32) }]);

    unsubscribe();
    expect(ws.closed).toBe(true);
  });
});
