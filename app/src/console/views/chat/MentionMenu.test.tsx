// The @mention typeahead exercised through the REAL Composer: token typing
// opens the listbox, ArrowUp/Down move aria-selected, Enter/Tab pick instead
// of sending, Escape dismisses, mousedown picks without blurring first.

import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { Composer } from "./Composer";
import { MentionMenu } from "./MentionMenu";
import type { MentionCandidate } from "./mention";

const agent = (
  agentId: string,
  displayName: string,
  status: AgentRecord["status"] = "active",
): AgentRecord => ({
  agent_id: agentId,
  owner: { external: [1] },
  display_name: displayName,
  capability: "echo",
  prompt_hash: Array(32).fill(7),
  allowed_actions: ["chat.post"],
  status,
  created_at: 1,
  updated_at: 1,
});

const ROSTER = [
  agent("quackbot", "Quackbot"),
  agent("scribe", "Scribe the Writer"),
  agent("idler", "Idler", "paused"),
];

const MIXED_CANDIDATES: MentionCandidate[] = [
  { kind: "agent", agent: agent("quackbot", "Quackbot") },
  {
    kind: "user",
    userKeyHex: "abcdef1234567890",
    handle: "jess",
    label: "Jess Example",
  },
];

describe("MentionMenu mixed candidates", () => {
  it("renders agents and users, including the user's label and handle", () => {
    render(<MentionMenu candidates={MIXED_CANDIDATES} activeIndex={0} onPick={vi.fn()} />);

    expect(screen.getByRole("listbox", { name: "Mention a person or agent" })).toBeTruthy();
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Quackbot@quackbot",
      "Jess Example@jess",
    ]);
  });

  it("mousedown on a user row picks the user's handle", () => {
    const onPick = vi.fn();
    render(<MentionMenu candidates={MIXED_CANDIDATES} activeIndex={0} onPick={onPick} />);

    fireEvent.mouseDown(screen.getAllByRole("option")[1]!);

    expect(onPick).toHaveBeenCalledWith("jess");
  });

  it("marks the active row by mixed-list index", () => {
    render(<MentionMenu candidates={MIXED_CANDIDATES} activeIndex={1} onPick={vi.fn()} />);

    const options = screen.getAllByRole("option");
    expect(options[0]!.getAttribute("aria-selected")).toBe("false");
    expect(options[1]!.getAttribute("aria-selected")).toBe("true");
  });
});

