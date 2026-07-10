import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";

import * as browser from "../../../domain/duck-browser";
import * as files from "../../../domain/files-client";
import * as gateway from "../../../domain/gateway-client";
import * as identity from "../../../domain/identity-client";
import { normalizeKey } from "../../../domain/names";
import { isTauri } from "../../../domain/node-bootstrap";
import * as workspaces from "../../../domain/workspace-client";
import type { LoadedDuckPage } from "../../../domain/duck-browser";
import type { RouteMethod, RouteRecord, RouteStatement } from "../../../domain/gateway-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const METHOD_ORDER: RouteMethod[] = ["get", "head", "post", "put", "patch", "delete"];

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

const fieldStyle: CSSProperties = {
  minWidth: 0,
  height: 30,
  boxSizing: "border-box",
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.sm,
  background: color.paper,
  padding: "0 8px",
  color: color.ink,
  font: `500 11px ${font.mono}`,
};

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

function PublishPanel({
  onClose,
  onPublished,
}: {
  onClose: () => void;
  onPublished: (address: string) => void;
}) {
  const { state, transport } = useDucktape();
  const publisherNode = normalizeKey(state.status?.publicKey ?? state.workspace?.pubkey ?? "");
  const binding = publisherNode ? state.nodeUsers[publisherNode] : undefined;
  const accountId = normalizeKey(binding?.accountId ?? "");
  const handle = accountId ? state.accountHandles[accountId] : undefined;
  const chainId = state.workspace?.chainId ?? "";
  const workspaceId = state.workspace?.id ?? "";
  const [label, setLabel] = useState("");
  const [target, setTarget] = useState<"duck_fs" | "loopback_http">("duck_fs");
  const [port, setPort] = useState("3000");
  const [defaultPath, setDefaultPath] = useState("index.html");
  const [audience, setAudience] = useState<"network" | "owner">("network");
  const [methods, setMethods] = useState<RouteMethod[]>(["get", "head"]);
  const [requestKiB, setRequestKiB] = useState("256");
  const [responseKiB, setResponseKiB] = useState("4096");
  const [allowAuthorization, setAllowAuthorization] = useState(false);
  const [record, setRecord] = useState<RouteRecord | null>(null);
  const [localRoutes, setLocalRoutes] = useState<workspaces.GatewayLocalRoute[]>([]);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const name = useMemo(() => gateway.routeName(label), [label]);
  const address = handle
    ? `${name.label ? `${name.label}.` : ""}${handle}.duck`
    : null;
  const local = localRoutes.find((route) => route.name.label === name.label);

  const refresh = useCallback(async () => {
    if (!transport || !accountId) {
      setRecord(null);
      setLocalRoutes([]);
      return;
    }
    gateway.validateRouteName(name);
    const [next, routes] = await Promise.all([
      gateway.getRoute(transport, identity.hexToBytes(accountId), name),
      workspaceId && isTauri()
        ? workspaces.listGatewayRoutes(workspaceId)
        : Promise.resolve([]),
    ]);
    setRecord(next);
    setLocalRoutes(routes);
  }, [transport, accountId, name, workspaceId]);

  useEffect(() => {
    refresh().catch((error: unknown) => setNote(String(error)));
  }, [refresh]);

  const statement = (route: gateway.RouteDefinition | null): RouteStatement => ({
    version: gateway.ROUTE_FORMAT_VERSION,
    chain_id: chainId,
    account_id: identity.hexToBytes(accountId),
    name,
    publisher_node: identity.hexToBytes(publisherNode),
    revision: (record?.statement.revision ?? 0) + 1,
    route,
  });

  const numericCap = (value: string, maximum: number, label: string): number => {
    const kib = Number(value);
    const bytes = kib * 1024;
    if (!Number.isSafeInteger(bytes) || bytes < 0 || bytes > maximum) {
      throw new Error(`${label} must be 0..${maximum / 1024} KiB.`);
    }
    return bytes;
  };

  const publish = async (): Promise<void> => {
    if (!transport || !workspaceId) throw new Error("Connect a managed workspace first.");
    gateway.validateRouteName(name);
    const maxResponse = numericCap(
      responseKiB,
      gateway.MAX_RESPONSE_BODY_BYTES,
      "Response cap",
    );
    let definition: gateway.RouteDefinition;
    const previousPort = local?.port ?? null;
    if (target === "duck_fs") {
      const content = await browser.buildContentDefinition(
        transport,
        publisherNode,
        name,
        defaultPath,
      );
      definition = {
        target: { kind: "duck_fs", content },
        policy: {
          audience: { kind: audience },
          methods: ["get", "head"],
          max_request_bytes: 0,
          max_response_bytes: maxResponse,
          allow_authorization: false,
        },
      };
      // A content-backed route has no loopback half. Remove an older binding
      // before publication and restore it if consensus rejects the update.
      if (previousPort !== null) {
        await workspaces.unbindGatewayRoute(workspaceId, name.label);
      }
    } else {
      const loopbackPort = Number(port);
      if (!Number.isInteger(loopbackPort) || loopbackPort < 1 || loopbackPort > 65535) {
        throw new Error("Loopback port must be 1..65535.");
      }
      const sortedMethods = METHOD_ORDER.filter((method) => methods.includes(method));
      const maxRequest = numericCap(
        requestKiB,
        gateway.MAX_REQUEST_BODY_BYTES,
        "Request cap",
      );
      definition = {
        target: { kind: "loopback_http" },
        policy: {
          audience: { kind: audience },
          methods: sortedMethods,
          max_request_bytes: maxRequest,
          max_response_bytes: maxResponse,
          allow_authorization: allowAuthorization,
        },
      };
      gateway.validateStatement(statement(definition));
      await workspaces.bindGatewayRoute(workspaceId, name.label, loopbackPort);
    }
    try {
      await gateway.submitStatement(transport, statement(definition));
    } catch (error) {
      if (previousPort !== null) {
        await workspaces.bindGatewayRoute(workspaceId, name.label, previousPort);
      } else if (target === "loopback_http") {
        await workspaces.unbindGatewayRoute(workspaceId, name.label);
      }
      throw error;
    }
    await refresh();
    setNote(address ? `Published ${address}.` : "Published; DuckDNS registration remains optional.");
    if (address) onPublished(address);
  };

  const unpublish = async (): Promise<void> => {
    if (!transport || !workspaceId || !record?.statement.route) {
      throw new Error("This route is not published.");
    }
    const previousPort = local?.port ?? null;
    if (previousPort !== null) {
      await workspaces.unbindGatewayRoute(workspaceId, name.label);
    }
    try {
      await gateway.submitStatement(transport, statement(null));
    } catch (error) {
      if (previousPort !== null) {
        await workspaces.bindGatewayRoute(workspaceId, name.label, previousPort);
      }
      throw error;
    }
    await refresh();
    setNote("Route removed; its revision tombstone prevents replay.");
  };

  const createStarter = async (): Promise<void> => {
    if (!transport) throw new Error("Connect a workspace first.");
    const routeAddress = address ?? "this account's optional .duck name";
    await files.uploadFile(transport, {
      path: `${gateway.contentRoot(publisherNode, name)}/index.html`,
      bytes: browser.starterDocument(binding?.name ?? handle ?? "Gateway", routeAddress),
      meta: { mime: "text/html" },
      message: "create gateway starter",
    });
    setDefaultPath("index.html");
    setTarget("duck_fs");
    setNote("Starter created in the route's DuckFS root. Publish when ready.");
  };

  const run = (operation: () => Promise<void>): void => {
    setBusy(true);
    setNote(null);
    operation()
      .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
      .finally(() => setBusy(false));
  };

  const canPublish = Boolean(
    transport && isTauri() && workspaceId && accountId && publisherNode && chainId && !busy,
  );

  return (
    <aside aria-label="Gateway publishing" style={{
      width: 326,
      flexShrink: 0,
      borderLeft: `1px solid ${color.borderSoft}`,
      background: color.sidebar,
      padding: 16,
      overflowY: "auto",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ font: `650 13px ${font.sans}`, color: color.dark }}>Gateway route</span>
        <button onClick={onClose} aria-label="Close publishing panel" style={{ ...buttonStyle(), width: 28, padding: 0, textAlign: "center" }}>×</button>
      </div>
      <p style={{ font: `400 11px/1.55 ${font.sans}`, color: color.muted3, margin: "12px 0 14px" }}>
        One signed route can serve exact DuckFS files or reverse-proxy one local HTTP server.
        DuckDNS is only the optional account name.
      </p>

      <dl style={{ margin: 0, display: "grid", gridTemplateColumns: "72px 1fr", gap: "7px 8px", font: `500 10.5px ${font.mono}` }}>
        <dt style={{ color: color.muted }}>Account</dt><dd style={{ margin: 0, color: color.inkSoft }}>{short(accountId, 10)}</dd>
        <dt style={{ color: color.muted }}>Node</dt><dd style={{ margin: 0, color: color.inkSoft }}>{short(publisherNode, 10)}</dd>
        <dt style={{ color: color.muted }}>Address</dt><dd style={{ margin: 0, color: color.inkSoft }}>{address ?? "DNS optional"}</dd>
        <dt style={{ color: color.muted }}>Revision</dt><dd style={{ margin: 0, color: color.inkSoft }}>{record?.statement.revision ?? "—"}</dd>
      </dl>

      <div style={{ display: "grid", gap: 8, marginTop: 16 }}>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Route label <span style={{ color: color.muted }}>(blank = account apex)</span>
          <input aria-label="Route label" value={label} onChange={(event) => setLabel(event.target.value.toLowerCase())} placeholder="api" spellCheck={false} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
        </label>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Target
          <select aria-label="Route target" value={target} onChange={(event) => setTarget(event.target.value as typeof target)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }}>
            <option value="duck_fs">DuckFS content</option>
            <option value="loopback_http">Loopback HTTP</option>
          </select>
        </label>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Audience
          <select aria-label="Route audience" value={audience} onChange={(event) => setAudience(event.target.value as typeof audience)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }}>
            <option value="network">All identified network members</option>
            <option value="owner">Owning account only</option>
          </select>
        </label>
      </div>

      {target === "duck_fs" ? (
        <div style={{ marginTop: 13 }}>
          <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
            Default path
            <input aria-label="Default path" value={defaultPath} onChange={(event) => setDefaultPath(event.target.value)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
          </label>
          <div style={{ marginTop: 7, color: color.muted, font: `500 9.5px/1.45 ${font.mono}`, overflowWrap: "anywhere" }}>
            {publisherNode ? gateway.contentRoot(publisherNode, name) : "—"}
          </div>
          <button disabled={!canPublish} onClick={() => run(createStarter)} style={{ ...buttonStyle(!canPublish), marginTop: 9, width: "100%", textAlign: "center" }}>Create starter file</button>
        </div>
      ) : (
        <div style={{ marginTop: 13 }}>
          <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
            Loopback port
            <input aria-label="Loopback port" value={port} onChange={(event) => setPort(event.target.value)} inputMode="numeric" style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
          </label>
          <div style={{ marginTop: 10, color: color.muted3, font: `600 10px ${font.sans}` }}>Allowed methods</div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "5px 9px", marginTop: 6 }}>
            {METHOD_ORDER.map((method) => (
              <label key={method} style={{ color: color.inkSoft, font: `500 9.5px ${font.mono}` }}>
                <input type="checkbox" checked={methods.includes(method)} onChange={(event) => setMethods((current) => event.target.checked ? [...current, method] : current.filter((item) => item !== method))} /> {method.toUpperCase()}
              </label>
            ))}
          </div>
          <label style={{ display: "block", marginTop: 9, color: color.inkSoft, font: `500 9.5px ${font.sans}` }}>
            <input type="checkbox" checked={allowAuthorization} onChange={(event) => setAllowAuthorization(event.target.checked)} /> Allow explicit Authorization forwarding
          </label>
          <div style={{ marginTop: 7, color: color.muted, font: `500 9.5px ${font.mono}` }}>
            local port {local?.port ?? "—"} · never published in consensus
          </div>
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 7, marginTop: 13 }}>
        {target === "loopback_http" && (
          <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
            Request KiB
            <input aria-label="Request cap" value={requestKiB} onChange={(event) => setRequestKiB(event.target.value)} inputMode="numeric" style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
          </label>
        )}
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Response KiB
          <input aria-label="Response cap" value={responseKiB} onChange={(event) => setResponseKiB(event.target.value)} inputMode="numeric" style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
        </label>
      </div>

      <div style={{ display: "grid", gap: 7, marginTop: 14 }}>
        <button disabled={!canPublish} onClick={() => run(publish)} style={{ ...buttonStyle(!canPublish), textAlign: "center" }}>Publish signed route</button>
        {record?.statement.route && (
          <button disabled={!canPublish} onClick={() => run(unpublish)} style={{ ...buttonStyle(!canPublish), textAlign: "center", color: color.danger }}>Unpublish route</button>
        )}
      </div>
      {!isTauri() && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Publishing requires the desktop user-key signer.</p>}
      {isTauri() && !accountId && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Bind this node to your Identity account before publishing.</p>}
      {note && <div role="status" style={{ marginTop: 12, padding: 10, borderRadius: radius.sm, background: color.paper, border: `1px solid ${color.border}`, color: color.muted3, font: `500 10.5px/1.45 ${font.sans}` }}>{note}</div>}
    </aside>
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
  const [publishOpen, setPublishOpen] = useState(false);

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
        await gateway.openWindow(loaded.srcUrl, loaded.address.hostname);
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
        <button onClick={() => setPublishOpen((openPanel) => !openPanel)} style={buttonStyle()}>Publish</button>
      </div>

      {page && <SecurityBar page={page} />}
      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <div style={{ flex: 1, minWidth: 0, minHeight: 0, position: "relative", background: "#f4f4f2" }}>
          {loading && <div role="status" style={{ position: "absolute", inset: 0, zIndex: 2, display: "grid", placeItems: "center", background: "rgba(255,255,255,.82)", color: color.muted3, font: `600 11px ${font.sans}` }}>Resolving route…</div>}
          {error && !loading && <div role="alert" style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", padding: 30 }}><div style={{ maxWidth: 480, padding: 18, borderRadius: radius.lg, border: `1px solid ${color.dangerBorder}`, background: color.paper }}><div style={{ color: color.dark, font: `650 13px ${font.sans}` }}>Route refused</div><div style={{ marginTop: 7, color: color.muted3, font: `450 11px/1.55 ${font.sans}` }}>{error}</div></div></div>}
          {page?.hosting === "network" && !error && (
            <iframe title={page.title} data-testid="duck-browser-frame" sandbox="" allow="" referrerPolicy="no-referrer" srcDoc={page.srcDoc} style={{ width: "100%", height: "100%", display: "block", border: 0, background: color.paper }} />
          )}
          {page?.hosting === "gateway" && !error && (
            <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", padding: 30 }}>
              <div style={{ maxWidth: 430, padding: 20, borderRadius: radius.lg, border: `1px solid ${color.border}`, background: color.paper, textAlign: "center" }}>
                <div style={{ color: color.dark, font: `650 13px ${font.sans}` }}>Opened in an isolated gateway window</div>
                <div style={{ marginTop: 7, color: color.muted3, font: `450 11px/1.55 ${font.sans}` }}>Publisher code has one short-lived route origin and no Ducktape application capabilities.</div>
                <button style={{ ...buttonStyle(), marginTop: 13 }} onClick={() => page.srcUrl && void gateway.openWindow(page.srcUrl, page.address.hostname)}>Reopen {page.address.hostname}</button>
              </div>
            </div>
          )}
          {!page && !error && !loading && <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", color: color.muted, font: `500 11px ${font.sans}` }}><div>Enter <span style={{ fontFamily: font.mono }}>net.duck</span>, <span style={{ fontFamily: font.mono }}>&lt;account&gt;.duck</span>, or <span style={{ fontFamily: font.mono }}>&lt;label&gt;.&lt;account&gt;.duck</span>.</div></div>}
        </div>
        {publishOpen && <PublishPanel onClose={() => setPublishOpen(false)} onPublished={(next) => { setPublishOpen(false); void open(next); }} />}
      </div>
    </div>
  );
}
