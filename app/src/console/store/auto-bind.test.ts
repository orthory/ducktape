// auto-bind contract: on a desktop connect, silently offer this machine's user
// key to bind the workspace's node — see auto-bind.ts for the step-by-step.
// Every failure mode (no tauri shell, no user key yet, a nonce race on
// submit) must resolve a status, never throw — the caller fires this without
// awaiting. The nonce comes from the ACCOUNT the local key belongs to
// (of_member), not a lookup by account id.

import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { autoBindUserIdentity } from "./auto-bind";
import type { AccountView } from "../../domain/identity-client";
import type { NodeTransport } from "../../domain/transport";
import { makeTransportStub } from "../../test/transport-stub";

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  localStorage.removeItem("ducktape.accountLinkPending");
  vi.clearAllMocks();
});

const wireAccount = (patch: Partial<AccountView> = {}): AccountView => ({
  account_id: [1, 2, 3],
  display_name: null,
  nonce: 0,
  member_keys: [{ pubkey: [1, 2, 3], kind: "ed25519", label: null, added_at: 1 }],
  nodes: [[9, 9, 9]],
  updated_at: 1,
  ...patch,
});

const boundMsg = (sig: number[]) =>
  JSON.stringify({
    bind_node: {
      authorizer: { key: [1, 2, 3], kind: "ed25519", proof: { signature: { sig } } },
    },
  });

const stubTransport = (
  queryImpl: (target: string, query: unknown) => unknown,
): NodeTransport => ({
  ...makeTransportStub({
    // wrap the sync stub so the mock's type matches NodeTransport.query's
    // Promise<unknown> return (await already coerces the plain value at runtime).
    query: vi.fn((target: string, query: unknown) => Promise.resolve(queryImpl(target, query))),
  }),
});

const workspace = { chainId: "team#abcd", pubkey: "ab12" };

