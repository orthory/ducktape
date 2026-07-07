// Typed client for the node's `valset` system module — the TS mirror of
// `crates/system/valset-interface`. Reads are committed public keys of the two
// membership tiers: VALIDATORS (the consensus quorum) and RESIDENTS (mesh +
// statesync standing, no quorum seat — the staged-admission tier a joiner
// syncs in before promotion). The tiers never overlap: valset's Grant refuses
// validators and Join clears resident standing.

import { keyHex } from "./chat-client";
import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "valset";

// ── Queries (reads) ─────────────────────────────────────

export const validators = (transport: NodeTransport): Promise<number[][]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "validators"))
    .then((reply) => replyVariant<number[][]>(reply, "validators"));

export const residents = (transport: NodeTransport): Promise<number[][]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "residents"))
    .then((reply) => replyVariant<number[][]>(reply, "residents"));

export const validatorHex = (key: number[]): string => keyHex(key);
