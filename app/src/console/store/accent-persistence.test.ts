import { afterEach, describe, expect, it } from "vitest";

import { createInitialState, DEFAULT_ACCENT, loadAccent, saveAccent } from "./state";

afterEach(() => {
  localStorage.clear();
});

describe("accent persistence", () => {
  it("round-trips a saved accent", () => {
    saveAccent("#3d63b8");
    expect(loadAccent()).toBe("#3d63b8");
  });

  it("falls back to the default on a missing or malformed value", () => {
    expect(loadAccent()).toBe(DEFAULT_ACCENT);
    localStorage.setItem("ducktape.accent", "javascript:alert(1)");
    expect(loadAccent()).toBe(DEFAULT_ACCENT);
  });

  it("hydrates the initial state accent from storage", () => {
    saveAccent("#3f7d54");
    expect(createInitialState().accent).toBe("#3f7d54");
  });
});
