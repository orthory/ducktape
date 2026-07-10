import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import * as browser from "../../../domain/duck-browser";
import * as files from "../../../domain/files-client";
import * as gateway from "../../../domain/gateway-client";
import * as identity from "../../../domain/identity-client";
import { normalizeKey } from "../../../domain/names";
import { isTauri } from "../../../domain/node-bootstrap";
import * as workspaces from "../../../domain/workspace-client";
import type { LoadedDuckPage } from "../../../domain/duck-browser";
import type { RouteMethod, RouteRecord, RouteStatement, RouteSummary } from "../../../domain/gateway-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const METHOD_ORDER: RouteMethod[] = ["get", "head", "post", "put", "patch", "delete"];

type RouteHealth =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "serving"; status: number; path: string }
  | { kind: "reachable"; status: number; path: string }
  | { kind: "failing"; status: number; path: string }
  | { kind: "disabled" }
  | { kind: "unavailable" };

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

const routeKey = (name: gateway.RouteName): string => name.label ?? "_apex";

const routeTargetLabel = (route: RouteSummary): string =>
  route.target === "duck_fs" ? "DuckFS" : "Local HTTP";

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

function RoutesPanel({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: (address: string) => void;
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
  const [audience, setAudience] = useState<"network" | "owner" | "accounts">("network");
  const [methods, setMethods] = useState<RouteMethod[]>(["get", "head"]);
  const [requestKiB, setRequestKiB] = useState("256");
  const [responseKiB, setResponseKiB] = useState("4096");
  const [allowAuthorization, setAllowAuthorization] = useState(false);
  const [record, setRecord] = useState<RouteRecord | null>(null);
  const [records, setRecords] = useState<RouteSummary[]>([]);
  const [localRoutes, setLocalRoutes] = useState<workspaces.GatewayLocalRoute[]>([]);
  const [health, setHealth] = useState<RouteHealth>({ kind: "idle" });
  const refreshRun = useRef(0);
  const healthRun = useRef(0);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const name = useMemo(() => gateway.routeName(label), [label]);
  const address = handle
    ? `${name.label ? `${name.label}.` : ""}${handle}.duck`
    : null;
  const local = localRoutes.find((route) => route.name.label === name.label);

  const resetEditor = useCallback(() => {
    setTarget("duck_fs");
    setPort("3000");
    setDefaultPath("index.html");
    setAudience("network");
    setMethods(["get", "head"]);
    setRequestKiB("256");
    setResponseKiB("4096");
    setAllowAuthorization(false);
  }, []);

  const hydrateEditor = useCallback((next: RouteRecord | null, routes: workspaces.GatewayLocalRoute[]) => {
    const definition = next?.statement.route;
    if (!definition) {
      resetEditor();
      return;
    }
    setTarget(definition.target.kind);
    setAudience(definition.policy.audience.kind);
    setMethods(definition.policy.methods);
    setRequestKiB(String(definition.policy.max_request_bytes / 1024));
    setResponseKiB(String(definition.policy.max_response_bytes / 1024));
    setAllowAuthorization(definition.policy.allow_authorization);
    if (definition.target.kind === "duck_fs") {
      setDefaultPath(definition.target.content.default_path ?? "index.html");
    } else {
      const bound = routes.find((route) => route.name.label === next.statement.name.label);
      setPort(String(bound?.port ?? 3000));
    }
  }, [resetEditor]);

  const refresh = useCallback(async () => {
    const run = ++refreshRun.current;
    if (!transport || !accountId) {
      setRecord(null);
      setRecords([]);
      setLocalRoutes([]);
      resetEditor();
      return;
    }
    gateway.validateRouteName(name);
    const accountBytes = identity.hexToBytes(accountId);
    const result = await Promise.all([
      gateway.getRoute(transport, accountBytes, name),
      gateway.listRoutes(transport, accountBytes),
      workspaceId && isTauri()
        ? workspaces.listGatewayRoutes(workspaceId)
        : Promise.resolve([]),
    ]).catch((error: unknown) => {
      if (refreshRun.current !== run) return null;
      throw error;
    });
    if (!result || refreshRun.current !== run) return;
    const [next, published, routes] = result;
    setRecord(next);
    setRecords(published);
    setLocalRoutes(routes);
    hydrateEditor(next, routes);
  }, [transport, accountId, name, workspaceId, hydrateEditor, resetEditor]);

  useEffect(() => {
    setRecord(null);
    setHealth({ kind: "idle" });
    refresh().catch((error: unknown) => setNote(String(error)));
    return () => {
      refreshRun.current += 1;
    };
  }, [refresh]);

  const checkHealth = useCallback(async (): Promise<void> => {
    const run = ++healthRun.current;
    const definition = record?.statement.route;
    if (!transport || !record || !definition) {
      setHealth({ kind: "idle" });
      return;
    }
    if (!definition.policy.methods.includes("head")) {
      setHealth({ kind: "disabled" });
      return;
    }
    setHealth({ kind: "checking" });
    try {
      const result = await gateway.probeRouteHealth(transport, record);
      if (healthRun.current !== run) return;
      setHealth(result.status >= 500
        ? { kind: "failing", ...result }
        : result.status >= 400
          ? { kind: "reachable", ...result }
          : { kind: "serving", ...result });
    } catch {
      if (healthRun.current === run) setHealth({ kind: "unavailable" });
    }
  }, [transport, record]);

  useEffect(() => {
    void checkHealth();
    if (!record?.statement.route?.policy.methods.includes("head")) return;
    const timer = window.setInterval(() => void checkHealth(), 30_000);
    return () => {
      window.clearInterval(timer);
      healthRun.current += 1;
    };
  }, [checkHealth, record]);

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
    if (audience === "accounts") {
      throw new Error("Explicit account audiences are read-only in this Routes editor.");
    }
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
    setNote(address ? `Saved ${address}.` : "Route saved. Register a Duck name to make it browsable.");
    if (address) onSaved(address);
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
    setNote("Route removed. Its signed revision tombstone prevents replay.");
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
    setNote("Starter created in the route's DuckFS root. Save when ready.");
  };

  const run = (operation: () => Promise<void>): void => {
    setBusy(true);
    setNote(null);
    operation()
      .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
      .finally(() => setBusy(false));
  };

  const canMutate = Boolean(
    transport && isTauri() && workspaceId && accountId && publisherNode && chainId && !busy,
  );
  const canPublish = canMutate && audience !== "accounts";

  const healthText = health.kind === "checking" ? "Checking end to end…"
    : health.kind === "serving" ? `Healthy · HTTP ${health.status}`
      : health.kind === "reachable" ? `Reachable · HTTP ${health.status}`
        : health.kind === "failing" ? `Unhealthy · HTTP ${health.status}`
          : health.kind === "disabled" ? "Not checked · HEAD is not allowed"
            : health.kind === "unavailable" ? "Unavailable"
              : "Not checked";
  const healthColor = health.kind === "serving" ? "#397047"
    : health.kind === "failing" || health.kind === "unavailable" ? color.danger
      : color.muted3;
  const displayAddress = (item: RouteSummary): string => {
    const prefix = item.name.label ? `${item.name.label}.` : "";
    return handle ? `${prefix}${handle}.duck` : item.name.label ?? "Account apex";
  };

  return (
    <aside aria-label="Routes" style={{
      width: 356,
      flexShrink: 0,
      borderLeft: `1px solid ${color.borderSoft}`,
      background: color.sidebar,
      padding: 16,
      overflowY: "auto",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ font: `650 13px ${font.sans}`, color: color.dark }}>Routes</span>
        <button onClick={onClose} aria-label="Close routes" style={{ ...buttonStyle(), width: 28, padding: 0, textAlign: "center" }}>×</button>
      </div>
      <p style={{ font: `400 11px/1.55 ${font.sans}`, color: color.muted3, margin: "12px 0 14px" }}>
        Connect one account address to exact DuckFS content or a local HTTP service.
        The address, reverse proxy, and signed access policy are saved together.
      </p>

      {!handle && accountId && (
        <div style={{ padding: "9px 10px", border: `1px solid ${color.border}`, borderRadius: radius.sm, background: color.paper, color: color.muted3, font: `500 10.5px/1.45 ${font.sans}` }}>
          Routes can exist by Account ID. Register a Duck name in Account to make them browsable as <span style={{ fontFamily: font.mono }}>.duck</span> addresses.
        </div>
      )}

      <section aria-label="Published routes" style={{ marginTop: 14 }}>
        <div style={{ color: color.muted3, font: `650 10px ${font.sans}`, marginBottom: 6 }}>Published routes</div>
        {records.length === 0 ? (
          <div style={{ padding: "10px 0", color: color.muted, font: `500 10px ${font.sans}` }}>No routes published.</div>
        ) : (
          <div style={{ display: "grid", gap: 5 }}>
            {records.map((item) => {
              const selected = routeKey(item.name) === routeKey(name);
              const routePublisher = gateway.bytesToHex(item.publisher_node);
              return (
                <button
                  key={routeKey(item.name)}
                  aria-label={`Edit ${displayAddress(item)}`}
                  onClick={() => {
                    setLabel(item.name.label ?? "");
                    setNote(null);
                  }}
                  style={{
                    all: "unset",
                    cursor: "pointer",
                    display: "grid",
                    gridTemplateColumns: "1fr auto",
                    gap: "3px 8px",
                    padding: "8px 9px",
                    borderRadius: radius.sm,
                    border: `1px solid ${selected ? color.borderStrong : color.border}`,
                    background: selected ? color.paper : "transparent",
                  }}
                >
                  <span style={{ color: color.ink, font: `600 10.5px ${font.mono}`, overflowWrap: "anywhere" }}>{displayAddress(item)}</span>
                  <span style={{ color: color.muted3, font: `600 9px ${font.sans}` }}>
                    {routeTargetLabel(item)} · {routePublisher === publisherNode ? "this node" : short(routePublisher, 6)}
                  </span>
                  <span style={{ color: selected ? healthColor : color.muted, font: `500 9px ${font.sans}` }}>
                    {selected ? healthText : "Published"}
                  </span>
                  <span style={{ color: color.muted, font: `500 9px ${font.mono}` }}>r{item.revision}</span>
                </button>
              );
            })}
          </div>
        )}
      </section>

      <div style={{ marginTop: 15, paddingTop: 14, borderTop: `1px solid ${color.border}` }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: 8 }}>
          <span style={{ color: color.dark, font: `650 11px ${font.sans}` }}>{record?.statement.route ? "Edit route" : "New route"}</span>
          <span style={{ color: color.muted, font: `500 9px ${font.mono}` }}>revision {record?.statement.revision ?? "—"}</span>
        </div>
        <div style={{ marginTop: 5, color: color.inkSoft, font: `500 10.5px ${font.mono}`, overflowWrap: "anywhere" }}>{address ?? "Account ID route"}</div>
        {record?.statement.route && (
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 7 }}>
            <span role="status" style={{ color: healthColor, font: `600 10px ${font.sans}` }}>{healthText}</span>
            <button disabled={health.kind === "checking"} onClick={() => void checkHealth()} aria-label="Check route health" style={{ ...buttonStyle(health.kind === "checking"), height: 24, padding: "0 8px", fontSize: 9 }}>Check</button>
          </div>
        )}
      </div>

      <div style={{ display: "grid", gap: 8, marginTop: 16 }}>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Route label <span style={{ color: color.muted }}>(blank = account apex)</span>
          <input aria-label="Route label" value={label} onChange={(event) => setLabel(event.target.value.toLowerCase())} placeholder="api" spellCheck={false} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
        </label>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Source
          <select aria-label="Route target" value={target} onChange={(event) => setTarget(event.target.value as typeof target)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }}>
            <option value="duck_fs">DuckFS content</option>
            <option value="loopback_http">Local HTTP service</option>
          </select>
        </label>
        <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
          Audience
          <select aria-label="Route audience" value={audience} onChange={(event) => setAudience(event.target.value as typeof audience)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }}>
            <option value="network">All identified network members</option>
            <option value="owner">Owning account only</option>
            {audience === "accounts" && <option value="accounts">Explicit accounts (read only)</option>}
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
          <button disabled={!canMutate} onClick={() => run(createStarter)} style={{ ...buttonStyle(!canMutate), marginTop: 9, width: "100%", textAlign: "center" }}>Create starter file</button>
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
        <button disabled={!canPublish} onClick={() => run(publish)} style={{ ...buttonStyle(!canPublish), textAlign: "center" }}>Save route</button>
        {record?.statement.route && (
          <button disabled={!canMutate} onClick={() => run(unpublish)} style={{ ...buttonStyle(!canMutate), textAlign: "center", color: color.danger }}>Remove route</button>
        )}
      </div>
      {!isTauri() && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Saving routes requires the desktop user-key signer.</p>}
      {isTauri() && !accountId && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Bind this node to your Identity account before saving routes.</p>}
      {audience === "accounts" && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>This explicit-account policy remains active but cannot be changed in this compact editor.</p>}
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
  const [routesOpen, setRoutesOpen] = useState(false);

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
        <button onClick={() => setRoutesOpen((openPanel) => !openPanel)} style={buttonStyle()}>Routes</button>
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
        {routesOpen && <RoutesPanel onClose={() => setRoutesOpen(false)} onSaved={setInput} />}
      </div>
    </div>
  );
}
