import { ed25519 } from "@noble/curves/ed25519.js";
import { afterEach, describe, expect, it, vi } from "vitest";

import { makeTransportStub } from "../test/transport-stub";
import { buildContentManifest, loadDuckPage, parseDuckAddress } from "./duck-browser";
import * as gateway from "./gateway-client";
import * as nodeBootstrap from "./node-bootstrap";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const encoder = new TextEncoder();
const b64 = (bytes: Uint8Array): string => {
  let binary = "";
  bytes.forEach((byte) => { binary += String.fromCharCode(byte); });
  return btoa(binary);
};
const fromB64 = (value: string): Uint8Array =>
  Uint8Array.from(atob(value), (char) => char.charCodeAt(0));

afterEach(() => vi.restoreAllMocks());

describe(".duck address model", () => {
  it("keeps net.duck reserved while apex and named account routes use one shape", () => {
    expect(parseDuckAddress("duck://net.duck/docs.html")).toMatchObject({
      kind: "network",
      hostname: "net.duck",
      pathAndQuery: "/docs.html",
      name: { label: null },
    });
    expect(parseDuckAddress("Alice.duck")).toMatchObject({
      kind: "account",
      hostname: "alice.duck",
      name: { label: null },
    });
    expect(parseDuckAddress("Api.Alice.duck/v1/health?q=1")).toMatchObject({
      kind: "account",
      handle: "alice",
      hostname: "api.alice.duck",
      pathAndQuery: "/v1/health?q=1",
      name: { label: "api" },
    });
    expect(parseDuckAddress("net.alice.duck")).toMatchObject({
      kind: "account",
      handle: "alice",
      name: { label: "net" },
    });
    expect(() => parseDuckAddress("api.net.duck")).toThrow(/reserved/);
    expect(() => parseDuckAddress("a.b.c.duck")).toThrow(/<label>\.<account>/);
    expect(() => parseDuckAddress("alice.duck/#fragment")).toThrow(/fragment/);
  });
});

