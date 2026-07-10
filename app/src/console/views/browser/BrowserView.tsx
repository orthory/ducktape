import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

import * as browser from "../../../domain/duck-browser";
import * as gateway from "../../../domain/gateway-client";
import type { LoadedDuckPage } from "../../../domain/duck-browser";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const short = (value?: string, width = 8): string =>
  value ? `${value.slice(0, width)}…${value.slice(-4)}` : "—";

const buttonStyle = (disabled = false): CSSProperties => ({
  all: "unset",
  boxSizing: "border-box",
  cursor: disabled ? "default" : "pointer",
  opacity: disabled ? 0.45 : 1,
  height: 30,
  padding: "0 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  color: color.inkSoft,
  font: `600 11px ${font.sans}`,
});

function SecurityBar({ page }: { page: LoadedDuckPage }) {
  const routed = page.hosting === "gateway";
  return (
    <div
      data-testid="browser-security"
      style={{
        minHeight: 34,
        display: "flex",
        alignItems: "center",
        gap: 9,
        padding: "0 14px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: routed ? "#f5faf6" : "#f5f7fa",
      }}
    >
      <span style={{
        padding: "3px 7px",
        borderRadius: 999,
        background: routed ? "#dfeee2" : "#e1e8f0",
        color: routed ? "#397047" : "#4f6783",
        font: `700 9px ${font.mono}`,
        letterSpacing: ".06em",
      }}>
        {routed ? "SIGNED GATEWAY ROUTE" : "NETWORK SNAPSHOT"}
      </span>
      <span style={{ color: color.muted3, font: `500 10.5px ${font.sans}` }}>
        {routed
          ? `publisher ${short(page.publisherNode)} · revision ${page.revision} · authenticated E2E stream`
          : `connected-node DuckFS · snapshot ${short(page.snapshot, 10)}`}
      </span>
      <span style={{ color: color.muted2, font: `500 10px ${font.sans}`, marginLeft: "auto" }}>
        {routed
          ? `${page.target === "duck_fs" ? "DuckFS" : "loopback HTTP"} · isolated incognito WebView · no app IPC`
          : `${page.totalBytes} bytes · scripts and network blocked`}
      </span>
    </div>
  );
}

