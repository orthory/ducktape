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
    expect(screen.getByText("SNAPSHOT")).toBeInTheDocument();
  });

  it("embeds every account target in the capability-free inline gateway view", async () => {
    // jsdom has no ResizeObserver; the inline effect needs one.
    vi.stubGlobal("ResizeObserver", class {
      observe() {}
      disconnect() {}
    });
    const openInline = vi.spyOn(gateway, "openInline").mockResolvedValue();
    const closeInline = vi.spyOn(gateway, "closeInline").mockResolvedValue();
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
    const view = renderBrowser();
    const address = screen.getByRole("textbox", { name: "Duck address" });
    fireEvent.change(address, { target: { value: "api.alice.duck/v1" } });
    fireEvent.submit(address.closest("form")!);

    await screen.findByTestId("gateway-inline-pane");
    await waitFor(() => expect(openInline).toHaveBeenCalledWith(
      "http://0123456789abcdef0123456789abcdef.localhost:49152/v1",
      // the .duck route, so a permission prompt can name the site rather than
      // the random loopback session origin it is served from
      "api.alice.duck",
      "tab-1",
      expect.objectContaining({ width: expect.any(Number), height: expect.any(Number) }),
    ));
    expect(screen.queryByTestId("duck-browser-frame")).toBeNull();
    expect(screen.getByText("SIGNED")).toBeInTheDocument();

    view.unmount();
    expect(closeInline).toHaveBeenCalled();
  });

  it("keeps independent addresses across browser tabs", () => {
    renderBrowser();
    const first = screen.getByRole("textbox", { name: "Duck address" });
    fireEvent.change(first, { target: { value: "site.demo.duck" } });

    fireEvent.click(screen.getByRole("button", { name: "New tab" }));
    expect(screen.getByRole("textbox", { name: "Duck address" })).toHaveValue("net.duck");
    fireEvent.change(screen.getByRole("textbox", { name: "Duck address" }), {
      target: { value: "app.demo.duck" },
    });

    fireEvent.click(screen.getByRole("tab", { name: "site.demo.duck" }));
    expect(screen.getByRole("textbox", { name: "Duck address" })).toHaveValue("site.demo.duck");
    expect(screen.getAllByRole("tab")).toHaveLength(2);
  });
});
