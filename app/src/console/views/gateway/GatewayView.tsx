// The Gateway operator screen: create, edit, and health-check the signed
// account routes that map a .duck address to DuckFS content or a local HTTP
// service. This was the Browser view's "Routes" side panel; routes are
// account/gateway publishing config, not a browsing concern, so they live in
// the node-operator rail next to the Modules surface they belong to.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import * as browser from "../../../domain/duck-browser";
import * as files from "../../../domain/files-client";
import * as gateway from "../../../domain/gateway-client";
import * as identity from "../../../domain/identity-client";
import { normalizeKey } from "../../../domain/names";
import { hasNativeShell } from "../../../domain/node-bootstrap";
import * as workspaces from "../../../domain/workspace-client";
import type { RouteMethod, RouteRecord, RouteStatement, RouteSummary } from "../../../domain/gateway-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { AccountAudiencePicker } from "./AccountAudiencePicker";

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

export const buttonStyle = (disabled = false): CSSProperties => ({
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

export const fieldStyle: CSSProperties = {
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

export function GatewayView() {
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
  const [audienceAccounts, setAudienceAccounts] = useState<string[]>([]);
  const [methods, setMethods] = useState<RouteMethod[]>(["get", "head"]);
  const [requestKiB, setRequestKiB] = useState("256");
  const [responseKiB, setResponseKiB] = useState("4096");
  const [allowAuthorization, setAllowAuthorization] = useState(false);
  const [allowUpgrade, setAllowUpgrade] = useState(false);
  const [record, setRecord] = useState<RouteRecord | null>(null);
  const [records, setRecords] = useState<RouteSummary[]>([]);
  const [localRoutes, setLocalRoutes] = useState<workspaces.GatewayLocalRoute[]>([]);
  const [health, setHealth] = useState<RouteHealth>({ kind: "idle" });
  const refreshRun = useRef(0);
  const healthRun = useRef(0);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const name = useMemo(() => gateway.routeName(label), [label]);
  const labelError = useMemo(() => {
    try {
      gateway.validateRouteName(name);
      return null;
    } catch {
      return "Use lowercase letters, numbers, and hyphens.";
    }
  }, [name]);
  const address = handle && !labelError
    ? `${name.label ? `${name.label}.` : ""}${handle}.duck`
    : null;
  const local = localRoutes.find((route) => route.name.label === name.label);

  const resetEditor = useCallback(() => {
    setTarget("duck_fs");
    setPort("3000");
    setDefaultPath("index.html");
    setAudience("network");
    setAudienceAccounts([]);
    setMethods(["get", "head"]);
    setRequestKiB("256");
    setResponseKiB("4096");
    setAllowAuthorization(false);
    setAllowUpgrade(false);
  }, []);

  const hydrateEditor = useCallback((next: RouteRecord | null, routes: workspaces.GatewayLocalRoute[]) => {
    const definition = next?.statement.route;
    if (!definition) {
      resetEditor();
      return;
    }
    setTarget(definition.target.kind);
    setAudience(definition.policy.audience.kind);
    setAudienceAccounts(
      definition.policy.audience.kind === "accounts"
        ? definition.policy.audience.account_ids.map((id) => gateway.bytesToHex(id))
        : [],
    );
    setMethods(definition.policy.methods);
    setRequestKiB(String(definition.policy.max_request_bytes / 1024));
    setResponseKiB(String(definition.policy.max_response_bytes / 1024));
    setAllowAuthorization(definition.policy.allow_authorization);
    setAllowUpgrade(definition.policy.allow_upgrade);
    if (definition.target.kind === "duck_fs") {
      // The default path lives in the off-consensus manifest, not the record.
      setDefaultPath("index.html");
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
    // The label is a live draft. IME composition and ordinary typing may
    // temporarily make it invalid; do not query (or throw) until it is a
    // complete route name.
    if (labelError) return;
    const accountBytes = identity.hexToBytes(accountId);
    const result = await Promise.all([
      gateway.getRoute(transport, accountBytes, name),
      gateway.listRoutes(transport, accountBytes),
      workspaceId && hasNativeShell()
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
  }, [transport, accountId, name, labelError, workspaceId, hydrateEditor, resetEditor]);

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
    gateway.validateRouteName(name);
    const maxResponse = numericCap(
      responseKiB,
      gateway.MAX_FILE_BYTES,
      "Response cap",
    );
    // Owner is never implicit; validatePolicy (via validateStatement) rejects an
    // empty, oversized, or unsorted account set before the signer is invoked.
    const audiencePolicy: gateway.RouteAudience =
      audience === "accounts" ? gateway.accountsAudience(audienceAccounts) : { kind: audience };
    let definition: gateway.RouteDefinition;
    const previousPort = local?.port ?? null;
    if (target === "duck_fs") {
      const manifest_sha256 = await browser.buildContentManifest(
        transport,
        publisherNode,
        name,
        defaultPath,
      );
      definition = {
        target: { kind: "duck_fs", manifest_sha256 },
        policy: {
          audience: audiencePolicy,
          methods: ["get", "head"],
          max_request_bytes: 0,
          max_response_bytes: maxResponse,
          allow_authorization: false,
          allow_upgrade: false,
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
          audience: audiencePolicy,
          methods: sortedMethods,
          max_request_bytes: maxRequest,
          max_response_bytes: maxResponse,
          allow_authorization: allowAuthorization,
          allow_upgrade: allowUpgrade,
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
    transport && hasNativeShell() && workspaceId && accountId && publisherNode && chainId && !busy,
  );
  const canPublish = canMutate && !labelError && (audience !== "accounts" || audienceAccounts.length > 0);

  const healthText = health.kind === "checking" ? "Checking end to end…"
    : health.kind === "serving" ? `Healthy · HTTP ${health.status}`
      : health.kind === "reachable" ? `Reachable · HTTP ${health.status}`
        : health.kind === "failing" ? `Unhealthy · HTTP ${health.status}`
          : health.kind === "disabled" ? "Not checked · HEAD is not allowed"
            : health.kind === "unavailable" ? "Unavailable"
              : "Not checked";
  const healthColor = health.kind === "serving" ? color.green
    : health.kind === "failing" || health.kind === "unavailable" ? color.danger
      : color.muted3;
  const displayAddress = (item: RouteSummary): string => {
    const prefix = item.name.label ? `${item.name.label}.` : "";
    return handle ? `${prefix}${handle}.duck` : item.name.label ?? "Account apex";
  };
  const accountLabel = (id: string): string =>
    state.accountHandles[id] ? `${state.accountHandles[id]}.duck` : state.authorNames[id] ?? short(id);

  return (
    <div
      data-screen-label="Gateway"
      style={{ flex: 1, minWidth: 0, minHeight: 0, display: "flex", flexDirection: "column", background: color.paper }}
    >
      <div style={{ height: 56, flexShrink: 0, display: "flex", alignItems: "center", gap: 10, padding: "0 22px", borderBottom: `1px solid ${color.borderSoft}`, background: color.paper }}>
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Gateway</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>{records.length}</span>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", background: color.sidebar }}>
        <div
          data-gateway-content="full-width"
          style={{ width: "100%", boxSizing: "border-box", padding: "22px 20px 40px" }}
        >
          <p style={{ font: `400 11px/1.55 ${font.sans}`, color: color.muted3, margin: "0 0 14px" }}>
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
              {labelError && <span role="alert" style={{ display: "block", marginTop: 4, color: color.danger, font: `500 9.5px ${font.sans}` }}>{labelError}</span>}
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
                <option value="accounts">Specific accounts</option>
              </select>
            </label>
          </div>

          {audience === "accounts" && (
            <AccountAudiencePicker
              roster={Object.keys(state.accountKeys)}
              label={accountLabel}
              selected={audienceAccounts}
              onChange={setAudienceAccounts}
              ownerAccountId={accountId}
            />
          )}

          {target === "duck_fs" ? (
            <div style={{ marginTop: 13 }}>
              <label style={{ font: `600 10px ${font.sans}`, color: color.muted3 }}>
                Default path
                <input aria-label="Default path" value={defaultPath} onChange={(event) => setDefaultPath(event.target.value)} style={{ ...fieldStyle, display: "block", width: "100%", marginTop: 5 }} />
              </label>
              <div style={{ marginTop: 7, color: color.muted, font: `500 9.5px/1.45 ${font.mono}`, overflowWrap: "anywhere" }}>
                {publisherNode && !labelError ? gateway.contentRoot(publisherNode, name) : "—"}
              </div>
              <button disabled={!canMutate || Boolean(labelError)} onClick={() => run(createStarter)} style={{ ...buttonStyle(!canMutate || Boolean(labelError)), marginTop: 9, width: "100%", textAlign: "center" }}>Create starter file</button>
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
              <label style={{ display: "block", marginTop: 6, color: color.inkSoft, font: `500 9.5px ${font.sans}` }}>
                <input type="checkbox" checked={allowUpgrade} onChange={(event) => setAllowUpgrade(event.target.checked)} /> Allow WebSocket upgrade
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
          {!hasNativeShell() && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Saving routes requires the desktop user-key signer.</p>}
          {hasNativeShell() && !accountId && <p style={{ color: color.muted, font: `500 10px/1.5 ${font.sans}` }}>Bind this node to your Identity account before saving routes.</p>}
          {note && <div role="status" style={{ marginTop: 12, padding: 10, borderRadius: radius.sm, background: color.paper, border: `1px solid ${color.border}`, color: color.muted3, font: `500 10.5px/1.45 ${font.sans}` }}>{note}</div>}
        </div>
      </div>
    </div>
  );
}
