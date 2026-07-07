// Typed client for the node's `files` module — the TS mirror of
// `crates/apps/files` (duckfs: a consensus-replicated, copy-on-write,
// content-addressed FILESYSTEM, not a flat manifest table). Paths are absolute
// ("/" is the root; every path starts with "/"), the tree is snapshot-versioned,
// and a write is an atomic multi-path `Commit` with per-path CAS against a base
// snapshot.
//
// Upload = inline for small files (b64 inside the commit op, ≤256 KiB total),
// else chunk at 1 MiB and `filesStage` each (the digest comes back from the
// node — the client computes nothing consensus-critical) then reference the
// digests in a `Commit`. Download = `readAll` (page `read` to eof). Directory
// browse = `ls` (name-ordered, cursor-paged). refs + diff have no dedicated
// route yet and ride the generic `query` lane.
//
// The wire shapes live in transport.ts (one home for the whole files plane, next
// to BlockRecord); this file re-exports them and layers the operations + the
// consensus caps on top. Everything is a pure function over a NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

export type {
  FileEntry,
  FileEntryKind,
  FileSnapshot,
  FileRefs,
  FileDiffEntry,
  FileDiffKind,
  FileContent,
  FileChange,
  FilePage,
  FileReadRange,
} from "./transport";
import type {
  FileChange,
  FileContent,
  FileDiffEntry,
  FileEntry,
  FilePage,
  FileRefs,
  FileSnapshot,
} from "./transport";

// ── Consensus caps (mirrored from crates/apps/files/src/wire.rs) ──────
//
// These are the module's execute-time rejection bounds; the client mirrors the
// load-bearing ones for pre-validation and chunk planning. wire.rs is the source
// of truth — keep these in step with it.

/** Fixed chunk size for large files (`CHUNK_SIZE`) and the per-stage body cap. */
export const CHUNK_SIZE = 1024 * 1024;
/** Total inline bytes a single commit may carry (`MAX_INLINE_COMMIT_BYTES`);
 *  a file at or under it rides inside the commit op instead of being staged. */
export const MAX_INLINE_COMMIT_BYTES = 256 * 1024;
/** Per-`read` byte ceiling (`MAX_READ_BYTES`) — the paging step for `readAll`. */
export const MAX_READ_BYTES = 1024 * 1024;
/** Default listing/query page size (`MAX_PAGE`). */
export const MAX_PAGE = 256;
/** Absolute path byte cap (`MAX_PATH_BYTES`). */
export const MAX_PATH_BYTES = 4096;
/** Single path-segment name byte cap (`MAX_NAME_BYTES`). */
export const MAX_NAME_BYTES = 255;
/** Changes per commit (`MAX_CHANGES_PER_COMMIT`). */
export const MAX_CHANGES_PER_COMMIT = 4096;
/** Commit-message byte cap (`MAX_MESSAGE_BYTES`). */
export const MAX_MESSAGE_BYTES = 4096;

const TARGET = "files";

// ── base64 (browser-safe, chunked so a large file doesn't blow the arg cap) ─

const B64_CHUNK = 0x8000;

/** Uint8Array → standard base64. Chunked through `String.fromCharCode` so a
 *  multi-hundred-KiB inline body never exceeds the apply-spread arg limit. */
export const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  for (let i = 0; i < bytes.length; i += B64_CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(i, i + B64_CHUNK));
  }
  return btoa(binary);
};

/** base64 → Uint8Array (ArrayBuffer-backed). The inverse of `bytesToBase64`,
 *  used to decode a `read` range's `b64` back to bytes. */
export const base64ToBytes = (b64: string): Uint8Array<ArrayBuffer> => {
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
};

// ── Path helpers ────────────────────────────────────────

/** The last segment of an absolute path (the file/dir name); "/" → "". */
export const basename = (path: string): string => {
  const trimmed = path.replace(/\/+$/, "");
  const slash = trimmed.lastIndexOf("/");
  return slash < 0 ? trimmed : trimmed.slice(slash + 1);
};

