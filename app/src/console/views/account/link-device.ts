// The device-link ceremony's copy/paste blobs (account-console spec §3).
// Two app-local, versioned wire shapes — both ends are this app, no consensus
// surface: the CHALLENGE (inviter → new device) carries the public facts a
// possession proof is signed over (chain id, account id, current account
// nonce); the RESPONSE (new device → inviter) carries the new member key, its
// scheme, and that possession proof, ready for `user_sign_add_member`.
// Decoding is strict and total: these strings arrive from a paste box, so any
// malformed input yields null — never a throw.

export interface LinkChallenge {
  chainId: string;
  /** The account's id (its founding key), lowercase hex. */
  accountId: string;
  /** The account nonce the possession proof must be signed at. */
  nonce: number;
  /** The account's display name, for the new device's "link to <name>" copy. */
  name: string | null;
}

export interface LinkResponse {
  /** The new device's member key, lowercase hex. */
  pubkey: string;
  /** The app's link flow only mints ed25519 machine keys. */
  kind: "ed25519";
  /** The new key's possession proof (`MemberProof` JSON, verbatim). */
  possession: string;
  /** Optional device label chosen on the new device. */
  label: string | null;
}

const CHALLENGE_PREFIX = "ducktape-link-challenge-v1:";
const RESPONSE_PREFIX = "ducktape-link-response-v1:";

const b64encode = (text: string): string => {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (const byte of bytes) bin += String.fromCharCode(byte);
  return btoa(bin);
};

const b64decode = (blob: string): string | null => {
  try {
    const bin = atob(blob);
    const bytes = Uint8Array.from(bin, (ch) => ch.charCodeAt(0));
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
};

const isHex = (value: unknown): value is string =>
  typeof value === "string" &&
  value.length > 0 &&
  value.length % 2 === 0 &&
  /^[0-9a-f]+$/.test(value);

const isNonce = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

const isOptionalString = (value: unknown): value is string | null =>
  value === null || typeof value === "string";

/** Strip `prefix` and parse the base64 JSON behind it, or null. */
const parsePrefixed = (blob: string, prefix: string): unknown | null => {
  const trimmed = blob.trim();
  if (!trimmed.startsWith(prefix)) return null;
  const json = b64decode(trimmed.slice(prefix.length));
  if (json === null) return null;
  try {
    return JSON.parse(json) as unknown;
  } catch {
    return null;
  }
};

export const encodeLinkChallenge = (challenge: LinkChallenge): string =>
  CHALLENGE_PREFIX + b64encode(JSON.stringify(challenge));

export const decodeLinkChallenge = (blob: string): LinkChallenge | null => {
  const raw = parsePrefixed(blob, CHALLENGE_PREFIX);
  if (raw === null || typeof raw !== "object") return null;
  const c = raw as Record<string, unknown>;
  if (typeof c.chainId !== "string" || c.chainId.length === 0) return null;
  if (!isHex(c.accountId)) return null;
  if (!isNonce(c.nonce)) return null;
  if (!isOptionalString(c.name)) return null;
  return { chainId: c.chainId, accountId: c.accountId, nonce: c.nonce, name: c.name };
};

// ── The QR / LAN link address (link_relay.rs) ────────────────────────────

// The inviter's relay URL: `http://<host:port>/link#<32-hex-token>`. Detection
// only — the strict parse (and the exchange itself) lives in the shell, which
// does the LAN HTTP. Mirrors `parse_link_url` in link_relay.rs.
const LINK_URL = /^http:\/\/[a-z0-9.:[\]-]+\/link#[0-9a-f]{32}$/i;

/** Does this pasted/typed input name a link relay address (vs a blob)? */
export const isLinkUrl = (text: string): boolean => LINK_URL.test(text.trim());

export const encodeLinkResponse = (response: LinkResponse): string =>
  RESPONSE_PREFIX + b64encode(JSON.stringify(response));

export const decodeLinkResponse = (blob: string): LinkResponse | null => {
  const raw = parsePrefixed(blob, RESPONSE_PREFIX);
  if (raw === null || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  if (!isHex(r.pubkey)) return null;
  if (r.kind !== "ed25519") return null;
  if (typeof r.possession !== "string" || r.possession.length === 0) return null;
  if (!isOptionalString(r.label)) return null;
  return { pubkey: r.pubkey, kind: r.kind, possession: r.possession, label: r.label };
};
