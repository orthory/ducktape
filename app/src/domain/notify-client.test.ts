import { afterEach, describe, expect, it, vi } from "vitest";

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  coreImported: vi.fn(),
  eventImported: vi.fn(),
}));

const {
  invoke: invokeMock,
  listen: listenMock,
  coreImported: coreImportedMock,
  eventImported: eventImportedMock,
} = tauriMocks;

vi.mock("@tauri-apps/api/core", () => {
  tauriMocks.coreImported();
  return { invoke: tauriMocks.invoke };
});
vi.mock("@tauri-apps/api/event", () => {
  tauriMocks.eventImported();
  return { listen: tauriMocks.listen };
});

import { DEFAULT_NOTIFY_PREFS } from "../console/store/state";
import {
  configure,
  markSeen,
  onUnread,
  type NotifyConfigPayload,
} from "./notify-client";

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

const emptyConfig = (): NotifyConfigPayload => ({
  nodeUrl: null,
  selfUserKeyHex: null,
  selfNodeKeysHex: [],
  focusedChannel: null,
  mainWindowFocused: false,
  authorNames: {},
  prefs: DEFAULT_NOTIFY_PREFS,
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.restoreAllMocks();
  vi.resetAllMocks();
});

describe("notify client", () => {
  it("no-ops in a web build without importing the Tauri modules", async () => {
    const cb = vi.fn();

    await expect(configure(emptyConfig())).resolves.toBeUndefined();
    await expect(markSeen()).resolves.toBeUndefined();
    const unlisten = await onUnread(cb);
    unlisten();

    expect(coreImportedMock).not.toHaveBeenCalled();
    expect(eventImportedMock).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(listenMock).not.toHaveBeenCalled();
    expect(cb).not.toHaveBeenCalled();
  });

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
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error = new Error("command notify_configure not found");
    invokeMock.mockRejectedValue(error);

    await expect(configure(emptyConfig())).resolves.toBeUndefined();

    expect(warn).toHaveBeenCalledWith(expect.stringContaining("configure"), error);
  });

  it("warns about an unexpected configure failure and still resolves", async () => {
    markTauri();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error = new Error("IPC transport disconnected");
    invokeMock.mockRejectedValue(error);

    await expect(configure(emptyConfig())).resolves.toBeUndefined();

    expect(warn).toHaveBeenCalledWith(expect.stringContaining("configure"), error);
  });

  it("resolves when markSeen command is not available yet", async () => {
    markTauri();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error = new Error("command notify_mark_seen not found");
    invokeMock.mockRejectedValue(error);

    await expect(markSeen()).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledWith("notify_mark_seen");
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("markSeen"), error);
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

  it("warns when the unread stream fails and returns a no-op unlisten", async () => {
    markTauri();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error = new Error("event subscription failed");
    const cb = vi.fn();
    listenMock.mockRejectedValue(error);

    const unlisten = await onUnread(cb);
    expect(() => unlisten()).not.toThrow();

    expect(warn).toHaveBeenCalledWith(
      expect.stringMatching(/onUnread.*unread stream is inactive.*no-op unlisten/i),
      error,
    );
    expect(cb).not.toHaveBeenCalled();
  });
});
