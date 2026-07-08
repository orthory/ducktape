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

/** One provider's slice of a node's announced executor tags. */
export interface ProviderGroup {
  /** The provider key — the tag text before the first `_`, or the whole tag
   *  when it has none. `claude`, `claude_opus_high`, and `claude_fable_low` all
   *  collapse to `claude`; `codex_gpt-5.5-high` (one `_`, hyphenated tail)
   *  collapses to `codex`. */
  provider: string;
  /** Title-cased provider name for display ("claude" → "Claude"). */
  label: string;
  /** Distinct models under the provider, with the effort suffix dropped and in
   *  first-seen order: `claude_{fable,haiku,opus,sonnet}_*` → `fable, haiku,
   *  opus, sonnet`. Empty when the node announced only the provider's bare
   *  default tag (no model). This is what a member/peer surface shows — a node
   *  runs "Claude: opus, sonnet", not sixteen effort permutations. */
  models: string[];
  /** Every raw tag this node announced under the provider, deduped in announced
   *  order. Kept for fidelity (tooltips / exact routing tags). */
  tags: string[];
}

// Effort is a routing knob, not something a directory browses, so it is dropped
// from the model name. Only a KNOWN trailing token is stripped, so a model that
// merely ends in a word is never truncated.
const EFFORT_TOKENS = new Set([
  "minimal",
  "low",
  "medium",
  "high",
  "max",
  "xhigh",
  "none",
  "default",
]);

/** The model a tag names, effort stripped: `claude_fable_high` → `fable`,
 *  `codex_gpt-5.5-high` → `gpt-5.5`, `codex_gpt-5.5-codex-xhigh` →
 *  `gpt-5.5-codex`. The bare provider tag (`claude`) names no model → null. */
const modelOf = (tag: string, provider: string): string | null => {
  if (tag === provider) return null;
  const rest = tag.startsWith(`${provider}_`) ? tag.slice(provider.length + 1) : tag;
  const split = rest.match(/^(.+)[_-]([a-z0-9]+)$/i);
  if (split && EFFORT_TOKENS.has(split[2].toLowerCase())) return split[1];
  return rest;
};

/** Collapse a node's announced executor tags into one entry per provider, each
 *  carrying its distinct models. The registry announces a distinct tag per
 *  provider×model×effort combo, so a busy node lists dozens
 *  (`claude_opus_high`, `codex_gpt-5.5-high`, …); a member or peer surface only
 *  wants WHICH providers it runs and, one level down, WHICH models — never the
 *  effort permutations. Grouping is by the provider key (text before the first
 *  `_`), which buckets both the 3-part `claude_opus_high` grammar and the
 *  hyphenated `codex_gpt-5.5-high` opaque tails a naive `_`-split would mis-key.
 *  First-seen order is preserved so the display stays stable across renders. */
export const providersOf = (tags: string[]): ProviderGroup[] => {
  const order: string[] = [];
  const byProvider = new Map<string, { tags: string[]; models: string[] }>();
  for (const tag of tags) {
    const cut = tag.indexOf("_");
    const provider = cut > 0 ? tag.slice(0, cut) : tag;
    let group = byProvider.get(provider);
    if (!group) {
      group = { tags: [], models: [] };
      byProvider.set(provider, group);
      order.push(provider);
    }
    if (!group.tags.includes(tag)) group.tags.push(tag);
    const model = modelOf(tag, provider);
    if (model !== null && !group.models.includes(model)) group.models.push(model);
  }
  return order.map((provider) => {
    const group = byProvider.get(provider) as { tags: string[]; models: string[] };
    return {
      provider,
      label: provider ? provider[0].toUpperCase() + provider.slice(1) : provider,
      models: group.models,
      tags: group.tags,
    };
  });
};
