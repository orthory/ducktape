// The transport must map the daemon's http/ws wire honestly: request shapes,
// error bodies, and the shared block stream.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { remoteTransport } from "./transport";
import type { BlockEvent, BlockRecord } from "./transport";

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
      create_channel: { channel_id: "general", name: "General" },
    });

    expect(block).toEqual({ height: 4, appHash: "ab".repeat(32) });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://node.example:8844/v1/submit");
    expect(JSON.parse(String(init.body))).toEqual({
      target: "chat",
      payload: { create_channel: { channel_id: "general", name: "General" } },
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
      jsonResponse(200, { tasks: [] }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.query("tasks", "list")).resolves.toEqual({
      tasks: [],
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

  it("fetches GET /v1/blocks and returns the records, with an optional limit", async () => {
    const records: BlockRecord[] = [
      {
        height: 7,
        hash: "aa".repeat(32),
        commitHash: "bb".repeat(32),
        ops: [
          {
            proposer: "cc".repeat(32),
            disposition: "applied",
            target: "chat",
            operations: [
              { module: "chat", origin: "external", emittedMsgs: 0, emittedEvents: 0 },
            ],
            payload: '{"Post":{}}',
            opHash: "dd".repeat(32),
          },
        ],
      },
    ];
    // a Response body reads once, and this test fetches twice — hand back a
    // fresh Response per call. the url param types mock.calls for assertions.
    const fetchMock = vi.fn((_url: string) =>
      Promise.resolve(jsonResponse(200, { blocks: records })),
    );
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.blocks()).resolves.toEqual(records);
    expect(fetchMock.mock.calls[0][0]).toBe("http://node.example:8844/v1/blocks");

    await transport.blocks(50);
    expect(fetchMock.mock.calls[1][0]).toBe(
      "http://node.example:8844/v1/blocks?limit=50",
    );
  });

  it("reads a node without a blocks surface as no blocks, not an error", async () => {
    // an older node has no /v1/blocks route — a malformed (non-{blocks}) body
    // must degrade to empty, matching the surface's best-effort contract.
    const fetchMock = vi.fn(() => Promise.resolve(jsonResponse(200, {})));
    vi.stubGlobal("fetch", fetchMock);

    const transport = remoteTransport("http://node.example:8844");
    await expect(transport.blocks()).resolves.toEqual([]);
  });

  it("ignores unknown ws frame kinds — the stream may grow", () => {
    const transport = remoteTransport("http://node.example:8844");
    const blocks: BlockEvent[] = [];
    const offBlock = transport.onBlock((block) => blocks.push(block));

    expect(FakeWebSocket.instances).toHaveLength(1);
    const ws = FakeWebSocket.instances[0];

    // an unknown frame kind must be ignored, not crash the stream.
    ws.onmessage?.({ data: JSON.stringify({ type: "mystery", height: 4 }) });
    ws.onmessage?.({
      data: JSON.stringify({ type: "block", height: 5, appHash: "ff".repeat(32) }),
    });

    expect(blocks).toEqual([{ height: 5, appHash: "ff".repeat(32) }]);

    offBlock();
    expect(ws.closed).toBe(true);
  });
});
