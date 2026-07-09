// Huddle device selection: enumerate the mic / camera / speaker options, and
// persist the user's choice so it survives a rejoin. Pure wrappers over
// navigator.mediaDevices + localStorage, so the picker logic is unit-tested
// without real hardware. Applying a choice to the live session lives in
// call-session.ts (setDevices); this module only lists + remembers.

/** One selectable device — the id we constrain on, plus a human label. */
export interface MediaDeviceOption {
  deviceId: string;
  label: string;
}

export interface HuddleDevices {
  mics: MediaDeviceOption[];
  cameras: MediaDeviceOption[];
  speakers: MediaDeviceOption[];
}

/** The remembered selection (undefined = system default). */
export interface DevicePrefs {
  micId?: string;
  cameraId?: string;
  speakerId?: string;
}

const PREFS_KEY = "ducktape.huddle.devices";

const KIND: Record<MediaDeviceKind, keyof HuddleDevices | undefined> = {
  audioinput: "mics",
  videoinput: "cameras",
  audiooutput: "speakers",
};

/** A stable, human label for a device — the OS label when granted, else a short
 *  fallback keyed by id (labels are empty until a getUserMedia grant). */
const labelFor = (d: MediaDeviceInfo, index: number, kind: string): string =>
  d.label || `${kind} ${index + 1}${d.deviceId ? ` (${d.deviceId.slice(0, 4)})` : ""}`;

/** List the mic / camera / speaker options. Empty lists (not a throw) when the
 *  API is absent or enumeration fails, so the caller can just hide the picker. */
export const enumerateHuddleDevices = async (): Promise<HuddleDevices> => {
  const empty: HuddleDevices = { mics: [], cameras: [], speakers: [] };
  if (!navigator.mediaDevices?.enumerateDevices) return empty;
  let devices: MediaDeviceInfo[];
  try {
    devices = await navigator.mediaDevices.enumerateDevices();
  } catch {
    return empty;
  }
  const out: HuddleDevices = { mics: [], cameras: [], speakers: [] };
  const seen = { audioinput: 0, videoinput: 0, audiooutput: 0 };
  for (const d of devices) {
    const bucket = KIND[d.kind];
    if (!bucket || !d.deviceId) continue;
    const n = seen[d.kind as keyof typeof seen]++;
    out[bucket].push({ deviceId: d.deviceId, label: labelFor(d, n, d.kind.replace("input", "").replace("output", "")) });
  }
  return out;
};

/** Whether the runtime can route audio to a chosen speaker — AudioContext or
 *  HTMLMediaElement setSinkId (Chromium has it; WebKitGTK does not → hide it). */
export const canSelectSpeaker = (): boolean =>
  typeof AudioContext !== "undefined" &&
  (typeof (AudioContext.prototype as { setSinkId?: unknown }).setSinkId === "function" ||
    typeof (HTMLMediaElement.prototype as { setSinkId?: unknown }).setSinkId === "function");

/** Load the remembered selection (never throws — a corrupt/absent blob → {}). */
export const loadDevicePrefs = (): DevicePrefs => {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as DevicePrefs;
    return {
      micId: typeof parsed.micId === "string" ? parsed.micId : undefined,
      cameraId: typeof parsed.cameraId === "string" ? parsed.cameraId : undefined,
      speakerId: typeof parsed.speakerId === "string" ? parsed.speakerId : undefined,
    };
  } catch {
    return {};
  }
};

/** Persist the selection (best-effort — a storage failure is non-fatal). */
export const saveDevicePrefs = (prefs: DevicePrefs): void => {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // storage disabled / full — the choice just won't survive a reload.
  }
};