export function BrowserView() {
  const { transport } = useDucktape();
  const [input, setInput] = useState("net.duck");
  const [page, setPage] = useState<LoadedDuckPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  // CEF shells embed the gateway session in the pane; wry opens the window.
  const [inline, setInline] = useState(false);
  const paneRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    void gateway.inlineSupported().then((ok) => {
      if (!cancelled) setInline(ok);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Mount/track/unmount the inline gateway webview over the pane. The child
  // webview lives in window coordinates, so the pane's viewport rect (the main
  // webview fills the window) is its bounds; ResizeObserver keeps it placed.
  // ponytail: console overlays (search modal) render under the native child
  // inside this rect; hide-on-modal if it ever matters.
  useEffect(() => {
    const el = paneRef.current;
    const srcUrl = page?.hosting === "gateway" ? page.srcUrl : undefined;
    if (!inline || !el || !srcUrl || error) return;
    const rectOf = (): gateway.InlineRect => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    };
    gateway.openInline(srcUrl, rectOf()).catch((reason: unknown) => {
      setError(reason instanceof Error ? reason.message : String(reason));
    });
    const observer = new ResizeObserver(() => void gateway.placeInline(rectOf()));
    observer.observe(el);
    return () => {
      observer.disconnect();
      void gateway.closeInline();
    };
  }, [inline, page, error]);

  const open = useCallback(async (raw: string, addHistory = true) => {
    if (!transport) {
      setError("Connect a workspace to browse .duck routes.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const loaded = await browser.loadDuckPage(transport, raw);
      if (loaded.hosting === "gateway") {
        if (!loaded.srcUrl) throw new Error("Gateway did not return a browser session URL.");
        // Inline shells mount the session in the pane (the effect above);
        // window shells open the separate isolated window here.
        if (!(await gateway.inlineSupported())) {
          await gateway.openWindow(loaded.srcUrl, loaded.address.hostname);
        }
      }
      setPage(loaded);
      setInput(loaded.address.canonical);
      if (addHistory) {
        setHistory((old) => {
          const next = [...old.slice(0, historyIndex + 1), loaded.address.canonical];
          setHistoryIndex(next.length - 1);
          return next;
        });
      }
    } catch (reason) {
      setPage(null);
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }, [transport, historyIndex]);

  const goHistory = (nextIndex: number): void => {
    const address = history[nextIndex];
    if (!address) return;
    setHistoryIndex(nextIndex);
    void open(address, false);
  };

  return (
    <div data-screen-label="Browser" style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}>
      <div style={{ height: 48, flexShrink: 0, display: "flex", alignItems: "center", gap: 7, padding: "0 12px", borderBottom: `1px solid ${color.borderSoft}`, background: color.sidebar }}>
        <span style={{ font: `650 14px ${font.sans}`, color: color.dark, marginRight: 5 }}>Browser</span>
        <button aria-label="Back" disabled={historyIndex <= 0} onClick={() => goHistory(historyIndex - 1)} style={{ ...buttonStyle(historyIndex <= 0), width: 30, padding: 0, textAlign: "center" }}>‹</button>
        <button aria-label="Forward" disabled={historyIndex >= history.length - 1} onClick={() => goHistory(historyIndex + 1)} style={{ ...buttonStyle(historyIndex >= history.length - 1), width: 30, padding: 0, textAlign: "center" }}>›</button>
        <button aria-label="Reload" disabled={!page || loading} onClick={() => page && void open(page.address.canonical, false)} style={{ ...buttonStyle(!page || loading), width: 30, padding: 0, display: "grid", placeItems: "center" }}><Icon name="refresh" size={14} /></button>
        <form onSubmit={(event) => { event.preventDefault(); void open(input); }} style={{ flex: 1, display: "flex", minWidth: 0 }}>
          <div style={{ flex: 1, minWidth: 0, height: 31, display: "flex", alignItems: "center", border: `1px solid ${error ? color.dangerBorder : color.borderStrong}`, borderRadius: radius.md, background: color.paper, overflow: "hidden" }}>
            <span style={{ paddingLeft: 10, color: color.muted2, font: `500 11px ${font.mono}` }}>duck://</span>
            <input aria-label="Duck address" value={input} onChange={(event) => setInput(event.target.value)} spellCheck={false} autoCapitalize="none" autoCorrect="off" style={{ flex: 1, minWidth: 0, border: 0, outline: 0, padding: "0 9px 0 2px", background: "transparent", color: color.ink, font: `500 11.5px ${font.mono}` }} />
          </div>
        </form>
      </div>

      {page && <SecurityBar page={page} />}
      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <div style={{ flex: 1, minWidth: 0, minHeight: 0, position: "relative", background: "#f4f4f2" }}>
          {loading && <div role="status" style={{ position: "absolute", inset: 0, zIndex: 2, display: "grid", placeItems: "center", background: "rgba(255,255,255,.82)", color: color.muted3, font: `600 11px ${font.sans}` }}>Resolving route…</div>}
          {error && !loading && <div role="alert" style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", padding: 30 }}><div style={{ maxWidth: 480, padding: 18, borderRadius: radius.lg, border: `1px solid ${color.dangerBorder}`, background: color.paper }}><div style={{ color: color.dark, font: `650 13px ${font.sans}` }}>Route refused</div><div style={{ marginTop: 7, color: color.muted3, font: `450 11px/1.55 ${font.sans}` }}>{error}</div></div></div>}
          {page?.hosting === "network" && !error && (
            <iframe title={page.title} data-testid="duck-browser-frame" sandbox="" allow="" referrerPolicy="no-referrer" srcDoc={page.srcDoc} style={{ width: "100%", height: "100%", display: "block", border: 0, background: color.paper }} />
          )}
          {page?.hosting === "gateway" && !error && (inline ? (
            <div ref={paneRef} data-testid="gateway-inline-pane" style={{ position: "absolute", inset: 0 }} />
          ) : (
            <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", padding: 30 }}>
              <div style={{ maxWidth: 430, padding: 20, borderRadius: radius.lg, border: `1px solid ${color.border}`, background: color.paper, textAlign: "center" }}>
                <div style={{ color: color.dark, font: `650 13px ${font.sans}` }}>Opened in an isolated gateway window</div>
                <div style={{ marginTop: 7, color: color.muted3, font: `450 11px/1.55 ${font.sans}` }}>Publisher code has one short-lived route origin and no Ducktape application capabilities.</div>
                <button style={{ ...buttonStyle(), marginTop: 13 }} onClick={() => page.srcUrl && void gateway.openWindow(page.srcUrl, page.address.hostname)}>Reopen {page.address.hostname}</button>
              </div>
            </div>
          ))}
          {!page && !error && !loading && <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", color: color.muted, font: `500 11px ${font.sans}` }}><div>Enter <span style={{ fontFamily: font.mono }}>net.duck</span>, <span style={{ fontFamily: font.mono }}>&lt;account&gt;.duck</span>, or <span style={{ fontFamily: font.mono }}>&lt;label&gt;.&lt;account&gt;.duck</span>.</div></div>}
        </div>
      </div>
    </div>
  );
}
