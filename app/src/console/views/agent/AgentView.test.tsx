import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Channel } from "../../../domain/chat-client";
import { AgentView } from "./AgentView";

const bytes = (value: number) => Array.from({ length: 32 }, () => value);

const channels: Channel[] = [
  {
    id: "general",
    name: "General",
    created_at: 1,
    head_seq: 42,
    post_policy: "Open",
    hooks: [],
    pinned: [],
  },
  {
    id: "project",
    name: "Project",
    created_at: 2,
    head_seq: 7,
    post_policy: "Open",
    hooks: [],
    pinned: [],
  },
];

const renderAgents = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    channels,
    activeChannel: "general",
    agents: [
      {
        agent_id: "summarizer",
        owner: "System" as const,
        display_name: "Summary Agent",
        capability: "alpha",
        prompt_hash: bytes(0xab),
        prompt_doc: null,
        allowed_actions: ["chat.post", "tasks.create"],
        status: "Active" as const,
        created_at: 10,
        updated_at: 20,
      },
      {
        agent_id: "qa-agent",
        owner: "System" as const,
        display_name: "QA Agent",
        capability: "beta",
        prompt_hash: bytes(0xcd),
        prompt_doc: null,
        allowed_actions: ["chat.post"],
        status: "Paused" as const,
        created_at: 11,
        updated_at: 21,
      },
    ],
    watches: [{ channel_id: "general", policy: { Assigned: "summarizer" } }],
    pendingRuns: [
      {
        run_id: "general/42/summarizer",
        dispatch_id: "ef".repeat(32),
        agent_id: "summarizer",
        channel_id: "general",
        anchor_seq: 42,
        thread_root: null,
        job_id: null,
        job_claim_height: 0,
        requester: "System" as const,
        created_at: 30,
      },
    ],
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
        <AgentView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);
  return { spies };
};

describe("AgentView", () => {
  it("renders a complete agent operations surface over real store data", () => {
    const { spies } = renderAgents();

    expect(screen.getByRole("heading", { name: "Agents" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    expect(within(detail).getByText("Summary Agent")).toBeInTheDocument();
    expect(within(detail).getByText("summarizer")).toBeInTheDocument();
    expect(within(detail).getByText("alpha")).toBeInTheDocument();
    expect(within(detail).getByText("Post to chat")).toBeInTheDocument();
    expect(within(detail).getByText("Create tasks")).toBeInTheDocument();

    fireEvent.click(within(detail).getByRole("button", { name: /pause agent/i }));
    expect(spies.pauseAgent).toHaveBeenCalledWith("summarizer");

    fireEvent.click(screen.getByRole("button", { name: /open details for qa agent/i }));
    fireEvent.click(screen.getByRole("button", { name: /resume agent/i }));
    expect(spies.resumeAgent).toHaveBeenCalledWith("qa-agent");

    fireEvent.change(screen.getByLabelText("Channel to watch"), {
      target: { value: "project" },
    });
    fireEvent.change(screen.getByLabelText("Turn policy"), {
      target: { value: "Mention" },
    });
    fireEvent.click(screen.getByRole("button", { name: /watch channel/i }));
    expect(spies.watchChannel).toHaveBeenCalledWith({
      channelId: "project",
      policy: "Mention",
    });

    fireEvent.click(screen.getByRole("button", { name: /stop watching general/i }));
    expect(spies.unwatchChannel).toHaveBeenCalledWith("general");

    fireEvent.change(screen.getByLabelText("Run channel"), {
      target: { value: "general" },
    });
    fireEvent.change(screen.getByLabelText("Anchor sequence"), {
      target: { value: "42" },
    });
    fireEvent.click(screen.getByRole("button", { name: /request run/i }));
    expect(spies.requestRun).toHaveBeenCalledWith({
      agentId: "qa-agent",
      channelId: "general",
      anchorSeq: 42,
    });

    fireEvent.click(screen.getByRole("button", { name: /cancel run general\/42\/summarizer/i }));
    expect(spies.cancelRun).toHaveBeenCalledWith("general/42/summarizer");

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Agent ID"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Capability"), {
      target: { value: "beta" },
    });
    fireEvent.change(screen.getByLabelText("System prompt"), {
      target: { value: "Summarize incoming incidents." },
    });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith({
      displayName: "Triage Agent",
      agentId: "triage-agent",
      capability: "beta",
      prompt: "Summarize incoming incidents.",
      allowedActions: ["chat.post"],
    });
  });

  it("edits an agent through the inline Edit form", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });

    fireEvent.click(within(detail).getByRole("button", { name: /^edit$/i }));

    const nameField = screen.getByLabelText("Edit display name");
    expect(nameField).toHaveValue("Summary Agent");
    fireEvent.change(nameField, { target: { value: "Renamed Agent" } });

    fireEvent.click(screen.getByRole("button", { name: /save changes/i }));
    // Exact match proves a blank prompt is omitted (never sent as an empty
    // string), while the other fields keep their current values.
    expect(spies.updateAgent).toHaveBeenCalledWith({
      agentId: "summarizer",
      displayName: "Renamed Agent",
      capability: "alpha",
      allowedActions: ["chat.post", "tasks.create"],
    });
  });

  it("toggles the agent module into jobs-board work", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("switch", { name: /jobs worker/i }));
    expect(spies.enableJobWorker).toHaveBeenCalledWith(true);
  });
});
