import { isTauri } from "./node-bootstrap";
import type { NotifyPrefs } from "../console/store/state";

export interface NotifyConfigPayload {
  nodeUrl: string | null;
  selfUserKeyHex: string | null;
  selfNodeKeysHex: string[];
  focusedChannel: string | null;
  mainWindowFocused: boolean;
  authorNames: Record<string, string>;
  prefs: NotifyPrefs;
}

const noop = (): void => {};

// configure() re-fires on every channel switch / focus / blur, so a
// persistently failing command would otherwise warn on every keystroke of
// normal use. Warn once per distinct (function, error) pair — a NEW failure
// mode still surfaces, a repeated one stays quiet.
const warnedFailures = new Set<string>();

const warnFailure = (functionName: string, impact: string, error: unknown): void => {
  const key = `${functionName}:${String(error)}`;
  if (warnedFailures.has(key)) return;
  warnedFailures.add(key);
  console.warn(`[notify-client] ${functionName} failed; ${impact}`, error);
};

export const configure = async (config: NotifyConfigPayload): Promise<void> => {
  if (!isTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("notify_configure", { config });
  } catch (error) {
    warnFailure("configure", "notification configuration was not applied", error);
  }
};

export const markSeen = async (): Promise<void> => {
  if (!isTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("notify_mark_seen");
  } catch (error) {
    warnFailure("markSeen", "notification read state was not updated", error);
  }
};

export const onUnread = async (
  cb: (unread: number) => void,
): Promise<() => void> => {
  if (!isTauri()) return noop;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    return await listen<{ unread: number }>("ducktape://notify-unread", (event) => {
      cb(event.payload.unread);
    });
  } catch (error) {
    warnFailure(
      "onUnread",
      "unread stream is inactive; returning a no-op unlisten",
      error,
    );
    return noop;
  }
};

/** One entry of the notifier's recent ring (the bell dropdown's rows). */
export interface NotifyItem {
  category: "mention" | "reply" | "huddle" | "run" | "forge" | "governance";
  title: string;
  body: string;
  channelId: string | null;
  at: number;
}

/** The bell's boot snapshot: unread rides along because the engine's
 *  boot-time badge event fires before the webview subscribes. */
export interface NotifySnapshot {
  unread: number;
  items: NotifyItem[];
}

export const recent = async (): Promise<NotifySnapshot> => {
  if (!isTauri()) return { unread: 0, items: [] };
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    // The IPC payload is untyped at runtime — a null/malformed reply (e.g. a
    // stubbed invoke in tests) must read as empty, not crash the bell.
    const snapshot = await invoke<NotifySnapshot | null>("notify_recent");
    return snapshot && Array.isArray(snapshot.items)
      ? snapshot
      : { unread: 0, items: [] };
  } catch (error) {
    warnFailure("recent", "notification history is unavailable", error);
    return { unread: 0, items: [] };
  }
};

export const onItem = async (
  cb: (item: NotifyItem) => void,
): Promise<() => void> => {
  if (!isTauri()) return noop;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    return await listen<NotifyItem>("ducktape://notify-item", (event) => {
      cb(event.payload);
    });
  } catch (error) {
    warnFailure(
      "onItem",
      "live notification items are inactive; returning a no-op unlisten",
      error,
    );
    return noop;
  }
};
