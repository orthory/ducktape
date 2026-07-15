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

// Native notifications belong to Iced. The static web twin intentionally
// exposes an inert implementation so shared console components stay usable.
export const configure = async (_config: NotifyConfigPayload): Promise<void> => {};

export const markSeen = async (): Promise<void> => {};

export const onUnread = async (
  _cb: (unread: number) => void,
): Promise<() => void> => noop;

/** One entry of the notifier's recent ring (the bell dropdown's rows). */
export interface NotifyItem {
  category: "mention" | "reply" | "huddle" | "run" | "forge" | "governance";
  title: string;
  body: string;
  channelId: string | null;
  messageId: string | null;
  at: number;
}

/** The bell's boot snapshot: unread rides along because the engine's
 *  boot-time badge event fires before the webview subscribes. */
export interface NotifySnapshot {
  unread: number;
  items: NotifyItem[];
}

export const recent = async (): Promise<NotifySnapshot> => ({ unread: 0, items: [] });

export const onItem = async (
  _cb: (item: NotifyItem) => void,
): Promise<() => void> => noop;