/** Join a directory path and a child name into one absolute path. The root "/"
 *  never doubles its slash. */
export const joinPath = (dir: string, name: string): string =>
  dir === "/" ? `/${name}` : `${dir.replace(/\/+$/, "")}/${name}`;

// ── Reads ───────────────────────────────────────────────

/** One page of a directory's entries in name order (GET /v1/files/ls). Echo the
 *  returned `next` as the following call's `after` to page. */
export const ls = (
  transport: NodeTransport,
  params: { path: string; snapshot?: string; after?: string; limit?: number },
): Promise<FilePage> => transport.filesLs(params);

/** The entry at `path` (kind/size/exec/object/meta), or null when absent. */
export const stat = (
  transport: NodeTransport,
  params: { path: string; snapshot?: string },
): Promise<FileEntry | null> => transport.filesStat(params);

/** A byte range of a file (base64 + eof). `len` is clamped to MAX_READ_BYTES. */
export const read = (
  transport: NodeTransport,
  params: { path: string; snapshot?: string; offset?: number; len?: number },
): Promise<{ b64: string; eof: boolean }> => transport.filesRead(params);

/** Reassemble a whole file by paging `read` at MAX_READ_BYTES until eof. Returns
 *  the concatenated bytes; a page that yields nothing without signalling eof
 *  breaks the loop rather than spinning forever. */
export const readAll = async (
  transport: NodeTransport,
  params: { path: string; snapshot?: string },
): Promise<Uint8Array<ArrayBuffer>> => {
  const parts: Uint8Array[] = [];
  let offset = 0;
  for (;;) {
    const range = await transport.filesRead({
      path: params.path,
      snapshot: params.snapshot,
      offset,
      len: MAX_READ_BYTES,
    });
    const bytes = base64ToBytes(range.b64);
    if (bytes.length > 0) {
      parts.push(bytes);
      offset += bytes.length;
    }
    if (range.eof || bytes.length === 0) break;
  }
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let cursor = 0;
  for (const part of parts) {
    out.set(part, cursor);
    cursor += part.length;
  }
  return out;
};

/** The bounded commit history, newest-first (GET /v1/files/history). */
export const history = (
  transport: NodeTransport,
  params: { limit?: number } = {},
): Promise<FileSnapshot[]> => transport.filesHistory(params);

/** The refs image — live head, named pins, window length. Over the generic
 *  query lane (no dedicated route): reply `{ refs: {...} }`. */
export const refs = (transport: NodeTransport): Promise<FileRefs> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { refs: {} }))
    .then((reply) => replyVariant<FileRefs>(reply, "refs"));

/** The path-level diff between two snapshots under an optional prefix. Over the
 *  generic query lane: reply `{ diff: [...] }`. */
export const diff = (
  transport: NodeTransport,
  params: { from: string; to: string; prefix?: string },
): Promise<FileDiffEntry[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        diff: { from: params.from, to: params.to, prefix: params.prefix ?? "" },
      }),
    )
    .then((reply) => replyVariant<FileDiffEntry[]>(reply, "diff"));

/** Paths under a raw path prefix, in full-path order (used for the global file
 *  index that feeds search). Over the generic query lane: reply
 *  `{ find: { entries, next } }`. */
export const find = (
  transport: NodeTransport,
  params: { prefix?: string; snapshot?: string; after?: string; limit?: number } = {},
): Promise<FilePage> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        find: {
          prefix: params.prefix ?? "/",
          snapshot: params.snapshot ?? null,
          after: params.after ?? null,
          limit: params.limit ?? MAX_PAGE,
        },
      }),
    )
    .then((reply) => replyVariant<FilePage>(reply, "find"));

// ── Writes ──────────────────────────────────────────────

/** Stage one chunk's raw bytes and return its object-id digest (64-hex). The
 *  digest is the node's — the client trusts it and never recomputes. */
export const stage = (
  transport: NodeTransport,
  bytes: Uint8Array<ArrayBuffer>,
): Promise<string> => transport.filesStage(bytes).then((r) => r.digest);

