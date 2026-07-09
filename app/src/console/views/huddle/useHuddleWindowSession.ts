// The popped-out huddle window's OWN media session. Unlike the old mirror, the
// window runs a real CallSession — mic, camera, screen share, peer decode —
// dialed straight at the local node, seeded by the context the main window
// pushes (nodeUrl + channelId + raw roster + capability) and by the same
// persisted device prefs main uses (shared localStorage). Main keeps consensus;
// this controller keeps everything media/ephemeral (peers, mute, camera/share,
// speaking, status) and projects the roster with its OWN beacons.
//
// StrictMode note: a CallSession cannot restart after stop(), and StrictMode
// double-invokes effects — so the session is created INSIDE the effect and
// stopped in its cleanup (a fresh instance per mount / channel / retry), never
// reused across a stop().

import { useCallback, useEffect, useRef, useState } from "react";

import { createCallSession } from "../../../domain/call-session";
import type { CallEvent, CallSession } from "../../../domain/call-session";
import { keyHex } from "../../../domain/chat-client";
import { loadDevicePrefs } from "../../../domain/media-devices";
import { callSocketUrl } from "../../../domain/transport";
import { huddleRecipients } from "../../../domain/voice-session";
import { buildParticipants } from "../../store/huddle-roster";
import type { HuddleParticipant, PeerBeacon } from "../../store/huddle-roster";
import type { HuddleContext } from "../../store/huddle-window";
import type { HuddleStatus } from "../chat/HuddleCard";

export interface WindowSessionView {
  channelName: string;
  status: HuddleStatus;
  muted: boolean;
  cameraOn: boolean;
  sharing: boolean;
  canEncode: boolean;
  canDecode: boolean;
  canScreenShare: boolean;
  /** Transient camera/screen acquire failure — surfaced for a few seconds. */
  mediaNote: "camera-failed" | "screen-failed" | null;
  participants: HuddleParticipant[];
  peers: Record<string, PeerBeacon>;
  memberNodes: Record<string, string>;
  setMuted(m: boolean): void;
  setCamera(on: boolean): void;
  setScreenShare(on: boolean): void;
  bindPreview(el: HTMLVideoElement | null): void;
  bindTile(nodeHex: string, el: HTMLCanvasElement | null): void;
}

/** Own the window's media session against `ctx`. `onMediaEnded` fires when the
 *  session dies (hard error or replaced) — the window closes itself so main
 *  re-takes and the call is never stranded in a dead float. `makeSession` is
 *  injectable for tests. Returns null when there is no context yet. */