describe("browser authority boundaries", () => {
  it("orders DuckFS manifests by the same ASCII byte order as Rust", async () => {
    const publisher = "11".repeat(32);
    const name = gateway.routeName();
    const root = gateway.contentRoot(publisher, name);
    const paths = ["a.html", "A.html", "_x.html", "-x.html", ".x.html", "0.html"];
    const contents = new Map(paths.map((path) => [
      `${root}/${path}`,
      encoder.encode(path),
    ]));
    const transport = makeTransportStub({
      query: vi.fn(async (_target, request) => "refs" in (request as object)
        ? { refs: { head: "22".repeat(32), pins: {}, window_len: 1 } }
        : { find: {
          entries: paths.map((path) => ({
            path: `${root}/${path}`,
            kind: "file" as const,
            size: contents.get(`${root}/${path}`)!.length,
            exec: false,
            object: "33".repeat(32),
            meta: { mime: "text/html" },
          })),
          next: null,
        } }),
      filesRead: vi.fn(async ({ path }) => ({
        b64: b64(contents.get(path)!),
        eof: true,
      })),
      filesCommit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
    });

    const hash = await buildContentManifest(transport, publisher, name, "A.html");
    expect(hash).toMatch(/^[0-9a-f]{64}$/);

    // The committed .manifest.json carries the canonically-ordered file table,
    // and the returned hash is its exact byte digest.
    const calls = vi.mocked(transport.filesCommit).mock.calls;
    const request = calls[calls.length - 1]![0] as {
      changes: { put: { path: string; content: { inline: { b64: string } } } }[];
    };
    const put = request.changes[0].put;
    expect(put.path).toBe(`${gateway.contentRoot(publisher, name)}/${gateway.MANIFEST_FILE}`);
    const manifestBytes = fromB64(put.content.inline.b64);
    const manifest = JSON.parse(new TextDecoder().decode(manifestBytes)) as gateway.RouteManifest;
    expect(manifest.files.map((file) => file.path)).toEqual([
      "-x.html", ".x.html", "0.html", "A.html", "_x.html", "a.html",
    ]);
    expect(() => gateway.validateManifest(manifest)).not.toThrow();
    expect(hash).toBe(gateway.sha256Hex(manifestBytes));
  });

  it("loads net.duck from one pinned local snapshot and strips executable markup", async () => {
    const html = encoder.encode(`<!doctype html><title>Network</title>
      <style>body{color:#222}</style><script>fetch("https://evil.test")</script>
      <main onload="steal()">hello net</main><iframe src="https://evil.test"></iframe>`);
    const snapshot = "11".repeat(32);
    const query = vi.fn(async (target: string) => {
      expect(target).toBe("files");
      return { refs: { head: snapshot, pins: {}, window_len: 1 } };
    });
    const filesStat = vi.fn().mockResolvedValue({
      path: "/shared/.duck/net/index.html",
      kind: "file",
      size: html.length,
      exec: false,
      object: "22".repeat(32),
      meta: { mime: "text/html" },
    });
    const filesRead = vi.fn().mockResolvedValue({ b64: b64(html), eof: true });
    const page = await loadDuckPage(makeTransportStub({ query, filesStat, filesRead }), "net.duck");

    expect(page).toMatchObject({ hosting: "network", snapshot, title: "Network" });
    expect(page.srcDoc).toContain("hello net");
    expect(page.srcDoc).toContain("script-src 'none'");
    expect(page.srcDoc).not.toMatch(/<script|<iframe|onload|evil\.test/i);
    expect(filesStat).toHaveBeenCalledWith({
      path: "/shared/.duck/net/index.html",
      snapshot,
    });
    expect(query).not.toHaveBeenCalledWith("duckdns", expect.anything());
  });

  it("verifies one signed route, mints a scoped origin, and closes the resolution race", async () => {
    vi.spyOn(nodeBootstrap, "isTauri").mockReturnValue(true);
    const secret = new Uint8Array(32).fill(5);
    const signer = ed25519.getPublicKey(secret);
    const publisher = new Array(32).fill(8);
    const statement: gateway.RouteStatement = {
      version: gateway.ROUTE_FORMAT_VERSION,
      chain_id: "test",
      account_id: [...signer],
      name: { label: "api" },
      publisher_node: publisher,
      revision: 2,
      route: {
        target: { kind: "loopback_http" },
        policy: {
          audience: { kind: "network" },
          methods: ["get", "head", "post"],
          max_request_bytes: 1024,
          max_response_bytes: 4096,
          allow_authorization: false,
          allow_upgrade: false,
        },
      },
    };
    const record: gateway.RouteRecord = {
      statement,
      authorization: {
        signer: [...signer],
        signature: [...ed25519.sign(gateway.verificationPayload(statement), secret)],
      },
    };
    const account = {
      account_id: [...signer],
      display_name: "Alice",
      nonce: 0,
      member_keys: [{ pubkey: [...signer], kind: "ed25519" as const, label: null, added_at: 0 }],
      nodes: [publisher],
      updated_at: 0,
    };
    const query = vi.fn(async (target: string) => {
      if (target === "duckdns") return { resolved: { account_id: [...signer] } };
      if (target === "identity") return { account };
      if (target === "gateway") return { route: record };
      throw new Error(`unexpected query target ${target}`);
    });
    const gatewayBrowserBase = vi
      .fn()
      .mockResolvedValue({ base: "http://127.0.0.1:49152" });
    const page = await loadDuckPage(
      makeTransportStub({ query, gatewayBrowserBase }),
      "api.alice.duck/v1/health?q=1",
    );

    expect(page).toMatchObject({
      hosting: "gateway",
      target: "loopback_http",
      // A stable duck:// origin — no session token; the node re-resolves each request.
      srcUrl: "duck://api.alice.duck/v1/health?q=1",
      revision: 2,
    });
    expect(gatewayBrowserBase).toHaveBeenCalled();
    expect(query.mock.calls.filter(([target]) => target === "duckdns")).toHaveLength(1);
    expect(query.mock.calls.filter(([target]) => target === "gateway")).toHaveLength(1);
  });
});
