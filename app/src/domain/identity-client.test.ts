// The identity client mirrors identity-interface: IdentityMsg encoding
// (BindNode/UnbindNode carry a user-signed cert minted by the tauri shell,
// SetUserName is origin-gated) + IdentityReply decoding for All/Get/UserOf.

import { describe, expect, it, vi } from "vitest";

import {
  allUsers,
  getUser,
  hexToBytes,
  setUserName,
  submitRawMsg,
  userOf,
} from "./identity-client";
import type { UserView } from "./identity-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn(),
  getBlob: vi.fn(),
  status: vi.fn(),
  metrics: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
});

const wireUser = (patch: Partial<UserView> = {}): UserView => ({
  user_key: [1, 2, 3],
  display_name: "jess",
  nonce: 0,
  nodes: [[4, 5, 6]],
  updated_at: 1,
  ...patch,
});

describe("hexToBytes", () => {
  it("decodes a lowercase hex string into byte ints", () => {
    expect(hexToBytes("0a0b0c")).toEqual([10, 11, 12]);
  });

  it("decodes the empty string to an empty array", () => {
    expect(hexToBytes("")).toEqual([]);
  });
});

describe("identity queries", () => {
  it("sends All with from/limit and decodes Users", async () => {
    const wire = [wireUser()];
    const transport = stubTransport({ users: wire });
    await expect(allUsers(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      all: { from: 0, limit: 256 },
    });
  });

  it("passes explicit from/limit through", async () => {
    const transport = stubTransport({ users: [] });
    await allUsers(transport, { from: 10, limit: 5 });
    expect(transport.query).toHaveBeenCalledWith("identity", {
      all: { from: 10, limit: 5 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ profiles: [] });
    await expect(allUsers(transport)).rejects.toThrow(
      "unexpected module reply: wanted users",
    );
  });

  it("userOf hex-decodes the node key and decodes the User reply", async () => {
    const user = wireUser();
    const transport = stubTransport({ user });
    await expect(userOf(transport, "040506")).resolves.toEqual(user);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      user_of: { node_key: [4, 5, 6] },
    });
  });

  it("userOf resolves null when the node is unbound", async () => {
    const transport = stubTransport({ user: null });
    await expect(userOf(transport, "ff")).resolves.toBeNull();
  });

  it("getUser hex-decodes the user key and decodes the User reply", async () => {
    const user = wireUser();
    const transport = stubTransport({ user });
    await expect(getUser(transport, "010203")).resolves.toEqual(user);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      get: { user_key: [1, 2, 3] },
    });
  });
});

describe("identity msgs", () => {
  it("submitRawMsg parses the tauri-signed payload and submits it untouched", async () => {
    const transport = stubTransport();
    const raw = JSON.stringify({
      bind_node: { user_key: [1, 2, 3], user_sig: [9, 9, 9] },
    });
    await submitRawMsg(transport, raw);
    expect(transport.submit).toHaveBeenCalledWith("identity", {
      bind_node: { user_key: [1, 2, 3], user_sig: [9, 9, 9] },
    });
  });

  it("setUserName encodes SetUserName and stamps the origin (origin-gated write)", async () => {
    const transport = stubTransport();
    await setUserName(transport, { displayName: "jess", origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "identity",
      { set_user_name: { display_name: "jess" } },
      "jess",
    );
  });
});
