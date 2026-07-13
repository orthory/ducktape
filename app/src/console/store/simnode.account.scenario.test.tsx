// Account/identity scenarios against the deterministic sim node: the
// identity→duckdns account chain, seeded over the BARE WIRE from TypeScript
// (real ed25519 consent, not a precomputed vector), then read back through the
// store's people slice — the node→account→handle mapping the console projects.
//
// Why bare-wire seeding: the store's account WRITE paths (accountBindNode,
// setDuckHandle landing) require a tauri shell to sign (`user_sign_bind`, gated
// on isTauri()) AND a real self node key. The sim reports an empty publicKey and
// the vitest env is not tauri, so those write paths short-circuit — their
// outcome vocabulary (bound/already/locked/deferred/failed/skipped) is unit
// territory (auto-bind.test.ts). What the scenario lane CAN prove end-to-end is
// the ceremony over the wire + the store's READ projection of committed state,
// plus the store's own guard when no self identity exists.
//
// Signing (mirrors identity/src: bind_preimage + IDENTITY_BIND_NS, then
// commonware's union_unique(ns,msg) = varint(ns.len)‖ns‖msg, ed25519 over that):
// reproduced here because the founding key is one we mint in TS, so no fixed
// Rust vector is needed — any well-formed ed25519 key founds the account.
//
// Skips (visibly) without a built binary: cargo build -p simnode.

import { ed25519 } from "@noble/curves/ed25519.js";
import { act, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { keyHex } from "../../domain/chat-client";
import { simnodeBinary } from "../../test/simnode-harness";
import { useSimScenario } from "../../test/sim-scenario";

const bin = simnodeBinary();
if (!bin) {
  console.warn(
    "[simnode.account.scenario] ducktape-simnode not built — skipping (cargo build -p simnode, or set DUCKTAPE_SIMNODE_BIN)",
  );
}

// ── bind-consent signing (TS mirror of crates/system/identity) ──

const IDENTITY_BIND_NS = new TextEncoder().encode("ducktape-identity-bind-v1");

/** commonware_codec unsigned varint (LEB128). */
const varint = (n: number): number[] => {
  const out: number[] = [];
  let v = n;
  do {
    let b = v & 0x7f;
    v >>>= 7;
    if (v) b |= 0x80;
    out.push(b);
  } while (v);
  return out;
};

const u64le = (n: number): number[] => {
  const out = new Array<number>(8).fill(0);
  let v = BigInt(n);
  for (let i = 0; i < 8; i += 1) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
};

/** identity's push_len: an 8-byte LE length prefix, then the bytes. */
const pushLen = (bytes: Uint8Array): number[] => [...u64le(bytes.length), ...bytes];

/** bind_preimage(chain_id, node_key, nonce). */
const bindPreimage = (chainId: string, nodeKey: Uint8Array, nonce: number): number[] => [
  ...pushLen(new TextEncoder().encode(chainId)),
  ...pushLen(nodeKey),
  ...u64le(nonce),
];

/** union_unique(namespace, msg) = varint(namespace.len) ‖ namespace ‖ msg. */
const unionUnique = (ns: Uint8Array, msg: number[]): Uint8Array =>
  Uint8Array.from([...varint(ns.length), ...ns, ...msg]);

/** A MemberAuth whose ed25519 key consents to binding `nodeKey` at `nonce`. */
const edBindAuth = (
  secret: Uint8Array,
  nodeKey: Uint8Array,
  chainId: string,
  nonce: number,
) => {
  const payload = unionUnique(IDENTITY_BIND_NS, bindPreimage(chainId, nodeKey, nonce));
  return {
    key: Array.from(ed25519.getPublicKey(secret)),
    kind: "ed25519" as const,
    proof: { signature: { sig: Array.from(ed25519.sign(payload, secret)) } },
  };
};

describe.skipIf(!bin)("account scenarios against the sim node", () => {
  const { boot, state, actions } = useSimScenario();

  it(
    "the bind ceremony seeds an account over the bare wire; the store maps node → account → name → handle",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ auto: true });

      // duckdns/identity derive the account from a 32-BYTE origin; the sim
      // stamps an origin string's bytes verbatim, so a 32-char string is a
      // 32-byte node key. chain_id is empty, the account nonce starts at 0.
      const secret = new Uint8Array(32).fill(7);
      const nodeOrigin = "a".repeat(32);
      const nodeKey = new TextEncoder().encode(nodeOrigin);
      const accountIdHex = keyHex(Array.from(ed25519.getPublicKey(secret)));
      const nodeHex = keyHex(Array.from(nodeKey));

      // 1. the founding key consents to binding the node → founds account A.
      await sim.peerBlock(
        "identity",
        { bind_node: { authorizer: edBindAuth(secret, nodeKey, "", 0) } },
        nodeOrigin,
      );
      // 2. the now-bound node (origin-trusted) names the account…
      await sim.peerBlock(
        "identity",
        { set_account_name: { display_name: "Eddy" } },
        nodeOrigin,
      );
      // 3. …and registers a duck handle for it.
      await sim.peerBlock(
        "duckdns",
        { set_handle: { handle: "eddy" } },
        nodeOrigin,
      );

      // the people slice hydrates the whole chain: the node maps to its account,
      // the name reaches both the account id and the node key, and the handle is
      // keyed by account id.
      await waitFor(() =>
        expect(state().nodeUsers[nodeHex]?.accountId).toBe(accountIdHex),
      );
      expect(state().nodeUsers[nodeHex]?.name).toBe("Eddy");
      await waitFor(() =>
        expect(state().accountHandles[accountIdHex]).toBe("eddy"),
      );
      expect(state().authorNames[accountIdHex]).toBe("Eddy");
      expect(state().authorNames[nodeHex]).toBe("Eddy");
      // the founding ed25519 member key is projected under the account.
      expect(
        state().accountKeys[accountIdHex]?.some((k) => k.kind === "ed25519"),
      ).toBe(true);
    },
  );

  it(
    "the store's duck-handle write is guarded when no self identity is bound (the sim reports no node key)",
    { timeout: 30_000 },
    async () => {
      await boot({ auto: true });

      // setDuckHandle resolves the self account through
      // nodeUsers[status.publicKey ?? workspace.pubkey]; the sim reports neither,
      // so the store refuses up front rather than submitting a doomed op. This
      // is the observable seam for why the WRITE path can't be exercised here.
      act(() => actions().setDuckHandle("mine"));
      await waitFor(() =>
        expect(state().error).toMatch(
          /bind this node to an identity account before registering a \.duck name/,
        ),
      );
    },
  );
});
