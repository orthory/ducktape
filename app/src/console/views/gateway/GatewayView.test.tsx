import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as gateway from "../../../domain/gateway-client";
import { makeTransportStub } from "../../../test/transport-stub";
import type { ConsoleActions } from "../../store/actions";
import type { ConsoleState } from "../../store/state";
import type { NodeTransport } from "../../../domain/transport";
import { ConsoleContext } from "../../store/context";
import { createInitialState } from "../../store/state";
import { GatewayView } from "./GatewayView";

const actions = new Proxy({}, { get: () => vi.fn() }) as ConsoleActions;

const renderGateway = (
  patch: Partial<ConsoleState> = {},
  transport: NodeTransport = makeTransportStub(),
) => render(
  <ConsoleContext.Provider value={{
    state: { ...createInitialState(), connected: true, ...patch },
    actions,
    transport,
  }}>
    <GatewayView />
  </ConsoleContext.Provider>,
);

afterEach(() => vi.restoreAllMocks());

describe("GatewayView route editor", () => {
  it("describes one route abstraction", () => {
    renderGateway();
    const content = document.querySelector('[data-gateway-content="full-width"]') as HTMLElement;
    expect(content).toHaveStyle({ width: "100%" });
    expect(content.style.maxWidth).toBe("");
    expect(screen.getByText("Gateway")).toBeInTheDocument();
    expect(screen.getByText(/address, reverse proxy, and signed access policy are saved together/)).toBeInTheDocument();
  });

  it("keeps an invalid IME route label inside the editor instead of crashing", async () => {
    const node = "22".repeat(32);
    const account = "11".repeat(32);
    renderGateway({
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
    });

    fireEvent.change(screen.getByRole("textbox", { name: "Route label" }), {
      target: { value: "ㄷ" },
    });

    expect(await screen.findByText("Use lowercase letters, numbers, and hyphens.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Route label" })).toHaveValue("ㄷ");
    expect(screen.getByText("Gateway")).toBeInTheDocument();
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
            allow_upgrade: false,
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
    renderGateway({
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

    const route = await screen.findByRole("button", { name: "Edit api.alice.duck" });
    fireEvent.click(route);

    await waitFor(() => expect(gatewayProxy).toHaveBeenCalledWith(expect.objectContaining({
      head: expect.objectContaining({ method: "head", path_and_query: "/", headers: [] }),
    })));
    expect(await screen.findAllByText("Healthy · HTTP 204")).not.toHaveLength(0);
  });
});
