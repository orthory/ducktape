// The identity client mirrors identity-interface: IdentityMsg encoding
// (BindNode/UnbindNode/AddMemberKey/RemoveMemberKey carry a MemberAuth minted
// by the tauri shell, SetAccountName is origin-gated) + IdentityReply decoding
// for All/Get/OfNode/OfMember over the account registry.

import { describe, expect, it, vi } from "vitest";

import {
  accountOfMember,
  accountOfNode,
  allAccounts,
  getAccount,
  hexToBytes,
  setAccountName,
  submitRawMsg,
} from "./identity-client";
import type { AccountView } from "./identity-client";
import { makeTransportStub } from "../test/transport-stub";

const stubTransport = (reply?: unknown) =>
  makeTransportStub({ query: vi.fn().mockResolvedValue(reply) });

const wireAccount = (patch: Partial<AccountView> = {}): AccountView => ({
  account_id: [1, 2, 3],
  display_name: "jess",
  avatar: null,
  bio: null,
  nonce: 0,
  member_keys: [{ pubkey: [1, 2, 3], kind: "ed25519", label: null, added_at: 1 }],
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
  it("sends All with from/limit and decodes Accounts", async () => {
    const wire = [wireAccount()];
    const transport = stubTransport({ accounts: wire });
    await expect(allAccounts(transport)).resolves.toEqual(wire);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      all: { from: 0, limit: 256 },
    });
  });

  it("passes explicit from/limit through", async () => {
    const transport = stubTransport({ accounts: [] });
    await allAccounts(transport, { from: 10, limit: 5 });
    expect(transport.query).toHaveBeenCalledWith("identity", {
      all: { from: 10, limit: 5 },
    });
  });

  it("throws on a mismatched reply variant", async () => {
    const transport = stubTransport({ tasks: [] });
    await expect(allAccounts(transport)).rejects.toThrow(
      "unexpected module reply: wanted accounts",
    );
  });

  it("accountOfNode hex-decodes the node key and decodes the Account reply", async () => {
    const account = wireAccount();
    const transport = stubTransport({ account });
    await expect(accountOfNode(transport, "040506")).resolves.toEqual(account);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      of_node: { node_key: [4, 5, 6] },
    });
  });

  it("accountOfNode resolves null when the node is unbound", async () => {
    const transport = stubTransport({ account: null });
    await expect(accountOfNode(transport, "ff")).resolves.toBeNull();
  });

  it("accountOfMember hex-decodes the member key and decodes the Account reply", async () => {
    const account = wireAccount();
    const transport = stubTransport({ account });
    await expect(accountOfMember(transport, "010203")).resolves.toEqual(account);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      of_member: { member_key: [1, 2, 3] },
    });
  });

  it("getAccount hex-decodes the account id and decodes the Account reply", async () => {
    const account = wireAccount();
    const transport = stubTransport({ account });
    await expect(getAccount(transport, "010203")).resolves.toEqual(account);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      get: { account_id: [1, 2, 3] },
    });
  });
});

describe("identity msgs", () => {
  it("submitRawMsg parses the tauri-signed payload and submits it untouched", async () => {
    const transport = stubTransport();
    const raw = JSON.stringify({
      bind_node: {
        authorizer: { key: [1, 2, 3], kind: "ed25519", proof: { signature: { sig: [9, 9, 9] } } },
      },
    });
    await submitRawMsg(transport, raw);
    expect(transport.submit).toHaveBeenCalledWith("identity", {
      bind_node: {
        authorizer: { key: [1, 2, 3], kind: "ed25519", proof: { signature: { sig: [9, 9, 9] } } },
      },
    });
  });

  it("setAccountName encodes SetAccountName and stamps the origin (origin-gated write)", async () => {
    const transport = stubTransport();
    await setAccountName(transport, { displayName: "jess", origin: "jess" });
    expect(transport.submit).toHaveBeenCalledWith(
      "identity",
      { set_account_name: { display_name: "jess" } },
      "jess",
    );
  });
});
