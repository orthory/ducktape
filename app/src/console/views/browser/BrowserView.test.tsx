import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as duckBrowser from "../../../domain/duck-browser";
import * as gateway from "../../../domain/gateway-client";
import { makeTransportStub } from "../../../test/transport-stub";
import type { ConsoleActions } from "../../store/actions";
import type { ConsoleState } from "../../store/state";
import type { NodeTransport } from "../../../domain/transport";
import { ConsoleContext } from "../../store/context";
import { createInitialState } from "../../store/state";
import { BrowserView } from "./BrowserView";

const actions = new Proxy({}, { get: () => vi.fn() }) as ConsoleActions;

const renderBrowser = (
  patch: Partial<ConsoleState> = {},
  transport: NodeTransport = makeTransportStub(),
) => render(
  <ConsoleContext.Provider value={{
    state: { ...createInitialState(), connected: true, ...patch },
    actions,
    transport,
  }}>
    <BrowserView />
  </ConsoleContext.Provider>,
);

afterEach(() => vi.restoreAllMocks());

describe("BrowserView security boundary", () => {
  it("renders net.duck only in an empty-sandbox iframe", async () => {
    vi.spyOn(duckBrowser, "loadDuckPage").mockResolvedValue({
      address: {
        kind: "network",
        handle: "net",
        name: { label: null },
        hostname: "net.duck",
        pathAndQuery: "/",
        canonical: "net.duck",
      },
      hosting: "network",
      snapshot: "44".repeat(32),
      title: "Network",
      srcDoc: "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'\"><main>network</main>",
      fileCount: 1,
      totalBytes: 7,
    });
    renderBrowser();
    fireEvent.submit(screen.getByRole("textbox", { name: "Duck address" }).closest("form")!);

    const frame = await screen.findByTestId("duck-browser-frame");
    expect(frame.getAttribute("sandbox")).toBe("");
    expect(frame.getAttribute("referrerpolicy")).toBe("no-referrer");
    expect(frame.getAttribute("srcdoc")).toContain("script-src 'none'");
    expect(screen.getByText("NETWORK SNAPSHOT")).toBeInTheDocument();
  });

  it("opens every account target in a capability-free gateway window", async () => {
    const openWindow = vi.spyOn(gateway, "openWindow").mockResolvedValue();
    vi.spyOn(duckBrowser, "loadDuckPage").mockResolvedValue({
      address: {
        kind: "account",
        handle: "alice",
        name: { label: "api" },
        hostname: "api.alice.duck",
        pathAndQuery: "/v1",
        canonical: "api.alice.duck/v1",
      },
      hosting: "gateway",
      target: "loopback_http",
      accountId: "11".repeat(32),
      publisherNode: "22".repeat(32),
      signer: "33".repeat(32),
      revision: 3,
      title: "api.alice.duck",
      srcUrl: "http://0123456789abcdef0123456789abcdef.localhost:49152/v1",
      fileCount: 0,
      totalBytes: 0,
    });
    renderBrowser();
    const address = screen.getByRole("textbox", { name: "Duck address" });
    fireEvent.change(address, { target: { value: "api.alice.duck/v1" } });
    fireEvent.submit(address.closest("form")!);

    await waitFor(() => expect(openWindow).toHaveBeenCalledWith(
      "http://0123456789abcdef0123456789abcdef.localhost:49152/v1",
      "api.alice.duck",
    ));
    expect(screen.queryByTestId("duck-browser-frame")).toBeNull();
    expect(screen.getByText("SIGNED GATEWAY ROUTE")).toBeInTheDocument();
    expect(screen.getByText("Opened in an isolated gateway window")).toBeInTheDocument();
  });

  it("describes one route abstraction", () => {
    renderBrowser();
    fireEvent.click(screen.getByRole("button", { name: "Routes" }));
    expect(screen.getByRole("complementary", { name: "Routes" })).toBeInTheDocument();
    expect(screen.getByText(/address, reverse proxy, and signed access policy are saved together/)).toBeInTheDocument();
  });

  it("lists account routes and health-checks the selected route end to end", async () => {
    const node = "22".repeat(32);
    const account = "11".repeat(32);
    const record: gateway.RouteRecord = {
      statement: {
        version: gateway.ROUTE_FORMAT_VERSION,
        chain_id: "test",
        account_id: new Array(32).fill(0x11),
        name: { label: "api" },
        publisher_node: new Array(32).fill(0x22),
        revision: 3,
        route: {
          target: { kind: "loopback_http" },
          policy: {
            audience: { kind: "network" },
            methods: ["get", "head"],
            max_request_bytes: 0,
            max_response_bytes: 4096,
            allow_authorization: false,
          },
        },
      },
      authorization: { signer: new Array(32).fill(0x33), signature: new Array(64).fill(0x44) },
    };
    const summary: gateway.RouteSummary = {
      name: record.statement.name,
      publisher_node: record.statement.publisher_node,
      revision: record.statement.revision,
      target: "loopback_http",
    };
    const query = vi.fn().mockImplementation((_target: string, request: unknown) => {
      if (typeof request === "object" && request && "list" in request) return { routes: [summary] };
      if (
        typeof request === "object" && request && "get" in request &&
        (request as { get: { name: { label: string | null } } }).get.name.label === "api"
      ) return { route: record };
      return { route: null };
    });
    const gatewayProxy = vi.fn().mockResolvedValue({
      head: { status: 204, headers: [] },
      body: new Uint8Array(0),
    });
    renderBrowser({
      workspace: {
        id: "test",
        name: "Test",
        chainId: "test",
        pubkey: node,
        founder: true,
        member: true,
        ports: { listen: 1, http: 2, rpc: 3 },
      },
      nodeUsers: { [node]: { accountId: account, name: "Alice" } },
      accountHandles: { [account]: "alice" },
    }, makeTransportStub({ query, gatewayProxy }));

    fireEvent.click(screen.getByRole("button", { name: "Routes" }));
    const route = await screen.findByRole("button", { name: "Edit api.alice.duck" });
    fireEvent.click(route);

    await waitFor(() => expect(gatewayProxy).toHaveBeenCalledWith(expect.objectContaining({
      head: expect.objectContaining({ method: "head", path_and_query: "/", headers: [] }),
    })));
    expect(await screen.findAllByText("Healthy · HTTP 204")).not.toHaveLength(0);
  });
});
