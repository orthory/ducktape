// The main-window half of the huddle pop-out bridge. The popped window is a
// pure event mirror: it renders exactly what we emit and sends commands back —
// the store here stays the single source of truth (audio session included).
//
//   ducktape://huddle-state   main → window   full display state, on change +
//                                             replayed on the window's ready
//   ducktape://huddle-cmd     window → main   {op} mapped onto store actions
//   ducktape://huddle-closed  Rust → main     the window died (any way) — the
//                                             in-app card must come back
//
// The pure pieces (payload builder, command mapping) are exported for tests;
// the Tauri emit/listen wiring lives in DucktapeProvider.

import type { Channel } from "../../domain/chat-client";
import { isTauri } from "../../domain/node-bootstrap";
import type { VoiceError } from "../../domain/voice-session";
import { buildParticipants } from "./huddle-roster";
import type { HuddleParticipant } from "./huddle-roster";
import type { VoiceSlice } from "./state";

export const HUDDLE_STATE_EVENT = "ducktape://huddle-state";
export const HUDDLE_CMD_EVENT = "ducktape://huddle-cmd";
export const HUDDLE_CLOSED_EVENT = "ducktape://huddle-closed";

/** Ask Rust to create/show the huddle window. No-op outside Tauri. */
export const openHuddleWindow = (): void => {
  if (!isTauri()) return;
  void import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("huddle_pop_out"))
    .catch(() => {});
};

/** Ask Rust to close the huddle window (idempotent). No-op outside Tauri. */
export const closeHuddleWindow = (): void => {
  if (!isTauri()) return;
  void import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("huddle_pop_in"))
    .catch(() => {});
};

/** Everything the popped window renders — fully-resolved participant rows (the
 *  window has no member records or profile registry of its own), so the same
 *  mute + stale-sweep affordances work there as in the in-app dock. */
export interface HuddleWindowState {
  channelName: string;
  status: VoiceSlice["status"];
  error: VoiceError | null;
  muted: boolean;
  participants: HuddleParticipant[];
}

export type HuddleWindowCmd =
  | { op: "ready" }
  | { op: "set-muted"; muted: boolean }
  | { op: "leave" }
  | { op: "retry" }
  | { op: "sweep"; user: number[] };

/** Project the voice slice + committed roster into the window's display state.
 *  Null when not in a huddle — the caller closes the window instead. `selfNodeHex`
 *  + `now` drive the self/mute/stale computation (staleness is time-based, so the
 *  sender re-pushes on a tick). */
export const buildHuddleWindowState = (
  voice: VoiceSlice,
  channels: Channel[],
  authorNames: Record<string, string>,
  selfNodeHex: string,
  now: number,
): HuddleWindowState | null => {
  if (!voice.channelId) return null;
  const channel = channels.find((c) => c.id === voice.channelId);
  return {
    channelName: channel?.name ?? voice.channelId,
    status: voice.status,
    error: voice.error,
    muted: voice.muted,
    participants: buildParticipants({
      roster: channel?.huddle ?? [],
      peers: voice.peers,
      selfNodeHex,
      authorNames,
      selfMuted: voice.muted,
      sessionStartMs: voice.sessionStartMs,
      now,
    }),
  };
};

/** Map a window command onto the store's existing huddle actions. Unknown ops
 *  are ignored — an old window build must not crash a newer main. */
export const applyHuddleWindowCmd = (
  cmd: HuddleWindowCmd,
  actions: {
    setHuddleMuted(muted: boolean): void;
    leaveHuddle(): void;
    joinHuddle(channelId: string): void;
    sweepHuddle(channelId: string, user: number[]): void;
  },
  currentChannelId: string | null,
): void => {
  switch (cmd.op) {
    case "set-muted":
      actions.setHuddleMuted(cmd.muted);
      return;
    case "leave":
      actions.leaveHuddle();
      return;
    case "retry":
      if (currentChannelId) actions.joinHuddle(currentChannelId);
      return;
    case "sweep":
      if (currentChannelId) actions.sweepHuddle(currentChannelId, cmd.user);
      return;
    case "ready":
      // handled by the wiring (replays the current state); nothing to do here.
      return;
  }
};
