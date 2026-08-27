// node ops/auth-page/test.mjs
import assert from "node:assert/strict";
import { b64u, parseRequest, accountNumberLE, spkiToSec1, derToRawSig } from "./public/auth.js";

// base64url round-trip, including the no-padding + url-safe alphabet cases
const bytes = Uint8Array.from([0xfb, 0xff, 0xbf, 0x00, 0x01]);
assert.equal(b64u.enc(bytes), "-_-_AAE");
assert.deepEqual(b64u.dec("-_-_AAE"), bytes);

// fragment contract
const req = parseRequest("#op=create&challenge=AQID&user=42&name=demo&cb=http://127.0.0.1:9/");
assert.equal(req.op, "create");
assert.deepEqual(req.challenge, Uint8Array.from([1, 2, 3]));
assert.equal(req.user, 42n);
assert.equal(req.name, "demo");
assert.equal(req.cb, "http://127.0.0.1:9/");
assert.equal(parseRequest("#op=get&challenge=AQID").cb, null);
assert.throws(() => parseRequest("#op=get"), /missing challenge/);
assert.throws(() => parseRequest("#op=create&challenge=AQID&user=1"), /missing name/);
assert.throws(() => parseRequest("#op=nope&challenge=AQID"), /unknown op/);
// the callback is loopback-only: a crafted link cannot relay the signature elsewhere
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://localhost:9/x").cb, "http://localhost:9/x");
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://[::1]:9/").cb, "http://[::1]:9/");
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=https://evil.example/"), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=http://127.0.0.1.evil.example/"), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=javascript:alert(1)"), /cb must be/);

// u64 LE
assert.deepEqual(accountNumberLE(1n), Uint8Array.from([1, 0, 0, 0, 0, 0, 0, 0]));
assert.deepEqual(accountNumberLE(0x0102n), Uint8Array.from([2, 1, 0, 0, 0, 0, 0, 0]));

// SPKI → compressed SEC1: P-256 generator point G (odd y → 0x03 prefix)
const gx = "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296";
const gy = "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";
const unhex = (h) => Uint8Array.from(h.match(/../g), (x) => parseInt(x, 16));
const spkiPrefix = unhex("3059301306072a8648ce3d020106082a8648ce3d030107034200");
const spki = Uint8Array.from([...spkiPrefix, 0x04, ...unhex(gx), ...unhex(gy)]);
assert.equal(spki.length, 91);
assert.deepEqual(spkiToSec1(spki), Uint8Array.from([0x03, ...unhex(gx)]));

// DER → raw R‖S: a high-bit r gets a 0x00 pad byte in DER; a short s gets left-padded
const r = Uint8Array.from({ length: 32 }, (_, i) => (i === 0 ? 0xff : i));
const s = Uint8Array.from({ length: 31 }, (_, i) => i + 1);
const der = Uint8Array.from([0x30, 2 + 33 + 2 + 31, 0x02, 33, 0x00, ...r, 0x02, 31, ...s]);
const raw = derToRawSig(der);
assert.deepEqual(raw.slice(0, 32), r);
assert.deepEqual(raw.slice(32), Uint8Array.from([0, ...s]));
assert.throws(() => derToRawSig(Uint8Array.from([0x31])), /not a DER sequence/);

console.log("auth-page: ok");
