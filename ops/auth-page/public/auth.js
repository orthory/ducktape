// Pure helpers for the auth page. No DOM, no WebAuthn — importable from node
// for `test.mjs`. Everything that touches `navigator` lives in index.html.

export const b64u = {
  enc(bytes) {
    let s = "";
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  },
  dec(str) {
    const pad = "=".repeat((4 - (str.length % 4)) % 4);
    const bin = atob(str.replace(/-/g, "+").replace(/_/g, "/") + pad);
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
  },
};

export function hex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/// The request rides in the URL fragment so it never reaches a server log.
/// `#op=create&challenge=<b64u>&user=<decimal>&name=<text>&cb=<url>`
export function parseRequest(fragment) {
  const p = new URLSearchParams(fragment.replace(/^#/, ""));
  const get = (k) => {
    const v = p.get(k);
    if (v === null || v === "") throw new Error(`missing ${k}`);
    return v;
  };
  const op = get("op");
  const req = { op, challenge: b64u.dec(get("challenge")), cb: p.get("cb") };
  if (op === "create") {
    req.user = BigInt(get("user"));
    req.name = get("name");
  } else if (op !== "get" && op !== "eth") {
    throw new Error(`unknown op ${op}`);
  }
  return req;
}

/// Account number as the WebAuthn `user.id`: u64 little-endian.
export function accountNumberLE(n) {
  const out = new Uint8Array(8);
  let v = BigInt(n);
  for (let i = 0; i < 8; i++) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

/// `getPublicKey()` returns SPKI DER; for P-256 the uncompressed point is the
/// last 65 bytes (0x04 ‖ X ‖ Y). Returns the 33-byte compressed SEC1 point.
export function spkiToSec1(spki) {
  const point = spki.slice(spki.length - 65);
  if (point[0] !== 0x04) throw new Error("not an uncompressed P-256 point");
  const x = point.slice(1, 33);
  const y = point.slice(33, 65);
  const out = new Uint8Array(33);
  out[0] = 0x02 | (y[31] & 1);
  out.set(x, 1);
  return out;
}

/// WebAuthn ES256 signatures are DER `SEQUENCE { INTEGER r, INTEGER s }`;
/// the envelope wants raw `R ‖ S` (32 ‖ 32).
export function derToRawSig(der) {
  if (der[0] !== 0x30) throw new Error("not a DER sequence");
  let i = 2;
  const int = () => {
    if (der[i++] !== 0x02) throw new Error("not a DER integer");
    let len = der[i++];
    let start = i;
    i += len;
    while (len > 32 && der[start] === 0) {
      start++;
      len--;
    }
    if (len > 32) throw new Error("integer wider than 32 bytes");
    const out = new Uint8Array(32);
    out.set(der.slice(start, start + len), 32 - len);
    return out;
  };
  const r = int();
  const s = int();
  const out = new Uint8Array(64);
  out.set(r, 0);
  out.set(s, 32);
  return out;
}
