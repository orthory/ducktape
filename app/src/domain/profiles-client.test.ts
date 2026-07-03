// The profiles client mirrors profiles-interface: ProfileMsg encoding (SetName,
// origin-gated so it threads the submit origin) + ProfileReply decoding for the
// All query.

import { describe, expect, it, vi } from "vitest";

import { allProfiles, setName } from "./profiles-client";
import type { Profile } from "./profiles-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  putBlob: vi.fn(),
  status: vi.fn(),
  onBlock: vi.fn(),
});

describe("profile msgs", () => {
  it("encodes SetName and stamps the origin (origin-gated write)", async () => {
    const transport = stubTransport();
    await setName(transport, { displayName: "jess", origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "profiles",
      { SetName: { display_name: "jess" } },
      "jess",
    );
  });
});

describe("profile queries", () => {
  it("sends All with from/limit and decodes Profiles", async () => {
    const wire: Profile[] = [
      { key: [1, 2, 3], display_name: "jess", updated_at: 1 },
    ];
    const transport = stubTransport({ Profiles: wire });
    await expect(allProfiles(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("profiles", {
      All: { from: 0, limit: 256 },
    });
  });

  it("passes explicit from/limit through", async () => {
    const transport = stubTransport({ Profiles: [] });
    await allProfiles(transport, { from: 10, limit: 5 });
    expect(transport.query).toHaveBeenCalledWith("profiles", {
      All: { from: 10, limit: 5 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ Tasks: [] });
    await expect(allProfiles(transport)).rejects.toThrow(
      "unexpected module reply: wanted Profiles",
    );
  });
});
