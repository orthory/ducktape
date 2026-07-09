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
