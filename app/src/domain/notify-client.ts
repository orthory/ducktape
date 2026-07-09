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

const warnFailure = (functionName: string, impact: string, error: unknown): void => {
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
