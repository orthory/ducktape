// The pure binary framing of /v1/call/ws. These assertions pin the wire layout
// to bin/noded/src/lib.rs byte-for-byte (little-endian on this leg): the tag
// bytes, the captured-video header, and the peer-video header + 32-byte key.

import { describe, expect, it } from "vitest";

import {
  WS_TAG_AUDIO,
  WS_TAG_VIDEO_CAPTURED,
  WS_TAG_VIDEO_PEER,
  decodeServerFrame,
  encodeAudioFrame,
  encodeCapturedVideo,
} from "./call-frames";

describe("encodeAudioFrame", () => {
  it("prefixes the audio tag and lays out the pcm little-endian", () => {
    const pcm = new Int16Array([0x0102, -1, 0x7fff]);
    const bytes = new Uint8Array(encodeAudioFrame(pcm));
    expect(bytes[0]).toBe(WS_TAG_AUDIO);
    // 0x0102 → [0x02, 0x01] LE; -1 → [0xff, 0xff]; 0x7fff → [0xff, 0x7f]
    expect(Array.from(bytes.subarray(1))).toEqual([0x02, 0x01, 0xff, 0xff, 0xff, 0x7f]);
  });

  it("produces exactly 1 + 2·N bytes", () => {
    expect(encodeAudioFrame(new Int16Array(960)).byteLength).toBe(1 + 1920);
  });

  it("honours a subarray view's byteOffset (only the view's samples ship)", () => {
    const backing = new Int16Array([9, 9, 5, 6, 9]);
    const view = backing.subarray(2, 4); // [5, 6]
    const bytes = new Uint8Array(encodeAudioFrame(view));
    expect(bytes[0]).toBe(WS_TAG_AUDIO);
    expect(Array.from(bytes.subarray(1))).toEqual([5, 0, 6, 0]);
  });
});

describe("encodeCapturedVideo", () => {
  it("lays out [0x02][flags][ts_ms u32 LE][data] with the keyframe flag set", () => {
    const data = new Uint8Array([0xaa, 0xbb, 0xcc]);
    const bytes = new Uint8Array(encodeCapturedVideo(true, 0x01020304, data));
    expect(bytes[0]).toBe(WS_TAG_VIDEO_CAPTURED);
    expect(bytes[1]).toBe(0x01); // WS_FLAG_KEYFRAME
    // ts_ms 0x01020304 little-endian
    expect(Array.from(bytes.subarray(2, 6))).toEqual([0x04, 0x03, 0x02, 0x01]);
    expect(Array.from(bytes.subarray(6))).toEqual([0xaa, 0xbb, 0xcc]);
  });

  it("clears the flags byte for a delta frame", () => {
    const bytes = new Uint8Array(encodeCapturedVideo(false, 7, new Uint8Array([1])));
    expect(bytes[1]).toBe(0x00);
    expect(Array.from(bytes.subarray(2, 6))).toEqual([7, 0, 0, 0]);
  });
});

describe("decodeServerFrame — audio", () => {
  it("round-trips an encoded audio frame back to identical pcm", () => {
    const pcm = new Int16Array([-32768, -1, 0, 1, 32767, 12345]);
    const frame = decodeServerFrame(encodeAudioFrame(pcm));
    expect(frame?.kind).toBe("audio");
    expect(Array.from((frame as { pcm: Int16Array }).pcm)).toEqual(Array.from(pcm));
  });

  it("decodes pcm that begins at the unaligned 1-byte offset behind the tag", () => {
    // The tag occupies byte 0, so the pcm body starts at byte 1 — an odd offset
    // that `new Int16Array(buf, 1)` rejects outright (offset must be a multiple
    // of 2). decodeServerFrame must COPY the body off the tag, never view it in
    // place; this frame would throw a RangeError under the naive view.
    const buf = new ArrayBuffer(5); // tag + two i16
    const view = new DataView(buf);
    view.setUint8(0, WS_TAG_AUDIO);
    view.setInt16(1, -12345, true);
    view.setInt16(3, 6789, true);
    const frame = decodeServerFrame(buf);
    expect(frame?.kind).toBe("audio");
    expect(Array.from((frame as { pcm: Int16Array }).pcm)).toEqual([-12345, 6789]);
  });

  it("rejects an audio frame with an odd pcm byte count", () => {
    const buf = new Uint8Array([WS_TAG_AUDIO, 0x01]).buffer; // 1 stray byte
    expect(decodeServerFrame(buf)).toBeNull();
  });
});

describe("decodeServerFrame — peer video", () => {
  const peerKey = Uint8Array.from({ length: 32 }, (_v, i) => i); // 00..1f
  const buildPeer = (keyframe: boolean, tsMs: number, data: number[]): ArrayBuffer => {
    const out = new Uint8Array(38 + data.length);
    out[0] = WS_TAG_VIDEO_PEER;
    out[1] = keyframe ? 0x01 : 0x00;
    new DataView(out.buffer).setUint32(2, tsMs, true);
    out.set(peerKey, 6);
    out.set(data, 38);
    return out.buffer;
  };

  it("parses the peer key (lowercased hex), keyframe flag, ts, and vp8 data", () => {
    const frame = decodeServerFrame(buildPeer(true, 0x0a0b0c0d, [0x11, 0x22]));
    expect(frame).toEqual({
      kind: "video",
      peer: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
      keyframe: true,
      tsMs: 0x0a0b0c0d,
      data: new Uint8Array([0x11, 0x22]),
    });
  });

  it("reads a delta frame's cleared keyframe flag", () => {
    const frame = decodeServerFrame(buildPeer(false, 42, [0xff]));
    expect((frame as { keyframe: boolean }).keyframe).toBe(false);
    expect((frame as { tsMs: number }).tsMs).toBe(42);
  });

  it("returns null for a peer frame carrying no data (header only)", () => {
    const out = new Uint8Array(38);
    out[0] = WS_TAG_VIDEO_PEER;
    expect(decodeServerFrame(out.buffer)).toBeNull();
  });
});

describe("decodeServerFrame — rejects", () => {
  it("returns null on the empty buffer", () => {
    expect(decodeServerFrame(new ArrayBuffer(0))).toBeNull();
  });

  it("returns null on an unknown tag", () => {
    expect(decodeServerFrame(new Uint8Array([0x99, 1, 2, 3]).buffer)).toBeNull();
  });

  it("returns null on a captured-video tag (client→server only, never inbound)", () => {
    const bytes = new Uint8Array(encodeCapturedVideo(true, 1, new Uint8Array([1, 2, 3])));
    expect(bytes[0]).toBe(WS_TAG_VIDEO_CAPTURED);
    expect(decodeServerFrame(bytes.buffer)).toBeNull();
  });
});
