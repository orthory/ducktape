import { afterEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { DEFAULT_NOTIFY_PREFS } from "../console/store/state";
import { configure, onUnread, type NotifyConfigPayload } from "./notify-client";

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe("notify client", () => {
  it("passes configure payload through verbatim", async () => {
    markTauri();
    invokeMock.mockResolvedValue(undefined);
    const config: NotifyConfigPayload = {
      nodeUrl: "http://127.0.0.1:8844",
      selfUserKeyHex: "user-a",
      selfNodeKeysHex: ["node-a", "node-b"],
      focusedChannel: "general",
      mainWindowFocused: true,
      authorNames: { "user-a": "Ada" },
      prefs: {
        ...DEFAULT_NOTIFY_PREFS,
        replies: false,
        mutedChannels: ["quiet"],
      },
    };

    await expect(configure(config)).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("notify_configure", { config });
  });

  it("resolves when configure command is not available yet", async () => {
    markTauri();
    invokeMock.mockRejectedValue(new Error("command notify_configure not found"));

    await expect(
      configure({
        nodeUrl: null,
        selfUserKeyHex: null,
        selfNodeKeysHex: [],
        focusedChannel: null,
        mainWindowFocused: false,
        authorNames: {},
        prefs: DEFAULT_NOTIFY_PREFS,
      }),
    ).resolves.toBeUndefined();
  });

  it("unwraps unread events and returns a working unlisten", async () => {
    markTauri();
    const unlisten = vi.fn();
    const cb = vi.fn();
    listenMock.mockImplementation(async (_eventName, handler) => {
      handler({ payload: { unread: 7 } });
      return unlisten;
    });

    const returned = await onUnread(cb);
    returned();

    expect(listenMock).toHaveBeenCalledWith("ducktape://notify-unread", expect.any(Function));
    expect(cb).toHaveBeenCalledWith(7);
    expect(unlisten).toHaveBeenCalled();
  });
});