export function useHuddleWindowSession(
  ctx: HuddleContext | null,
  onMediaEnded: (reason: "closed" | "error") => void,
  makeSession: (cb: (e: CallEvent) => void) => CallSession = createCallSession,
): WindowSessionView | null {
  const [muted, setMutedState] = useState(true);
  const [cameraOn, setCameraOnState] = useState(false);
  const [sharing, setSharingState] = useState(false);
  const [status, setStatus] = useState<HuddleStatus>("connecting");
  const [peers, setPeers] = useState<Record<string, PeerBeacon>>({});
  const [speaking, setSpeaking] = useState(false);
  const [mediaNote, setMediaNote] = useState<"camera-failed" | "screen-failed" | null>(null);
  const [sessionStartMs, setSessionStartMs] = useState<number | null>(null);
  const [nowTick, setNowTick] = useState(() => Date.now());

  const sessionRef = useRef<CallSession | null>(null);
  const mediaNoteTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const endedRef = useRef(onMediaEnded);
  endedRef.current = onMediaEnded;

  const channelId = ctx?.channelId ?? null;
  const nodeUrl = ctx?.nodeUrl ?? null;
  const seedMuted = ctx?.seedMuted ?? true;
  const roster = ctx?.roster ?? [];
  const selfNodeHex = ctx?.selfNodeHex ?? "";

  // Start a fresh session per (channel, node, retry). seedMuted is read from the
  // closure at (re)start — a later seed change must NOT restart a live session.
  useEffect(() => {
    if (!channelId || !nodeUrl) return;
    const onEvent = (e: CallEvent): void => {
      if (e.kind === "peerBeacon") {
        setPeers((p) => ({
          ...p,
          [e.peer]: { muted: e.muted, cameraOn: e.cameraOn, sharing: e.sharing, atMs: e.atMs },
        }));
        return;
      }
      if (e.kind === "selfSpeaking") {
        setSpeaking(e.speaking);
        return;
      }
      if (e.kind === "selfVideo") {
        // Authoritative lane state (a failed acquire, encoder death, the
        // browser's own "Stop sharing") — mirrors both camera and share.
        setCameraOnState(e.cameraOn);
        setSharingState(e.sharing);
        return;
      }
      if (e.kind === "mediaNote") {
        // Same transient surfacing as the dock: say why the toggle snapped back.
        if (mediaNoteTimer.current !== null) clearTimeout(mediaNoteTimer.current);
        setMediaNote(e.note);
        mediaNoteTimer.current = setTimeout(() => {
          mediaNoteTimer.current = null;
          setMediaNote(null);
        }, 5_000);
        return;
      }
      // Any terminal end (hard error or replaced) ends this float — the window
      // closes and main re-takes, so recovery is "fall back to the dock", not an
      // in-window retry (there is deliberately no retry control here).
      if (e.status === "closed" || e.status === "error") {
        endedRef.current(e.status);
        return;
      }
      setStatus(e.status);
    };
    const session = makeSession(onEvent);
    sessionRef.current = session;
    setStatus("connecting");
    setPeers({});
    setCameraOnState(false);
    setSharingState(false);
    setSpeaking(false);
    setMediaNote(null);
    setMutedState(seedMuted);
    setSessionStartMs(Date.now());
    session.setMuted(seedMuted);
    // The window session honors the same persisted device choices as main —
    // same origin, same localStorage — so a pop-out never silently reverts to
    // the system-default mic/camera.
    session.setDevices(loadDevicePrefs());
    session.start(callSocketUrl(nodeUrl, channelId));
    return () => {
      session.stop();
      sessionRef.current = null;
    };
    // seedMuted intentionally excluded — see comment above.

  }, [channelId, nodeUrl, makeSession]);

  // Re-push the fan-out set whenever it changes (roster edits). Runs after the
  // start effect (declared first), so the session exists on the initial mount.
  const recipientsFp = huddleRecipients(roster, selfNodeHex).join(",");
  useEffect(() => {
    if (!sessionRef.current) return;
    sessionRef.current.setRecipients(recipientsFp ? recipientsFp.split(",") : []);
  }, [recipientsFp]);

  // Staleness is time-based → tick once a second so members cross the threshold.
  useEffect(() => {
    const id = setInterval(() => setNowTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const setMuted = useCallback((m: boolean) => {
    sessionRef.current?.setMuted(m);
    setMutedState(m);
  }, []);
  const setCamera = useCallback((on: boolean) => {
    sessionRef.current?.setCamera(on);
    // Camera XOR screen: the session swaps the lane; mirror it optimistically
    // (the settled selfVideo event corrects a failed acquire).
    setCameraOnState(on);
    if (on) setSharingState(false);
  }, []);
  const setScreenShare = useCallback((on: boolean) => {
    sessionRef.current?.setScreenShare(on);
    setSharingState(on);
    if (on) setCameraOnState(false);
  }, []);
  const bindPreview = useCallback((el: HTMLVideoElement | null) => sessionRef.current?.bindPreview(el), []);
  const bindTile = useCallback(
    (nodeHex: string, el: HTMLCanvasElement | null) => sessionRef.current?.bindTile(nodeHex, el),
    [],
  );

  if (!ctx) return null;

  const participants = buildParticipants({
    roster,
    peers,
    selfNodeHex,
    authorNames: ctx.authorNames,
    selfMuted: muted,
    selfSpeaking: speaking,
    sessionStartMs,
    now: nowTick,
  });
  const memberNodes = Object.fromEntries(roster.map((m) => [keyHex(m.user), keyHex(m.node)]));

  return {
    channelName: ctx.channelName,
    status,
    muted,
    cameraOn,
    sharing,
    canEncode: ctx.canEncode,
    canDecode: ctx.canDecode,
    canScreenShare: ctx.canScreenShare,
    mediaNote,
    participants,
    peers,
    memberNodes,
    setMuted,
    setCamera,
    setScreenShare,
    bindPreview,
    bindTile,
  };
}
