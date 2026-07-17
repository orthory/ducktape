import { describe, expect, it, vi } from "vitest";

import { DEFAULT_NOTIFY_PREFS } from "../console/store/state";
import {
  configure,
  markSeen,
  onItem,
  onUnread,
  recent,
  type NotifyConfigPayload,
} from "./notify-client";

const emptyConfig = (): NotifyConfigPayload => ({
  nodeUrl: null,
  selfUserKeyHex: null,
  selfNodeKeysHex: [],
  focusedChannel: null,
  mainWindowFocused: false,
  authorNames: {},
  prefs: DEFAULT_NOTIFY_PREFS,
});

describe("web notification fallback", () => {
  it("keeps configuration and read-state updates inert", async () => {
    await expect(configure(emptyConfig())).resolves.toBeUndefined();
    await expect(markSeen()).resolves.toBeUndefined();
  });

  it("returns an empty initial snapshot", async () => {
    await expect(recent()).resolves.toEqual({ unread: 0, items: [] });
  });

  it("returns safe no-op subscriptions", async () => {
    const unread = vi.fn();
    const item = vi.fn();
    const stopUnread = await onUnread(unread);
    const stopItem = await onItem(item);

    expect(() => stopUnread()).not.toThrow();
    expect(() => stopItem()).not.toThrow();
    expect(unread).not.toHaveBeenCalled();
    expect(item).not.toHaveBeenCalled();
  });
});
