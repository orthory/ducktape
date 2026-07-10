import { afterEach, describe, expect, it } from "vitest";

import { createInitialState, DEFAULT_THEME, loadTheme, saveTheme } from "./state";

afterEach(() => {
  localStorage.clear();
});

describe("theme persistence", () => {
  it("round-trips a saved theme", () => {
    saveTheme("dark");
    expect(loadTheme()).toBe("dark");
    saveTheme("light");
    expect(loadTheme()).toBe("light");
  });

  it("falls back to the default on a missing or malformed value", () => {
    // jsdom has no matchMedia, so first-run resolves to the default.
    expect(loadTheme()).toBe(DEFAULT_THEME);
    localStorage.setItem("ducktape.theme", "chartreuse");
    expect(loadTheme()).toBe(DEFAULT_THEME);
  });

  it("hydrates the initial state theme from storage", () => {
    saveTheme("dark");
    expect(createInitialState().theme).toBe("dark");
  });
});
