// The duck:// protocol core — the ONE module table for module-plane URIs
// (`duck://<module>/<path>[#<fragment>]`, module = a single dotless label).
// Dotted authorities (`<name>.duck`) are the gateway plane and are NOT
// classified here — duck-browser.ts owns them. `memory` is reserved for the
// agent runtime's evidence URIs and never chips. Normative doc:
// docs/adr/2026-07-14-duck-uri-protocol.mdx.
//
// Adding a module = one ref interface + one `DuckRef` variant + one classify
// arm here, one chip, one `openDuckRef` arm, one ADR table row.

export const ATTACHMENTS_ROOT = "/shared/attachments";

// Bidi overrides + zero-width chars: a hostile name can spoof its extension
// (`photo<RLO>gnp.exe` reads as `photoexe.png` in a label) or hide a dot.
// Strip them everywhere a name is shown or saved.
// eslint-disable-next-line no-control-regex
export const UNSAFE_NAME_CHARS =
  /[\u0000-\u001f\u007f\u200b-\u200f\u202a-\u202e\u2066-\u2069\ufeff]/g;

/** The received-side display name — a ref's label/last-segment is authored by
 *  ANY sender, so strip control/bidi/zero-width chars before it reaches a
 *  label or a download filename. It cannot affect the fetch path (the
 *  tokenizer already confined that); this only stops label spoofing. */
export const displayName = (name: string): string =>
  name.replace(UNSAFE_NAME_CHARS, "") || "file";

export interface PageRef {
  id: string;
  /** the markdown label — decorative; the chip prefers the live store title. */
  label: string;
}

export interface FileRef {
  /** absolute duckfs path, `/shared/attachments/<dir>/<name>`. */
  path: string;
  /** display/download name — the markdown label, spoof-stripped. */
  name: string;
  /** `![..]` embed form: an image previews inline; a non-image still downloads. */
  embed: boolean;
}

export interface ForgeRef {
  repo: string;
  /** item (PR/issue — one number space) number; null = a repo-only ref. */
  number: number | null;
  /** `#<seq>` Discussion-message anchor — only meaningful on an item ref. */
  seq?: number;
}

export interface ChannelRef {
  id: string;
  /** `#<seq>` message anchor (jump-to-message). */
  seq?: number;
}

export type DuckRef =
  | { page: PageRef }
  | { file: FileRef }
  | { forge: ForgeRef }
  | { channel: ChannelRef };

/** Classify one module-plane duck:// url into a typed ref, or null when it
 *  doesn't validate (unknown module, malformed path, gateway host). Callers
 *  render a null as literal text — a ref never errors, it just doesn't chip.
 *  The page/files rules are byte-identical to the pre-protocol tokenizer. */
export function classifyDuckRef(url: string, label: string, embed: boolean): DuckRef | null {
  const page = url.match(/^duck:\/\/page\/([^/\s)]+)$/);
  if (page) return { page: { id: page[1], label } };

  if (url.startsWith("duck://files")) return classifyFile(url, label, embed);

  // repo excludes `:` (the item-channel separator) and `#` (the fragment
  // delimiter); item numbers and seqs are 1-based decimals.
  const forge = url.match(/^duck:\/\/forge\/([^/\s)#:]+)(?:\/(\d+))?(?:#(\d+))?$/);
  if (forge) {
    const number = forge[2] ? Number(forge[2]) : null;
    const seq = forge[3] ? Number(forge[3]) : undefined;
    if (forge[2] && !number) return null; // item 0 is not mintable
    if (seq !== undefined && (!seq || number === null)) return null; // an anchor needs an item
    return { forge: { repo: forge[1], number, ...(seq ? { seq } : {}) } };
  }

  const channel = url.match(/^duck:\/\/channel\/([^\s)#]+)(?:#(\d+))?$/);
  if (channel) {
    const seq = channel[2] ? Number(channel[2]) : undefined;
    if (channel[2] && !seq) return null; // seqs are 1-based
    return { channel: { id: channel[1], ...(seq ? { seq } : {}) } };
  }

  return null;
}

// duck://files<absolute-path>; the path already carries its leading slash.
// FILE refs are confined to `/shared/attachments/<dir>/<name>` — this check is
// the only guard against a crafted ref steering a client read at another
// duckfs path (reads are not authority-gated on the node). A ref outside that
// root stays literal text and fetches nothing.
function classifyFile(url: string, label: string, embed: boolean): DuckRef | null {
  const filePath = url.slice("duck://files".length);
  if (!filePath.startsWith(`${ATTACHMENTS_ROOT}/`)) return null;
  const rest = filePath.slice(ATTACHMENTS_ROOT.length + 1);
  const parts = rest.split("/");
  // exactly <dir>/<name>, both non-empty, no dot-segments.
  if (parts.length !== 2 || parts.some((p) => p === "" || p === "." || p === "..")) {
    return null;
  }
  const name = displayName(label) || displayName(parts[1]);
  return { file: { path: filePath, name, embed } };
}
