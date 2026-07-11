import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

import * as browser from "../../../domain/duck-browser";
import * as gateway from "../../../domain/gateway-client";
import type { LoadedDuckPage } from "../../../domain/duck-browser";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, tint } from "../../theme/tokens";

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

function SecurityBadge({ page }: { page: LoadedDuckPage }) {
  const routed = page.hosting === "gateway";
  return (
    <span
      data-testid="browser-security"
      title={routed
        ? `Signed by ${short(page.publisherNode)} · revision ${page.revision} · isolated incognito WebView · no app IPC`
        : `Connected-node DuckFS · snapshot ${short(page.snapshot, 10)} · scripts and network blocked`}
      style={{
        flexShrink: 0,
        marginLeft: 7,
        padding: "3px 6px",
        borderRadius: 999,
        background: routed ? tint(color.green).bg : tint(color.blue).bg,
        color: routed ? tint(color.green).text : tint(color.blue).text,
        font: `700 8.5px ${font.mono}`,
        letterSpacing: ".04em",
      }}
    >
      {routed ? "SIGNED" : "SNAPSHOT"}
    </span>
  );
}

interface BrowserTab {
  id: string;
  input: string;
  page: LoadedDuckPage | null;
  error: string | null;
  loading: boolean;
  history: string[];
  historyIndex: number;
}

const freshTab = (id: string): BrowserTab => ({
  id,
  input: "net.duck",
  page: null,
  error: null,
  loading: false,
  history: [],
  historyIndex: -1,
});

