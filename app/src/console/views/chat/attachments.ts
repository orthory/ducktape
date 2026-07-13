// Chat attachments ride as `duck://files/<path>` URIs in ordinary message
// text — ZERO chat wire change (a new ChatMsg variant is a flag day; a URI in
// a plain span is not). The composer uploads a pasted file into duckfs under
// ATTACHMENTS_ROOT and inserts the URI; the renderer chips URIs under that
// root and ONLY that root — a chip claims "this was attached here", and only
// composer uploads earn it. Paths elsewhere stay literal text.
//
// Security posture (audited before merge):
// - names are sanitized to a single path segment: separators, dots-only,
//   control chars, and whitespace can never reach the duckfs path;
// - each upload lands under a fresh uuid directory, so pasting `a.png` twice
//   never CAS-conflicts and no sender can overwrite another's attachment;
// - the tokenizer accepts only whitespace-free URIs under the root, and the
//   renderer treats the name as React text (escaped) — never markup.

import { MAX_NAME_BYTES, uploadFile } from "../../../domain/files-client";
import type { BlockEvent, NodeTransport } from "../../../domain/transport";

export const ATTACHMENTS_ROOT = "/shared/attachments";
const URI_SCHEME = "duck://files";
const URI_PREFIX = `${URI_SCHEME}${ATTACHMENTS_ROOT}/`;

/** Paste-upload cap. Every byte is replicated through consensus blocks on
 *  every node, and a paste is the easiest accident in the app — keep it an
 *  order of magnitude under the object facade's 64 MiB. */
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;

/** Image extensions the chip previews inline. Everything else (including
 *  svg — script-bearing by design) downloads instead of rendering. */
const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "avif"]);

export interface AttachmentRef {
  /** Absolute duckfs path, `/shared/attachments/<uuid>/<name>`. */
  path: string;
  /** The display name — the path's last segment. */
  name: string;
}

export type AttachmentSegment = { text: string } | { attachment: AttachmentRef };

export const isAttachment = (
  segment: AttachmentSegment,
): segment is { attachment: AttachmentRef } => "attachment" in segment;

/** True when the chip may render an inline <img> preview. Extension-based on
 *  purpose: previews of OTHER senders' attachments must not need a metadata
 *  query, and an <img> renders nothing executable regardless of real bytes. */
export const isImageName = (name: string): boolean => {
  const dot = name.lastIndexOf(".");
  return dot > 0 && IMAGE_EXTENSIONS.has(name.slice(dot + 1).toLowerCase());
};

/** Collapse an arbitrary (attacker-typed) filename to one safe duckfs path
 *  segment: path separators and brackets become `-`, whitespace runs become
 *  one `-`, control characters drop, leading dots strip (no dotfiles, no `..`),
 *  and the result is clamped to the module's name byte cap. Empty in → "file". */
// Bidi overrides + zero-width chars: a hostile name can spoof its extension
// (`photo\u202Egnp.exe` reads as `photo​exe.png` in a label) or hide a
// dot. Strip them everywhere a name is shown or saved.
// eslint-disable-next-line no-control-regex
const UNSAFE_NAME_CHARS = /[\u0000-\u001f\u007f\u200b-\u200f\u202a-\u202e\u2066-\u2069\ufeff]/g;

export const sanitizeAttachmentName = (raw: string): string => {
  let name = raw
    // NFC first: the duckfs module rejects non-NFC path segments, so an NFD
    // name (common from macOS) would pass every rule below then fail the commit.
    .normalize("NFC")
    .trim()
    .replace(/[/\\[\]]/g, "-")
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

/** The received-side display name — the URI's last segment is authored by ANY
 *  sender, so strip control/bidi/zero-width chars before it reaches a label or
 *  a download filename. It cannot affect the fetch path (the tokenizer already
 *  confined that); this only stops label spoofing. */
export const displayName = (name: string): string =>
  name.replace(UNSAFE_NAME_CHARS, "") || "file";

/** The URI a sent message carries for `path`. */
export const attachmentUri = (path: string): string => `${URI_SCHEME}${path}`;

/** Split message text into literal runs and attachment refs, in order and
 *  lossless (concatenating segment sources reproduces the input) — the
 *  splitPageRefs discipline. A URI ends at the first whitespace; sanitized
 *  names never contain any. Only two-level paths under the root are accepted
 *  (`<uuid>/<name>`) — anything shallower or deeper stays literal. */
export const splitAttachments = (text: string): AttachmentSegment[] => {
  const out: AttachmentSegment[] = [];
  let literalFrom = 0;
  let scan = 0;
  for (;;) {
    const open = text.indexOf(URI_PREFIX, scan);
    if (open === -1) break;
    let end = open + URI_PREFIX.length;
    while (end < text.length && !/\s/.test(text[end])) end += 1;
    scan = end;
    const rest = text.slice(open + URI_PREFIX.length, end);
    const segments = rest.split("/");
    // exactly <dir>/<name>, both non-empty, no dot-segments.
    if (
      segments.length !== 2 ||
      segments.some((s) => s === "" || s === "." || s === "..")
    ) {
      continue; // malformed: stays in the literal run, verbatim
    }
    if (open > literalFrom) out.push({ text: text.slice(literalFrom, open) });
    out.push({
      attachment: {
        // path is for FETCHING — kept verbatim (tokenizer-confined, node
        // re-canonicalizes); name is for DISPLAY — stripped of bidi/control
        // spoofing before it reaches any label or download filename.
        path: `${ATTACHMENTS_ROOT}/${rest}`,
        name: displayName(segments[1]),
      },
    });
    literalFrom = scan;
  }
  if (literalFrom < text.length) out.push({ text: text.slice(literalFrom) });
  return out;
};

/** Upload one pasted file into duckfs and return the URI to insert. The
 *  fresh uuid directory isolates every upload: no name collisions, no
 *  cross-sender overwrites, and the per-path CAS never conflicts. */
export const uploadAttachment = async (
  transport: NodeTransport,
  file: { name: string; type: string; bytes: Uint8Array<ArrayBuffer> },
): Promise<{ uri: string; block: BlockEvent }> => {
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
  return { uri: attachmentUri(path), block };
};
