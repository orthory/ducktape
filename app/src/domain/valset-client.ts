// Typed client for the node's `valset` system module — the TS mirror of
// `crates/system/valset-interface`. Reads are committed validator public keys.

import { keyHex } from "./chat-client";
import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "valset";

// ── Queries (reads) ─────────────────────────────────────

export const validators = (transport: NodeTransport): Promise<number[][]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Validators"))
    .then((reply) => replyVariant<number[][]>(reply, "Validators"));

export const validatorHex = (key: number[]): string => keyHex(key);
