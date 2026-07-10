import { describe, expect, it } from "vitest";

import {
  ROUTE_FORMAT_VERSION,
  bytesToHex,
  routeSigningPreimage,
  validateStatement,
} from "./gateway-client";

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
        },
      },
    }))).toBe(
      "010400000000000000746573740200000000000000010201030000000000000061706920000000000000000303030303030303030303030303030303030303030303030303030303030303070000000000000001020300000000000000010203000400000000000000100000000000000002",
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
        target: {
          kind: "duck_fs",
          content: {
            default_path: "index.html",
            files: [{
              path: "index.html",
              mime: "text/html",
              size: 1,
              sha256: "00".repeat(32),
            }],
          },
        },
        policy: {
          audience: { kind: "network" },
          methods: ["get", "head", "post"],
          max_request_bytes: 1,
          max_response_bytes: 1024,
          allow_authorization: true,
        },
      },
    })).toThrow(/DuckFS routes require GET\+HEAD/);
  });
});
