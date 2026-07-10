// The pure huddle roster projection — turns the committed roster
// (`channel.huddle`) plus the ephemeral 1 Hz beacons (`voice.peers`) into
// per-member display rows. Shared by the in-app dock (Huddle.tsx) and the
// popped-out window (via huddle-window.ts) so mute state and the stale-member
// sweep work in EVERY huddle — audio-only included — not only when video tiles
// are showing. Runtime-free: unit-tested directly.

import { authorName, keyHex } from "../../domain/chat-client";
import type { HuddleMember } from "../../domain/chat-client";

/** A peer's ephemeral call state from the hub's 1 Hz beacons (see VoiceSlice). */
export type PeerBeacon = { muted: boolean; cameraOn: boolean; sharing: boolean; atMs: number };

/** After this much beacon silence a huddle member is offered for sweeping. */
export const STALE_BEACON_MS = 10_000;

/** Whether a member's beacon is stale enough to offer the sweep chip. A member
 *  WITH a beacon is stale once it has been silent past STALE_BEACON_MS. A member
 *  we've NEVER heard from is stale only after our own session has been up that
 *  long — a fresh peer's first beacon takes ~1 s to arrive, so we mustn't offer
 *  to evict the whole roster the instant we join. */
export const isBeaconStale = (
  beacon: PeerBeacon | undefined,
  sessionStartMs: number,
  now: number,
): boolean =>
  beacon ? now - beacon.atMs > STALE_BEACON_MS : now - sessionStartMs > STALE_BEACON_MS;

/** One roster row as the huddle card renders it. */
export interface HuddleParticipant {
  /** The member's user key as hex — React key + identity. */
  key: string;
  /** Resolved account display name (identity, else the readable fallback). */
  name: string;
  /** Known-muted: a peer whose beacon says muted, or ourselves when muted. A
   *  member we have no beacon from yet is NOT shown muted (unknown != muted). */
  muted: boolean;
  /** Removable: a non-self member whose beacon has gone silent past the window. */
  stale: boolean;
  /** This row is us (matched by node key). Never stale/removable. */
  isSelf: boolean;
  /** Above the speaking threshold right now. Only the self row can be true —
   *  peer speaking isn't derivable client-side (audio is server-mixed). */
  speaking: boolean;
  /** The member's user key bytes — carried for SweepHuddle. */
  user: number[];
}

export interface BuildParticipantsParams {
  roster: HuddleMember[];
  peers: Record<string, PeerBeacon>;
  /** Our own node key hex (already lowercase). Empty when voice is unavailable. */
  selfNodeHex: string;
  authorNames: Record<string, string>;
  /** Our own mute state — self mute is authoritative locally, not beaconed to us. */
  selfMuted: boolean;
  /** Our own speaking state (mic above threshold) — only the self row uses it. */
  selfSpeaking: boolean;
  /** When our session started (staleness baseline for never-beaconed members).
   *  Null → treat as `now` so nothing is stale before a session is established. */
  sessionStartMs: number | null;
  now: number;
}

/** Project the roster into display rows in roster order. */
export const buildParticipants = ({
  roster,
  peers,
  selfNodeHex,
  authorNames,
  selfMuted,
  selfSpeaking,
  sessionStartMs,
  now,
}: BuildParticipantsParams): HuddleParticipant[] => {
  const self = selfNodeHex.toLowerCase();
  const start = sessionStartMs ?? now;
  return roster.map((m) => {
    const nodeHex = keyHex(m.node);
    const isSelf = nodeHex === self;
    const beacon = peers[nodeHex];
    return {
      key: keyHex(m.user),
      name: authorName({ user: m.user }, authorNames),
      muted: isSelf ? selfMuted : !!beacon?.muted,
      stale: !isSelf && isBeaconStale(beacon, start, now),
      isSelf,
      speaking: isSelf && selfSpeaking,
      user: m.user,
    };
  });
};
