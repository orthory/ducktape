// node ops/auth-page/test.mjs — runs the page's pure helper block and the
// relay worker under node.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");
const src = html.match(/<script id="pure">([\s\S]*?)<\/script>/)[1];
const { b64u, parseRequest, spkiToSec1, derToRawSig } = new Function(`${src}; return pure;`)();
const ORIGIN = "https://auth.example";

// base64url round-trip, including the no-padding + url-safe alphabet cases
const bytes = Uint8Array.from([0xfb, 0xff, 0xbf, 0x00, 0x01]);
assert.equal(b64u.enc(bytes), "-_-_AAE");
assert.deepEqual(b64u.dec("-_-_AAE"), bytes);

// fragment contract
const user42 = "KgAAAAAAAAA"; // 42u64 LE
const req = parseRequest(`#op=create&challenge=AQID&user=${user42}&name=de%20mo&cb=http://127.0.0.1:9/`, ORIGIN);
assert.equal(req.op, "create");
assert.deepEqual(req.challenge, Uint8Array.from([1, 2, 3]));
assert.deepEqual(req.user, Uint8Array.from([42, 0, 0, 0, 0, 0, 0, 0]));
assert.equal(req.name, "de mo");
assert.equal(req.cb, "http://127.0.0.1:9/");
assert.equal(parseRequest("#op=get&challenge=AQID", ORIGIN).cb, null);
assert.throws(() => parseRequest("#op=get", ORIGIN), /missing challenge/);
assert.throws(() => parseRequest(`#op=create&challenge=AQID&user=${user42}`, ORIGIN), /missing name/);
assert.throws(() => parseRequest("#op=create&challenge=AQID&user=AQ&name=x", ORIGIN), /8-byte/);
assert.throws(() => parseRequest("#op=nope&challenge=AQID", ORIGIN), /unknown op/);

// the callback is loopback, or this origin's relay: a crafted link cannot relay the signature elsewhere
const id = "A".repeat(43);
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://localhost:9/x", ORIGIN).cb, "http://localhost:9/x");
assert.equal(parseRequest("#op=get&challenge=AQID&cb=http://[::1]:9/", ORIGIN).cb, "http://[::1]:9/");
assert.equal(parseRequest(`#op=get&challenge=AQID&cb=${ORIGIN}/r/${id}`, ORIGIN).cb, `${ORIGIN}/r/${id}`);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=https://evil.example/", ORIGIN), /cb must be/);
assert.throws(() => parseRequest(`#op=get&challenge=AQID&cb=https://evil.example/r/${id}`, ORIGIN), /cb must be/);
assert.throws(() => parseRequest(`#op=get&challenge=AQID&cb=${ORIGIN}/x`, ORIGIN), /cb must be/);
assert.throws(() => parseRequest(`#op=get&challenge=AQID&cb=${ORIGIN}/r/short`, ORIGIN), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=http://127.0.0.1.evil.example/", ORIGIN), /cb must be/);
assert.throws(() => parseRequest("#op=get&challenge=AQID&cb=javascript:alert(1)", ORIGIN), /cb must be/);

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

// the relay worker, with a Map standing in for KV: 204 until the POST, the
// JSON exactly once, 204 again; a malformed id is 404, an oversized body 413,
// and every other path is the static asset.
const { handle } = await import(new URL("./worker.js", import.meta.url));
const store = new Map();
const kv = {
  async put(k, v, opts) { store.set(k, { v, ttl: opts?.expirationTtl }); },
  async get(k) { return store.get(k)?.v ?? null; },
  async delete(k) { store.delete(k); },
};
const env = { CEREMONIES: kv, ASSETS: { fetch: async () => new Response("asset", { status: 200 }) } };
const relay = (path, init) => handle(new Request(`${ORIGIN}${path}`, init), env);
const post = (path, body) => relay(path, { method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body });
assert.equal((await relay(`/r/${id}`)).status, 204);
const posted = await post(`/r/${id}`, `result=${encodeURIComponent('{"op":"get"}')}`);
assert.equal(posted.status, 200);
assert.match(await posted.text(), /return to ducktape/);
assert.equal(store.get(id).ttl, 300);
const taken = await relay(`/r/${id}`);
assert.equal(taken.status, 200);
assert.equal(taken.headers.get("content-type"), "application/json");
assert.equal(await taken.text(), '{"op":"get"}');
assert.equal((await relay(`/r/${id}`)).status, 204);
assert.equal((await relay("/r/short")).status, 404);
assert.equal((await post(`/r/${id}`, "nothing=here")).status, 400);
assert.equal((await post(`/r/${id}`, `result=${"x".repeat(17 * 1024)}`)).status, 413);
assert.equal((await relay(`/r/${id}`, { method: "PUT" })).status, 405);
assert.equal(await (await relay("/")).text(), "asset");

console.log("auth-page: ok");
