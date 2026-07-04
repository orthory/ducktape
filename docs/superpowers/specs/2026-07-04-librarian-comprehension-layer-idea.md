# Librarian — a human-comprehension layer for ducktape (IDEA)

- **Status:** IDEA. Not a spec, not planned, **deliberately not built.** Parked 2026-07-04.
- **Origin:** a conversation asking whether ducktape has an "agentic memory system / MCP-integrated storage,"
  triggered by looking at the philosophy of `gudos-team/lattice`.
- **Scope of this doc:** capture the concept and the open tensions so it can be picked up later. No implementation
  is implied or approved.

---

## TL;DR

ducktape already has `crates/apps/memory` — a consensus, filesystem-shaped ("NoKV") **raw-truth** store for agents.
It is an excellent Layer-1 substrate. It is **not** an agentic memory system in the Lattice sense, and it structurally
**cannot** become one, because Lattice's identity is LLM-driven synthesis and a consensus module must be deterministic.

`Librarian` is the missing tier: a **node-local, LLM-compiled comprehension layer** that reads the verifiable raw truth
below the determinism boundary and produces artifacts a **human** can understand — a restart briefing (boot packet), a
living wiki page, a "what happened / why / where do I restart" digest. It is Lattice's L2–L4 (compiled wiki, retrieval,
session compiler) mapped onto ducktape's substrate, with most of the primitives already present in the codebase.

---

## 1. The question that started this

"Do we have an agentic memory system, like MCP-integrated essential storage?"

- The connected MCP servers (Vercel, Adobe, Google Drive) are service integrations, not memory.
- Claude Code's file-based auto-memory exists but is a flat markdown index, not on ducktape.
- **`crates/apps/memory` + `memory-interface` already exist**, are registered in genesis on both `bin/node` and
  `bin/noded`, statesync'd, and referenced by the app frontend. ~1400 LOC, 24 tests green. It is real and mature.

So the substrate exists. The question became: is it the same thing as Lattice? No — and understanding *why not* is the
whole idea.

## 2. The fundamental difference — the determinism boundary

`memory` and Lattice are the same wall approached from two sides:

- **From mechanism:** a consensus module must be deterministic. `memory`'s grep is substring-only (no regex) *by
  necessity*; it can never call an LLM; it stores and returns but never **synthesizes**. Lattice's whole value —
  capture normalization, rescoring, lane compile, boot packet — is LLM-driven synthesis, which is non-deterministic and
  therefore cannot live inside a consensus module.
- **From purpose:** `memory` is a **machine substrate**. Its verbs (`ls`/`stat`/`read`/`find`/`grep`), `duck://`
  citation URIs, and write-once generations are things an *agent* consumes. It has no opinion about whether a *human*
  could understand what is in it. Lattice is built for **human understanding** — its north star is a "living LLM wiki"
  and a boot packet that re-orients a person.

Human understanding requires synthesis / judgment / narrative → that needs an LLM (or a human) → non-deterministic →
off consensus. The two framings meet at the same conclusion: **the new layer lives off-consensus, node-local, and its
identity is comprehension, not storage.**

## 3. What Librarian is

A node-local comprehension layer. The layering rule (which is exactly Lattice's guardrail "compiled views are
replaceable") is: **truth lives below and is shared + verifiable; understanding lives above and is local + throwaway.**

```
┌─ node-local · non-deterministic · LLM ─────────────── for humans ─┐
│  librarian:  subscribe → compile → serve                          │  ← NEW (understanding)
│  outputs: boot packet / wiki page / "what happened" digest        │
└──────────────────────── determinism boundary ────────────────────┘
┌─ consensus · deterministic · verifiable ───────────── for machines ┐
│  memory (raw truth) · chat · tasks · agent · forge · files · …     │  ← EXISTING (substrate)
└───────────────────────────────────────────────────────────────────┘
```

Compiled views are **not** put on consensus. They are a node-local cache you can delete and recompile. That makes them
cheap, private, and — importantly — *subjective per node* (see §7).

## 4. The engine (one pipeline, shared by both lenses)

```
raw truth  →  compile (LLM, node-local)  →  serve
  (memory,       distill → boot packet         app view (human)
   events)                                     MCP surface (external agents: Claude Code / codex)
```

The MCP surface is the direct answer to the original "MCP-integrated memory" question: an external coding agent pulls
the compiled boot packet at session start and writes captures back into raw truth.

## 5. ducktape already has the primitives

Librarian is mostly **orchestration + one LLM compile step** over primitives that already exist:

| Lattice concept | ducktape-native mapping | status |
|---|---|---|
| capture (decision / question / pattern / note) | `memory` Publish + `kind=` meta (`META_KIND` exists; `kind=skill` already used) | exists |
| lineage / superseded | `memory` immutable generations | exists |
| `as_of` time travel | `memory` snapshots | exists |
| citation grounding | `duck://memory/<path>@<gen>#L<n>` evidence URIs | exists |
| skill / procedural memory (Lattice Horizon 3) | `memory` `/skills/` + `kind=skill` | exists |
| lane = coordination ledger | `tasks` module (Lattice defines "task" as exactly a lane-local ledger) | exists |
| review inbox / review pressure | `inbox` module | exists (reuse TBD) |
| **the LLM compile step itself** | **`agent-oracle`** — ducktape's existing off-consensus LLM oracle pattern | exists (reuse) |
| **boot packet / wiki synthesis** | ❌ the one gap Librarian fills | NEW |

The key leverage: **`agent-oracle` already solves "how to do non-deterministic LLM work relative to consensus."**
Librarian is another oracle consumer, not a new LLM plumbing project.

## 6. Two lenses, one engine

- **Lens A — personal restart.** Subscribe to *my* captures + *my* agent's execution traces → compile *my* boot packet.
  This is Lattice's core, verbatim.
- **Lens B — network comprehension.** Subscribe to the network's consensus event stream (chat, tasks, agent runs, forge
  pushes) → compile a "what is happening" digest / wiki. This is the *same compiler* pointed at a different stream.

