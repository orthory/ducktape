import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Rule } from "../../../domain/automations-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { AutomationsView } from "./AutomationsView";

const rules: Rule[] = [
  {
    rule_id: "rule-deploy-123456",
    enabled: true,
    trigger: {
      MessagePosted: {
        channel_id: "general",
        mention: null,
        text_contains: "deploy",
      },
    },
    action: { PostMessage: { channel_id: "ops", template: "Heads up: {text}" } },
    created_at: 1_700_000_000,
    fire_count: 3,
  },
];

const renderAutomations = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "automations", root: "bb".repeat(32) }],
    },
    rules,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const noop = vi.fn() as (...args: unknown[]) => void;

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn() as (...args: unknown[]) => void;
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <AutomationsView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("AutomationsView", () => {
  it("summarizes a rule and toggles its enable control", () => {
    const { spies } = renderAutomations();

    expect(
      screen.getByText('When a message in #general contains "deploy" → post to #ops'),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("switch"));
    expect(spies.setRuleEnabled).toHaveBeenCalledWith("rule-deploy-123456", false);
  });

  it("is honest when the automations module is not backed by the node", () => {
    renderAutomations({
      rules: [],
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(
      screen.getByText(/automations module is not available/i),
    ).toBeInTheDocument();
  });
});
