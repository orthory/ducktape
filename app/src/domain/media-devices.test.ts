import { afterEach, describe, expect, it, vi } from "vitest";

import { canSelectSpeaker, enumerateHuddleDevices, loadDevicePrefs, saveDevicePrefs } from "./media-devices";

const device = (kind: MediaDeviceKind, deviceId: string, label = ""): MediaDeviceInfo =>
  ({ kind, deviceId, label, groupId: "", toJSON: () => ({}) }) as MediaDeviceInfo;

afterEach(() => {
  vi.unstubAllGlobals();
  try {
    localStorage.clear();
  } catch {
    // no-op
  }
});

describe("enumerateHuddleDevices", () => {
  it("buckets devices by kind and keeps ids + labels", async () => {
    vi.stubGlobal("navigator", {
      mediaDevices: {
        enumerateDevices: vi.fn(async () => [
          device("audioinput", "mic-1", "Built-in Mic"),
          device("videoinput", "cam-1", "FaceTime HD"),
          device("audiooutput", "spk-1", "Speakers"),
          device("audioinput", "mic-2", "USB Mic"),
        ]),
      },
    });
    const d = await enumerateHuddleDevices();
    expect(d.mics.map((m) => m.deviceId)).toEqual(["mic-1", "mic-2"]);
    expect(d.mics[0].label).toBe("Built-in Mic");
    expect(d.cameras).toEqual([{ deviceId: "cam-1", label: "FaceTime HD" }]);
    expect(d.speakers).toEqual([{ deviceId: "spk-1", label: "Speakers" }]);
  });

  it("labels unlabelled devices (pre-permission) with a stable fallback", async () => {
    vi.stubGlobal("navigator", {
      mediaDevices: { enumerateDevices: vi.fn(async () => [device("audioinput", "mic-abcdef", "")]) },
    });
    const d = await enumerateHuddleDevices();
    expect(d.mics[0].label).toMatch(/audio 1/i);
  });

  it("returns empty lists when the API is absent or throws", async () => {
    vi.stubGlobal("navigator", {});
    expect(await enumerateHuddleDevices()).toEqual({ mics: [], cameras: [], speakers: [] });
    vi.stubGlobal("navigator", {
      mediaDevices: {
        enumerateDevices: vi.fn(async () => {
          throw new Error("blocked");
        }),
      },
    });
    expect(await enumerateHuddleDevices()).toEqual({ mics: [], cameras: [], speakers: [] });
  });
});

describe("canSelectSpeaker", () => {
  it("is true when AudioContext.setSinkId exists", () => {
    vi.stubGlobal("AudioContext", function () {} as unknown);
    (AudioContext.prototype as { setSinkId?: unknown }).setSinkId = () => Promise.resolve();
    expect(canSelectSpeaker()).toBe(true);
    delete (AudioContext.prototype as { setSinkId?: unknown }).setSinkId;
  });

  it("is false when no setSinkId is available", () => {
    vi.stubGlobal("AudioContext", function () {} as unknown);
    delete (AudioContext.prototype as { setSinkId?: unknown }).setSinkId;
    delete (HTMLMediaElement.prototype as { setSinkId?: unknown }).setSinkId;
    expect(canSelectSpeaker()).toBe(false);
  });
});

describe("device prefs persistence", () => {
  it("round-trips through localStorage", () => {
    saveDevicePrefs({ micId: "mic-1", speakerId: "spk-2" });
    expect(loadDevicePrefs()).toEqual({ micId: "mic-1", cameraId: undefined, speakerId: "spk-2" });
  });

  it("returns {} for absent or corrupt storage", () => {
    expect(loadDevicePrefs()).toEqual({});
    localStorage.setItem("ducktape.huddle.devices", "{not json");
    expect(loadDevicePrefs()).toEqual({});
  });
});
