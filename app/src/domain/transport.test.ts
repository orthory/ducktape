// The seam must be honest on both sides: remote maps the gateway's http/ws
// wire, tauri maps commands + window events, and getTransport picks by the
// webview marker.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getTransport, remoteTransport, tauriTransport } from "./transport";
import type { BlockEvent } from "./transport";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

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

// ── Remote variant ──────────────────────────────────────

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

  it("surfaces the gateway's error body as the thrown message", async () => {
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

  it("fetches GET /v1/status", async () => {
    const status = { appHash: "cd".repeat(32), height: 2, modules: [] };
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

// ── Tauri variant ───────────────────────────────────────

describe("tauriTransport", () => {
  it("routes submit/query/status through the node_* commands", async () => {
    invokeMock.mockResolvedValue({ height: 1, appHash: "aa".repeat(32) });

    const transport = tauriTransport();
    await transport.submit("chat", { CreateChannel: { channel_id: "g", name: "G" } });
    await transport.query("tasks", "List");
    await transport.status();

    expect(invokeMock).toHaveBeenNthCalledWith(1, "node_submit", {
      target: "chat",
      payload: { CreateChannel: { channel_id: "g", name: "G" } },
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "node_query", {
      target: "tasks",
      query: "List",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "node_status");
  });

  it("subscribes to ducktape://block window events", async () => {
    const stop = vi.fn();
    listenMock.mockResolvedValue(stop);

    const transport = tauriTransport();
    const seen: BlockEvent[] = [];
    const unsubscribe = transport.onBlock((block) => seen.push(block));

    // let the listen promise resolve, then feed one event through
    await Promise.resolve();
    const [eventName, handler] = listenMock.mock.calls[0];
    expect(eventName).toBe("ducktape://block");
    handler({ payload: { height: 2, appHash: "bb".repeat(32) } });
    expect(seen).toEqual([{ height: 2, appHash: "bb".repeat(32) }]);

    unsubscribe();
    expect(stop).toHaveBeenCalled();
  });
});

// ── Variant selection ───────────────────────────────────

describe("getTransport", () => {
  it("picks the tauri variant inside a tauri webview", async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    invokeMock.mockResolvedValue({ appHash: "", height: 0, modules: [] });

    await getTransport().status();
    expect(invokeMock).toHaveBeenCalledWith("node_status");

    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it("picks the remote variant in a plain browser", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      jsonResponse(200, { appHash: "", height: 0, modules: [] }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await getTransport().status();
    expect(String(fetchMock.mock.calls[0][0])).toContain("/v1/status");
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
