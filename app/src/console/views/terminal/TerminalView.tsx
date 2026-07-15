// Interactive terminal surface: attach to a node-hosted `codex` session and
// drive it. Creates the session over the node's RPC, mounts xterm.js, and
// bridges the existing ws stream — output (base64 chunks on the `term:<id>`
// topic, incl. the ring's catch-up) → term.write. How INPUT flows depends on
// the session mode:
//   • single (default): keystrokes → termInput, the raw solo terminal.
//   • shared: raw input is refused by the node; the only way in is the ordered,
//     attributed `termCommand` lane, and the command log rides a SECOND topic
//     `term-cmd:<id>` — the shared conversation object rendered below the
//     (read-only) xterm output.
// Mirrors RunOutputPane's per-topic subscribe; input is the new direction.
//
// PR1 hosts codex only (the claude broker is PR2). The session runs on the
// node and burns the operator's subscription — placed on the operator
// (node-control) rail so it appears only where node control is available.

import { useEffect, useRef, useState } from "react";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

import { Icon } from "../../components/Icon";
import {
  decodeTermChunk,
  termCommandMsg,
  termInputMsg,
  termResizeMsg,
} from "../../../domain/term-client";
import { commandTopic } from "../../../domain/stream";
import { NodeError, type TermSessionMode } from "../../../domain/transport";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const AGENT = "codex";

/** One row of a shared session's ordered command log. */
interface CommandRow {
  seq: number;
  origin: string;
  text: string;
}

