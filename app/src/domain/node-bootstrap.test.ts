// Bootstrap contract: web builds only dial; desktop builds adopt a live
// daemon or spawn one detached and wait for it to answer.

import { afterEach, describe, expect, it, vi } from "vitest";

import { ensureDaemon, resolveNode, shutdownNode } from "./node-bootstrap";
import type { NodeTransport } from "./transport";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

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
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("resolveNode", () => {
  it("web build: dials the configured url, unmanaged, never spawns", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, status)));

    const resolution = await resolveNode();

    expect(resolution.managed).toBe(false);
    expect(resolution.url).toMatch(/^http:\/\//);
    expect(invokeMock).not.toHaveBeenCalled();
    await expect(resolution.transport.status()).resolves.toEqual(status);
  });

  it("desktop build: adopts a daemon that already answers", async () => {
    markTauri();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, status)));

    const resolution = await resolveNode();

    expect(resolution.managed).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("desktop build: spawns when nothing answers, then polls until up", async () => {
    markTauri();
    // dead until the spawn, then alive
    let up = false;
    invokeMock.mockImplementation(() => {
      up = true;
      return Promise.resolve("log-path");
    });
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(() =>
        up
          ? Promise.resolve(jsonResponse(200, status))
          : Promise.reject(new Error("connection refused")),
      ),
    );

    const resolution = await resolveNode();

    expect(invokeMock).toHaveBeenCalledWith("daemon_spawn", {
      listen: "127.0.0.1:8844",
    });
    expect(resolution.managed).toBe(true);
  });
});

describe("ensureDaemon", () => {
  it("does not spawn when the transport answers", async () => {
    const transport = {
      status: vi.fn().mockResolvedValue(status),
    } as unknown as NodeTransport;

    await ensureDaemon(transport);
    expect(invokeMock).not.toHaveBeenCalled();
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
