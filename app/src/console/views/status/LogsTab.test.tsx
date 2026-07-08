import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { RuntimeFacts, Workspace } from "../../../domain/workspace-client";
import { LogsTab } from "./LogsTab";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "ab".repeat(32),
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const facts: RuntimeFacts = {
  pid: 4242,
  alive: true,
  uptimeSecs: 3725, // 1h 02m 05s
  binaryPath: "/opt/ducktape/ducktape-node",
  dataDir: "/home/x/.ducktape/workspaces/acme-research",
  logPath: "/home/x/.ducktape/workspaces/acme-research/daemon.log",
};

const TAIL = "2026-07-09T12:03:01Z  INFO alpha up\n2026-07-09T12:03:02Z ERROR beta down\n";

interface Opts {
  log?: string | null;
  facts?: RuntimeFacts | null;
}

const renderLogs = (patch: Partial<ConsoleState> = {}, opts: Opts = {}) => {
  const initialState: ConsoleState = {
    ...createInitialState(),
    managed: true,
    connected: true,
    workspace,
    status: { version: "0.4.2", height: 9, appHash: "aa".repeat(32), modules: [] },
    ...patch,
  };

  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  const actions = new Proxy(
    {},
    {
      get: (_t, key: string) => {
        if (key === "readDaemonLog") {
          spies[key] ??= vi.fn().mockResolvedValue(
            opts.log === undefined
              ? { path: facts.logPath, tail: TAIL }
              : opts.log === null
                ? null
                : { path: facts.logPath, tail: opts.log },
          );
          return spies[key];
        }
        if (key === "readRuntimeFacts") {
          spies[key] ??= vi
            .fn()
            .mockResolvedValue(opts.facts === undefined ? facts : opts.facts);
          return spies[key];
        }
        spies[key] ??= vi.fn();
        return spies[key];
      },
    },
  ) as ConsoleActions;

  render(
    <ConsoleContext.Provider value={{ state: initialState, actions }}>
      <LogsTab />
    </ConsoleContext.Provider>,
  );
  return { spies };
};

describe("LogsTab", () => {
  it("shows a managed-only empty state for a remote node and never reads the log", () => {
    const { spies } = renderLogs({ managed: false });
    expect(screen.getByText(/only available for the local daemon/i)).toBeInTheDocument();
    expect(screen.queryByText("RUNTIME")).not.toBeInTheDocument();
    expect(spies.readDaemonLog).toBeUndefined();
  });

  it("renders runtime facts and the live tail", async () => {
    renderLogs();
    // runtime facts row
    expect(await screen.findByText("4242")).toBeInTheDocument();
    expect(screen.getByText("v0.4.2")).toBeInTheDocument();
    expect(screen.getByText("1h 02m")).toBeInTheDocument();
    expect(screen.getByText("/opt/ducktape/ducktape-node")).toBeInTheDocument();
    // tail lines, with the ERROR line classified
    expect(await screen.findByText(/alpha up/)).toBeInTheDocument();
    expect(screen.getByText(/beta down/)).toBeInTheDocument();
    expect(screen.getByText("following")).toBeInTheDocument();
    expect(screen.getByText("2 lines")).toBeInTheDocument();
  });

  it("marks an exited process", async () => {
    renderLogs({}, { facts: { ...facts, alive: false } });
    expect(await screen.findByText("4242 (exited)")).toBeInTheDocument();
  });

  it("renders an ANSI-colorized tail with the escape codes stripped", async () => {
    const ESC = String.fromCharCode(27);
    const colored = `${ESC}[2m2026-07-08T17:19:37Z${ESC}[0m ${ESC}[31mERROR${ESC}[0m boringtun expired\n`;
    renderLogs({}, { log: colored });
    // the cleaned text is present…
    expect(
      await screen.findByText(/2026-07-08T17:19:37Z ERROR boringtun expired/),
    ).toBeInTheDocument();
    // …and no raw escape byte leaked into the DOM.
    expect(document.body.textContent ?? "").not.toContain(ESC);
  });

  it("filters lines by search query", async () => {
    renderLogs();
    expect(await screen.findByText(/alpha up/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Search daemon log"), {
      target: { value: "beta" },
    });
    await waitFor(() =>
      expect(screen.queryByText(/alpha up/)).not.toBeInTheDocument(),
    );
    expect(screen.getByText(/beta down/)).toBeInTheDocument();
    expect(screen.getByText("1/2 match")).toBeInTheDocument();
  });

  it("hides a level when its chip is toggled off", async () => {
    renderLogs();
    expect(await screen.findByText(/alpha up/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "INFO lines" }));
    await waitFor(() =>
      expect(screen.queryByText(/alpha up/)).not.toBeInTheDocument(),
    );
    expect(screen.getByText(/beta down/)).toBeInTheDocument();
  });

  it("copies the visible lines to the clipboard", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    renderLogs();
    expect(await screen.findByText(/alpha up/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(
      "2026-07-09T12:03:01Z  INFO alpha up\n2026-07-09T12:03:02Z ERROR beta down",
    );
  });

  it("pauses following and offers Jump to latest when scrolled up", async () => {
    renderLogs();
    const logEl = await screen.findByRole("log");
    // jsdom has no layout; fake a scrolled-up viewport.
    Object.defineProperty(logEl, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(logEl, "clientHeight", { configurable: true, value: 200 });
    Object.defineProperty(logEl, "scrollTop", { configurable: true, value: 0, writable: true });
    fireEvent.scroll(logEl);
    expect(await screen.findByText("paused")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Jump to latest" })).toBeInTheDocument();
  });
});
