import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { computeAccessibleDescription, computeAccessibleName } from "dom-accessibility-api";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { opKey } from "../../store/finalization";
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
import type { PendingRun, RunRecord } from "../../../domain/runs-client";

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
    capabilities: ["beta"],
    capabilitiesStatus: "ready" as const,
    agents: [
      {
        agent_id: "summarizer",
        owner: "system" as const,
        display_name: "Summary Agent",
        capability: "alpha",
        allowed_actions: ["chat.post", "tasks.create"],
        caps: { forge_read: ["ducktape"], subagent_budget: 2 },
        status: "active" as const,
        created_at: 10,
        updated_at: 20,
        skills: [
          {
            name: "persona",
            source_prefix: "/shared/agents/summarizer/persona",
            load: "always" as const,
          },
        ],
      },
      {
        agent_id: "qa-agent",
        owner: "system" as const,
        display_name: "QA Agent",
        capability: "beta",
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
  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  let updateCapabilities: (capabilities: string[]) => void;
  let updateOps: (ops: ConsoleState["ops"]) => void;

  function Harness() {
    const [state, setState] = useState(initialState);
    updateCapabilities = (capabilities) =>
      setState((prev) => ({ ...prev, capabilitiesStatus: "ready", capabilities }));
    updateOps = (ops) => setState((prev) => ({ ...prev, ops }));
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??=
            key === "registerAgent" || key === "updateAgent"
              ? vi.fn().mockResolvedValue(true)
              : vi.fn();
          return spies[key];
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
  return {
    spies,
    setCapabilities: (capabilities: string[]) => act(() => updateCapabilities(capabilities)),
    setOps: (ops: ConsoleState["ops"]) => act(() => updateOps(ops)),
  };
};

const deferredResult = () => {
  let resolve!: (result: boolean) => void;
  const promise = new Promise<boolean>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};

const openTab = (name: RegExp) => fireEvent.click(screen.getByRole("tab", { name }));

describe("AgentView", () => {
  // A clicked @agent mention hands off through state.agentFocus. If the agent is
  // gone from the roster, the pane used to fall back to `agents[0]` — showing a
  // DIFFERENT agent as if it were the one clicked. A miss must read as a miss.
  it("says an agent is missing rather than showing a different one", () => {
    renderAgents({ agentFocus: "ghost-bot" });

    expect(screen.getByText(/agent not found/i)).toBeInTheDocument();
    expect(screen.getByText("ghost-bot")).toBeInTheDocument();
    // the first agent's detail must NOT be standing in for the one that was
    // clicked. (Summary Agent still appears in the ROSTER — that's the list, not
    // the pane; the defect was the DETAIL pane silently showing it.)
    expect(screen.queryByRole("region", { name: /agent detail/i })).not.toBeInTheDocument();
  });

  it("still defaults to the first agent when nothing is selected", () => {
    renderAgents();
    const detail = screen.getByRole("region", { name: /agent detail/i });
    expect(within(detail).getByText("Summary Agent")).toBeInTheDocument();
  });

  it("shows the selected agent's detail and pauses/resumes it", () => {
    const { spies } = renderAgents();

    expect(screen.getByRole("heading", { name: "Agents" })).toBeInTheDocument();

    // The Agents tab is the default; the first agent's detail shows without a click.
    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    expect(within(detail).getByText("Summary Agent")).toBeInTheDocument();
    expect(within(detail).getByText("summarizer")).toBeInTheDocument();
    expect(within(detail).getByText("Alpha")).toBeInTheDocument();
    // "chat.post" is the REPLY grant — it only lets an agent answer where it was
    // engaged. Posting into arbitrary channels is the separate chat.post_message
    // grant, so the two must never read as the same permission.
    expect(within(detail).getByText("Reply in chat")).toBeInTheDocument();
    expect(within(detail).getByText("Create tasks")).toBeInTheDocument();
    expect(within(detail).getByText("Forge read: ducktape")).toBeInTheDocument();
    expect(within(detail).getByText("Peer calls: 2")).toBeInTheDocument();

    fireEvent.click(within(detail).getByRole("button", { name: /pause agent/i }));
    expect(spies.pauseAgent).toHaveBeenCalledWith("summarizer");

    fireEvent.click(screen.getByRole("button", { name: /open details for qa agent/i }));
    fireEvent.click(screen.getByRole("button", { name: /resume agent/i }));
    expect(spies.resumeAgent).toHaveBeenCalledWith("qa-agent");

    // Ask-to-respond moved onto the message in chat — the management page
    // no longer hosts the form.
    expect(screen.queryByText(/ask to respond/i)).toBeNull();
  });

  it("disables pause or resume while the agent write is pending", () => {
    const { spies } = renderAgents({
      ops: {
        [opKey.agent("summarizer")]: {
          seq: 1,
          phase: "pending",
          startedAt: 10,
        },
      },
    });

    const pause = screen.getByRole("button", { name: /pause agent/i });
    const edit = screen.getByRole("button", { name: /^edit$/i });
    expect(pause).toBeDisabled();
    expect(edit).toBeDisabled();
    fireEvent.click(pause);
    fireEvent.click(edit);
    expect(spies.pauseAgent).not.toHaveBeenCalled();
    expect(screen.queryByRole("form", { name: /edit agent/i })).not.toBeInTheDocument();
  });

  it("blocks an open edit form when another agent write becomes pending", () => {
    const { spies, setOps } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /^edit$/i }));
    const form = screen.getByRole("form", { name: /edit agent/i });
    fireEvent.change(within(form).getByLabelText(/edit display name/i), {
      target: { value: "Draft survives" },
    });

    setOps({
      [opKey.agent("summarizer")]: {
        seq: 1,
        phase: "pending",
        startedAt: 10,
      },
    });

    expect(screen.getByRole("button", { name: /close edit/i })).toBeDisabled();
    const save = within(form).getByRole("button", { name: /save changes/i });
    const cancel = within(form).getByRole("button", { name: /^cancel$/i });
    expect(save).toBeDisabled();
    expect(cancel).toBeDisabled();
    expect(form).toHaveAttribute("aria-busy", "true");
    expect(within(form).getByLabelText(/edit display name/i)).toHaveValue("Draft survives");
    fireEvent.click(save);
    fireEvent.click(cancel);
    expect(spies.updateAgent).not.toHaveBeenCalled();
    expect(screen.getByRole("form", { name: /edit agent/i })).toBeInTheDocument();
  });

  it("renders roster, detail, and the three tabs after the split", () => {
    renderAgents();

    const content = document.querySelector('[data-agent-content="full-width"]') as HTMLElement;
    expect(content).toHaveStyle({ width: "100%" });
    expect(content.style.maxWidth).toBe("");

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

    // No always-on register form: it opens on demand. And no prompt textarea —
    // an agent's persona is a document, not form text.
    expect(screen.queryByLabelText("System prompt")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    expect(screen.queryByLabelText("System prompt")).toBeNull();

    // Agent ID is auto-derived from the name; the announced executor is
    // selected without asking the operator to type a routing tag.
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    // Exact match: an agent with no curated skills sends no skills key at all
    // (and never a prompt). It DOES send the library read cap: that grant is on
    // by default, and it is what earns the run's assembled context the paragraph
    // telling the agent the shared library exists.
    expect(spies.registerAgent).toHaveBeenCalledWith({
      displayName: "Triage Agent",
      agentId: "triage-agent",
      capability: "beta",
      allowedActions: ["chat.post"],
      caps: { duckfs_read: ["/shared/skills"] },
    });
  });

  it("keeps the registration draft through pending and failure, then closes on success", async () => {
    const { spies } = renderAgents();
    const pending = deferredResult();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    let attempts = 0;
    spies.registerAgent.mockImplementation(() =>
      (attempts += 1) === 1 ? pending.promise : Promise.resolve(false),
    );
    const name = screen.getByLabelText("Agent display name");
    fireEvent.change(name, { target: { value: "Triage Agent" } });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
    const submit = screen.getByRole("button", { name: /register agent/i });
    const cancel = screen.getByRole("button", { name: /^cancel$/i });
    const form = screen.getByRole("region", { name: /register agent/i }).querySelector("form")!;

    await act(async () => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    expect(spies.registerAgent).toHaveBeenCalledTimes(1);
    expect(submit).toBeDisabled();
    expect(cancel).toBeDisabled();
    expect(form).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("button", { name: "Registering…" })).toBeDisabled();
    fireEvent.click(cancel);
    expect(screen.getByRole("region", { name: /register agent/i })).toBeInTheDocument();
    expect(name).toHaveValue("Triage Agent");

    await act(async () => {
      pending.resolve(false);
      await pending.promise;
    });
    expect(submit).toBeEnabled();
    expect(cancel).toBeEnabled();
    expect(form).toHaveAttribute("aria-busy", "false");
    expect(name).toHaveValue("Triage Agent");

    spies.registerAgent.mockResolvedValue(true);
    await act(async () => {
      fireEvent.click(submit);
      await Promise.resolve();
    });
    expect(screen.queryByRole("region", { name: /register agent/i })).not.toBeInTheDocument();
  });

  it("grants the skill library by default, and lets the operator withhold it", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });

    // The affordance is a plain checkbox, ON out of the box.
    const grant = screen.getByLabelText(/search the global skill library/i);
    expect((grant as HTMLInputElement).checked).toBe(true);

    // Unticked, the agent registers with NO duckfs_read grant — and the node's
    // assembler, asking the same caps, then never tells it the library is there.
    fireEvent.click(grant);
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith({
      displayName: "Triage Agent",
      agentId: "triage-agent",
      capability: "beta",
      allowedActions: ["chat.post"],
    });
  });

  it("an agent registered without the library grant can be given it by editing", () => {
    const { spies } = renderAgents();

    // `summarizer` (the fixture) carries no duckfs_read cap at all.
    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });
    fireEvent.click(within(detail).getByRole("button", { name: /^edit$/i }));
    const grant = screen.getByLabelText(/search the global skill library/i);
    expect((grant as HTMLInputElement).checked).toBe(false);

    fireEvent.click(grant);
    fireEvent.click(screen.getByRole("button", { name: /save changes/i }));
    // caps REPLACE wholesale, so the save carries the library grant alongside
    // every other cap the record already held.
    expect(spies.updateAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        agentId: "summarizer",
        caps: {
          forge_read: ["ducktape"],
          duckfs_read: ["/shared/skills"],
          subagent_budget: 2,
        },
      }),
    );
  });

  it("submits every resource cap and the whole-tree peer-call budget", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Builder Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
    for (const [label, value] of [
      ["Forge read repositories", "alpha, beta"],
      ["Forge push repositories", "beta"],
      ["Additional DuckFS read prefixes", "/shared/data"],
      ["DuckFS write prefixes", "/shared/output"],
      ["Allowed tool IDs", "browser.search"],
      ["Secret references", "vault/github"],
      ["Page write access", "page-1 *"],
      ["Peer-call budget", "3"],
    ]) {
      fireEvent.change(screen.getByLabelText(label), { target: { value } });
    }
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));

    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        caps: {
          forge_read: ["alpha", "beta"],
          forge_push: ["beta"],
          duckfs_read: ["/shared/data", "/shared/skills"],
          duckfs_write: ["/shared/output"],
          tools: ["browser.search"],
          secrets: ["vault/github"],
          pages_write: ["page-1", "*"],
          subagent_budget: 3,
        },
      }),
    );
  });

  it("keeps the edit draft through pending and failure, then closes on success", async () => {
    const { spies } = renderAgents();
    const pending = deferredResult();
    let attempts = 0;
    spies.updateAgent.mockImplementation(() =>
      (attempts += 1) === 1 ? pending.promise : Promise.resolve(false),
    );

    fireEvent.click(screen.getByRole("button", { name: /^edit$/i }));
    const name = screen.getByLabelText("Edit display name");
    fireEvent.change(name, { target: { value: "Revised Agent" } });
    const submit = screen.getByRole("button", { name: /save changes/i });
    const form = screen.getByRole("form", { name: /edit agent/i });
    const cancel = within(form).getByRole("button", { name: /^cancel$/i });

    await act(async () => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    expect(spies.updateAgent).toHaveBeenCalledTimes(1);
    expect(submit).toBeDisabled();
    expect(cancel).toBeDisabled();
    expect(form).toHaveAttribute("aria-busy", "true");
    expect(within(form).getByRole("button", { name: "Saving…" })).toBeDisabled();
    fireEvent.click(cancel);
    expect(screen.getByRole("form", { name: /edit agent/i })).toBeInTheDocument();
    expect(name).toHaveValue("Revised Agent");

    await act(async () => {
      pending.resolve(false);
      await pending.promise;
    });
    expect(submit).toBeEnabled();
    expect(cancel).toBeEnabled();
    expect(form).toHaveAttribute("aria-busy", "false");
    expect(name).toHaveValue("Revised Agent");

    spies.updateAgent.mockResolvedValue(true);
    await act(async () => {
      fireEvent.click(submit);
      await Promise.resolve();
    });
    expect(screen.queryByRole("form", { name: /edit agent/i })).not.toBeInTheDocument();
  });

  it("curates skills: the persona is always-loaded, the rest on demand", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });

    // The persona affordance seeds an always-loaded skill under the agent's own
    // duckfs folder — the document lives in Files, not in this form.
    fireEvent.click(screen.getByRole("button", { name: /persona \(always loaded\)/i }));
    expect(screen.getByLabelText("Skill name")).toHaveValue("persona");
    expect(screen.getByLabelText("Document folder (duckfs)")).toHaveValue(
      "/shared/agents/triage-agent/persona",
    );

    // A second, on-demand skill: named + pointed at a shared skill folder.
    fireEvent.click(screen.getByRole("button", { name: /skill \(on demand\)/i }));
    const names = screen.getAllByLabelText("Skill name");
    const folders = screen.getAllByLabelText("Document folder (duckfs)");
    fireEvent.change(names[1], { target: { value: "release" } });
    fireEvent.change(folders[1], { target: { value: "/shared/skills/release" } });

    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        agentId: "triage-agent",
        skills: [
          {
            name: "persona",
            source_prefix: "/shared/agents/triage-agent/persona",
            load: "always",
          },
          { name: "release", source_prefix: "/shared/skills/release", load: "on_demand" },
        ],
      }),
    );
  });

  it("the always-load toggle flips a skill between soul and on-demand", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });
    fireEvent.click(screen.getByRole("button", { name: /persona \(always loaded\)/i }));

    // Untick "always load": the same document becomes an on-demand skill.
    fireEvent.click(screen.getByLabelText("Always load persona"));
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        skills: [
          {
            name: "persona",
            source_prefix: "/shared/agents/triage-agent/persona",
            load: "on_demand",
          },
        ],
      }),
    );
  });

  it("opens a curated skill's document in the Files surface", () => {
    const { spies } = renderAgents();

    fireEvent.click(screen.getByRole("button", { name: /open details for summary agent/i }));
    const detail = screen.getByRole("region", { name: /agent detail/i });

    // The detail pane names the persona and its document, and hands off to Files.
    expect(within(detail).getByText("ALWAYS")).toBeInTheDocument();
    expect(
      within(detail).getByText("/shared/agents/summarizer/persona/SKILL.md"),
    ).toBeInTheDocument();
    fireEvent.click(within(detail).getByRole("button", { name: /^open$/i }));
    expect(spies.openFiles).toHaveBeenCalledWith("/shared/agents/summarizer/persona");
  });

  it("seeds a persona document in duckfs — and never clobbers an existing one", async () => {
    const filesStat = vi.fn().mockResolvedValue(null);
    const filesCommit = vi
      .fn()
      .mockResolvedValue({ height: 2, appHash: "aa".repeat(32) });
    // The commit's CAS base is the live head, read over the generic query lane.
    const query = vi.fn().mockResolvedValue({ refs: { head: "beef", pins: {}, window: 1 } });
    const transport = makeTransportStub({ filesStat, filesCommit, query });
    renderAgents({}, transport);

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.click(screen.getByRole("button", { name: /persona \(always loaded\)/i }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /create doc/i }));
    });

    // One ordinary duckfs commit — a put of the starter SKILL.md at the prefix.
    expect(filesStat).toHaveBeenCalledWith({
      path: "/shared/agents/triage-agent/persona/SKILL.md",
    });
    const body = filesCommit.mock.calls[0][0];
    expect(body.changes[0].put.path).toBe("/shared/agents/triage-agent/persona/SKILL.md");
    expect(await screen.findByRole("status")).toHaveTextContent(/Created .*Edit it in Files/);

    // A second click finds the document and refuses to overwrite it.
    filesStat.mockResolvedValue({ path: "x", kind: "file", size: 1, exec: false, object: "", meta: {} });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /create doc/i }));
    });
    expect(filesCommit).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent(/already exists/);
  });

  it("composes an agent out of the global skill library — and publishes into it", async () => {
    const dir = (path: string) => ({
      path,
      kind: "dir" as const,
      size: 0,
      exec: false,
      object: "",
      meta: {},
    });
    const docs: Record<string, string> = {
      "/shared/skills/release/SKILL.md": "---\nname: release\ndescription: Cut a release.\n---\n",
      // No frontmatter: still a library skill, listed under its folder name.
      "/shared/skills/triage/SKILL.md": "# triage\n",
    };
    const filesLs = vi.fn().mockResolvedValue({
      entries: [dir("/shared/skills/release"), dir("/shared/skills/triage")],
      next: null,
    });
    const filesRead = vi.fn(async ({ path }: { path: string }) => ({
      b64: btoa(docs[path] ?? ""),
      eof: true,
    }));
    const filesStat = vi.fn().mockResolvedValue(null);
    const filesCommit = vi.fn().mockResolvedValue({ height: 2, appHash: "aa".repeat(32) });
    const query = vi.fn().mockResolvedValue({ refs: { head: "beef", pins: {}, window: 1 } });
    const transport = makeTransportStub({ filesLs, filesRead, filesStat, filesCommit, query });
    const { spies } = renderAgents({}, transport);

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage Agent" },
    });
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "beta" } });

    // The library is one duckfs directory — browsed with the ordinary files
    // client, described by each SKILL.md's frontmatter.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /from library/i }));
    });
    expect(filesLs).toHaveBeenCalledWith({ path: "/shared/skills" });
    expect(await screen.findByText("Cut a release.")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /add triage from the library/i }),
    ).toBeInTheDocument();

    // Search narrows the pool; picking curates the skill with its prefix filled in.
    fireEvent.change(screen.getByLabelText("Search the skill library"), {
      target: { value: "release" },
    });
    expect(screen.queryByRole("button", { name: /add triage from the library/i })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /add release from the library/i }));
    expect(screen.getByLabelText("Document folder (duckfs)")).toHaveValue(
      "/shared/skills/release",
    );

    // Publishing a skill the library doesn't have yet seeds its SKILL.md through
    // the same create-doc commit — under the shared root, not the agent's folder.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /from library/i }));
    });
    fireEvent.change(screen.getByLabelText("Search the skill library"), {
      target: { value: "Release Notes" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /publish .*to the library/i }));
    });
    const put = filesCommit.mock.calls[0][0].changes[0].put;
    expect(put.path).toBe("/shared/skills/release-notes/SKILL.md");

    // Exact match: library skills ride the ordinary skills key (with `load`),
    // and no prompt blob is ever sent.
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith({
      displayName: "Triage Agent",
      agentId: "triage-agent",
      capability: "beta",
      allowedActions: ["chat.post"],
      caps: { duckfs_read: ["/shared/skills"] },
      skills: [
        { name: "release", source_prefix: "/shared/skills/release", load: "on_demand" },
        {
          name: "Release Notes",
          source_prefix: "/shared/skills/release-notes",
          load: "on_demand",
        },
      ],
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

  it("blocks both auto-reply controls while their channel key is pending", () => {
    const { spies } = renderAgents({
      ops: {
        [opKey.watch("general")]: {
          seq: 1,
          phase: "pending",
          startedAt: 10,
        },
      },
    });

    openTab(/auto-reply/i);
    const turnOff = screen.getByRole("button", { name: /stop watching general/i });
    expect(turnOff).toBeDisabled();
    fireEvent.click(turnOff);
    expect(spies.unwatchChannel).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Channel to watch"), {
      target: { value: "general" },
    });
    const add = screen.getByRole("button", { name: /add auto-reply/i });
    expect(add).toBeDisabled();
    fireEvent.click(add);
    expect(spies.watchChannel).not.toHaveBeenCalled();
  });

  it("reassigns or cancels an in-progress run and offers honest worker actions", () => {
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
      screen.getByRole("button", {
        name: /force reassign run summary agent.*general @42/i,
      }),
    );
    expect(spies.reassignRun).toHaveBeenCalledWith("general/42/summarizer", 0);

    fireEvent.click(
      screen.getByRole("button", { name: /cancel run summary agent.*general @42/i }),
    );
    expect(spies.cancelRun).toHaveBeenCalledWith("general/42/summarizer");

    expect(screen.queryByRole("switch", { name: /jobs worker/i })).toBeNull();
    expect(screen.getByText(/committed status is not readable/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /enable worker/i }));
    expect(spies.enableJobWorker).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByRole("button", { name: /disable worker/i }));
    expect(spies.enableJobWorker).toHaveBeenCalledWith(false);
  });

  it("exposes a rejected worker action without claiming a registration state", () => {
    renderAgents({
      ops: {
        [opKey.jobWorker()]: {
          seq: 1,
          phase: "failed",
          startedAt: 10,
          settledAt: 20,
          error: "worker cap reached",
        },
      },
    });

    openTab(/activity/i);
    expect(screen.queryByRole("switch", { name: /jobs worker/i })).toBeNull();
    expect(screen.getByRole("button", { name: "rejected" })).toBeInTheDocument();
  });

  it("disables both jobs-worker actions while one is pending", () => {
    renderAgents({
      ops: {
        [opKey.jobWorker()]: {
          seq: 1,
          phase: "pending",
          startedAt: 10,
        },
      },
    });

    openTab(/activity/i);
    expect(screen.getByRole("button", { name: /enable worker/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /disable worker/i })).toBeDisabled();
    expect(screen.getByText(/waiting for confirmation/i)).toBeInTheDocument();
  });

  it.each([
    ["force reassign", "reassignRun", "cancelRun"],
    ["cancel", "cancelRun", "reassignRun"],
  ] as const)(
    "disables both run actions when %s starts for their shared key",
    (firstAction, firstSpy, blockedSpy) => {
      const { spies, setOps } = renderAgents({
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
      const reassign = screen.getByRole("button", {
        name: /force reassign run summary agent.*general @42/i,
      });
      const cancel = screen.getByRole("button", {
        name: /cancel run summary agent.*general @42/i,
      });

      fireEvent.click(firstAction === "force reassign" ? reassign : cancel);
      expect(spies[firstSpy]).toHaveBeenCalledTimes(1);

      setOps({
        [opKey.run("general/42/summarizer")]: {
          seq: 1,
          phase: "pending",
          startedAt: 10,
        },
      });

      expect(reassign).toBeDisabled();
      expect(cancel).toBeDisabled();
      fireEvent.click(reassign);
      fireEvent.click(cancel);
      expect(spies[firstSpy]).toHaveBeenCalledTimes(1);
      expect(spies[blockedSpy]).not.toHaveBeenCalled();
    },
  );

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

    // Power users can still pin a specific id under Advanced.
    fireEvent.click(screen.getByRole("button", { name: /^advanced$/i }));
    fireEvent.change(screen.getByLabelText("Agent ID"), { target: { value: "custom-id" } });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));

    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ agentId: "custom-id", capability: "beta" }),
    );
  });

  it("blocks dead-agent registration when no provider is available", () => {
    const { spies } = renderAgents({ capabilities: [] });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    expect(screen.getByLabelText("Runs on")).toBeDisabled();
    expect(screen.getByPlaceholderText("No provider available")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(
      /direct.*node host.*podman or tart.*image or vm.*node.toml.*restart/i,
    );

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    expect(screen.getByRole("button", { name: /register agent/i })).toBeDisabled();
    expect(spies.registerAgent).not.toHaveBeenCalled();
  });

  it("offers a real retry action when the provider registry fails", () => {
    const { spies } = renderAgents({
      capabilities: [],
      capabilitiesStatus: "error",
    });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    expect(screen.getByRole("alert")).toHaveTextContent(/could not load.*retry/i);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(spies.refreshCapabilities).toHaveBeenCalledTimes(1);
  });

  it("blocks registration if the selected provider disappears", () => {
    const { spies, setCapabilities } = renderAgents({ capabilities: ["codex"] });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    expect(screen.getByRole("button", { name: /register agent/i })).toBeEnabled();

    setCapabilities([]);
    expect(screen.getByRole("button", { name: /register agent/i })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).not.toHaveBeenCalled();
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
    // Exact match: no prompt key survives anywhere in the edit payload, and the
    // other fields keep their current values.
    expect(spies.updateAgent).toHaveBeenCalledWith({
      agentId: "summarizer",
      displayName: "Renamed Agent",
      capability: "alpha",
      allowedActions: ["chat.post", "tasks.create"],
      caps: { forge_read: ["ducktape"], subagent_budget: 2 },
      // so does the curated skill set — an update REPLACES it wholesale, so an
      // untouched form must send the record's own skills back unchanged.
      skills: [
        {
          name: "persona",
          source_prefix: "/shared/agents/summarizer/persona",
          load: "always",
        },
      ],
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

  it("does not wait forever when a terminal run has no retained output", async () => {
    const record: RunRecord = {
      run_id: "forge:ducktape:56/7/summarizer",
      agent_id: "summarizer",
      channel_id: "forge:ducktape:56",
      anchor_seq: 7,
      outcome: "delivered",
      degraded: false,
      created_at: 30,
      delivered_at: 35,
      executing_node: "unknown",
      output_ref: "agent/item-56@abcdef0123456789",
      pr_number: 140,
    };
    const transport = makeTransportStub({
      query: vi.fn().mockResolvedValue({ recent_runs: [record] }),
      view: vi.fn().mockResolvedValue({ usage: [] }),
      subscribe: vi.fn(() => () => {}),
    });
    renderAgents({ pendingRuns: [] }, transport);

    openTab(/activity/i);
    fireEvent.click(
      await screen.findByRole("button", {
        name: /show execution log for run summary agent.*forge:ducktape:56 @7/i,
      }),
    );

    const log = screen.getByRole("log");
    expect(
      within(log).getByText("No retained output received — older output may have been evicted."),
    ).toBeInTheDocument();
    expect(within(log).queryByText("Waiting for retained output…")).not.toBeInTheDocument();
  });

  it("keyboard-opens a production-format terminal log and catches up retained output", async () => {
    const runId = "chat\x1fforge:ducktape:56\x1f7\x1fsummarizer";
    const record: RunRecord = {
      run_id: runId,
      agent_id: "summarizer",
      channel_id: "forge:ducktape:56",
      anchor_seq: 7,
      outcome: "delivered",
      degraded: false,
      created_at: 30,
      delivered_at: 35,
      executing_node: "unknown",
      output_ref: "agent/item-56@abcdef0123456789",
      pr_number: 140,
    };
    let handlers: TopicHandlers | undefined;
    const subscribe = vi.fn((_topics, next: TopicHandlers) => {
      handlers = next;
      return () => {};
    });
    const transport = makeTransportStub({
      query: vi.fn().mockResolvedValue({ recent_runs: [record] }),
      view: vi.fn().mockResolvedValue({ usage: [] }),
      subscribe,
    });
    renderAgents({ pendingRuns: [] }, transport);

    openTab(/activity/i);
    const toggle = await screen.findByRole("button", {
      name: /show execution log for run summary agent.*forge:ducktape:56 @7/i,
    });
    expect(toggle.getAttribute("aria-label")).not.toContain("\x1f");
    toggle.focus();
    expect(toggle).toHaveFocus();
    fireEvent.click(toggle);

    const log = screen.getByRole("log", {
      name: /execution log for run summary agent.*forge:ducktape:56 @7/i,
    });
    expect(log.getAttribute("aria-label")).not.toContain("\x1f");
    expect(log).toHaveFocus();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(toggle).toHaveAttribute("aria-controls", log.id);
    for (const element of document.body.querySelectorAll("*")) {
      expect(computeAccessibleName(element)).not.toContain("\x1f");
      expect(computeAccessibleDescription(element)).not.toContain("\x1f");
    }
    expect(subscribe).toHaveBeenCalledWith(
      ["run-output:ef0d635e287bb66490c26824198278cf8011f5679de48b0faeaf388843e9e5df"],
      expect.any(Object),
    );

    act(() => {
      handlers?.onTail?.({
        type: "tail",
        topic:
          "run-output:ef0d635e287bb66490c26824198278cf8011f5679de48b0faeaf388843e9e5df",
        cursor: "3",
        item: { stream: "stderr", line: "focused test failed" },
      });
    });
    expect(within(log).getByText("focused test failed")).toBeInTheDocument();
  });

  it("opens history anchors and PRs through user-facing navigation", async () => {
    const forgeRecord: RunRecord = {
      run_id: "forge:ducktape:56/7/summarizer",
      agent_id: "summarizer",
      channel_id: "forge:ducktape:56",
      anchor_seq: 7,
      outcome: "delivered",
      degraded: false,
      created_at: 30,
      delivered_at: 35,
      executing_node: "unknown",
      output_ref: null,
      pr_number: 140,
    };
    const chatRecord: RunRecord = {
      ...forgeRecord,
      run_id: "general/42/summarizer",
      channel_id: "general",
      anchor_seq: 42,
      pr_number: null,
    };
    const transport = makeTransportStub({
      query: vi.fn().mockResolvedValue({ recent_runs: [forgeRecord, chatRecord] }),
      view: vi.fn().mockResolvedValue({ usage: [] }),
    });
    const { spies } = renderAgents({ pendingRuns: [] }, transport);

    openTab(/activity/i);
    fireEvent.click(
      await screen.findByRole("button", { name: "Open forge:ducktape:56 @7" }),
    );
    expect(spies.openForgeItem).toHaveBeenCalledWith({
      repo: "ducktape",
      number: 56,
      messageSeq: 7,
    });

    fireEvent.click(screen.getByRole("button", { name: "Open General @42" }));
    expect(spies.focusMessage).toHaveBeenCalledWith("general", 42);

    fireEvent.click(screen.getByRole("button", { name: "Open PR #140" }));
    expect(spies.openForgeItem).toHaveBeenLastCalledWith({
      repo: "ducktape",
      number: 140,
    });
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
      screen.getByRole("button", { name: /cancel run summary agent.*general @50/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /cancel run summary agent.*general @42/i }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /requested by you/i }));
    // Only the run I requested remains.
    expect(
      screen.getByRole("button", { name: /cancel run summary agent.*general @50/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /cancel run summary agent.*general @42/i }),
    ).not.toBeInTheDocument();
  });
});

describe("RunsOnPicker", () => {
  it("prefers medium when creating an Agent from an unpinned variant", () => {
    renderAgents({
      capabilities: [
        "codex_gpt-5.6-sol_high",
        "codex_gpt-5.6-sol_low",
        "codex_gpt-5.6-sol_medium",
      ],
    });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));

    expect(screen.getByLabelText("Effort")).toHaveValue("medium");
    expect(screen.getByText("codex_gpt-5.6-sol_medium")).toBeInTheDocument();
  });

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
        "codex_gpt-5.6-terra_medium",
        "claude_opus_max",
        "claude_opus_medium",
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

    // A model without medium keeps the first-announced fallback; the composed
    // tag is shown verbatim under the picker.
    fireEvent.change(model, { target: { value: "gpt-5.5" } });
    expect(screen.getByLabelText("Effort")).toHaveValue("low");
    expect(screen.getByText("codex_gpt-5.5_low")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Effort"), { target: { value: "xhigh" } });
    expect(screen.getByText("codex_gpt-5.5_xhigh")).toBeInTheDocument();

    // Switching model narrows efforts to what that model announced.
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "gpt-5.6-terra" } });
    expect(screen.getByLabelText("Effort")).toHaveValue("medium");

    // A provider with no base tag prefers its announced medium variant.
    fireEvent.change(runsOn, { target: { value: "claude" } });
    expect(screen.getByLabelText("Model")).toHaveValue("opus");
    expect(screen.getByText("claude_opus_medium")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Agent display name"), {
      target: { value: "Triage" },
    });
    fireEvent.click(screen.getByRole("button", { name: /register agent/i }));
    expect(spies.registerAgent).toHaveBeenCalledWith(
      expect.objectContaining({ capability: "claude_opus_medium" }),
    );
  });

  it("keeps provider switching within the first announced model", () => {
    renderAgents({
      capabilities: ["codex", "claude_opus_max", "claude_sonnet_medium"],
    });

    fireEvent.click(screen.getByRole("button", { name: /add agent/i }));
    fireEvent.change(screen.getByLabelText("Runs on"), { target: { value: "claude" } });

    expect(screen.getByLabelText("Model")).toHaveValue("opus");
    expect(screen.getByLabelText("Effort")).toHaveValue("max");
    expect(screen.getByText("claude_opus_max")).toBeInTheDocument();
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
