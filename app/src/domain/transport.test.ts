// The transport must map the daemon's http/ws wire honestly: request shapes,
// error bodies, and the shared block stream.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { remoteTransport } from "./transport";
import type { BlockRecord } from "./transport";

// ── Fake websocket (records instances, scriptable) ──────

class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static instances: FakeWebSocket[] = [];
  url: string;
  readyState = FakeWebSocket.CONNECTING;
  closed = false;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  open() {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.();
  }

  send(data: string) {
    this.sent.push(data);
  }

  close() {
    if (this.closed) return;
    this.readyState = FakeWebSocket.CLOSED;
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

  it("sends one union subscribe frame on open", () => {
    const transport = remoteTransport("http://node.example:8844");
    const unsubscribe = transport.subscribe(["module:chat", "logs"], {});

    expect(FakeWebSocket.instances).toHaveLength(1);
    const ws = FakeWebSocket.instances[0];
    expect(ws.url).toBe("ws://node.example:8844/v1/ws");
    ws.open();

    expect(ws.sent.map((msg) => JSON.parse(msg))).toEqual([
      { op: "subscribe", topics: ["module:chat", "logs"], resume: {} },
    ]);

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

  it("routes event and tail frames by topic", () => {
    const transport = remoteTransport("http://node.example:8844");
    const chatEvents: unknown[] = [];
    const logTail: unknown[] = [];
    const offChat = transport.subscribe(["module:chat"], {
      onEvent: (frame) => chatEvents.push(frame),
    });
    const offLogs = transport.subscribe(["logs"], {
      onTail: (frame) => logTail.push(frame),
    });

    expect(FakeWebSocket.instances).toHaveLength(1);
    const ws = FakeWebSocket.instances[0];
    ws.open();

    ws.onmessage?.({
      data: JSON.stringify({
        type: "event",
        topic: "module:chat",
        cursor: "op/0000000000000001/0000",
        op: {
          height: 1,
          seq: 0,
          time: 1,
          origin: { kind: "external", id: "eddy" },
          payload: { post_message: {} },
        },
      }),
    });
    ws.onmessage?.({
      data: JSON.stringify({
        type: "tail",
        topic: "logs",
        cursor: "1",
        item: { line: "hello" },
      }),
    });

    expect(chatEvents).toHaveLength(1);
    expect(logTail).toHaveLength(1);

    offChat();
    offLogs();
  });

  it("refcounts topic subscriptions at the wire edge", () => {
    const transport = remoteTransport("http://node.example:8844");
    const h1 = {};
    const h2 = {};
    const off1 = transport.subscribe(["module:chat"], h1);
    const off2 = transport.subscribe(["module:chat"], h2);
    const ws = FakeWebSocket.instances[0];
    ws.open();

    expect(ws.sent.map((msg) => JSON.parse(msg))).toEqual([
      { op: "subscribe", topics: ["module:chat"], resume: {} },
    ]);

    off1();
    expect(ws.sent).toHaveLength(1);
    off2();
    expect(ws.sent.map((msg) => JSON.parse(msg))).toEqual([
      { op: "subscribe", topics: ["module:chat"], resume: {} },
      { op: "unsubscribe", topics: ["module:chat"] },
    ]);
    expect(ws.closed).toBe(true);
  });

  it("reconnects with the last cursor as resume", async () => {
    vi.useFakeTimers();
    try {
      const transport = remoteTransport("http://node.example:8844");
      const off = transport.subscribe(["module:chat"], { onEvent: vi.fn() });
      const ws = FakeWebSocket.instances[0];
      ws.open();
      ws.onmessage?.({
        data: JSON.stringify({
          type: "event",
          topic: "module:chat",
          cursor: "op/0000000000000002/0000",
          op: {
            height: 2,
            seq: 0,
            time: 2,
            origin: { kind: "external", id: "eddy" },
          },
        }),
      });

      ws.close();
      await vi.advanceTimersByTimeAsync(1_000);
      expect(FakeWebSocket.instances).toHaveLength(2);
      const next = FakeWebSocket.instances[1];
      next.open();
      expect(next.sent.map((msg) => JSON.parse(msg))).toEqual([
        {
          op: "subscribe",
          topics: ["module:chat"],
          resume: { "module:chat": "op/0000000000000002/0000" },
        },
      ]);
      off();
    } finally {
      vi.useRealTimers();
    }
  });

  it("adopts lagged cursors and notifies handlers", () => {
    const transport = remoteTransport("http://node.example:8844");
    const lagged: Array<[string, string]> = [];
    const off = transport.subscribe(["logs"], {
      onLagged: (topic, cursor) => lagged.push([topic, cursor]),
    });
    const ws = FakeWebSocket.instances[0];
    ws.open();
    ws.onmessage?.({
      data: JSON.stringify({ type: "lagged", topic: "logs", cursor: "9" }),
    });
    off();
    expect(lagged).toEqual([["logs", "9"]]);
  });

  it("closes and emits down when the heartbeat watchdog expires", async () => {
    vi.useFakeTimers();
    try {
      const transport = remoteTransport("http://node.example:8844");
      const signals: string[] = [];
      const off = transport.onStream((signal) => signals.push(signal.kind));
      const ws = FakeWebSocket.instances[0];
      ws.open();
      expect(signals).toEqual(["up"]);

      await vi.advanceTimersByTimeAsync(7_500);
      expect(ws.closed).toBe(true);
      expect(signals).toEqual(["up", "down"]);
      off();
    } finally {
      vi.useRealTimers();
    }
  });

  it("ignores malformed and unknown stream frames", () => {
    const transport = remoteTransport("http://node.example:8844");
    const onEvent = vi.fn();
    const off = transport.subscribe(["module:chat"], { onEvent });
    const ws = FakeWebSocket.instances[0];
    ws.open();

    ws.onmessage?.({ data: "not json" });
    ws.onmessage?.({ data: JSON.stringify({ type: "mystery", height: 4 }) });

    expect(onEvent).not.toHaveBeenCalled();
    off();
  });
});
