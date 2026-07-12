import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { color } from "../../theme/tokens";
import type { Channel } from "../../../domain/chat-client";
import type { Workspace } from "../../../domain/workspace-client";
import type { NodeTransport, TopicHandlers } from "../../../domain/transport";
import { makeTransportStub } from "../../../test/transport-stub";
import { AgentView, runIsMine } from "./AgentView";
import {
  FILLED_IDENTITY_TEXT_PERCENT,
  FILLED_SEMANTIC_TEXT_PERCENT,
  filledForeground,
  filledMix,
} from "./parts";
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

const renderAgents = (
  patch: Partial<ConsoleState> = {},
  transport?: NodeTransport,
) => {
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
      <ConsoleContext.Provider value={{ state, actions, transport }}>
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

    // Ask-to-respond moved onto the message in chat — the management page
    // no longer hosts the form.
    expect(screen.queryByText(/ask to respond/i)).toBeNull();
  });

  it("renders roster, detail, and the three tabs after the split", () => {
    renderAgents();

    expect(screen.getByRole("complementary", { name: /agent roster/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /agent detail/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /agents/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /auto-reply/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /activity/i })).toBeInTheDocument();
  });

  it("keeps the Agents title on an ink token for light and dark headers", () => {
    document.documentElement.dataset.theme = "dark";
    renderAgents();

    const heading = screen.getByRole("heading", { name: "Agents" });
    expect(heading).toHaveStyle({ color: color.ink, letterSpacing: "0" });

    document.documentElement.dataset.theme = "light";
  });

  it("derives identity-band overlays from filled tokens in both themes", () => {
    document.documentElement.dataset.theme = "dark";
    renderAgents();

    const detail = screen.getByRole("region", { name: /agent detail/i });
    const agentId = within(detail).getByText("summarizer");
    const status = within(detail).getByText("Active", { exact: true });
    const edit = within(detail).getByRole("button", { name: /^edit$/i });
    const pause = within(detail).getByRole("button", { name: /pause agent/i });

    const styleText = (element: HTMLElement) => element.getAttribute("style") ?? "";
    expect(styleText(agentId)).toContain(`color: ${filledMix(FILLED_IDENTITY_TEXT_PERCENT)}`);
    expect(styleText(status)).toContain(`background: ${filledMix(8)}`);
    expect(styleText(status)).toContain(`border: 1px solid ${filledMix(16)}`);
    for (const button of [edit, pause]) {
      expect(styleText(button)).toContain(`background: ${filledMix(7)}`);
      expect(styleText(button)).toContain(`border: 1px solid ${filledMix(22)}`);
    }
    expect(styleText(edit)).toContain(`color: ${color.onDark}`);
    expect(styleText(status)).toContain(`color: ${filledForeground(color.green)}`);
    expect(styleText(pause)).toContain(`color: ${filledForeground(color.amber)}`);

    fireEvent.click(screen.getByRole("button", { name: /open details for qa agent/i }));
    const pausedDetail = screen.getByRole("region", { name: /agent detail/i });
    const pausedStatus = within(pausedDetail).getByText("Paused", { exact: true });
    const resume = within(pausedDetail).getByRole("button", { name: /resume agent/i });
    expect(styleText(pausedStatus)).toContain(`color: ${filledForeground(color.amber)}`);
    expect(styleText(resume)).toContain(`color: ${filledForeground(color.green)}`);

    // The inline styles keep referring to live theme variables after the
    // filled surface changes polarity, rather than baking in light overlays.
    document.documentElement.dataset.theme = "light";
    expect(styleText(agentId)).toContain(`color: ${filledMix(FILLED_IDENTITY_TEXT_PERCENT)}`);
    expect(styleText(pausedStatus)).toContain(`color: ${filledForeground(color.amber)}`);
    expect(styleText(resume)).toContain(`color: ${filledForeground(color.green)}`);
    expect(styleText(status)).toContain(`background: ${filledMix(8)}`);
    document.documentElement.dataset.theme = "light";
  });

  it("keeps every identity-band label at 4.5:1 in both committed palettes", () => {
    const palettes = [
      {
        name: "light",
        filled: "#26251f",
        onFilled: "#efefef",
        green: "#5cb45f",
        amber: "#c08a3e",
      },
      {
        name: "dark",
        filled: "#ecebe5",
        onFilled: "#1b1a17",
        green: "#6cc06f",
        amber: "#d3a25c",
      },
    ] as const;

    const channels = (hex: string): [number, number, number] => [
      parseInt(hex.slice(1, 3), 16) / 255,
      parseInt(hex.slice(3, 5), 16) / 255,
      parseInt(hex.slice(5, 7), 16) / 255,
    ];
    const mix = (foreground: string, percent: number, background: string) => {
      const fg = channels(foreground);
      const bg = channels(background);
      const weight = percent / 100;
      return fg.map((value, index) => value * weight + bg[index] * (1 - weight));
    };
    const luminance = (rgb: number[]) => {
      const linear = rgb.map((value) =>
        value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4,
      );
      return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
    };
    const contrast = (foreground: number[], background: number[]) => {
      const foregroundLuminance = luminance(foreground);
      const backgroundLuminance = luminance(background);
      return (
        (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
        (Math.min(foregroundLuminance, backgroundLuminance) + 0.05)
      );
    };

    for (const palette of palettes) {
      const band = channels(palette.filled);
      const chip = mix(palette.onFilled, 8, palette.filled);
      const control = mix(palette.onFilled, 7, palette.filled);
      const identityText = mix(
        palette.onFilled,
        FILLED_IDENTITY_TEXT_PERCENT,
        palette.filled,
      );

      expect(contrast(identityText, band), `${palette.name} agent id`).toBeGreaterThanOrEqual(4.5);
      for (const [label, hue] of [
        ["green", palette.green],
        ["amber", palette.amber],
      ] as const) {
        const semanticText = mix(hue, FILLED_SEMANTIC_TEXT_PERCENT, palette.onFilled);
        expect(contrast(semanticText, chip), `${palette.name} ${label} status`).toBeGreaterThanOrEqual(
          4.5,
        );
        expect(contrast(semanticText, control), `${palette.name} ${label} action`).toBeGreaterThanOrEqual(
          4.5,
        );
      }
      expect(contrast(channels(palette.onFilled), control), `${palette.name} edit action`).toBeGreaterThanOrEqual(
        4.5,
      );
    }
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

  it("reassigns or cancels an in-progress run and toggles the jobs worker", () => {
    const { spies } = renderAgents({
      runLease: new Map([
        [
          "general/42/summarizer",
          {
            assigneeHex: "cd".repeat(32),
            attempt: 0,
            maxAttempts: 2,
            expiresAt: 80,
            deadline: 100,
            updatedAt: 40,
            reassignable: true,
          },
        ],
      ]),
    });

    openTab(/activity/i);

    fireEvent.click(
      screen.getByRole("button", { name: /force reassign run general\/42\/summarizer/i }),
    );
    expect(spies.reassignRun).toHaveBeenCalledWith("general/42/summarizer", 0);

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
      // the caps record rides every save (untouched field -> empty list).
      caps: { pages_write: [] },
    });
  });

  it("shows which node is executing an in-flight run", () => {
    const nodeKey = "cd".repeat(32);
    renderAgents({
      runLease: new Map([
        [
          "general/42/summarizer",
          {
            assigneeHex: nodeKey,
            attempt: 0,
            maxAttempts: 2,
            expiresAt: 80,
            deadline: 100,
            updatedAt: 40,
            reassignable: true,
          },
        ],
      ]),
      authorNames: { [nodeKey]: "Node Bob" },
    });

    openTab(/activity/i);
    expect(screen.getByText("on Node Bob")).toBeInTheDocument();
  });

  it("opens a running session and tails its live output", () => {
    let handlers: TopicHandlers | undefined;
    const transport = makeTransportStub({
      query: vi.fn().mockResolvedValue({ recent_runs: [] }),
      view: vi.fn().mockResolvedValue({ usage: [] }),
      subscribe: vi.fn((_topics, next) => {
        handlers = next;
        return () => {};
      }),
    });
    renderAgents({}, transport);

    openTab(/activity/i);
    fireEvent.click(screen.getByRole("button", { name: /show live log for run/i }));
    act(() => {
      const frame = {
        type: "tail",
        topic: `run-output:${"ef".repeat(32)}`,
        cursor: "1",
        item: { stream: "stdout", line: "[node cafe1234] working" },
      } as const;
      handlers?.onTail?.(frame);
      handlers?.onTail?.(frame); // StrictMode can race a subscribe replay.
    });

    expect(screen.getAllByText("[node cafe1234] working")).toHaveLength(1);
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

describe("RunsOnPicker", () => {
  it("collapses Model and Effort to Default when only bare tags are announced", () => {
    const { spies } = renderAgents({ capabilities: ["claude", "codex"] });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));

    // Bare provider tags have no variants: nothing to cascade into.
    const model = screen.getByLabelText("Model");
    const effort = screen.getByLabelText("Effort");
    expect(model).toHaveValue("");
    expect(within(model).getByRole("option", { name: "Default" })).toBeInTheDocument();
    expect(model).toBeDisabled();
    expect(within(effort).getByRole("option", { name: "Default" })).toBeInTheDocument();
    expect(effort).toBeDisabled();

    // The stored capability is the bare announced tag, through the
    // unchanged register payload.
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    fireEvent.change(screen.getByLabelText("System prompt"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ capability: "claude" }),
    );
  });

  it("cascades provider → model → effort across the announced matrix", () => {
    const { spies } = renderAgents({
      capabilities: [
        "codex",
        "codex_gpt-5.5_low",
        "codex_gpt-5.5_xhigh",
        "codex_gpt-5.6-terra_high",
        "claude_opus_max",
      ],
    });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    const runsOn = screen.getByLabelText("Runs on");

    // Defaults to the first announced tag: bare codex → Default/Default.
    expect(runsOn).toHaveValue("codex");
    expect(screen.getByLabelText("Model")).toHaveValue("");

    // Model options = only combinations announced for the provider.
    const model = screen.getByLabelText("Model");
    expect(within(model).getByRole("option", { name: "Default" })).toBeInTheDocument();
    expect(within(model).getByRole("option", { name: "gpt-5.5" })).toBeInTheDocument();
    expect(within(model).getByRole("option", { name: "gpt-5.6-terra" })).toBeInTheDocument();

    // Picking a model adopts its first announced effort; the composed tag
    // is shown verbatim under the picker.
    fireEvent.change(model, { target: { value: "gpt-5.5" } });
    expect(screen.getByLabelText("Effort")).toHaveValue("low");
    expect(screen.getByText("codex_gpt-5.5_low")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Effort"), { target: { value: "xhigh" } });
    expect(screen.getByText("codex_gpt-5.5_xhigh")).toBeInTheDocument();

    // Switching model narrows efforts to what that model announced.
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "gpt-5.6-terra" } });
    expect(screen.getByLabelText("Effort")).toHaveValue("high");

    // A provider with no base tag composes its first variant.
    fireEvent.change(runsOn, { target: { value: "claude" } });
    expect(screen.getByLabelText("Model")).toHaveValue("opus");
    expect(screen.getByText("claude_opus_max")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    fireEvent.change(screen.getByLabelText("System prompt"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ capability: "claude_opus_max" }),
    );
  });

  it("pins a stored tag that is no longer announced, marked offline", () => {
    // Agent "summarizer" stores capability "alpha" — no longer announced.
    const { spies } = renderAgents({ capabilities: ["codex"] });

    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    fireEvent.click(within(detail).getByRole("button", { name: /^edit$/i }));

    const runsOn = screen.getByLabelText("Runs on");
    expect(runsOn).toHaveValue("alpha");
    expect(within(runsOn).getByRole("option", { name: "Alpha (offline)" })).toBeInTheDocument();
    expect(within(runsOn).getByRole("option", { name: "Codex" })).toBeInTheDocument();

    // Saving without touching the field keeps the stored tag — an edit never
    // silently rewrites which executor the agent runs on.
    fireEvent.click(screen.getByRole("button", { name: /save changes/i }));
    expect(spies.updateAgent).toHaveBeenCalledWith(
      expect.objectContaining({ capability: "alpha" }),
    );
  });

  it("pins an offline variant tag through the whole cascade", () => {
    renderAgents({
      capabilities: ["codex"],
      agents: [
        {
          agent_id: "researcher",
          owner: "system" as const,
          display_name: "Researcher",
          capability: "claude_opus_max",
          prompt_hash: bytes(0x11),
          allowed_actions: ["chat.post"],
          status: "active" as const,
          created_at: 10,
          updated_at: 20,
        },
      ],
    });

    fireEvent.click(screen.getByRole("button", { name: /open details for researcher/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    fireEvent.click(within(detail).getByRole("button", { name: /^edit$/i }));

    const runsOn = screen.getByLabelText("Runs on");
    expect(runsOn).toHaveValue("claude");
    expect(
      within(runsOn).getByRole("option", { name: "Claude (offline)" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Model")).toHaveValue("opus");
    expect(
      within(screen.getByLabelText("Model")).getByRole("option", { name: "opus (offline)" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Effort")).toHaveValue("max");
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
