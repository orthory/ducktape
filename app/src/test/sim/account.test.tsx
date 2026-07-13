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
import { simnodeBinary } from "../simnode-harness";
import { useSimScenario } from "../sim-scenario";
import { opKey } from "../../console/store/finalization";

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

  // ── WRITE tier: --node-key gives the sim a self identity ─────────────
  //
  // With `--node-key K`, status.publicKey serves K, so the store resolves the
  // self account via nodeUsers[K] and its WRITE paths become reachable. One
  // subtlety the sim exposes: a REAL node discards the /v1/submit origin and
  // stamps its OWN node key as author; the sim keeps the client origin verbatim
  // (a testing divergence). duckdns's set_handle requires a 32-byte, account-
  // bound origin, so the store's write must author AS K here — we set the store
  // author to the `hex:` escape of K, which the sim decodes to K's raw bytes,
  // reproducing exactly what a real node does automatically.

  /** K, the fabricated node key --node-key seeds (status.publicKey serves it). */
  const nodeKeyBytes = Array.from({ length: 32 }, () => 0x11);
  const nodeKeyHex = keyHex(nodeKeyBytes);
  /** Author the store's submits AS K: the sim's `hex:` origin escape decodes to
   *  K's raw bytes (a real node stamps K itself). */
  const asNodeKeyOrigin = "hex:" + nodeKeyHex;

  /** Found account A over the bare wire and bind `nodeHex` to it, naming the
   *  node key's RAW bytes via the `hex:` origin escape. Returns the account id. */
  const bindAccount = async (
    sim: Awaited<ReturnType<typeof boot>>["sim"],
    secret: Uint8Array,
    node: number[],
    displayName: string,
  ): Promise<string> => {
    const nodeHex = keyHex(node);
    await sim.peerBlock(
      "identity",
      { bind_node: { authorizer: edBindAuth(secret, Uint8Array.from(node), "", 0) } },
      "hex:" + nodeHex,
    );
    await sim.peerBlock(
      "identity",
      { set_account_name: { display_name: displayName } },
      "hex:" + nodeHex,
    );
    return keyHex(Array.from(ed25519.getPublicKey(secret)));
  };

  it(
    "setDuckHandle lands end-to-end: the store's write registers the .duck name for the bound account",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ nodeKey: nodeKeyHex, auto: true });

      const accountId = await bindAccount(
        sim,
        new Uint8Array(32).fill(7),
        nodeKeyBytes,
        "Eddy",
      );
      // the store hydrates the self node → account mapping off status.publicKey.
      await waitFor(() =>
        expect(state().nodeUsers[nodeKeyHex]?.accountId).toBe(accountId),
      );

      // author AS K so the set_handle submit lands under the bound node.
      act(() => actions().setAuthor(asNodeKeyOrigin));
      act(() => actions().setDuckHandle("eddy"));

      // the write settled: the op finalized and committed truth carries the
      // handle keyed by the account id (the store's own read projection).
      await waitFor(() =>
        expect(state().ops[opKey.duckHandle()]?.phase).toBe("finalized"),
      );
      await waitFor(() =>
        expect(state().accountHandles[accountId]).toBe("eddy"),
      );

      // accountBindNode's auto-bind vocabulary (bound/already/locked/deferred/
      // skipped) is NOT reachable in this provider-only lane: the action guards
      // on a selected workspace (null here) before it ever reaches the tauri-
      // signed bind. Pin the honest guard; the vocabulary lives in
      // auto-bind.test.ts (which drives a stub transport + a mocked shell).
      await expect(actions().accountBindNode()).rejects.toThrow(
        /not connected to a workspace node/,
      );
    },
  );

  it(
    "claiming an already-taken handle surfaces the module's rejection in store state",
    { timeout: 30_000 },
    async () => {
      const { sim } = await boot({ nodeKey: nodeKeyHex, auto: true });

      // a RIVAL account (its own bound node) claims "duke" first, over the wire.
      const rivalNode = Array.from({ length: 32 }, () => 0x22);
      await bindAccount(sim, new Uint8Array(32).fill(9), rivalNode, "Rival");
      await sim.peerBlock(
        "duckdns",
        { set_handle: { handle: "duke" } },
        "hex:" + keyHex(rivalNode),
      );

      // OUR account binds to K…
      const accountId = await bindAccount(
        sim,
        new Uint8Array(32).fill(7),
        nodeKeyBytes,
        "Eddy",
      );
      await waitFor(() =>
        expect(state().nodeUsers[nodeKeyHex]?.accountId).toBe(accountId),
      );

      // …and the store tries to claim the taken handle: the REAL duckdns module
      // refuses, and the failure lands in the op ledger.
      act(() => actions().setAuthor(asNodeKeyOrigin));
      act(() => actions().setDuckHandle("duke"));
      await waitFor(() =>
        expect(state().ops[opKey.duckHandle()]?.phase).toBe("failed"),
      );
      expect(state().ops[opKey.duckHandle()]?.error).toMatch(
        /already claimed by another account/,
      );
      // the optimistic handle rolled back — our account holds no name.
      await waitFor(() =>
        expect(state().accountHandles[accountId]).toBeUndefined(),
      );
    },
  );
});
