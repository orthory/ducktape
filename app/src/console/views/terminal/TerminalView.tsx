// Interactive terminal surface: attach to a node-hosted `codex` session and
// drive its native TUI. Creates the session over the node's RPC, mounts
// xterm.js, and bridges the existing ws stream both ways — output (base64
// chunks on the `term:<id>` topic, incl. the ring's catch-up) → term.write,
// keystrokes → termInput, fit → termResize. Mirrors RunOutputPane's per-topic
// subscribe; input is the genuinely new direction.
//
// PR1 hosts codex only (the claude broker is PR2). The session runs on the
// node and burns the operator's subscription — placed on the operator
// (node-control) rail so it appears only where node control is available.

import { useEffect, useRef, useState } from "react";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

import { Icon } from "../../components/Icon";
import { decodeTermChunk, termInputMsg, termResizeMsg } from "../../../domain/term-client";
import { NodeError } from "../../../domain/transport";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const AGENT = "codex";

export function TerminalView() {
  const { transport } = useDucktape();
  const host = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    setError(null);
    setReady(false);
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

    let disposed = false;
    let term: Terminal | null = null;
    let unsubscribe: (() => void) | null = null;
    let observer: ResizeObserver | null = null;
    let sessionId: string | null = null;

    create(AGENT)
      .then((session) => {
        if (disposed) {
          // created but the view already unmounted — close it so the node
          // doesn't leak the session against its per-node cap.
          close?.(session.sessionId);
          return;
        }
        sessionId = session.sessionId;
        const t = new Terminal({
          cursorBlink: true,
          fontFamily: font.mono,
          fontSize: 13,
          scrollback: 5000,
        });
        term = t;
        const fit = new FitAddon();
        t.loadAddon(fit);
        t.open(container);
        fit.fit();
        t.focus();

        // output: base64 chunk (incl. ring catch-up on subscribe) → raw bytes.
        unsubscribe = subscribe([session.topic], {
          onTermChunk: (item) => t.write(decodeTermChunk(item)),
        });
        // input: keystrokes → termInput; size: xterm's resize event + an
        // initial one after fit() so the pty starts at the real geometry.
        // (onData/onResize disposables are freed by t.dispose() below.)
        t.onData((data) => send(termInputMsg(session.sessionId, data)));
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
      observer?.disconnect();
      unsubscribe?.();
      term?.dispose();
      if (sessionId) close?.(sessionId);
    };
  }, [transport]);

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
    </div>
  );
}
