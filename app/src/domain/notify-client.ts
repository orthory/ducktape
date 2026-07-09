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

export const configure = async (config: NotifyConfigPayload): Promise<void> => {
  if (!isTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("notify_configure", { config });
  } catch {
    // Non-tauri/test environments and Phase B's missing Rust command both no-op.
  }
};

export const markSeen = async (): Promise<void> => {
  if (!isTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("notify_mark_seen");
  } catch {
    // Non-tauri/test environments and Phase B's missing Rust command both no-op.
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
  } catch {
    return noop;
  }
};
