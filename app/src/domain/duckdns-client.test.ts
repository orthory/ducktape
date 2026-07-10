import { describe, expect, it, vi } from "vitest";

import {
  handleError,
  normalizeHandle,
  registrations,
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
    expect(handleError("-rae")).toMatch(/start or end/);
    expect(handleError("rae_team")).toMatch(/lowercase/);
  });
});
