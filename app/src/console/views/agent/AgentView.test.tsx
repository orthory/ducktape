import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Channel } from "../../../domain/chat-client";
import type { Workspace } from "../../../domain/workspace-client";
import { AgentView, runIsMine } from "./AgentView";
import type { PendingRun } from "../../../domain/runs-client";

const bytes = (value: number) => Array.from({ length: 32 }, () => value);

const channels: Channel[] = [
  {
    id: "general",
    name: "General",
    created_at: 1,
    head_seq: 42,
    post_policy: "open",
    hooks: [],
    pinned: [],
  },
  {
    id: "project",
    name: "Project",
    created_at: 2,
    head_seq: 7,
    post_policy: "open",
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
        owner: "system" as const,
        display_name: "Summary Agent",
        capability: "alpha",
        prompt_hash: bytes(0xab),
        prompt_doc: null,
        allowed_actions: ["chat.post", "tasks.create"],
        status: "active" as const,
        created_at: 10,
        updated_at: 20,
      },
      {
        agent_id: "qa-agent",
        owner: "system" as const,
        display_name: "QA Agent",
        capability: "beta",
        prompt_hash: bytes(0xcd),
        prompt_doc: null,
        allowed_actions: ["chat.post"],
        status: "paused" as const,
        created_at: 11,
        updated_at: 21,
      },
    ],
    watches: [{ channel_id: "general", policy: { assigned: "summarizer" } }],
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
        requester: "system" as const,
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

const openTab = (name: RegExp) => fireEvent.click(screen.getByRole("tab", { name }));

