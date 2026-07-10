import { afterEach, describe, expect, it } from "vitest";

import {
  createInitialState,
  DEFAULT_ACCENT,
  DEFAULT_NOTIFY_PREFS,
  loadAccent,
  loadNotifyPrefs,
  saveAccent,
  saveNotifyPrefs,
} from "./state";

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

describe("notify prefs persistence", () => {
  it("returns defaults when storage is missing", () => {
    expect(loadNotifyPrefs()).toEqual(DEFAULT_NOTIFY_PREFS);
  });

  it("returns defaults when storage is corrupt", () => {
    localStorage.setItem("ducktape.notifyPrefs", "{not json");
    expect(loadNotifyPrefs()).toEqual(DEFAULT_NOTIFY_PREFS);
  });

  it("fills missing fields from defaults when storage is partial", () => {
    localStorage.setItem("ducktape.notifyPrefs", JSON.stringify({ enabled: false }));
    expect(loadNotifyPrefs()).toEqual({
      ...DEFAULT_NOTIFY_PREFS,
      enabled: false,
    });
  });

  it("falls back field-by-field for wrong-typed or unknown stored values", () => {
    localStorage.setItem(
      "ducktape.notifyPrefs",
      JSON.stringify({
        enabled: "yes",
        mentions: false,
        mutedChannels: ["general", 42],
        extra: false,
      }),
    );
    expect(loadNotifyPrefs()).toEqual({
      ...DEFAULT_NOTIFY_PREFS,
      mentions: false,
      mutedChannels: [],
    });
  });

  it("round-trips saved prefs", () => {
    const saved = {
      enabled: false,
      mentions: true,
      replies: false,
      huddles: true,
      runs: false,
      forge: true,
      governance: false,
      mutedChannels: ["general", "ops"],
    };
    saveNotifyPrefs(saved);
    expect(loadNotifyPrefs()).toEqual(saved);
  });

  it("hydrates the initial state notify prefs from storage", () => {
    saveNotifyPrefs({
      ...DEFAULT_NOTIFY_PREFS,
      governance: false,
      mutedChannels: ["quiet"],
    });
    expect(createInitialState().notifyPrefs).toEqual({
      ...DEFAULT_NOTIFY_PREFS,
      governance: false,
      mutedChannels: ["quiet"],
    });
  });
});
