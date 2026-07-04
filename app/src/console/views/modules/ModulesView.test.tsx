import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { NodeStatus } from "../../../domain/transport";
import { ModulesView } from "./ModulesView";

const statusWith = (modules: NodeStatus["modules"]): NodeStatus => ({
  version: "0.1.0",
  height: 7,
  appHash: "aa".repeat(32),
  modules,
});

const renderModules = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    ...patch,
  };
  const actions = new Proxy(
    {},
    { get: () => vi.fn() as (...args: unknown[]) => void },
  ) as ConsoleActions;

  render(
    <ConsoleContext.Provider value={{ state: initialState, actions }}>
      <ModulesView />
    </ConsoleContext.Provider>,
  );
};

describe("ModulesView", () => {
  it("groups modules under category section headers in catalog order", () => {
    renderModules({
      status: statusWith([
        { id: "chat", root: "bb".repeat(32), category: "workspace" },
        { id: "forge", root: "cc".repeat(32), category: "developer" },
        { id: "automations", root: "dd".repeat(32), category: "automation" },
        { id: "files", root: "ee".repeat(32), category: "system" },
        // no category → an older/unknown node; must still render, under System.
        { id: "mystery", root: "ff".repeat(32) },
      ]),
    });

    const headers = screen
      .getAllByText(/^(WORKSPACE|DEVELOPER|AUTOMATION|SYSTEM)$/)
      .map((el) => el.textContent);
    expect(headers).toEqual(["WORKSPACE", "DEVELOPER", "AUTOMATION", "SYSTEM"]);

    // the uncategorized module falls into the System group, not a phantom one.
    const systemSection = screen.getByText("SYSTEM").closest("section");
    if (!systemSection) throw new Error("SYSTEM section missing");
    expect(within(systemSection).getByText("Files")).toBeInTheDocument();
    expect(within(systemSection).getAllByText("mystery").length).toBeGreaterThan(0);
  });

  it("omits empty category sections", () => {
    renderModules({
      status: statusWith([
        { id: "chat", root: "bb".repeat(32), category: "workspace" },
        { id: "tasks", root: "cc".repeat(32), category: "workspace" },
      ]),
    });

    expect(screen.getByText("WORKSPACE")).toBeInTheDocument();
    expect(screen.queryByText("DEVELOPER")).not.toBeInTheDocument();
    expect(screen.queryByText("AUTOMATION")).not.toBeInTheDocument();
    expect(screen.queryByText("SYSTEM")).not.toBeInTheDocument();
  });
});