describe("AgentView", () => {
  it("shows the selected agent's detail and pauses/resumes it", () => {
    const { spies } = renderAgents();

    expect(screen.getByRole("heading", { name: "Agents" })).toBeInTheDocument();

    // The Agents tab is the default; the first agent's detail shows without a click.
    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    expect(within(detail).getByText("Summary Agent")).toBeInTheDocument();
    expect(within(detail).getByText("summarizer")).toBeInTheDocument();
    expect(within(detail).getByText("Alpha")).toBeInTheDocument();
    expect(within(detail).getByText("Post to chat")).toBeInTheDocument();
    expect(within(detail).getByText("Create tasks")).toBeInTheDocument();

    fireEvent.click(within(detail).getByRole("button", { name: /pause agent/i }));
    expect(spies.pauseAgent).toHaveBeenCalledWith("summarizer");

    fireEvent.click(screen.getByRole("button", { name: /open details for qa agent/i }));
    fireEvent.click(screen.getByRole("button", { name: /resume agent/i }));
    expect(spies.resumeAgent).toHaveBeenCalledWith("qa-agent");

    // Ask-to-respond defaults to the channel's latest message (42), no anchor step.
    fireEvent.change(screen.getByLabelText("Channel"), { target: { value: "general" } });
    fireEvent.click(screen.getByRole("button", { name: /ask to respond/i }));
    expect(spies.requestRun).toHaveBeenCalledWith({
      agentId: "qa-agent",
      channelId: "general",
      anchorSeq: 42,
    });
  });

  it("adds an agent through the focused Add-agent pane", () => {
    const { spies } = renderAgents();

    // No always-on register form: it opens on demand.
    expect(screen.queryByLabelText("System prompt")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));

    // Agent ID is auto-derived from the name; with no executors announced,
    // "Runs on" degrades to a text field so setup is never blocked.
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
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

  it("manages auto-reply on its own tab", () => {
    const { spies } = renderAgents();

    openTab(/auto-reply/i);

    fireEvent.change(screen.getByLabelText("Channel to watch"), {
      target: { value: "project" },
    });
    fireEvent.change(screen.getByLabelText("When to reply"), {
      target: { value: "mention" },
    });
    fireEvent.click(screen.getByRole("button", { name: /add auto-reply/i }));
    expect(spies.watchChannel).toHaveBeenCalledWith({
      channelId: "project",
      policy: "mention",
    });

    fireEvent.click(screen.getByRole("button", { name: /stop watching general/i }));
    expect(spies.unwatchChannel).toHaveBeenCalledWith("general");
  });

  it("cancels an in-progress run and toggles the jobs worker on the Activity tab", () => {
    const { spies } = renderAgents();

    openTab(/activity/i);

    fireEvent.click(screen.getByRole("button", { name: /cancel run general\/42\/summarizer/i }));
    expect(spies.cancelRun).toHaveBeenCalledWith("general/42/summarizer");

    fireEvent.click(screen.getByRole("switch", { name: /jobs worker/i }));
    expect(spies.enableJobWorker).toHaveBeenCalledWith(true);
  });

  it("offers announced executors as a Runs on picker, defaulting to the first", () => {
    renderAgents({ capabilities: ["claude", "codex"] });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    const runsOn = screen.getByLabelText("Runs on");
    expect(runsOn.tagName).toBe("SELECT");
    // A single glance, no typing: the first announced executor is pre-selected.
    expect(runsOn).toHaveValue("claude");
    // Raw tags surface as friendly, title-cased labels.
    expect(within(runsOn).getByRole("option", { name: "Claude" })).toBeInTheDocument();
    expect(within(runsOn).getByRole("option", { name: "Codex" })).toBeInTheDocument();
  });

  it("keeps Agent ID out of the default flow, with an Advanced override", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));

    // The id is derived, not asked for.
    expect(screen.queryByLabelText("Agent ID")).toBeNull();

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
    fireEvent.change(screen.getByLabelText("System prompt"), {
      target: { value: "Do things." },
    });

    // Power users can still pin a specific id under Advanced.
    fireEvent.click(screen.getByRole("button", { name: /^advanced$/i }));
    fireEvent.change(screen.getByLabelText("Agent ID"), { target: { value: "custom-id" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));

    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ agentId: "custom-id", capability: "beta" }),
    );
  });

  it("keeps Runs on a free-text field across keystrokes when no executors are announced", () => {
    const { spies } = renderAgents(); // capabilities: [] by default

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    expect(screen.getByLabelText("Runs on").tagName).toBe("INPUT");

    // A partial value must NOT morph the input into a single-option <select>:
    // typing one character mid-word previously trapped the field.
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "c" } });
    expect(screen.getByLabelText("Runs on").tagName).toBe("INPUT");
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "codex" } });
    expect(screen.getByLabelText("Runs on")).toHaveValue("codex");

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    fireEvent.change(screen.getByLabelText("System prompt"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ capability: "codex" }),
    );
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

  it("shows which node is executing an in-flight run", () => {
    const nodeKey = "cd".repeat(32);
    renderAgents({
      runAssignee: new Map([["general/42/summarizer", nodeKey]]),
      authorNames: { [nodeKey]: "Node Bob" },
    });

    openTab(/activity/i);
    expect(screen.getByText("on Node Bob")).toBeInTheDocument();
  });

  it("filters the timeline to the runs I requested", () => {
    const myKey = "ab".repeat(32); // 32 bytes of 0xab as hex
    const mineRun: PendingRun = {
      run_id: "general/50/summarizer",
      dispatch_id: "aa".repeat(32),
      agent_id: "summarizer",
      channel_id: "general",
      anchor_seq: 50,
      thread_root: null,
      job_id: null,
      job_claim_height: 0,
      requester: { external: Array.from({ length: 32 }, () => 0xab) },
      created_at: 31,
    };
    const systemRun: PendingRun = {
      run_id: "general/42/summarizer",
      dispatch_id: "ef".repeat(32),
      agent_id: "summarizer",
      channel_id: "general",
      anchor_seq: 42,
      thread_root: null,
      job_id: null,
      job_claim_height: 0,
      requester: "system" as const,
      created_at: 30,
    };
    renderAgents({
      workspace: {
        id: "w",
        name: "W",
        chainId: "w#1",
        pubkey: myKey,
        founder: true,
        member: true,
        ports: { listen: 1, http: 2, rpc: 3 },
      } as Workspace,
      pendingRuns: [mineRun, systemRun],
    });

    openTab(/activity/i);
    // Both runs show under the default "All" filter.
    expect(
      screen.getByRole("button", { name: /cancel run general\/50\/summarizer/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /cancel run general\/42\/summarizer/i }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /requested by you/i }));
    // Only the run I requested remains.
    expect(
      screen.getByRole("button", { name: /cancel run general\/50\/summarizer/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /cancel run general\/42\/summarizer/i }),
    ).not.toBeInTheDocument();
  });
});

describe("runIsMine", () => {
  const mkRun = (requester: PendingRun["requester"]): PendingRun =>
    ({ run_id: "r", requester }) as PendingRun;

  it("matches an external requester equal to my pubkey (any hex case)", () => {
    expect(runIsMine(mkRun({ external: [0xab, 0xcd] }), "ABCD")).toBe(true);
  });

  it("rejects a different external requester", () => {
    expect(runIsMine(mkRun({ external: [0x01, 0x02] }), "abcd")).toBe(false);
  });

  it("is false for module/system requesters and when I have no pubkey", () => {
    expect(runIsMine(mkRun({ module: "tagging" }), "abcd")).toBe(false);
    expect(runIsMine(mkRun("system"), "abcd")).toBe(false);
    expect(runIsMine(mkRun({ external: [0xab, 0xcd] }), null)).toBe(false);
  });
});
