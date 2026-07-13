// Chat attachment UPLOAD + name hygiene. A pasted file lands in duckfs under
// ATTACHMENTS_ROOT/<uuid>/<name>; the composer then inserts a markdown
// reference to it (`![name](duck://files/<path>)`) — the ref GRAMMAR and its
// render-time tokenizer live in `duck-ref.ts`, the single place all duck://
// references (pages and files) are parsed. This module owns only the write
// side and the two name transforms it needs.
//
// Security posture (audited): each upload gets a fresh uuid directory (no
// collisions, no cross-sender overwrite); names are sanitized to a single
// markdown- and path-safe segment; the render-side confinement to
// ATTACHMENTS_ROOT lives in duck-ref.ts.

import { MAX_NAME_BYTES, uploadFile } from "../../../domain/files-client";
import type { BlockEvent, NodeTransport } from "../../../domain/transport";

export const ATTACHMENTS_ROOT = "/shared/attachments";

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

// Bidi overrides + zero-width chars: a hostile name can spoof its extension
// (`photo<RLO>gnp.exe` reads as `photoexe.png` in a label) or hide a dot.
// Strip them everywhere a name is shown or saved.
// eslint-disable-next-line no-control-regex
const UNSAFE_NAME_CHARS =
  /[\u0000-\u001f\u007f\u200b-\u200f\u202a-\u202e\u2066-\u2069\ufeff]/g;

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

/** The received-side display name — a ref's label/last-segment is authored by
 *  ANY sender, so strip control/bidi/zero-width chars before it reaches a
 *  label or a download filename. It cannot affect the fetch path (the
 *  tokenizer already confined that); this only stops label spoofing. */
export const displayName = (name: string): string =>
  name.replace(UNSAFE_NAME_CHARS, "") || "file";

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
