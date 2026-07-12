import { describe, expect, it, vi } from "vitest";

import { repoFile } from "../test/repo-file";
import {
  handleError,
  normalizeHandle,
  registrations,
  RESERVED_ROOT_LABELS,
  resolve,
  setHandle,
} from "./duckdns-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

describe("DuckDNS handle registration", () => {
  it("sets and clears the authenticated account's optional handle", async () => {
    const transport = stubTransport();

    await setHandle(transport, { handle: "rae", origin: "Rae" });
    await setHandle(transport, { handle: null, origin: "Rae" });

    expect(transport.submit).toHaveBeenNthCalledWith(
      1,
      "duckdns",
      { set_handle: { handle: "rae" } },
      "Rae",
    );
    expect(transport.submit).toHaveBeenNthCalledWith(
      2,
      "duckdns",
      { set_handle: { handle: null } },
      "Rae",
    );
  });

  it("reads the deterministic handle to AccountId projection", async () => {
    const rows = [{ handle: "rae", account_id: [1, 2, 3] }];
    const transport = stubTransport({ registrations: rows });

    await expect(registrations(transport)).resolves.toEqual(rows);
    expect(transport.query).toHaveBeenCalledWith("duckdns", {
      registrations: { from: 0, limit: 256 },
    });

    await registrations(transport, { from: 4, limit: 8 });
    expect(transport.query).toHaveBeenLastCalledWith("duckdns", {
      registrations: { from: 4, limit: 8 },
    });
  });

  it("resolves an account name to AccountId and nothing transport-related", async () => {
    const resolved = { account_id: [1, 2, 3] };
    const transport = stubTransport({ resolved });
    const name = { handle: "rae" };

    await expect(resolve(transport, name)).resolves.toEqual(resolved);
    expect(transport.query).toHaveBeenCalledWith("duckdns", {
      resolve: { name },
    });
    expect(JSON.stringify(resolved)).not.toMatch(/node|service|endpoint|route|port/);
  });
});

describe("DuckDNS handle validation", () => {
  it("canonicalizes user input and accepts DNS labels", () => {
    const handle = normalizeHandle("  Rae-Team  ");
    expect(handle).toBe("rae-team");
    expect(handleError(handle)).toBeNull();
  });

  it("rejects reserved and malformed root labels", () => {
    expect(handleError("net")).toMatch(/reserved/);
    expect(handleError("agents")).toMatch(/reserved/);
    expect(handleError("-rae")).toMatch(/start or end/);
    expect(handleError("rae_team")).toMatch(/lowercase/);
  });

  // Consensus is the authority: a handle this client accepts but the node
  // reserves is a squat the UI walks the user into (`agents.duck` owns every
  // agent's attribution ident). Read the Rust const so a label added on one
  // side only turns this red.
  it("mirrors the consensus reserved-root-label set exactly", () => {
    const wire = repoFile("crates/system/duckdns/src/wire.rs");
    const literal = /RESERVED_ROOT_LABELS: &\[&str\] = &\[([^\]]*)\]/.exec(wire);
    expect(literal, "RESERVED_ROOT_LABELS not found in wire.rs").not.toBeNull();
    const consensus = [...literal![1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect(consensus.length).toBeGreaterThan(0);
    expect([...RESERVED_ROOT_LABELS].sort()).toEqual([...consensus].sort());

    // …and ops/demo-gateway.mjs, the third copy (a plain node script — it can't
    // import this module).
    const seed = repoFile("ops/demo-gateway.mjs");
    const seedLiteral = /RESERVED_ROOT_LABELS = \[([^\]]*)\]/.exec(seed);
    expect(seedLiteral, "RESERVED_ROOT_LABELS not found in demo-gateway.mjs").not.toBeNull();
    const seedLabels = [...seedLiteral![1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect([...seedLabels].sort()).toEqual([...consensus].sort());
  });
});