export function TerminalView() {
  const { transport, state } = useDucktape();
  const host = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [mode, setMode] = useState<TermSessionMode>("single");
  const [commands, setCommands] = useState<CommandRow[]>([]);
  const [draft, setDraft] = useState("");
  // set by the effect once the session exists, read by the command form so it
  // can send without re-running (and tearing down) the whole session.
  const sessionRef = useRef<string | null>(null);
  const sendRef = useRef<NonNullable<typeof transport>["sendTerm"] | null>(null);

  useEffect(() => {
    setError(null);
    setReady(false);
    setCommands([]);
    sessionRef.current = null;
    sendRef.current = null;
    const container = host.current;
    if (!container) return;
    if (!transport?.createTermSession || !transport.sendTerm) {
      setError("This connection can't host terminal sessions.");
      return;
    }
    const create = transport.createTermSession;
    const send = transport.sendTerm;
    const close = transport.closeTermSession;
    const subscribe = transport.subscribe;
    const shared = mode === "shared";

    let disposed = false;
    let term: Terminal | null = null;
    let unsubscribe: (() => void) | null = null;
    let observer: ResizeObserver | null = null;
    let sessionId: string | null = null;

    // each (re)created session is a fresh command log; clear any prior session's
    // rows so a new session's seq (which restarts at 1) is not swallowed by the
    // monotonic-seq dedupe below.
    setCommands([]);

    create(AGENT, mode)
      .then((session) => {
        if (disposed) {
          // created but the view already unmounted / switched mode — close it so
          // the node doesn't leak the session against its per-node cap.
          close?.(session.sessionId);
          return;
        }
        sessionId = session.sessionId;
        sessionRef.current = session.sessionId;
        sendRef.current = send;
        const t = new Terminal({
          cursorBlink: !shared,
          disableStdin: shared, // shared output is read-only; input rides the lane
          fontFamily: font.mono,
          fontSize: 13,
          scrollback: 5000,
        });
        term = t;
        const fit = new FitAddon();
        t.loadAddon(fit);
        t.open(container);
        fit.fit();
        if (!shared) t.focus();

        // output: base64 chunk (incl. ring catch-up on subscribe) → raw bytes.
        // On the FIRST chunk the ws is provably open (it just delivered), so
        // re-fit + send the real geometry then — the initial resize below can be
        // dropped while the socket is still CONNECTING, and on a static layout
        // no later resize event would ever correct it. (The pty also has a sane
        // 80x24 default server-side, so nothing renders at 0x0 meanwhile.)
        let sized = false;
        const topics = shared
          ? [session.topic, commandTopic(session.sessionId)]
          : [session.topic];
        unsubscribe = subscribe(topics, {
          onTermChunk: (item) => {
            t.write(decodeTermChunk(item));
            if (!sized) {
              sized = true;
              fit.fit();
              send(termResizeMsg(session.sessionId, t.cols, t.rows));
            }
          },
          // shared only: the ordered command log. The node replays the whole
          // ring on (re)subscribe, so dedupe by the monotonic seq — a reconnect
          // replays commands we already hold.
          onTermCommandLog: (seq, origin, text) =>
            setCommands((prev) =>
              prev.length > 0 && seq <= prev[prev.length - 1].seq
                ? prev
                : [...prev, { seq, origin, text }],
            ),
        });
        // A shared session REFUSES raw keystrokes (raw_input_on_shared) — the
        // only way in is termCommand, so leave onData unwired. Resize applies to
        // the shared pty either way. Single wires the raw keystroke path.
        if (!shared) {
          t.onData((data) => send(termInputMsg(session.sessionId, data)));
        }
        t.onResize(({ cols, rows }) => send(termResizeMsg(session.sessionId, cols, rows)));
        send(termResizeMsg(session.sessionId, t.cols, t.rows));

        observer = new ResizeObserver(() => fit.fit());
        observer.observe(container);
        setReady(true);
      })
      .catch((err: unknown) => {
        if (!disposed) setError(err instanceof NodeError ? err.message : String(err));
      });

    return () => {
      disposed = true;
      sessionRef.current = null;
      sendRef.current = null;
      observer?.disconnect();
      unsubscribe?.();
      term?.dispose();
      if (sessionId) close?.(sessionId);
    };
  }, [transport, mode]);

  const submitCommand = (event: React.FormEvent) => {
    event.preventDefault();
    const text = draft.trim();
    const session = sessionRef.current;
    const send = sendRef.current;
    if (!text || !session || !send) return;
    send(termCommandMsg(session, text, state?.author ?? ""));
    setDraft("");
  };

  const shared = mode === "shared";

  return (
    <div
      data-screen-label="Terminal"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.sm,
            background: color.dark,
            color: color.onDark,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="code" size={16} color="currentColor" strokeWidth={1.7} />
        </span>
        <h1 style={{ margin: 0, font: `650 18px ${font.sans}`, color: color.ink }}>
          Terminal
        </h1>
        <span style={{ marginLeft: 8, font: `500 12px ${font.mono}`, color: color.muted2 }}>
          {AGENT}
        </span>
        <div
          role="group"
          aria-label="Session mode"
          style={{
            marginLeft: "auto",
            display: "flex",
            gap: 2,
            padding: 2,
            borderRadius: radius.sm,
            background: color.canvas,
            border: `1px solid ${color.borderSoft}`,
          }}
        >
          {(["single", "shared"] as const).map((value) => {
            const active = mode === value;
            return (
              <button
                key={value}
                type="button"
                onClick={() => {
                  if (value !== mode) setMode(value);
                }}
                aria-pressed={active}
                style={{
                  border: "none",
                  cursor: "pointer",
                  padding: "5px 12px",
                  borderRadius: 5,
                  font: `600 12px ${font.sans}`,
                  textTransform: "capitalize",
                  color: active ? color.onDark : color.muted2,
                  background: active ? color.dark : "transparent",
                }}
              >
                {value}
              </button>
            );
          })}
        </div>
      </div>

      <div style={{ position: "relative", flex: 1, minHeight: 0, padding: 10, background: "#000" }}>
        <div ref={host} style={{ width: "100%", height: "100%" }} />
        {error !== null && (
          <div
            role="alert"
            style={{
              position: "absolute",
              inset: 10,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              padding: 22,
              textAlign: "center",
              font: `500 13px ${font.sans}`,
              color: color.red,
              background: color.canvas,
            }}
          >
            {error}
          </div>
        )}
        {error === null && !ready && (
          <div
            style={{
              position: "absolute",
              inset: 10,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `500 12px ${font.mono}`,
              color: color.muted2,
              background: color.canvas,
            }}
          >
            starting {AGENT} session…
          </div>
        )}
      </div>

      {shared && (
        <div
          style={{
            flexShrink: 0,
            display: "flex",
            flexDirection: "column",
            borderTop: `1px solid ${color.borderSoft}`,
            background: color.paper,
          }}
        >
          <div
            aria-label="Command log"
            style={{
              maxHeight: 160,
              overflowY: "auto",
              padding: "8px 14px",
              display: "flex",
              flexDirection: "column",
              gap: 4,
            }}
          >
            {commands.length === 0 ? (
              <span style={{ font: `500 12px ${font.mono}`, color: color.muted2 }}>
                No commands yet — the ordered log appears here.
              </span>
            ) : (
              commands.map((row) => (
                <div
                  key={row.seq}
                  style={{
                    display: "flex",
                    gap: 10,
                    font: `500 12px ${font.mono}`,
                    color: color.ink,
                  }}
                >
                  <span style={{ color: color.muted2, minWidth: 28, textAlign: "right" }}>
                    {row.seq}
                  </span>
                  <span style={{ color: color.muted2, flexShrink: 0 }}>{row.origin}</span>
                  <span style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                    {row.text}
                  </span>
                </div>
              ))
            )}
          </div>
          <form
            onSubmit={submitCommand}
            style={{
              display: "flex",
              gap: 8,
              padding: "10px 14px",
              borderTop: `1px solid ${color.borderSoft}`,
            }}
          >
            <input
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              disabled={!ready}
              placeholder="Send a command…"
              aria-label="Command"
              style={{
                flex: 1,
                minWidth: 0,
                padding: "8px 10px",
                borderRadius: radius.sm,
                border: `1px solid ${color.borderSoft}`,
                background: color.canvas,
                color: color.ink,
                font: `500 13px ${font.mono}`,
              }}
            />
            <button
              type="submit"
              disabled={!ready || draft.trim() === ""}
              style={{
                flexShrink: 0,
                padding: "8px 16px",
                borderRadius: radius.sm,
                border: "none",
                cursor: "pointer",
                font: `600 13px ${font.sans}`,
                color: color.onDark,
                background: color.dark,
                opacity: !ready || draft.trim() === "" ? 0.5 : 1,
              }}
            >
              Send
            </button>
          </form>
        </div>
      )}
    </div>
  );
}
