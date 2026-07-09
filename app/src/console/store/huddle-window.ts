// The main-window half of the huddle pop-out bridge. In PR-B the popped window
// is no longer a pure mirror — it runs its OWN media session (WS + mic + camera
// + decode). Main stays the single source of truth for CONSENSUS (roster) and
// hands the media session to the window on pop-out, taking it back on pop-in.
//
//   ducktape://huddle-context  main → window   node context + raw roster the
//                                              window needs to run its session +
//                                              render; replayed on `ready` and
//                                              re-pushed when the roster changes
//   ducktape://huddle-cmd      window → main   {op} — consensus-affecting acts
//                                              (leave/sweep); the window owns its
//                                              own mute/camera locally
//   ducktape://huddle-closed   Rust → main     the window died (any way, incl.
//                                              the window closing itself on a
//                                              media failure) — main re-takes the
//                                              media session so the call is never
//                                              stranded in a dead float
//
// The pure pieces (context builder, command mapping) are exported for tests;
// the Tauri emit/listen wiring lives in DucktapeProvider.

import type { Channel, HuddleMember } from "../../domain/chat-client";
import { isTauri } from "../../domain/node-bootstrap";
import type { VideoCapability } from "../../domain/video-capability";
import type { VoiceSlice } from "./state";

export const HUDDLE_CONTEXT_EVENT = "ducktape://huddle-context";
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

/** Everything the popped window needs to RUN its own session + render its roster.
 *  The roster is raw (not pre-projected): the window owns the beacons now, so it
 *  runs buildParticipants itself against its OWN peers. */
export interface HuddleContext {
  channelName: string;
  channelId: string;
  /** The daemon base url — the window dials callSocketUrl(nodeUrl, channelId). */
  nodeUrl: string;
  /** Our own node key hex (already lowercase) — self match + fan-out exclusion. */
  selfNodeHex: string;
  canEncode: boolean;
  canDecode: boolean;
  authorNames: Record<string, string>;
  roster: HuddleMember[];
  /** Main's mute at handoff — the window seeds its session from this. */
  seedMuted: boolean;
}

export type HuddleWindowCmd =
  | { op: "ready" }
  | { op: "leave" }
  | { op: "sweep"; user: number[] }
  // The window owns mute locally, but it reports each change so main can re-take
  // with the SAME mute on pop-in (call continuity — no silent re-mute).
  | { op: "mute"; muted: boolean };

/** Build the context to push to the window. Null when not in a huddle (the
 *  caller closes the window instead) or the node url is unresolved. */
export const buildHuddleContext = (
  voice: VoiceSlice,
  channels: Channel[],
  authorNames: Record<string, string>,
  nodeUrl: string | null,
  selfNodeHex: string,
  cap: VideoCapability,
): HuddleContext | null => {
  if (!voice.channelId || !nodeUrl) return null;
  const channel = channels.find((c) => c.id === voice.channelId);
  return {
    channelName: channel?.name ?? voice.channelId,
    channelId: voice.channelId,
    nodeUrl,
    selfNodeHex: selfNodeHex.toLowerCase(),
    canEncode: cap.canEncode,
    canDecode: cap.canDecode,
    authorNames,
    roster: channel?.huddle ?? [],
    seedMuted: voice.muted,
  };
};

/** Map a window command onto the main store. Only consensus-affecting acts cross
 *  the bridge (leave/sweep) — the window owns mute/camera locally, and a media
 *  failure closes the window (→ huddle-closed → main re-takes). Unknown ops are
 *  ignored — an old window build must not crash a newer main. */
export const applyHuddleWindowCmd = (
  cmd: HuddleWindowCmd,
  actions: {
    leaveHuddle(): void;
    sweepHuddle(channelId: string, user: number[]): void;
    noteHuddleMuted(muted: boolean): void;
  },
  currentChannelId: string | null,
): void => {
  switch (cmd.op) {
    case "leave":
      actions.leaveHuddle();
      return;
    case "sweep":
      if (currentChannelId) actions.sweepHuddle(currentChannelId, cmd.user);
      return;
    case "mute":
      actions.noteHuddleMuted(cmd.muted);
      return;
    case "ready":
      // handled by the wiring (replays the current context); nothing to do here.
      return;
  }
};