They are **not two projects.** They are one engine with two subscriptions and two read-scopes.

**Recommended sequencing (parked, not now):** build the engine against **Lens A first** — it is the hardest and
highest-value core (compile quality is the hard part, and A pressures it most purely), the most self-contained (one
user's data → simplest privacy/scope), matches the original ask, and mirrors Lattice's own "Restart Core first"
strategy. Lens B then falls out as a follow-on spec: "point the same compile at the consensus event stream."

## 7. The property that makes it human

Because compilation is non-deterministic, **every node's wiki / briefing differs.** Normally that is a bug; here it is
the feature:

> **Facts are shared and verifiable (consensus `memory`); understanding is subjective and per-reader (node-local).**

That is exactly how humans work — the same facts, understood differently by each person. Lattice's "compiled views are
replaceable" guardrail lands naturally as "each node's subjective reading."

## 8. Open tensions (parked, developed)

### 8.1 Privacy — where personal raw truth lives (the structural fork for Lens A)

Consensus `memory` is **network-global and byte-identical to everyone**. Writing a personal capture there publishes it
to the whole network. So the two lenses cannot share a raw-truth home:

- **Lens B** raw truth is already-public consensus activity → no problem.
- **Lens A** raw truth must be private. Options:
  - (a) **Pure node-local store** — private, but not durable across machines, not portable, not tamper-evident.
  - (b) **`vaults` module** — if it provides per-user private (ideally encrypted) storage, this gives durability +
    privacy together. Investigate `vaults` capabilities first; this is the most promising path.
  - (c) **Hybrid anchor** — keep the private body local/in-vault, but publish only a *hash anchor* (provenance) to
    consensus for tamper-evidence without revealing content.

Decision deferred; it hinges on what `vaults` can actually do. This is the single biggest structural fork for Lens A.

### 8.2 The observation seam — off-consensus observing on-consensus

`memory` emits `MemoryEvent::Published` only to **watcher modules** via in-consensus dispatch. Librarian is not a
consensus module, so it cannot be a watch target. How does a node-local service see committed truth?

- (a) **Poll queries** (`ls`/`find`/`grep`/`read`) periodically — simple, but polling; no fine-grained push.
- (b) **Tap the local node's block-commit stream** — when the local node commits a block, expose the committed module
  events / state deltas to node-local subscribers. The app frontend (`DucktapeProvider`) already reads module state, so
  a local read path has precedent; the open question is whether there is (or should be) a local *event* tap, not just
  *state* reads. **This is the recommended path** and the key new integration point in the runtime.
- (c) A dedicated consensus "egress" module that `RegisterWatch`es and forwards — but a consensus module cannot perform
  a non-deterministic local side effect (socket write), so egress must be done by the node runtime / host anyway. This
  collapses back into (b).

Recommendation: (b) — a node-local commit/event tap consumed by Librarian.

### 8.3 Staying an understanding tool, not a PM suite

Lattice's repeated guardrail: do not drift into a project-management / scheduler / governance suite; task is a
coordination unit only. For Librarian this means its output is **understanding** (boot packet, wiki, digest), never
**control** (it does not assign tasks, schedule, or gate anything). It may *read* `tasks` / `inbox` to compile
understanding, but must not become the thing that *manages* them. Keep the write surface minimal: Librarian writes
compiled views (node-local) and at most review-signal captures; it does not mutate product state. Success test, borrowed
from Lattice: "does this raise restart / comprehension speed, or does it only add governance tax?"

## 9. Deliberately not deciding now

Parked under YAGNI. The substrate (`memory`) is complete and needs nothing. Librarian is a real, coherent next layer but
is not being built. Triggers that would justify starting: a concrete need for external agents (Claude Code / codex) to
share a durable, restart-quality memory across sessions on this network, or Lens-B network-comprehension becoming a
product requirement.

## 10. Pointers

- Substrate: `crates/apps/memory`, `crates/apps/memory-interface` (NoKV consensus filesystem; `duck://` citations;
  generations; snapshots; `/skills/` + `kind=skill`).
- LLM/oracle precedent: `crates/apps/agent`, `crates/apps/agent-oracle`.
- Candidate reuse: `crates/apps/vaults` (private per-user truth for Lens A), `crates/apps/inbox` (review pressure),
  `crates/apps/tasks` (lane-as-ledger).
- Prior art / philosophy: `gudos-team/lattice` — `docs/lattice_vision.md`, `docs/lattice_best_practices.md`,
  `AGENTS.md` (capture normalization contract). Lattice is single-operator / Postgres / LLM-compiled; the ducktape
  substrate is a stronger raw-truth layer, but the compile philosophy is what Librarian borrows.