describe("autoBindUserIdentity", () => {
  it("skips on the web build (no tauri shell) without touching the node", async () => {
    const transport = stubTransport(() => ({ account: null }));

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "skipped",
    );
    expect(transport.query).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("short-circuits to 'already' when the node is already bound, no sign/status invoke calls", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "plaintext", mnemonicConfirmed: true });
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport(() => ({ account: wireAccount() }));

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "already",
    );
    expect(transport.query).toHaveBeenCalledTimes(1);
    expect(transport.query).toHaveBeenCalledWith("identity", {
      of_node: { node_key: [171, 18] }, // hexToBytes("ab12")
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("user_identity_state");
  });

  it("returns 'locked' and makes no further calls when the identity is encrypted and locked", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "locked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport(() => {
      throw new Error("must not query the node when locked");
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "locked",
    );
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("user_identity_state");
    expect(transport.query).not.toHaveBeenCalled();
    expect(transport.submit).not.toHaveBeenCalled();
  });

  it("binds identity with nonce 0 without implicitly registering DuckDNS", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "unlocked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      if (cmd === "user_sign_bind") return Promise.resolve(boundMsg([9, 9, 9]));
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport((_target, q) => {
      const query = q as Record<string, unknown>;
      if ("of_node" in query) return { account: null };
      if ("of_member" in query) return { account: null };
      throw new Error(`unexpected query ${JSON.stringify(q)}`);
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "bound",
    );

    // No legacy user_identity_status call — the pubkey to sign with comes
    // straight off identityState()'s own reply.
    expect(invokeMock).not.toHaveBeenCalledWith("user_identity_status");
    expect(transport.query).toHaveBeenCalledWith("identity", {
      of_member: { member_key: [205, 52] }, // hexToBytes("cd34")
    });
    expect(invokeMock).toHaveBeenCalledWith("user_sign_bind", {
      chainId: "team#abcd",
      nodePub: "ab12",
      nonce: 0,
    });
    expect(transport.submit).toHaveBeenCalledWith("identity", {
      bind_node: {
        authorizer: { key: [1, 2, 3], kind: "ed25519", proof: { signature: { sig: [9, 9, 9] } } },
      },
    });
    expect(transport.submit).toHaveBeenCalledTimes(1);
    expect(transport.submit).not.toHaveBeenCalledWith(
      "duckdns",
      expect.anything(),
      expect.anything(),
    );
  });

  it("signs with the existing account's nonce (3), not 0", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "unlocked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      if (cmd === "user_sign_bind") return Promise.resolve(boundMsg([7, 7, 7]));
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport((_target, q) => {
      const query = q as Record<string, unknown>;
      if ("of_node" in query) return { account: null };
      if ("of_member" in query) return { account: wireAccount({ nonce: 3 }) };
      throw new Error(`unexpected query ${JSON.stringify(q)}`);
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "bound",
    );

    expect(invokeMock).toHaveBeenCalledWith("user_sign_bind", {
      chainId: "team#abcd",
      nodePub: "ab12",
      nonce: 3,
    });
  });

  it("resolves 'failed' (not a throw) when the node rejects the submit", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "unlocked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      if (cmd === "user_sign_bind") return Promise.resolve(boundMsg([9, 9, 9]));
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport(() => ({ account: null }));
    (transport.submit as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error("nonce conflict — another device already bound this node"),
    );

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "failed",
    );
  });

  it("resolves 'failed' when unlocked/plaintext but identityState() carries no pubkey", async () => {
    // Shouldn't happen in practice (unlocked/plaintext always carry a pubkey
    // in the clear) — but if it ever did, there is nothing to sign a bind
    // with once we know the node isn't already bound, so this must resolve
    // 'failed' rather than throw, and never reach the sign/submit calls.
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "unlocked", mnemonicConfirmed: true });
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport((_target, q) => {
      const query = q as Record<string, unknown>;
      if ("of_node" in query) return { account: null };
      throw new Error(`unexpected query ${JSON.stringify(q)}`);
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "failed",
    );
    expect(invokeMock).not.toHaveBeenCalledWith("user_sign_bind", expect.anything());
    expect(transport.submit).not.toHaveBeenCalled();
  });

  it("defers instead of founding a duplicate account while a device link is pending", async () => {
    // The user chose "link this device to an existing account": until the
    // other device's AddMemberKey lands, this key has no membership — a bind
    // now would FOUND a fresh account, the exact split the link is avoiding.
    markTauri();
    localStorage.setItem("ducktape.accountLinkPending", "1");
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "unlocked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport((_target, q) => {
      const query = q as Record<string, unknown>;
      if ("of_node" in query) return { account: null };
      if ("of_member" in query) return { account: null };
      throw new Error(`unexpected query ${JSON.stringify(q)}`);
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "deferred",
    );
    expect(invokeMock).not.toHaveBeenCalledWith("user_sign_bind", expect.anything());
    expect(transport.submit).not.toHaveBeenCalled();
    // The flag survives — the next connect retries the membership lookup.
    expect(localStorage.getItem("ducktape.accountLinkPending")).toBe("1");
  });

  it("binds at the account's nonce and clears the pending-link flag once membership appears", async () => {
    markTauri();
    localStorage.setItem("ducktape.accountLinkPending", "1");
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({
          state: "unlocked",
          pubkey: "cd34",
          mnemonicConfirmed: true,
        });
      if (cmd === "user_sign_bind") return Promise.resolve(boundMsg([5, 5, 5]));
      throw new Error(`unexpected invoke ${cmd}`);
    });
    const transport = stubTransport((_target, q) => {
      const query = q as Record<string, unknown>;
      if ("of_node" in query) return { account: null };
      if ("of_member" in query) return { account: wireAccount({ nonce: 2 }) };
      throw new Error(`unexpected query ${JSON.stringify(q)}`);
    });

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "bound",
    );
    expect(invokeMock).toHaveBeenCalledWith("user_sign_bind", {
      chainId: "team#abcd",
      nodePub: "ab12",
      nonce: 2,
    });
    expect(localStorage.getItem("ducktape.accountLinkPending")).toBeNull();
  });

  it("resolves 'failed' when identityState() rejects (e.g. the shell can't read the user key)", async () => {
    markTauri();
    invokeMock.mockRejectedValue(new Error("no machine user key"));
    const transport = stubTransport(() => ({ account: null }));

    await expect(autoBindUserIdentity(transport, workspace)).resolves.toBe(
      "failed",
    );
    expect(transport.submit).not.toHaveBeenCalled();
  });
});
