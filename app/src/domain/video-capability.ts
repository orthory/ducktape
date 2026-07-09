// Whether this runtime can actually encode / decode huddle video (VP8/WebCodecs).
//
// API PRESENCE IS NOT ENOUGH. WebKitGTK 2.52 exposes VideoEncoder/VideoDecoder by
// default, yet reports `isConfigSupported({codec:'vp8'}).supported === false` when
// no GStreamer vp8 encoder element is registered — and encode vs decode can diverge
// (a box may decode but not encode). So we probe REAL codec support, once, and gate
// the camera (encode) and peer-tile rendering (decode) on the two answers separately.
// The old `typeof VideoEncoder !== 'undefined'` check would light a dead camera toggle.

/** The encoder config the live pipeline uses (mirrors call-session.ts). */
const PROBE_ENCODER_CONFIG = {
  codec: "vp8",
  width: 1280,
  height: 720,
  bitrate: 800_000,
  framerate: 30,
} as const;

/** The decoder config a peer's vp8 stream decodes with. */
const PROBE_DECODER_CONFIG = {
  codec: "vp8",
  codedWidth: 1280,
  codedHeight: 720,
} as const;

export interface VideoCapability {
  /** Can this runtime capture + VP8-encode the local camera (gates the camera UI). */
  canEncode: boolean;
  /** Can this runtime VP8-decode a peer's video (gates peer-tile <canvas> rendering). */
  canDecode: boolean;
  /** Can this runtime screen-share — VP8 encode AND getDisplayMedia present. */
  canScreenShare: boolean;
}

type ConfigProbe = { isConfigSupported?: (config: unknown) => Promise<{ supported?: boolean }> };

/** The synchronous API pre-gate for encoding: the whole capture→encode→send graph
 *  needs VideoFrame, requestVideoFrameCallback (the portable frame pump), and
 *  getUserMedia — not just VideoEncoder. */
const hasEncodeApis = (): boolean =>
  typeof VideoEncoder !== "undefined" &&
  typeof VideoFrame !== "undefined" &&
  typeof (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback ===
    "function" &&
  !!navigator.mediaDevices?.getUserMedia;

const isSupported = async (codec: ConfigProbe | undefined, config: unknown): Promise<boolean> => {
  if (typeof codec?.isConfigSupported !== "function") return false;
  try {
    return !!(await codec.isConfigSupported(config)).supported;
  } catch {
    // A throwing isConfigSupported (hostile/broken) is treated as unsupported.
    return false;
  }
};

/** Probe real VP8 encode/decode support. Async (isConfigSupported is async); run
 *  once at startup and cache the result on the voice slice. */
export const probeVideoCapability = async (): Promise<VideoCapability> => {
  const canEncode = hasEncodeApis()
    ? await isSupported(VideoEncoder as unknown as ConfigProbe, PROBE_ENCODER_CONFIG)
    : false;
  const canDecode =
    typeof VideoDecoder !== "undefined"
      ? await isSupported(VideoDecoder as unknown as ConfigProbe, PROBE_DECODER_CONFIG)
      : false;
  // Screen share reuses the VP8 encode path, so it needs canEncode plus the
  // getDisplayMedia API (WebKitGTK may lack a working portal → hide the button).
  const canScreenShare = canEncode && typeof navigator.mediaDevices?.getDisplayMedia === "function";
  return { canEncode, canDecode, canScreenShare };
};
