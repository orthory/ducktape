// node ops/auth-page/test.mjs — runs the page's pure helper block under node.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");
const src = html.match(/<script id="pure">([\s\S]*?)<\/script>/)[1];
const { b64u, parseRequest, spkiToSec1, derToRawSig } = new Function(`${src}; return pure;`)();

// base64url round-trip, including the no-padding + url-safe alphabet cases
const bytes = Uint8Array.from([0xfb, 0xff, 0xbf, 0x00, 0x01]);
assert.equal(b64u.enc(bytes), "-_-_AAE");
assert.deepEqual(b64u.dec("-_-_AAE"), bytes);

// fragment contract
const user42 = "KgAAAAAAAAA"; // 42u64 LE
const req = parseRequest(`#op=create&challenge=AQID&user=${user42}&name=de%20mo&cb=http://127.0.0.1:9/`);
assert.equal(req.op, "create");
assert.deepEqual(req.challenge, Uint8Array.from([1, 2, 3]));
assert.deepEqual(req.user, Uint8Array.from([42, 0, 0, 0, 0, 0, 0, 0]));
assert.equal(req.name, "de mo");
assert.equal(req.cb, "http://127.0.0.1:9/");
assert.equal(parseRequest("#op=get&challenge=AQID").cb, null);
assert.throws(() => parseRequest("#op=get"), /missing challenge/);
assert.throws(() => parseRequest(`#op=create&challenge=AQID&user=${user42}`), /missing name/);
assert.throws(() => parseRequest("#op=create&challenge=AQID&user=AQ&name=x"), /8-byte/);
assert.throws(() => parseRequest("#op=nope&challenge=AQID"), /unknown op/);

// the callback is loopback-only: a crafted link cannot relay the signature elsewhere
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://localhost:9/x").cb, "http://localhost:9/x");
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://[::1]:9/").cb, "http://[::1]:9/");
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=https://evil.example/"), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=http://127.0.0.1.evil.example/"), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=javascript:alert(1)"), /cb must be/);

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
