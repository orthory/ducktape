// The huddle video capability probe: WebCodecs API presence is NOT enough —
// WebKitGTK 2.52 exposes VideoEncoder/VideoDecoder by default yet reports
// isConfigSupported=false when no GStreamer vp8 encoder is registered (and
// encode/decode can diverge). These tests pin the decision table the real
// probe must implement, with the platform WebCodecs globals stubbed.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { probeVideoCapability } from "./video-capability";

/** Install a VideoEncoder/VideoDecoder-like global whose isConfigSupported
 *  resolves to the given support flag (or throws when `throws`). */
const stubCodec = (name: "VideoEncoder" | "VideoDecoder", opts: { supported?: boolean; throws?: boolean }) => {
  const fn = function () {} as unknown as { isConfigSupported: (c: unknown) => Promise<{ supported: boolean }> };
  fn.isConfigSupported = vi.fn(async () => {
    if (opts.throws) throw new Error("codec probe failed");
    return { supported: opts.supported ?? false };
  });
  vi.stubGlobal(name, fn);
};

/** Satisfy the fast encode API pre-gate (VideoFrame + rVFC + getUserMedia). */
const stubEncodeApis = (present: boolean) => {
  vi.stubGlobal("VideoFrame", present ? function () {} : undefined);
  if (present) {
    (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback = () => 0;
    vi.stubGlobal("navigator", { mediaDevices: { getUserMedia: vi.fn() } });
  } else {
    delete (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback;
    vi.stubGlobal("navigator", {});
  }
};

beforeEach(() => {
  stubEncodeApis(true);
});
afterEach(() => {
  vi.unstubAllGlobals();
  delete (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback;
});

describe("probeVideoCapability", () => {
  it("reports no capability when the WebCodecs APIs are absent", async () => {
    vi.stubGlobal("VideoEncoder", undefined);
    vi.stubGlobal("VideoDecoder", undefined);
    stubEncodeApis(false);
    expect(await probeVideoCapability()).toEqual({ canEncode: false, canDecode: false });
  });

  it("reports both false when isConfigSupported says unsupported (API present)", async () => {
    stubCodec("VideoEncoder", { supported: false });
    stubCodec("VideoDecoder", { supported: false });
    expect(await probeVideoCapability()).toEqual({ canEncode: false, canDecode: false });
  });

  it("reflects the encode/decode split: decode yes, encode no", async () => {
    // The real WebKitGTK-2.52 dev-box case: no encoder registers, decoder does.
    stubCodec("VideoEncoder", { supported: false });
    stubCodec("VideoDecoder", { supported: true });
    expect(await probeVideoCapability()).toEqual({ canEncode: false, canDecode: true });
  });

  it("reports both true when the platform supports vp8 encode and decode", async () => {
    stubCodec("VideoEncoder", { supported: true });
    stubCodec("VideoDecoder", { supported: true });
    expect(await probeVideoCapability()).toEqual({ canEncode: true, canDecode: true });
  });

  it("gates encode on getUserMedia even when isConfigSupported is true", async () => {
    stubCodec("VideoEncoder", { supported: true });
    stubCodec("VideoDecoder", { supported: true });
    vi.stubGlobal("navigator", {}); // no mediaDevices → cannot capture → cannot encode
    expect(await probeVideoCapability()).toEqual({ canEncode: false, canDecode: true });
  });

  it("treats an isConfigSupported that throws as unsupported", async () => {
    stubCodec("VideoEncoder", { throws: true });
    stubCodec("VideoDecoder", { throws: true });
    expect(await probeVideoCapability()).toEqual({ canEncode: false, canDecode: false });
  });
});
