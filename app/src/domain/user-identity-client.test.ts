// user-identity-client contract: every fn is a thin invoke wrapper over the
// Rust `user_identity` commands (Task 3), desktop-only. identityState() is
// the one exception that must work everywhere — the web build has no local
// user key at all, so it resolves the inert "absent" shape without invoking
// rather than rejecting, letting callers gate on it unconditionally.

import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

import {
  confirmMnemonic,
  createIdentity,
  encryptLegacy,
  identityState,
  lockIdentity,
  restoreIdentity,
  revealMnemonic,
  unlockIdentity,
} from "./user-identity-client";

const markNative = () => {
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__ = invokeMock;
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  vi.clearAllMocks();
});

describe("identityState", () => {
  it("resolves the inert absent shape on the web build without invoking", async () => {
    await expect(identityState()).resolves.toEqual({
      state: "absent",
      mnemonicConfirmed: true,
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("invokes user_identity_state and returns its shape verbatim on desktop", async () => {
    markNative();
    invokeMock.mockResolvedValue({
      state: "locked",
      pubkey: "cd34",
      mnemonicConfirmed: false,
    });

    await expect(identityState()).resolves.toEqual({
      state: "locked",
      pubkey: "cd34",
      mnemonicConfirmed: false,
    });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_state");
  });
});

describe("createIdentity", () => {
  it("invokes user_identity_create with the password", async () => {
    markNative();
    invokeMock.mockResolvedValue({ pubkey: "ab12", mnemonic: "one two three" });

    await expect(createIdentity("hunter2-plus")).resolves.toEqual({
      pubkey: "ab12",
      mnemonic: "one two three",
    });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_create", {
      password: "hunter2-plus",
    });
  });

  it("rejects on the web build without invoking", async () => {
    await expect(createIdentity("hunter2-plus")).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("restoreIdentity", () => {
  it("invokes user_identity_restore with mnemonic + password", async () => {
    markNative();
    invokeMock.mockResolvedValue({ pubkey: "ab12" });

    await expect(
      restoreIdentity("word list of twenty four", "hunter2-plus"),
    ).resolves.toEqual({ pubkey: "ab12" });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_restore", {
      mnemonic: "word list of twenty four",
      password: "hunter2-plus",
    });
  });

  it("rejects on the web build without invoking", async () => {
    await expect(restoreIdentity("words", "hunter2-plus")).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("unlockIdentity", () => {
  it("invokes user_identity_unlock with the password", async () => {
    markNative();
    invokeMock.mockResolvedValue({ pubkey: "ab12" });

    await expect(unlockIdentity("hunter2-plus")).resolves.toEqual({
      pubkey: "ab12",
    });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_unlock", {
      password: "hunter2-plus",
    });
  });

  it("rejects on the web build without invoking", async () => {
    await expect(unlockIdentity("hunter2-plus")).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("revealMnemonic", () => {
  it("invokes user_identity_reveal with the password", async () => {
    markNative();
    invokeMock.mockResolvedValue({ mnemonic: "one two three" });

    await expect(revealMnemonic("hunter2-plus")).resolves.toEqual({
      mnemonic: "one two three",
    });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_reveal", {
      password: "hunter2-plus",
    });
  });

  it("rejects on the web build without invoking", async () => {
    await expect(revealMnemonic("hunter2-plus")).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("encryptLegacy", () => {
  it("invokes user_identity_encrypt with the password", async () => {
    markNative();
    invokeMock.mockResolvedValue({ pubkey: "ab12" });

    await expect(encryptLegacy("hunter2-plus")).resolves.toEqual({
      pubkey: "ab12",
    });
    expect(invokeMock).toHaveBeenCalledWith("user_identity_encrypt", {
      password: "hunter2-plus",
    });
  });

  it("rejects on the web build without invoking", async () => {
    await expect(encryptLegacy("hunter2-plus")).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("confirmMnemonic", () => {
  it("invokes user_identity_confirm_mnemonic with no args", async () => {
    markNative();
    invokeMock.mockResolvedValue(undefined);

    await expect(confirmMnemonic()).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("user_identity_confirm_mnemonic");
  });

  it("rejects on the web build without invoking", async () => {
    await expect(confirmMnemonic()).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("lockIdentity", () => {
  it("invokes user_identity_lock with no args", async () => {
    markNative();
    invokeMock.mockResolvedValue(undefined);

    await expect(lockIdentity()).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("user_identity_lock");
  });

  it("rejects on the web build without invoking", async () => {
    await expect(lockIdentity()).rejects.toThrow();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
