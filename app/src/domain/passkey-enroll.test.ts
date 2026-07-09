// The passkey-enroll parsers are the raw-bytes bridge to the on-chain verifier:
// a wrong DER→raw or SPKI→point conversion means an enrolled passkey signs
// bytes the chain rejects. These exercise the parsers with hand-built DER/SPKI
// vectors (no JS ECDSA lib in the tree) — structure, padding, and rejection.

import { describe, expect, it } from "vitest";

import {
  assembleWebauthnProof,
  b64urlToBytes,
  derEcdsaToRaw,
  spkiToRawP256Point,
} from "./passkey-enroll";

const hexBytes = (hex: string): Uint8Array =>
  Uint8Array.from(hex.match(/.{2}/g)!.map((h) => parseInt(h, 16)));

const buf = (b: Uint8Array): ArrayBuffer =>
  b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;

// P-256 SPKI = 26-byte algorithm prefix + 0x04 + X(32) + Y(32).
const P256_PREFIX = "3059301306072a8648ce3d020106082a8648ce3d030107034200";

describe("derEcdsaToRaw", () => {
  it("left-pads short r/s to a 32-byte R‖S", () => {
    // SEQUENCE { INTEGER 0x01, INTEGER 0x02 }
    const der = hexBytes("3006020101020102");
    const raw = derEcdsaToRaw(der);
    expect(raw.length).toBe(64);
    expect(raw[31]).toBe(0x01);
    expect(raw[63]).toBe(0x02);
    expect(raw.slice(0, 31).every((x) => x === 0)).toBe(true);
    expect(raw.slice(32, 63).every((x) => x === 0)).toBe(true);
  });

  it("strips DER's sign-padding zero on a high-bit scalar", () => {
    // r = 0x00 || 32 bytes starting 0xff (DER pads because the high bit is set).
    const rBody = "ff" + "00".repeat(31);
    const der = hexBytes(`3025` + `0221` + `00` + rBody + `020100`);
    const raw = derEcdsaToRaw(der);
    expect(raw[0]).toBe(0xff); // padding byte dropped, full 32 bytes preserved
    expect(raw.slice(1, 32).every((x) => x === 0)).toBe(true);
    expect(raw.slice(32).every((x) => x === 0)).toBe(true);
  });

  it("rejects a non-SEQUENCE", () => {
    expect(() => derEcdsaToRaw(hexBytes("020101"))).toThrow("not a SEQUENCE");
  });

  it("rejects a scalar that overflows 32 bytes", () => {
    const rBody = "01".repeat(33); // 33-byte INTEGER, no sign pad → overflow
    expect(() => derEcdsaToRaw(hexBytes(`3023` + `0221` + rBody + `020100`))).toThrow(
      "exceeds 32 bytes",
    );
  });
});

describe("spkiToRawP256Point", () => {
  const point = hexBytes("04" + "11".repeat(32) + "22".repeat(32));

  it("extracts the 65-byte uncompressed point from a valid P-256 SPKI", () => {
    const spki = hexBytes(P256_PREFIX + "04" + "11".repeat(32) + "22".repeat(32));
    const got = spkiToRawP256Point(spki);
    expect(got).toEqual(point);
    expect(got.length).toBe(65);
    expect(got[0]).toBe(0x04);
  });

  it("rejects a non-P256 algorithm prefix", () => {
    const wrong = hexBytes("30".repeat(26) + "04" + "11".repeat(64));
    expect(() => spkiToRawP256Point(wrong)).toThrow("not a P-256");
  });

  it("rejects a point that is not uncompressed", () => {
    // right prefix, but the point starts 0x03 (compressed) — not 0x04.
    const spki = hexBytes(P256_PREFIX + "03" + "11".repeat(32) + "22".repeat(32));
    expect(() => spkiToRawP256Point(spki)).toThrow("malformed uncompressed");
  });
});

describe("b64urlToBytes", () => {
  it("decodes unpadded base64url with - and _ substitutions", () => {
    // 0xfb 0xff 0xbf encodes to "+/+/" in base64 → "-_-_" in base64url.
    expect(Array.from(b64urlToBytes("-_-_"))).toEqual([0xfb, 0xff, 0xbf]);
  });

  it("decodes an unpadded 32-byte challenge round-trip length", () => {
    // 43 base64url chars decode to 32 bytes (a SHA-256 challenge).
    const challenge = "A".repeat(43);
    expect(b64urlToBytes(challenge).length).toBe(32);
  });
});

describe("assembleWebauthnProof", () => {
  it("normalizes the DER signature to raw R‖S in the wire shape", () => {
    const proof = assembleWebauthnProof({
      authenticatorData: buf(hexBytes("aabb")),
      clientDataJSON: buf(hexBytes("7b7d")), // "{}"
      signature: buf(hexBytes("3006020101020102")),
    });
    expect(proof.webauthn.authenticator_data).toEqual([0xaa, 0xbb]);
    expect(proof.webauthn.client_data_json).toEqual([0x7b, 0x7d]);
    expect(proof.webauthn.signature.length).toBe(64);
    expect(proof.webauthn.signature[31]).toBe(0x01);
    expect(proof.webauthn.signature[63]).toBe(0x02);
  });
});