export function BrowserView() {
  const { transport } = useDucktape();
  const nextTab = useRef(2);
  const [tabs, setTabs] = useState<BrowserTab[]>(() => [freshTab("tab-1")]);
  const tabsRef = useRef(tabs);
  const [activeId, setActiveId] = useState("tab-1");
  const active = tabs.find((tab) => tab.id === activeId) ?? tabs[0];
  const { input, page, error, loading, history, historyIndex } = active;
  const updateTab = useCallback((id: string, update: (tab: BrowserTab) => BrowserTab) => {
    setTabs((old) => old.map((tab) => tab.id === id ? update(tab) : tab));
  }, []);

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  useEffect(() => () => {
    for (const tab of tabsRef.current) void gateway.closeInline(tab.id);
  }, []);
  const paneRef = useRef<HTMLDivElement | null>(null);

  // Mount/track/unmount the inline gateway webview over the pane. The child
  // webview lives in window coordinates, so the pane's viewport rect (the main
  // webview fills the window) is its bounds; ResizeObserver keeps it placed.
  // ponytail: console overlays (search modal) render under the native child
  // inside this rect; hide-on-modal if it ever matters.
  useEffect(() => {
    const el = paneRef.current;
    const gatewayPage = page?.hosting === "gateway" ? page : undefined;
    const srcUrl = gatewayPage?.srcUrl;
    if (!el || !gatewayPage || !srcUrl || error) return;
    const rectOf = (): gateway.InlineRect => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    };
    gateway
      .openInline(srcUrl, gatewayPage.address.hostname, active.id, rectOf())
      .catch((reason: unknown) => {
        updateTab(active.id, (tab) => ({
          ...tab,
          error: reason instanceof Error ? reason.message : String(reason),
        }));
      });
    const observer = new ResizeObserver(() => void gateway.placeInline(active.id, rectOf()));
    observer.observe(el);
    return () => {
      observer.disconnect();
      void gateway.hideAllInline();
    };
  }, [active.id, page, error, updateTab]);

  const open = useCallback(async (raw: string, addHistory = true, tabId = activeId) => {
    if (!transport) {
      updateTab(tabId, (tab) => ({ ...tab, error: "Connect a workspace to browse .duck routes." }));
      return;
    }
    updateTab(tabId, (tab) => ({ ...tab, loading: true, error: null }));
    try {
      const loaded = await browser.loadDuckPage(transport, raw);
      if (loaded.hosting === "gateway" && !loaded.srcUrl) {
        throw new Error("Gateway did not return a browser session URL.");
      }
      updateTab(tabId, (tab) => {
        const nextHistory = addHistory
          ? [...tab.history.slice(0, tab.historyIndex + 1), loaded.address.canonical]
          : tab.history;
        return {
          ...tab,
          page: loaded,
          input: loaded.address.canonical,
          history: nextHistory,
          historyIndex: addHistory ? nextHistory.length - 1 : tab.historyIndex,
        };
      });
    } catch (reason) {
      updateTab(tabId, (tab) => ({
        ...tab,
        page: null,
        error: reason instanceof Error ? reason.message : String(reason),
      }));
    } finally {
      updateTab(tabId, (tab) => ({ ...tab, loading: false }));
    }
  }, [activeId, transport, updateTab]);

  const goHistory = (nextIndex: number): void => {
    const address = history[nextIndex];
    if (!address) return;
    updateTab(active.id, (tab) => ({ ...tab, historyIndex: nextIndex }));
    void open(address, false, active.id);
  };

  const addTab = (): void => {
    const id = `tab-${nextTab.current++}`;
    setTabs((old) => [...old, freshTab(id)]);
    setActiveId(id);
    void gateway.hideAllInline();
  };

  const closeTab = (id: string): void => {
    if (tabs.length === 1) {
      updateTab(id, () => freshTab(id));
      void gateway.closeInline(id);
      return;
    }
    const index = tabs.findIndex((tab) => tab.id === id);
    const remaining = tabs.filter((tab) => tab.id !== id);
    setTabs(remaining);
    if (id === activeId) setActiveId(remaining[Math.min(index, remaining.length - 1)].id);
    void gateway.closeInline(id);
  };

  return (
    <div data-screen-label="Browser" style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}>
      <div role="tablist" aria-label="Browser tabs" style={{ height: 36, flexShrink: 0, display: "flex", alignItems: "end", gap: 3, padding: "5px 10px 0", borderBottom: `1px solid ${color.borderSoft}`, background: color.canvas, overflowX: "auto" }}>
        {tabs.map((tab) => {
          const selected = tab.id === active.id;
          return (
            <div key={tab.id} style={{ display: "flex", alignItems: "center", minWidth: 128, maxWidth: 220, height: 30, border: `1px solid ${selected ? color.borderStrong : color.borderSoft}`, borderBottomColor: selected ? color.paper : color.borderSoft, borderRadius: `${radius.md}px ${radius.md}px 0 0`, background: selected ? color.paper : color.sidebar }}>
              <button role="tab" aria-selected={selected} onClick={() => { setActiveId(tab.id); void gateway.hideAllInline(); }} style={{ all: "unset", cursor: "pointer", flex: 1, minWidth: 0, padding: "0 9px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: selected ? color.ink : color.muted3, font: `600 10.5px ${font.sans}` }}>
                {tab.page?.address.hostname ?? tab.input}
              </button>
              <button aria-label={`Close ${tab.page?.address.hostname ?? tab.input}`} onClick={() => closeTab(tab.id)} style={{ all: "unset", cursor: "pointer", padding: "4px 7px", color: color.muted2, font: `600 12px ${font.sans}` }}>×</button>
            </div>
          );
        })}
        <button aria-label="New tab" onClick={addTab} style={{ ...buttonStyle(), width: 30, height: 28, padding: 0, textAlign: "center", marginBottom: 1 }}>+</button>
      </div>
      <div style={{ height: 48, flexShrink: 0, display: "flex", alignItems: "center", gap: 7, padding: "0 12px", borderBottom: `1px solid ${color.borderSoft}`, background: color.sidebar }}>
        <span style={{ font: `650 14px ${font.sans}`, color: color.dark, marginRight: 5 }}>Browser</span>
        <button aria-label="Back" disabled={historyIndex <= 0} onClick={() => goHistory(historyIndex - 1)} style={{ ...buttonStyle(historyIndex <= 0), width: 30, padding: 0, textAlign: "center" }}>‹</button>
        <button aria-label="Forward" disabled={historyIndex >= history.length - 1} onClick={() => goHistory(historyIndex + 1)} style={{ ...buttonStyle(historyIndex >= history.length - 1), width: 30, padding: 0, textAlign: "center" }}>›</button>
        <button aria-label="Reload" disabled={!page || loading} onClick={() => page && void open(page.address.canonical, false)} style={{ ...buttonStyle(!page || loading), width: 30, padding: 0, display: "grid", placeItems: "center" }}><Icon name="refresh" size={14} /></button>
        <form onSubmit={(event) => { event.preventDefault(); void open(input); }} style={{ flex: 1, display: "flex", minWidth: 0 }}>
          <div style={{ flex: 1, minWidth: 0, height: 31, display: "flex", alignItems: "center", border: `1px solid ${error ? color.dangerBorder : color.borderStrong}`, borderRadius: radius.md, background: color.paper, overflow: "hidden" }}>
            {page && <SecurityBadge page={page} />}
            <span style={{ paddingLeft: 10, color: color.muted2, font: `500 11px ${font.mono}` }}>duck://</span>
            <input aria-label="Duck address" value={input} onChange={(event) => updateTab(active.id, (tab) => ({ ...tab, input: event.target.value }))} spellCheck={false} autoCapitalize="none" autoCorrect="off" style={{ flex: 1, minWidth: 0, border: 0, outline: 0, padding: "0 9px 0 2px", background: "transparent", color: color.ink, font: `500 11.5px ${font.mono}` }} />
          </div>
        </form>
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <div style={{ flex: 1, minWidth: 0, minHeight: 0, position: "relative", background: color.canvas }}>
          {loading && <div role="status" style={{ position: "absolute", inset: 0, zIndex: 2, display: "grid", placeItems: "center", background: `color-mix(in srgb, ${color.paper} 82%, transparent)`, color: color.muted3, font: `600 11px ${font.sans}` }}>Resolving route…</div>}
          {error && !loading && <div role="alert" style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", padding: 30 }}><div style={{ maxWidth: 480, padding: 18, borderRadius: radius.lg, border: `1px solid ${color.dangerBorder}`, background: color.paper }}><div style={{ color: color.dark, font: `650 13px ${font.sans}` }}>Route refused</div><div style={{ marginTop: 7, color: color.muted3, font: `450 11px/1.55 ${font.sans}` }}>{error}</div></div></div>}
          {page?.hosting === "network" && !error && (
            <iframe title={page.title} data-testid="duck-browser-frame" sandbox="" allow="" referrerPolicy="no-referrer" srcDoc={page.srcDoc} style={{ width: "100%", height: "100%", display: "block", border: 0, background: color.paper }} />
          )}
          {page?.hosting === "gateway" && !error && (
            <div ref={paneRef} data-testid="gateway-inline-pane" style={{ position: "absolute", inset: 0 }} />
          )}
          {!page && !error && !loading && <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", color: color.muted, font: `500 11px ${font.sans}` }}><div>Enter <span style={{ fontFamily: font.mono }}>net.duck</span>, <span style={{ fontFamily: font.mono }}>&lt;account&gt;.duck</span>, or <span style={{ fontFamily: font.mono }}>&lt;label&gt;.&lt;account&gt;.duck</span>.</div></div>}
        </div>
      </div>
    </div>
  );
}
