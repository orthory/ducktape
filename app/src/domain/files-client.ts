// Typed client for the node's `files` module — the TS mirror of
// `crates/apps/files-interface`. A Manifest is the CONSENSUS truth about a file:
// its identity, size, and ordered chunk digests. The chunk BYTES never enter
// consensus — they live in the node's content-addressed blob store (the same one
// `putBlob`/`getBlob` speak) and are verified by the receiver against the
// committed digests.
//
// Upload = chunk locally, `putBlob` each chunk (which returns sha256(bytes) hex
// == the chunk digest), then `AddManifest` with the digest list; the module
// computes the whole-file `digest` and origin-derived `owner`. Download = `Stat`
// the manifest, `getBlob` each chunk, `verifyChunk` it, concatenate.
//
// camelCase params in, verbatim serde wire out, pure fns over a NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Write-time caps (consensus constants, mirrored for pre-validation) ─

export const MAX_FILE_ID_BYTES = 256;
export const MAX_NAME_BYTES = 512;
export const MAX_MIME_BYTES = 128;
export const MIN_CHUNK_SIZE = 4 * 1024;
export const MAX_CHUNK_SIZE = 4 * 1024 * 1024;
export const MAX_CHUNKS = 4096;
export const MAX_LIST_LIMIT = 256;

// ── Wire types (verbatim serde shapes) ──────────────────

/** A content-addressed file manifest — the consensus commitment to one file. */
export interface Manifest {
  file_id: string;
  name: string;
  mime: string;
  size: number;
  chunk_size: number;
  /** per-chunk sha256 digests, in file order (64-char lowercase hex) */
  chunks: string[];
  /** whole-file commitment: sha256 over the concatenated chunk digest RAW bytes.
   *  A digest-of-digests, computed by the module — never trusted alone (use
   *  verifyChunk per chunk). */
  digest: string;
  /** origin-derived owner, set by the module */
  owner: string;
  created_at_height: number;
}

const TARGET = "files";

// ── sha256 + chunk verification (mirror of the Rust helpers) ─

/** sha256(bytes) as 64-char lowercase hex — the DigestHex rendering. */
export const digestHex = async (bytes: Uint8Array<ArrayBuffer>): Promise<string> => {
  const buf = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
};

/** The exact byte length a manifest implies for chunk `index`: `chunk_size` for
 *  every chunk except the last, and `size - (n-1)*chunk_size` for the last. */
export const expectedChunkLen = (manifest: Manifest, index: number): number => {
  const n = manifest.chunks.length;
  if (index === n - 1) return manifest.size - (n - 1) * manifest.chunk_size;
  return manifest.chunk_size;
};

/** Receiver-side verification of one fetched chunk against a committed manifest.
 *  Checks BOTH the digest AND the exact implied length — a digest match alone is
 *  not sufficient (a manifest can commit sha256("") for a chunk while claiming a
 *  non-zero size). Rejects (throws) on any mismatch. */
export const verifyChunk = async (
  manifest: Manifest,
  index: number,
  bytes: Uint8Array<ArrayBuffer>,
): Promise<void> => {
  const n = manifest.chunks.length;
  if (index >= n) throw new Error(`chunk index ${index} out of range (${n} chunks)`);
  const expected = expectedChunkLen(manifest, index);
  if (bytes.length !== expected) {
    throw new Error(
      `chunk ${index} length mismatch: manifest implies ${expected} bytes, got ${bytes.length}`,
    );
  }
  const got = await digestHex(bytes);
  if (got !== manifest.chunks[index]) {
    throw new Error(
      `chunk ${index} digest mismatch: committed ${manifest.chunks[index]}, got ${got}`,
    );
  }
};

// ── Msgs (writes) ───────────────────────────────────────

/** Register a manifest. The submitter supplies only identity, shape, and the
 *  chunk digest list; the module computes `digest` and `owner`. */
