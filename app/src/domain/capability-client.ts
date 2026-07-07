// Typed client for the node's `capability` module — the network-wide registry
// of which executor each node's host can run ("codex", "claude", ...). node key
// -> announced tag set, replicated in consensus so every node holds an
// identical view of who provides what (see crates/system/capability).
//
// The agent view reads this to offer a picker of REAL, announced executors
// instead of asking the user to type a routing tag blind. It is READ-ONLY here:
// announcing is host policy (the operator's capability specs), never a UI act —
// this file only lists.
//
// Agents care about which distinct executor tags exist network-wide, not which
// node announced them, so `capabilities` flattens the `All` reply's node->tags
// pairs into a deduped, sorted tag list. Pure functions over an injected
// NodeTransport, exactly like agent-client.

import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";
import { keyHex } from "./chat-client";

const TARGET = "capability";

/** One `All` entry: a node's key bytes and the tag set it announced. */
type RegistryEntry = [number[], string[]];

/** Every distinct executor tag announced anywhere on the network, sorted. The
 *  "Runs on" picker shows these; an empty list means no host has announced yet
 *  (or the node predates the module) — the caller degrades to a text field. */
export const capabilities = (transport: NodeTransport): Promise<string[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "all"))
    .then((reply) => replyVariant<RegistryEntry[]>(reply, "all"))
    .then((entries) => {
      const tags = new Set<string>();
      for (const [, announced] of entries) {
        for (const tag of announced) tags.add(tag);
      }
      return [...tags].sort();
    });

/** The registry as a per-node map: hex node key -> the executor tags that node
 *  announced. Same `All` query as `capabilities`, but keeps the node key so a
 *  member row can show what THAT node runs. Empty map when nothing is
 *  announced. */
export const capabilitiesByNode = (
  transport: NodeTransport,
): Promise<Map<string, string[]>> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "all"))
    .then((reply) => replyVariant<RegistryEntry[]>(reply, "all"))
    .then((entries) => new Map(entries.map(([key, tags]) => [keyHex(key), tags])));
