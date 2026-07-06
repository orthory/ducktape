// Typed client for the node's `memory` module — the TS mirror of
// `crates/apps/memory-interface`. Memory is a shared agent workspace shaped like
// a filesystem: every path is a metadata namespace with atomic write-once
// publish, generations are immutable, snapshots are time travel, reads are
// progressive-disclosure verbs (Ls/Stat/Read), and search is Find (by meta) or
// Grep (substring over inline bodies) yielding citable `duck://` URIs.
//
// Skills sharing rides the same namespace: a skill is a document under
// `/skills/<name>` with `kind=skill` meta, discovered via Find.
//
// `Body`/`PublishBody` are serde adjacently-tagged enums (`{kind, value}`); the
// *Msg/*Query/LsEntry enums are externally tagged. The types below mirror those
// shapes verbatim. camelCase params in, verbatim wire out, pure fns over an
// injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (verbatim serde shapes) ──────────────────

/** Free-form document metadata: a small key/value map. */
export type Meta = Record<string, string>;

/** Reserved meta key/value marking a skill document, and its home directory. */
export const META_KIND = "kind";
export const KIND_SKILL = "skill";
export const SKILLS_PREFIX = "/skills/";

/** A stored generation body — `#[serde(tag="kind", content="value")]`. */
export type Body =
  | { kind: "inline"; value: string }
  | { kind: "file"; value: { file_id: string; digest: string; size: number } };

/** Write-time body selector (file publishes copy the manifest digest server-side). */
export type PublishBody =
  | { kind: "inline"; value: string }
  | { kind: "file"; value: { file_id: string } };

export interface Generation {
  generation: number;
  body: Body;
  meta: Meta;
  /** origin-derived author, set by the module */
  author: string;
  published_at_height: number;
}

/** Summary view of a live file: latest generation's provenance + counters. */
export interface FileStat {
  path: string;
  latest_generation: number;
  generations: number;
  latest_meta: Meta;
  latest_author: string;
  latest_published_at_height: number;
  body_len: number;
}

/** One `Ls` entry — an implicit child directory, or a file with its stat. */
export type LsEntry = { dir: { path: string } } | { file: FileStat };

export const isDir = (e: LsEntry): e is { dir: { path: string } } => "dir" in e;

/** A single grep match with a citable `duck://memory/<path>@<gen>#L<line>` URI. */
export interface GrepHit {
  uri: string;
  path: string;
  generation: number;
  line: number;
  text: string;
}

const TARGET = "memory";

/** Query page ceiling (mirrors MAX_QUERY_LIMIT); larger limits are clamped. */
export const MAX_QUERY_LIMIT = 256;

// ── Body constructors (for the publish composer) ────────

export const inlineBody = (text: string): PublishBody => ({
  kind: "inline",
  value: text,
});

export const fileBody = (fileId: string): PublishBody => ({
  kind: "file",
  value: { file_id: fileId },
});

// ── Msgs (writes) ───────────────────────────────────────

/** Write-once publish: appends an immutable generation at `latest + 1`. */
export const publish = (
  transport: NodeTransport,
  params: { path: string; body: PublishBody; meta?: Meta; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { publish: { path: params.path, body: params.body, meta: params.meta ?? {} } },
    params.origin,
  );

/** Remove a file (all live generations); snapshots still pin what they captured. */
export const remove = (
  transport: NodeTransport,
  path: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { delete: { path } });

/** Pin the current `path -> latest generation` map of the whole namespace. */
export const snapshot = (
  transport: NodeTransport,
  name: string,
): Promise<BlockEvent> => transport.submit(TARGET, { snapshot: { name } });

export const dropSnapshot = (
  transport: NodeTransport,
  name: string,
): Promise<BlockEvent> => transport.submit(TARGET, { drop_snapshot: { name } });

// ── Queries (reads over committed state) ────────────────

/** Entries directly under `path` (child dirs + files), sorted, up to `limit`. */
export const ls = (
  transport: NodeTransport,
  params: { path: string; limit?: number },
): Promise<LsEntry[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        ls: { path: params.path, limit: params.limit ?? MAX_QUERY_LIMIT },
      }),
    )
    .then((reply) => replyVariant<LsEntry[]>(reply, "ls"));

export const stat = (
  transport: NodeTransport,
  path: string,
): Promise<FileStat | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { stat: { path } }))
    .then((reply) => replyVariant<FileStat | null>(reply, "stat"));

/** One generation of a file. `generation` and `snapshot` are mutually exclusive;
 *  neither reads the latest. */
export const read = (
  transport: NodeTransport,
  params: { path: string; generation?: number | null; snapshot?: string | null },
): Promise<Generation | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        read: {
          path: params.path,
          generation: params.generation ?? null,
          snapshot: params.snapshot ?? null,
        },
      }),
    )
    .then((reply) => replyVariant<Generation | null>(reply, "read"));

/** Live files under `prefix` whose latest meta matches every filter pair. This
 *  is how skills are listed: Find { prefix: "/skills/", metaFilter: {kind: skill} }. */
export const find = (
  transport: NodeTransport,
  params: { prefix: string; metaFilter?: Meta; limit?: number },
): Promise<FileStat[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        find: {
          prefix: params.prefix,
          meta_filter: params.metaFilter ?? {},
          limit: params.limit ?? MAX_QUERY_LIMIT,
        },
      }),
    )
    .then((reply) => replyVariant<FileStat[]>(reply, "find"));

/** Case-sensitive substring scan over inline latest generations under `prefix`. */
export const grep = (
  transport: NodeTransport,
  params: { prefix: string; pattern: string; limit?: number },
): Promise<GrepHit[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        grep: {
          prefix: params.prefix,
          pattern: params.pattern,
          limit: params.limit ?? MAX_QUERY_LIMIT,
        },
      }),
    )
    .then((reply) => replyVariant<GrepHit[]>(reply, "grep"));
