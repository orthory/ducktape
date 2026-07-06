// Bootstrap contract: the web build dials a configured url; the desktop build
// wraps a workspace's node url (spawned Rust-side) and polls it up.

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  connectWorkspace,
  resolveNode,
  shutdownNode,
  waitUntilUp,
} from "./node-bootstrap";
import { NodeError, type NodeTransport } from "./transport";

const status = {
  version: "0.1.0",
  appHash: "aa".repeat(32),
  height: 0,
  modules: [],
};

const jsonResponse = (statusCode: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: statusCode,
    headers: { "content-type": "application/json" },
  });

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("resolveNode", () => {
  it("web build: dials the configured url, unmanaged", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, status)));

    const resolution = resolveNode();

    expect(resolution.managed).toBe(false);
    expect(resolution.url).toMatch(/^http:\/\//);
    await expect(resolution.transport.status()).resolves.toEqual(status);
  });
});

describe("connectWorkspace", () => {
  it("wraps a workspace url as a managed resolution", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, status)));

    const resolution = connectWorkspace("http://127.0.0.1:9001");

    expect(resolution.managed).toBe(true);
    expect(resolution.url).toBe("http://127.0.0.1:9001");
    await expect(resolution.transport.status()).resolves.toEqual(status);
  });
});

describe("waitUntilUp", () => {
  it("resolves once the node answers", async () => {
    const transport = {
      status: vi.fn().mockResolvedValue(status),
    } as unknown as NodeTransport;

    await expect(waitUntilUp(transport)).resolves.toBeUndefined();
  });

  it("rejects after exhausting attempts when never up", async () => {
    const transport = {
      status: vi.fn().mockRejectedValue(new Error("connection refused")),
    } as unknown as NodeTransport;

    await expect(waitUntilUp(transport, 2)).rejects.toThrow(/did not come up/);
  });

  it("fails fast when the node is up but erroring (httpError) — no 40× poll", async () => {
    const statusFn = vi.fn().mockRejectedValue(new NodeError("httpError", "node replied 500", 500));
    const transport = { status: statusFn } as unknown as NodeTransport;

    await expect(waitUntilUp(transport, 40)).rejects.toThrow(/node replied 500/);
    expect(statusFn).toHaveBeenCalledTimes(1);
  });

  it("keeps polling while the node is merely not answering yet (refused)", async () => {
    const statusFn = vi
      .fn()
      .mockRejectedValueOnce(new NodeError("refused", "could not reach the node"))
      .mockResolvedValueOnce(status);
    const transport = { status: statusFn } as unknown as NodeTransport;

    await expect(waitUntilUp(transport, 3)).resolves.toBeUndefined();
    expect(statusFn).toHaveBeenCalledTimes(2);
  });
});

describe("shutdownNode", () => {
  it("posts /v1/shutdown and surfaces failures", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }));
    vi.stubGlobal("fetch", fetchMock);

    await shutdownNode("http://127.0.0.1:8844/");
    expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8844/v1/shutdown", {
      method: "POST",
    });

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(500, {})));
    await expect(shutdownNode("http://127.0.0.1:8844")).rejects.toThrow(
      "shutdown failed: 500",
    );
  });
});
