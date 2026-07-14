// Pure wire helpers for interactive terminal sessions — the base64 <-> bytes
// bridge between xterm.js and the node's term ops. Kept out of the view so the
// encoding (the only load-bearing logic) is unit-testable without a DOM. The
// session create/close + ws send live on NodeTransport; these just shape the
// payloads. See docs/…/2026-07-15-interactive-terminal-sessions-design.md.

import { base64ToBytes, bytesToBase64 } from "./files-client";
import type { TermClientMsg } from "./stream";

// the precise variants (not the wide union) so callers keep `.data` etc; both
// are still assignable to TermClientMsg for `sendTerm`.
type TermInputMsg = Extract<TermClientMsg, { op: "termInput" }>;
type TermResizeMsg = Extract<TermClientMsg, { op: "termResize" }>;

const encoder = new TextEncoder();

/** An `xterm.onData` string (utf-8 keystrokes) → a termInput op with the bytes
 *  base64-encoded, exactly as the node expects. */
export const termInputMsg = (session: string, data: string): TermInputMsg => ({
  op: "termInput",
  session,
  data: bytesToBase64(encoder.encode(data)),
});

/** A fit/resize → a termResize op. */
export const termResizeMsg = (
  session: string,
  cols: number,
  rows: number,
): TermResizeMsg => ({ op: "termResize", session, cols, rows });

/** A term output chunk's base64 `item` → raw bytes for `Terminal.write`. */
export const decodeTermChunk = (item: string): Uint8Array => base64ToBytes(item);
