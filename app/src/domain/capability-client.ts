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
   *  collapse to `claude`; `codex`, `codex_gpt-5.5_high`, and
   *  `codex_gpt-5.3-codex-spark_xhigh` all collapse to `codex`. */
  provider: string;
  /** Title-cased provider name for display ("claude" → "Claude"). */
  label: string;
  /** Distinct models under the provider, in first-seen order, with the effort
   *  suffix dropped: `claude_opus_high` → `opus`, `codex_gpt-5.5_low` →
   *  `gpt-5.5`, `codex_gpt-5.3-codex-spark_xhigh` → `gpt-5.3-codex-spark`.
   *  Empty when the node announced only the provider's bare default tag. This
   *  is what a member/peer surface shows — a node runs "Claude: opus, sonnet",
   *  not every effort permutation. */
  models: string[];
  /** Every raw tag this node announced under the provider, deduped in announced
   *  order. Kept for fidelity (tooltips / exact routing tags). */
  tags: string[];
}

/** The model a tag names, effort dropped. Tags are `provider_model_effort`
 *  (the host spec composes `{provider}_{suffix}` where a variant suffix is
 *  `<model>_<effort>`), and neither model nor effort contains `_` — so the
 *  model is everything between the first and last `_`. The bare provider tag
 *  (`claude`) names no model → null. */
const modelOf = (tag: string, provider: string): string | null => {
  if (tag === provider || !tag.startsWith(`${provider}_`)) return null;
  const rest = tag.slice(provider.length + 1);
  const cut = rest.lastIndexOf("_");
  return cut > 0 ? rest.slice(0, cut) : rest;
};

/** Collapse a node's announced executor tags into one entry per provider, each
 *  carrying its distinct models. The registry announces a distinct tag per
 *  provider×model×effort combo, so a busy node lists dozens
 *  (`claude_opus_high`, `codex_gpt-5.5_xhigh`, …); a member or peer surface
 *  only wants WHICH providers it runs and, one level down, WHICH models — never
 *  the effort permutations. Grouping is by the provider key (text before the
 *  first `_`); first-seen order is preserved so the display stays stable across
 *  renders. */
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
