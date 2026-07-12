// The device-link ceremony's copy/paste blob codec (see the account-console
// spec §3): challenge (inviter → new device) and response (new device →
// inviter). Decoding is strict and total — any malformed input yields null,
// never a throw, because these strings arrive from a paste box.

import { describe, expect, it } from "vitest";

import {
  decodeLinkChallenge,
  decodeLinkResponse,
  encodeLinkChallenge,
  encodeLinkResponse,
  isLinkUrl,
  type LinkChallenge,
  type LinkResponse,
} from "./link-device";

const challenge: LinkChallenge = {
  chainId: "duck-20260710",
  accountId: "ab01".repeat(16),
  nonce: 7,
  name: "Eddy",
};

const response: LinkResponse = {
  pubkey: "cd02".repeat(16),
  kind: "ed25519",
  possession: '{"ed25519":{"sig":"aabb"}}',
  label: "work laptop",
};

describe("link challenge codec", () => {
  it("round-trips", () => {
    const blob = encodeLinkChallenge(challenge);
    expect(blob.startsWith("ducktape-link-challenge-v1:")).toBe(true);
    expect(decodeLinkChallenge(blob)).toEqual(challenge);
  });

  it("round-trips a null name", () => {
    const anon = { ...challenge, name: null };
    expect(decodeLinkChallenge(encodeLinkChallenge(anon))).toEqual(anon);
  });

  it("tolerates surrounding whitespace", () => {
    const blob = `  \n${encodeLinkChallenge(challenge)}\n `;
    expect(decodeLinkChallenge(blob)).toEqual(challenge);
  });

  it("rejects a wrong prefix", () => {
    expect(decodeLinkChallenge("ducktape-link-challenge-v2:aaaa")).toBeNull();
    expect(decodeLinkChallenge("hello")).toBeNull();
    expect(decodeLinkChallenge("")).toBeNull();
  });

  it("rejects bad base64 and bad JSON", () => {
    expect(decodeLinkChallenge("ducktape-link-challenge-v1:!!!!")).toBeNull();
    expect(decodeLinkChallenge(`ducktape-link-challenge-v1:${btoa("not json")}`)).toBeNull();
  });

  it("rejects malformed fields", () => {
    const enc = (c: unknown) => `ducktape-link-challenge-v1:${btoa(JSON.stringify(c))}`;
    expect(decodeLinkChallenge(enc({ ...challenge, nonce: -1 }))).toBeNull();
    expect(decodeLinkChallenge(enc({ ...challenge, nonce: 1.5 }))).toBeNull();
    expect(decodeLinkChallenge(enc({ ...challenge, accountId: "XYZ" }))).toBeNull();
    expect(decodeLinkChallenge(enc({ ...challenge, accountId: "abc" }))).toBeNull(); // odd length
    expect(decodeLinkChallenge(enc({ ...challenge, chainId: "" }))).toBeNull();
    expect(decodeLinkChallenge(enc({ ...challenge, name: 3 }))).toBeNull();
  });

  it("rejects a response blob fed to the challenge decoder", () => {
    expect(decodeLinkChallenge(encodeLinkResponse(response))).toBeNull();
  });
});

describe("link response codec", () => {
  it("round-trips", () => {
    const blob = encodeLinkResponse(response);
    expect(blob.startsWith("ducktape-link-response-v1:")).toBe(true);
    expect(decodeLinkResponse(blob)).toEqual(response);
  });

  it("round-trips a null label", () => {
    const bare = { ...response, label: null };
    expect(decodeLinkResponse(encodeLinkResponse(bare))).toEqual(bare);
  });

  it("rejects malformed fields", () => {
    const enc = (r: unknown) => `ducktape-link-response-v1:${btoa(JSON.stringify(r))}`;
    expect(decodeLinkResponse(enc({ ...response, pubkey: "nope" }))).toBeNull();
    expect(decodeLinkResponse(enc({ ...response, kind: "p256" }))).toBeNull();
    expect(decodeLinkResponse(enc({ ...response, possession: "" }))).toBeNull();
    expect(decodeLinkResponse(enc({ ...response, label: 9 }))).toBeNull();
  });

  it("rejects a challenge blob fed to the response decoder", () => {
    expect(decodeLinkResponse(encodeLinkChallenge(challenge))).toBeNull();
  });
});

describe("link address detection", () => {
  const token = "0123456789abcdef0123456789abcdef";

  it("accepts a relay address, with whitespace and uppercase hex", () => {
    expect(isLinkUrl(`http://192.168.1.23:40000/link#${token}`)).toBe(true);
    expect(isLinkUrl(`  http://10.0.0.2:1/link#${token.toUpperCase()} \n`)).toBe(true);
  });

  it("rejects blobs, wrong paths, wrong schemes, and bad tokens", () => {
    expect(isLinkUrl(encodeLinkChallenge(challenge))).toBe(false);
    expect(isLinkUrl(`https://192.168.1.23:40000/link#${token}`)).toBe(false);
    expect(isLinkUrl(`http://192.168.1.23:40000/enroll#${token}`)).toBe(false);
    expect(isLinkUrl("http://192.168.1.23:40000/link#short")).toBe(false);
    expect(isLinkUrl("")).toBe(false);
  });
});
