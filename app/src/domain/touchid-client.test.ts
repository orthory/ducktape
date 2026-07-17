// touchid-client contract: thin invoke wrappers over the Rust `touchid_*`
// commands (Task 1-3), desktop-only. touchidAvailable/touchidEnrolled are
// non-prompting presence checks that must work everywhere — the web build has
// no Keychain, so they resolve false without invoking. randomPassphrase is a
// pure local mint (never invoked).

import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

import {
  randomPassphrase,
  touchidAvailable,
  touchidDisable,
  touchidEnroll,
  touchidEnrolled,
  touchidUnlock,
} from "./touchid-client";

const markNative = () => {
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__ = invokeMock;
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  vi.clearAllMocks();
});

describe("randomPassphrase", () => {
  it("is 32 bytes of base64, unique per call", () => {
    const a = randomPassphrase();
    const b = randomPassphrase();
    expect(a).not.toEqual(b);
    // 32 bytes → 44 base64 chars incl. padding
    expect(a.length).toBeGreaterThanOrEqual(43);
    // decodes back to exactly 32 bytes
    expect(atob(a).length).toBe(32);
  });
});

describe("touchidAvailable", () => {
  it("resolves false on the web build without invoking", async () => {
    await expect(touchidAvailable()).resolves.toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("invokes touchid_available on desktop", async () => {
    markNative();
    invokeMock.mockResolvedValue(true);
    await expect(touchidAvailable()).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("touchid_available");
  });
});

describe("touchidEnrolled", () => {
  it("resolves false on the web build without invoking", async () => {
    await expect(touchidEnrolled()).resolves.toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("invokes touchid_enrolled on desktop", async () => {
    markNative();
    invokeMock.mockResolvedValue(true);
    await expect(touchidEnrolled()).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("touchid_enrolled");
  });
});

describe("touchidEnroll", () => {
  it("invokes touchid_enroll with the passphrase", async () => {
    markNative();
    invokeMock.mockResolvedValue(undefined);
    await expect(touchidEnroll("RANDOMPASS")).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("touchid_enroll", {
      passphrase: "RANDOMPASS",
    });
  });
});

describe("touchidUnlock", () => {
  it("invokes touchid_unlock and returns the pubkey shape", async () => {
    markNative();
    invokeMock.mockResolvedValue({ pubkey: "ab12" });
    await expect(touchidUnlock()).resolves.toEqual({ pubkey: "ab12" });
    expect(invokeMock).toHaveBeenCalledWith("touchid_unlock");
  });
});

describe("touchidDisable", () => {
  it("invokes touchid_disable with no args", async () => {
    markNative();
    invokeMock.mockResolvedValue(undefined);
    await expect(touchidDisable()).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("touchid_disable");
  });
});