export const addManifest = (
  transport: NodeTransport,
  params: {
    fileId: string;
    name: string;
    mime: string;
    size: number;
    chunkSize: number;
    chunks: string[];
  },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    add_manifest: {
      file_id: params.fileId,
      name: params.name,
      mime: params.mime,
      size: params.size,
      chunk_size: params.chunkSize,
      chunks: params.chunks,
    },
  });

/** Remove a manifest. Owner-gated: only the stored owner origin may remove, so
 *  the caller must submit under the SAME identity that added it (the console
 *  omits `origin`, so both ride the daemon's default identity). */
export const removeManifest = (
  transport: NodeTransport,
  fileId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { remove_manifest: { file_id: fileId } });

// ── Queries (reads over committed state) ────────────────

export const stat = (
  transport: NodeTransport,
  fileId: string,
): Promise<Manifest | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { stat: { file_id: fileId } }))
    .then((reply) => replyVariant<Manifest | null>(reply, "stat"));

/** Manifests whose file_id starts with `prefix`, at most `limit` (clamped). */
export const list = (
  transport: NodeTransport,
  params: { prefix?: string; limit?: number },
): Promise<Manifest[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        list: { prefix: params.prefix ?? "", limit: params.limit ?? MAX_LIST_LIMIT },
      }),
    )
    .then((reply) => replyVariant<Manifest[]>(reply, "list"));

// ── Upload / download (chunk plane over putBlob / getBlob) ─

/** Pick a chunk size in [MIN_CHUNK_SIZE, MAX_CHUNK_SIZE] that keeps the chunk
 *  count within MAX_CHUNKS for `size` bytes. Starts at 256 KiB and grows by
 *  doubling until the count fits. */
export const planChunkSize = (size: number): number => {
  let chunkSize = 256 * 1024;
  if (chunkSize < MIN_CHUNK_SIZE) chunkSize = MIN_CHUNK_SIZE;
  while (chunkSize < MAX_CHUNK_SIZE && Math.ceil(size / chunkSize) > MAX_CHUNKS) {
    chunkSize = Math.min(chunkSize * 2, MAX_CHUNK_SIZE);
  }
  return chunkSize;
};

/** Stage a file's bytes into the node's blob store and commit its manifest.
 *  Chunks locally, `putBlob`s each chunk (whose returned sha256 IS the chunk
 *  digest), then `AddManifest`. `onProgress` (0..1) reports staging progress. */
export const uploadFile = async (
  transport: NodeTransport,
  params: {
    fileId: string;
    name: string;
    mime: string;
    bytes: Uint8Array<ArrayBuffer>;
    chunkSize?: number;
    onProgress?: (fraction: number) => void;
  },
): Promise<BlockEvent> => {
  const size = params.bytes.length;
  const chunkSize = params.chunkSize ?? planChunkSize(size);
  const count = size === 0 ? 0 : Math.ceil(size / chunkSize);
  const chunks: string[] = [];
  for (let i = 0; i < count; i += 1) {
    const start = i * chunkSize;
    // a fresh ArrayBuffer-backed copy so the fetch body is a plain buffer
    const slice = params.bytes.slice(start, Math.min(start + chunkSize, size));
    const digest = await transport.putBlob(slice);
    chunks.push(digest);
    params.onProgress?.((i + 1) / count);
  }
  return addManifest(transport, {
    fileId: params.fileId,
    name: params.name,
    mime: params.mime,
    size,
    chunkSize,
    chunks,
  });
};

/** Reassemble a file's bytes from the blob store, verifying every chunk against
 *  the committed manifest before trusting it. */
export const downloadFile = async (
  transport: NodeTransport,
  manifest: Manifest,
): Promise<Uint8Array<ArrayBuffer>> => {
  const out = new Uint8Array(manifest.size);
  let offset = 0;
  for (let i = 0; i < manifest.chunks.length; i += 1) {
    const bytes = await transport.getBlob(manifest.chunks[i]);
    await verifyChunk(manifest, i, bytes);
    out.set(bytes, offset);
    offset += bytes.length;
  }
  return out;
};
