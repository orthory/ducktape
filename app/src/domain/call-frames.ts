// The binary framing of /v1/call/ws — mirrors bin/noded/src/lib.rs (WS_TAG_*).
// Little-endian on THIS (browser ↔ node) leg, so a DataView reads it directly;
// the mesh leg between nodes stays big-endian and never reaches here.
//
// Tag byte layouts (first byte selects the frame kind):
//   audio    [0x01][pcm i16 LE …]                    — both directions
//   captured [0x02][flags u8][ts_ms u32 LE][vp8 …]   — client → server only
//   peer     [0x03][flags u8][ts_ms u32 LE][key 32][vp8 …] — server → client
// `flags` bit 0 (WS_FLAG_KEYFRAME) marks a decoder sync point.

export const WS_TAG_AUDIO = 0x01;
export const WS_TAG_VIDEO_CAPTURED = 0x02;
export const WS_TAG_VIDEO_PEER = 0x03;
const WS_FLAG_KEYFRAME = 0x01;
const CAPTURED_HEADER = 6; // tag + flags + ts_ms(u32 LE)
const PEER_HEADER = 38; // tag + flags + ts_ms(u32 LE) + peer key(32 raw)

/** Prefix a captured 20 ms pcm frame with the audio tag. The pcm's own
 *  byteOffset/byteLength are honoured, so a subarray view ships only its own
 *  samples (never the whole backing buffer). */
export const encodeAudioFrame = (pcm: Int16Array): ArrayBuffer => {
  const out = new Uint8Array(1 + pcm.length * 2);
  out[0] = WS_TAG_AUDIO;
  out.set(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), 1);
  return out.buffer;
};

/** Encode one captured (VP8) camera frame: `[0x02][flags][ts_ms u32 LE][data]`.
 *  `tsMs` is coerced to a u32 (the wire width) before write. */
export const encodeCapturedVideo = (
  keyframe: boolean,
  tsMs: number,
  data: Uint8Array,
): ArrayBuffer => {
  const out = new Uint8Array(CAPTURED_HEADER + data.length);
  const view = new DataView(out.buffer);
  out[0] = WS_TAG_VIDEO_CAPTURED;
  out[1] = keyframe ? WS_FLAG_KEYFRAME : 0;
  view.setUint32(2, tsMs >>> 0, true);
  out.set(data, CAPTURED_HEADER);
  return out.buffer;
};

const toHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

export type ServerBinaryFrame =
  | { kind: "audio"; pcm: Int16Array }
  | { kind: "video"; peer: string; keyframe: boolean; tsMs: number; data: Uint8Array };

/** Parse one inbound binary ws frame. Returns null on the empty buffer, an
 *  unknown/short frame, or a client-only tag (captured video) — the session
 *  drops those and stays alive, mirroring the node. */
export const decodeServerFrame = (buf: ArrayBuffer): ServerBinaryFrame | null => {
  const bytes = new Uint8Array(buf);
  if (bytes.length < 1) return null;
  if (bytes[0] === WS_TAG_AUDIO && bytes.length > 1 && (bytes.length - 1) % 2 === 0) {
    // COPY off the 1-byte tag: the pcm body begins at an odd byte offset, and
    // `new Int16Array(buf, 1)` throws (offset must be a multiple of 2). `slice`
    // hands back a fresh, aligned buffer we can view whole.
    const body = bytes.slice(1);
    return { kind: "audio", pcm: new Int16Array(body.buffer, 0, body.length / 2) };
  }
  if (bytes[0] === WS_TAG_VIDEO_PEER && bytes.length > PEER_HEADER) {
    const view = new DataView(buf);
    return {
      kind: "video",
      keyframe: (bytes[1] & WS_FLAG_KEYFRAME) !== 0,
      tsMs: view.getUint32(2, true),
      peer: toHex(bytes.subarray(6, 38)),
      data: bytes.slice(PEER_HEADER),
    };
  }
  return null;
};