/** Commit an atomic change set against a base snapshot. `baseSnapshot` null is
 *  the empty tree (a first commit); the per-path CAS checks the changed paths
 *  against the live head. */
export const commit = (
  transport: NodeTransport,
  params: { baseSnapshot: string | null; message: string; changes: FileChange[] },
): Promise<BlockEvent> =>
  transport.filesCommit({
    base_snapshot: params.baseSnapshot,
    message: params.message,
    changes: params.changes,
  });

/** Resolve the base snapshot for a write: the caller's explicit choice (which
 *  may legitimately be null), else the live head read from refs. Reading head
 *  here keeps CAS tight — the commit checks the changed path against a base that
 *  is current as of submit. */
const resolveBase = async (
  transport: NodeTransport,
  explicit: string | null | undefined,
): Promise<string | null> =>
  explicit !== undefined ? explicit : (await refs(transport)).head;

/** Build a file's Content: inline for small files, else stage each 1 MiB chunk
 *  and reference the returned digests (in order). `onProgress(staged, total)`
 *  reports staged-chunk progress. */
const buildContent = async (
  transport: NodeTransport,
  bytes: Uint8Array<ArrayBuffer>,
  onProgress?: (staged: number, total: number) => void,
): Promise<FileContent> => {
  if (bytes.length <= MAX_INLINE_COMMIT_BYTES) {
    return { inline: { b64: bytesToBase64(bytes) } };
  }
  const total = Math.ceil(bytes.length / CHUNK_SIZE);
  const chunks: string[] = [];
  for (let i = 0; i < total; i += 1) {
    const start = i * CHUNK_SIZE;
    // a fresh ArrayBuffer-backed slice so the fetch body is a plain buffer.
    const slice = bytes.slice(start, Math.min(start + CHUNK_SIZE, bytes.length));
    chunks.push(await stage(transport, slice));
    onProgress?.(i + 1, total);
  }
  return { chunks: { size: bytes.length, chunks } };
};

/** Upload a file into the tree at `path`: stage its chunks (or inline it), then
 *  Commit a `put`. Resolves to the committing block. */
export const uploadFile = async (
  transport: NodeTransport,
  params: {
    path: string;
    bytes: Uint8Array<ArrayBuffer>;
    exec?: boolean;
    meta?: Record<string, string>;
    message?: string;
    /** Explicit base snapshot; omit to resolve the live head automatically. */
    baseSnapshot?: string | null;
    onProgress?: (staged: number, total: number) => void;
  },
): Promise<BlockEvent> => {
  const [base, content] = await Promise.all([
    resolveBase(transport, params.baseSnapshot),
    buildContent(transport, params.bytes, params.onProgress),
  ]);
  const change: FileChange = {
    put: {
      path: params.path,
      exec: params.exec ?? false,
      meta: params.meta ?? {},
      content,
    },
  };
  return commit(transport, {
    baseSnapshot: base,
    message: params.message ?? `upload ${basename(params.path)}`,
    changes: [change],
  });
};

/** Remove the entry at `path` (file, symlink, or whole subtree). */
export const deletePath = async (
  transport: NodeTransport,
  params: { path: string; message?: string; baseSnapshot?: string | null },
): Promise<BlockEvent> => {
  const base = await resolveBase(transport, params.baseSnapshot);
  return commit(transport, {
    baseSnapshot: base,
    message: params.message ?? `rm ${params.path}`,
    changes: [{ rm: { path: params.path } }],
  });
};

/** Create a directory at `path` (its parents must already exist). */
export const mkdir = async (
  transport: NodeTransport,
  params: { path: string; message?: string; baseSnapshot?: string | null },
): Promise<BlockEvent> => {
  const base = await resolveBase(transport, params.baseSnapshot);
  return commit(transport, {
    baseSnapshot: base,
    message: params.message ?? `mkdir ${params.path}`,
    changes: [{ mkdir: { path: params.path } }],
  });
};
