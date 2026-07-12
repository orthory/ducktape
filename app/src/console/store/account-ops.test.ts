// The Account view's writes: assembled from fresh account facts, signed in
// the shell, refused on link-nonce drift — see account-ops.ts. The transport
// stub mirrors auto-bind.test.ts's.

import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addMemberFromResponse,
  approvePhoneEnrollment,
  mintLinkChallenge,
  removeMemberKey,
  startPhoneEnrollment,
  unbindNode,
} from "./account-ops";
import type { PhoneEnrollment } from "./account-ops";
import type { AccountView } from "../../domain/identity-client";
import type { NodeTransport } from "../../domain/transport";
import { encodeLinkResponse } from "../views/account/link-device";
import type { LinkChallenge } from "../views/account/link-device";

afterEach(() => vi.clearAllMocks());

const wireAccount = (patch: Partial<AccountView> = {}): AccountView => ({
  account_id: [1, 2, 3],
  display_name: "Eddy",
  nonce: 5,
  member_keys: [{ pubkey: [1, 2, 3], kind: "ed25519", label: null, added_at: 1 }],
  nodes: [[9, 9, 9]],
  updated_at: 1,
  ...patch,
});

const stubTransport = (account: AccountView | null): NodeTransport =>
  ({
    submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
    query: vi.fn().mockResolvedValue({ account }),
    view: vi.fn(),
    putBlob: vi.fn(),
    getBlob: vi.fn(),
    status: vi.fn(),
    blocks: vi.fn(),
    filesStage: vi.fn(),
    filesCommit: vi.fn(),
    filesStat: vi.fn(),
    filesLs: vi.fn(),
    filesRead: vi.fn(),
    filesHistory: vi.fn(),
    onBlock: vi.fn(),
  }) as unknown as NodeTransport;

const deps = (account: AccountView | null) => ({
  transport: stubTransport(account),
  chainId: "team#abcd",
  nodePub: "090909",
});

const challenge: LinkChallenge = {
  chainId: "team#abcd",
  accountId: "010203",
  nonce: 5,
  name: "Eddy",
};

const RESPONSE = encodeLinkResponse({
  pubkey: "cd34",
  kind: "ed25519",
  possession: '{"signature":{"sig":[7]}}',
  label: "work laptop",
});

describe("mintLinkChallenge", () => {
  it("reads the account fresh and emits its id, nonce, and name", async () => {
    await expect(mintLinkChallenge(deps(wireAccount()))).resolves.toEqual(challenge);
  });

  it("refuses when this node is unbound", async () => {
    await expect(mintLinkChallenge(deps(null))).rejects.toThrow(/isn't linked/);
  });
});

describe("addMemberFromResponse", () => {
  it("authorizes and submits at the challenge nonce", async () => {
    const d = deps(wireAccount());
    invokeMock.mockResolvedValue('{"add_member_key":{"ok":1}}');

    await expect(addMemberFromResponse(d, challenge, RESPONSE)).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("user_sign_add_member", {
      chainId: "team#abcd",
      accountId: "010203",
      newPub: "cd34",
      newKind: "ed25519",
      nonce: 5,
      possession: '{"signature":{"sig":[7]}}',
      label: "work laptop",
    });
    expect(d.transport.submit).toHaveBeenCalledWith("identity", {
      add_member_key: { ok: 1 },
    });
  });

  it("refuses on nonce drift instead of submitting a doomed msg", async () => {
    const d = deps(wireAccount({ nonce: 6 })); // an op landed since the mint

    await expect(addMemberFromResponse(d, challenge, RESPONSE)).rejects.toThrow(
      /changed since this link code/,
    );
    expect(invokeMock).not.toHaveBeenCalled();
    expect(d.transport.submit).not.toHaveBeenCalled();
  });

  it("rejects a malformed response blob before touching the node", async () => {
    const d = deps(wireAccount());

    await expect(addMemberFromResponse(d, challenge, "garbage")).rejects.toThrow(
      /link response code/,
    );
    expect(d.transport.query).not.toHaveBeenCalled();
  });

  it("maps the shell's identity-locked sentinel to an actionable error", async () => {
    const d = deps(wireAccount());
    invokeMock.mockRejectedValue(new Error("identity-locked"));

    await expect(addMemberFromResponse(d, challenge, RESPONSE)).rejects.toThrow(
      /locked on this device/,
    );
  });
});

describe("removeMemberKey", () => {
  it("signs against the live account id and nonce", async () => {
    const d = deps(wireAccount());
    invokeMock.mockResolvedValue('{"remove_member_key":{"ok":1}}');

    await expect(removeMemberKey(d, "cd34")).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("user_sign_remove_member", {
      chainId: "team#abcd",
      accountId: "010203",
      targetKey: "cd34",
      nonce: 5,
    });
    expect(d.transport.submit).toHaveBeenCalled();
  });
});

describe("phone enrollment", () => {
  const enrollment: PhoneEnrollment = {
    url: "http://192.168.1.7:40123/enroll#tok",
    accountId: "010203",
    nonce: 5,
  };

  it("startPhoneEnrollment reads the account fresh and pins its id + nonce", async () => {
    const d = deps(wireAccount());
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "enroll_start")
        return Promise.resolve({ url: "http://192.168.1.7:40123/enroll#tok" });
      throw new Error(`unexpected invoke ${cmd}`);
    });

    await expect(startPhoneEnrollment(d)).resolves.toEqual(enrollment);
    expect(invokeMock).toHaveBeenCalledWith("enroll_start", {
      chainId: "team#abcd",
      accountId: "010203",
      nonce: 5,
    });
  });

  it("approve authorizes at the pinned nonce with a Signature possession (raw bytes)", async () => {
    const d = deps(wireAccount());
    invokeMock.mockResolvedValue('{"add_member_key":{"ok":1}}');

    await expect(
      approvePhoneEnrollment(d, enrollment, "02ab", "0aff", "my phone"),
    ).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("user_sign_add_member", {
      chainId: "team#abcd",
      accountId: "010203",
      newPub: "02ab",
      newKind: "p256",
      nonce: 5,
      possession: JSON.stringify({ signature: { sig: [10, 255] } }),
      label: "my phone",
    });
    expect(d.transport.submit).toHaveBeenCalledWith("identity", {
      add_member_key: { ok: 1 },
    });
  });

  it("approve refuses on nonce drift instead of submitting a doomed msg", async () => {
    const d = deps(wireAccount({ nonce: 6 }));

    await expect(
      approvePhoneEnrollment(d, enrollment, "02ab", "0aff", null),
    ).rejects.toThrow(/changed since this QR/);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(d.transport.submit).not.toHaveBeenCalled();
  });

  it("approve maps the shell's identity-locked sentinel to an actionable error", async () => {
    const d = deps(wireAccount());
    invokeMock.mockRejectedValue(new Error("identity-locked"));

    await expect(
      approvePhoneEnrollment(d, enrollment, "02ab", "0aff", null),
    ).rejects.toThrow(/locked on this device/);
  });
});

describe("unbindNode", () => {
  it("signs the eviction at the live nonce", async () => {
    const d = deps(wireAccount({ nonce: 9 }));
    invokeMock.mockResolvedValue('{"unbind_node":{"ok":1}}');

    await expect(unbindNode(d, "0b0b")).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("user_sign_unbind", {
      chainId: "team#abcd",
      nodePub: "0b0b",
      nonce: 9,
    });
    expect(d.transport.submit).toHaveBeenCalled();
  });

  it("refuses when this node is unbound", async () => {
    await expect(unbindNode(deps(null), "0b0b")).rejects.toThrow(/isn't linked/);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
