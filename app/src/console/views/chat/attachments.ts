// Chat attachment UPLOAD + name hygiene. A pasted file lands in duckfs under
// ATTACHMENTS_ROOT/<uuid>/<name>; the composer then inserts a markdown
// reference to it (`![name](duck://files/<path>)`) — the ref GRAMMAR lives in
// `duck-ref.ts` and the module table (incl. the render-side confinement to
// ATTACHMENTS_ROOT) in `domain/duck-uri.ts`. This module owns only the write
// side and the upload-time name transform.
//
// Security posture (audited): each upload gets a fresh uuid directory (no
// collisions, no cross-sender overwrite); names are sanitized to a single
// markdown- and path-safe segment.

import { ATTACHMENTS_ROOT, UNSAFE_NAME_CHARS } from "../../../domain/duck-uri";
import { MAX_NAME_BYTES, uploadFile } from "../../../domain/files-client";
import type { BlockEvent, NodeTransport } from "../../../domain/transport";

/** Paste-upload cap. Every byte is replicated through consensus blocks on
 *  every node, and a paste is the easiest accident in the app — keep it an
 *  order of magnitude under the object facade's 64 MiB. */
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;

/** Image extensions the chip previews inline. Everything else (including
 *  svg — script-bearing by design) downloads instead of rendering. */
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "avif"]);

/** True when the chip may render an inline <img> preview. Extension-based on
 *  purpose: previews of OTHER senders' attachments must not need a metadata
 *  query, and an <img> renders nothing executable regardless of real bytes. */
export const isImageName = (name: string): boolean => {
  const dot = name.lastIndexOf(".");
  return dot > 0 && IMAGE_EXTENSIONS.has(name.slice(dot + 1).toLowerCase());
};

/** Collapse an arbitrary (attacker-typed) filename to one safe duckfs path
 *  segment: path separators, brackets, and markdown-active chars (`* ( )`, so
 *  the name is safe inside a `![..](..)` ref) become `-`, whitespace runs
 *  become one `-`, control/bidi drop, leading dots strip (no dotfiles, no
 *  `..`), clamped to the module's name byte cap. Empty in → "file". */
export const sanitizeAttachmentName = (raw: string): string => {
  let name = raw
    // NFC first: the duckfs module rejects non-NFC path segments, so an NFD
    // name (common from macOS) would pass every rule below then fail the commit.
    .normalize("NFC")
    .trim()
    .replace(/[/\\[\]*()]/g, "-")
    .replace(UNSAFE_NAME_CHARS, "")
    .replace(/\s+/g, "-")
    .replace(/^\.+/, "");
  if (name === "") name = "file";
  const encoder = new TextEncoder();
  while (encoder.encode(name).length > MAX_NAME_BYTES) {
    name = name.slice(0, name.length - 1);
  }
  return name || "file";
};

/** Upload one pasted file into duckfs and return the committed path + sanitized
 *  name (the composer builds the markdown ref). The fresh uuid directory
 *  isolates every upload: no name collisions, no cross-sender overwrites, and
 *  the per-path CAS never conflicts. */
export const uploadAttachment = async (
  transport: NodeTransport,
  file: { name: string; type: string; bytes: Uint8Array<ArrayBuffer> },
): Promise<{ path: string; name: string; isImage: boolean; block: BlockEvent }> => {
  if (file.bytes.length > MAX_ATTACHMENT_BYTES) {
    throw new Error(
      `attachment exceeds ${Math.floor(MAX_ATTACHMENT_BYTES / (1024 * 1024))} MiB`,
    );
  }
  const name = sanitizeAttachmentName(file.name);
  const path = `${ATTACHMENTS_ROOT}/${crypto.randomUUID()}/${name}`;
  const block = await uploadFile(transport, {
    path,
    bytes: file.bytes,
    meta: file.type ? { mime: file.type } : {},
    message: `attach ${name}`,
  });
  return { path, name, isImage: isImageName(name), block };
};
