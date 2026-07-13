import { describe, expect, it, vi } from "vitest";

import {
  ROUTE_FORMAT_VERSION,
  accountsAudience,
  bytesToHex,
  listRoutes,
  probeRouteHealth,
  routeSigningPreimage,
  validateStatement,
} from "./gateway-client";
import { makeTransportStub } from "../test/transport-stub";
import type { RouteRecord } from "./gateway-client";

const loopbackRecord = (methods: Array<"get" | "head" | "post"> = ["get", "head"]): RouteRecord => ({
  statement: {
    version: ROUTE_FORMAT_VERSION,
    chain_id: "test",
    account_id: [1],
    name: { label: "api" },
    publisher_node: new Array(32).fill(3),
    revision: 7,
    route: {
      target: { kind: "loopback_http" },
      policy: {
        audience: { kind: "network" },
        methods,
        max_request_bytes: methods.includes("post") ? 1024 : 0,
        max_response_bytes: 4096,
        allow_authorization: false,
        allow_upgrade: false,
      },
    },
  },
  authorization: { signer: new Array(32).fill(4), signature: new Array(64).fill(5) },
});

describe("gateway wire contract", () => {
  it("matches the Rust signing-preimage fixed vector", () => {
    expect(bytesToHex(routeSigningPreimage({
      version: ROUTE_FORMAT_VERSION,
      chain_id: "test",
      account_id: [1, 2],
      name: { label: "api" },
      publisher_node: new Array(32).fill(3),
      revision: 7,
      route: {
        target: { kind: "loopback_http" },
        policy: {
          audience: { kind: "network" },
          methods: ["get", "head", "post"],
          max_request_bytes: 1024,
          max_response_bytes: 4096,
          allow_authorization: false,
          allow_upgrade: true,
        },
      },
    }))).toBe(
      "01040000000000000074657374020000000000000001020103000000000000006170692000000000000000030303030303030303030303030303030303030303030303030303030303030307000000000000000102030000000000000001020300040000000000000010000000000000000102",
    );
  });

  it("binds only the manifest hash for content routes (matches Rust vector)", () => {
    expect(bytesToHex(routeSigningPreimage({
      version: ROUTE_FORMAT_VERSION,
      chain_id: "test",
      account_id: [1, 2],
      name: { label: "api" },
      publisher_node: new Array(32).fill(3),
      revision: 7,
      route: {
        target: { kind: "duck_fs", manifest_sha256: "b".repeat(64) },
        policy: {
          audience: { kind: "network" },
          methods: ["get", "head"],
          max_request_bytes: 0,
          max_response_bytes: 4096,
          allow_authorization: false,
          allow_upgrade: false,
        },
      },
    }))).toBe(
      "010400000000000000746573740200000000000000010201030000000000000061706920000000000000000303030303030303030303030303030303030303030303030303030303030303070000000000000001020200000000000000010200000000000000000010000000000000000001bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
  });

  it("rejects ambient credentials and mutation semantics on DuckFS targets", () => {
    expect(() => validateStatement({
      version: 1,
      chain_id: "test",
      account_id: [1],
      name: { label: null },
      publisher_node: new Array(32).fill(2),
      revision: 1,
      route: {
        target: { kind: "duck_fs", manifest_sha256: "0".repeat(64) },
        policy: {
          audience: { kind: "network" },
          methods: ["get", "head", "post"],
          max_request_bytes: 1,
          max_response_bytes: 1024,
          allow_authorization: true,
          allow_upgrade: false,
        },
      },
    })).toThrow(/DuckFS routes require GET\+HEAD/);
  });

  it("lists live account routes through one bounded management query", async () => {
    const summary = {
      name: { label: "api" },
      publisher_node: new Array(32).fill(3),
      revision: 7,
      target: "loopback_http" as const,
    };
    const query = vi.fn().mockResolvedValue({ routes: [summary] });
    await expect(listRoutes(makeTransportStub({ query }), [1])).resolves.toEqual([summary]);
    expect(query).toHaveBeenCalledWith("gateway", { list: { account_id: [1] } });
  });

  it("health-checks the real route path with a credential-free HEAD", async () => {
    const record = loopbackRecord();
    const gatewayProxy = vi.fn().mockResolvedValue({
      head: { status: 204, headers: [] },
      body: new Uint8Array(0),
    });
    await expect(probeRouteHealth(makeTransportStub({ gatewayProxy }), record)).resolves.toEqual({
      path: "/",
      status: 204,
    });
    expect(gatewayProxy).toHaveBeenCalledWith({
      head: {
        account_id: [1],
        name: { label: "api" },
        revision: 7,
        method: "head",
        path_and_query: "/",
        headers: [],
        body_len: 0,
      },
      body: new Uint8Array(0),
    });
  });

  it("builds a sorted, unique, capped explicit-account audience from hex ids", () => {
    expect(accountsAudience(["02", "01", "01", "03"])).toEqual({
      kind: "accounts",
      account_ids: [[1], [2], [3]],
    });

    const capped = accountsAudience(Array.from({ length: 40 }, (_, index) => bytesToHex([index])));
    expect(capped.kind).toBe("accounts");
    if (capped.kind === "accounts") {
      expect(capped.account_ids).toHaveLength(32);
      expect(capped.account_ids[0]).toEqual([0]);
      expect(capped.account_ids[31]).toEqual([31]);
    }
  });

  it("does not probe a route whose signed policy omits HEAD", async () => {
    const gatewayProxy = vi.fn();
    await expect(
      probeRouteHealth(makeTransportStub({ gatewayProxy }), loopbackRecord(["post"])),
    ).rejects.toThrow(/requires HEAD/);
    expect(gatewayProxy).not.toHaveBeenCalled();
  });
});