function Harness({ onSend }: { onSend: (value: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <Composer
      value={value}
      onChange={setValue}
      onSend={() => onSend(value)}
      placeholder="Message #general"
    />
  );
}

const setup = (onSend = vi.fn(), statePatch: Partial<ConsoleState> = {}) => {
  render(
    <ConsoleContext.Provider
      value={{
        state: { ...createInitialState(), agents: ROSTER, ...statePatch },
        actions: {} as ConsoleActions,
      }}
    >
      <Harness onSend={onSend} />
    </ConsoleContext.Provider>,
  );
  return { textarea: screen.getByPlaceholderText("Message #general"), onSend };
};

const type = (textarea: HTMLElement, value: string) =>
  fireEvent.change(textarea, { target: { value } });

describe("Composer @mention typeahead", () => {
  it("opens on @, lists only Active agents, and filters on the fragment", () => {
    const { textarea } = setup();
    type(textarea, "@");
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual([
      "Quackbot@quackbot",
      "Scribe the Writer@scribe",
    ]);

    // fragment matches display_name too, case-insensitive
    type(textarea, "@WRIT");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual([
      "Scribe the Writer@scribe",
    ]);

    // no match → no listbox
    type(textarea, "@zzz");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("does not open mid-word", () => {
    const { textarea } = setup();
    type(textarea, "mail me a@qu");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("ArrowDown/ArrowUp move the active row and Enter picks it instead of sending", () => {
    const { textarea, onSend } = setup();
    type(textarea, "@");
    expect(screen.getAllByRole("option")[0]!.getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(textarea, { key: "ArrowDown" });
    expect(screen.getAllByRole("option")[1]!.getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(textarea, { key: "ArrowUp" });
    expect(screen.getAllByRole("option")[0]!.getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(textarea, { key: "ArrowUp" });
    // wraps to the last row
    expect(screen.getAllByRole("option")[1]!.getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(textarea, { key: "Enter" });
    expect(onSend).not.toHaveBeenCalled();
    expect((textarea as HTMLTextAreaElement).value).toBe("@scribe ");
    expect(screen.queryByRole("listbox")).toBeNull();

    // with the menu closed, Enter sends as before
    fireEvent.keyDown(textarea, { key: "Enter" });
    expect(onSend).toHaveBeenCalledWith("@scribe ");
  });

  it("Tab picks the active row, replacing the typed fragment", () => {
    const { textarea } = setup();
    type(textarea, "hey @qu");
    fireEvent.keyDown(textarea, { key: "Tab" });
    expect((textarea as HTMLTextAreaElement).value).toBe("hey @quackbot ");
  });

  it("Escape dismisses the menu for that token and Enter falls through to send", () => {
    const { textarea, onSend } = setup();
    type(textarea, "@qu");
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();

    fireEvent.keyDown(textarea, { key: "Enter" });
    expect(onSend).toHaveBeenCalledWith("@qu");
  });

  it("a menu-dismissing Escape does not bubble (the ThreadPanel must not close)", () => {
    const onWrapperKeyDown = vi.fn();
    render(
      <ConsoleContext.Provider
        value={{
          state: { ...createInitialState(), agents: ROSTER },
          actions: {} as ConsoleActions,
        }}
      >
        <div onKeyDown={onWrapperKeyDown}>
          <Harness onSend={vi.fn()} />
        </div>
      </ConsoleContext.Provider>,
    );
    const textarea = screen.getByPlaceholderText("Message #general");
    type(textarea, "@qu");
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onWrapperKeyDown).not.toHaveBeenCalled();

    // without a menu, Escape bubbles as before (the thread panel's contract)
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(onWrapperKeyDown).toHaveBeenCalledTimes(1);
  });

  it("mousedown on a row picks it (no click-after-blur race)", () => {
    const { textarea } = setup();
    type(textarea, "@scr");
    fireEvent.mouseDown(screen.getByRole("option"));
    expect((textarea as HTMLTextAreaElement).value).toBe("@scribe ");
  });

  it("lists node users and inserts the picked user's handle", () => {
    const { textarea } = setup(vi.fn(), {
      nodeUsers: {
        "01": { userKey: "ABCDEF1234567890", name: "Jess Example" },
      },
    });

    type(textarea, "@je");

    expect(screen.getByRole("listbox", { name: "Mention a person or agent" })).toBeTruthy();
    expect(screen.getByText("Jess Example")).toBeTruthy();
    expect(screen.getByText("@jess-example")).toBeTruthy();

    fireEvent.mouseDown(screen.getByRole("option"));

    expect((textarea as HTMLTextAreaElement).value).toBe("@jess-example ");
  });

  it("Enter picks a person and inserts the user's handle", () => {
    const { textarea, onSend } = setup(vi.fn(), {
      nodeUsers: {
        "01": { userKey: "ABCDEF1234567890", name: "Jess Example" },
      },
    });

    type(textarea, "@je");
    fireEvent.keyDown(textarea, { key: "Enter" });

    expect(onSend).not.toHaveBeenCalled();
    expect((textarea as HTMLTextAreaElement).value).toBe("@jess-example ");
  });

  it("IME composition Enter neither picks nor sends", () => {
    const { textarea, onSend } = setup();
    type(textarea, "@qu");
    fireEvent.keyDown(textarea, { key: "Enter", isComposing: true });
    expect((textarea as HTMLTextAreaElement).value).toBe("@qu");
    expect(onSend).not.toHaveBeenCalled();
  });
});
