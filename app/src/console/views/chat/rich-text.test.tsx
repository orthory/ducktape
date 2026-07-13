// What a rendered body's references DO when clicked: a mention mark opens its
// principal (agent detail / the person in Members), a `[[page:<id>]]` chip
// opens the page — and neither pretends to resolve something it can't.

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import type { AuthorRef, ChatBlock } from "../../../domain/chat-client";
import type { PageMeta } from "../../../domain/pages-client";
import type { NodeTransport } from "../../../domain/transport";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { CommentText, RichText } from "./rich-text";

const AGENT: AuthorRef = { agent: { module: "runs", agent_id: "quackbot" } };
const GHOST: AuthorRef = { agent: { module: "runs", agent_id: "ghost" } };
const ROSTERED: AgentRecord = {
  agent_id: "quackbot",
  owner: { external: [1] },
  display_name: "Quackbot",
  capability: "echo",
  allowed_actions: ["chat.post"],
  status: "active",
  created_at: 1,
  updated_at: 1,
};
// A user mention carries the ACCOUNT id — the same bytes mention.ts marks with.
const ACCOUNT_HEX = "ab01";
const USER: AuthorRef = { user: [0xab, 0x01] };

const page = (id: string, title: string): PageMeta => ({ id, title, parent: null });

const withStore = (node: ReactNode, statePatch: Partial<ConsoleState> = {}) => {
  const actions = {
    openAgent: vi.fn(),
    openMember: vi.fn(),
    openPage: vi.fn(),
    setScreen: vi.fn(),
  } as unknown as ConsoleActions;
  render(
    <ConsoleContext.Provider
      value={{ state: { ...createInitialState(), ...statePatch }, actions }}
    >
      {node}
    </ConsoleContext.Provider>,
  );
  return actions;
};

const para = (...spans: { text: string; marks?: ChatBlock extends never ? never : unknown[] }[]) =>
  [{ paragraph: spans.map((s) => ({ text: s.text, marks: (s.marks ?? []) as never })) }] as ChatBlock[];

describe("mentions are click targets", () => {
  it("an agent mention opens that agent's detail", () => {
    const actions = withStore(
      <RichText blocks={para({ text: "@quackbot", marks: [{ mention: AGENT }] })} names={{}} />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Open agent quackbot/ }));

    expect(actions.openAgent).toHaveBeenCalledWith("quackbot");
  });

  it("a user mention opens that person in Members, keyed by ACCOUNT id", () => {
    const actions = withStore(
      <RichText
        blocks={para({ text: "@jess", marks: [{ mention: USER }] })}
        names={{ [ACCOUNT_HEX]: "Jess Example" }}
      />,
    );

    // The account-keyed name resolves — the mention reads as the person, not hex.
    fireEvent.click(screen.getByRole("button", { name: /Open Jess Example in Members/ }));

    expect(actions.openMember).toHaveBeenCalledWith(ACCOUNT_HEX);
  });

  it("leaves a plain @token inert — no mark means no principal behind it", () => {
    withStore(<RichText blocks={para({ text: "hi @nobody" })} names={{}} />);

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("@nobody")).toBeTruthy();
  });

  it("renders a mention without a store as inert text — the id, not the module path", () => {
    render(<RichText blocks={para({ text: "@quackbot", marks: [{ mention: AGENT }] })} names={{}} />);

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("@quackbot")).toBeTruthy();
  });

  it("labels an agent mention from the roster — never `runs/quackbot`", () => {
    withStore(
      <RichText blocks={para({ text: "@quackbot", marks: [{ mention: AGENT }] })} names={{}} />,
      { agents: [ROSTERED] },
    );

    expect(screen.getByRole("button", { name: /Open agent quackbot/ }).textContent).toBe("@Quackbot");
  });

  it("falls back to the agent_id for an agent the roster no longer holds", () => {
    withStore(
      <RichText blocks={para({ text: "@ghost", marks: [{ mention: GHOST }] })} names={{}} />,
      { agents: [ROSTERED] },
    );

    const button = screen.getByRole("button", { name: /Open agent ghost/ });
    expect(button.textContent).toBe("@ghost");
    expect(button.textContent).not.toContain("runs/");
  });

  // The authored TEXT is the handle typed at the time; the MARK is the durable
  // ref. Renaming must not break the link, so neither label nor target may read
  // span.text.
  it("labels and navigates off the MARK, not the span's stale text", () => {
    const actions = withStore(
      <RichText
        blocks={para({ text: "@old-handle", marks: [{ mention: AGENT }] })}
        names={{}}
      />,
      { agents: [ROSTERED] },
    );

    const button = screen.getByRole("button", { name: /Open agent quackbot/ });
    expect(button.textContent).toBe("@Quackbot");
    fireEvent.click(button);

    expect(actions.openAgent).toHaveBeenCalledWith("quackbot");
  });
});

describe("duck://page chips", () => {
  it("shows the page's title and opens it (screen switch included)", () => {
    const actions = withStore(
      <RichText blocks={para({ text: "see [Launch plan](duck://page/p1) first" })} names={{}} />,
      { pages: [page("p1", "Launch plan")] },
    );

    const chip = screen.getByRole("button", { name: "Open page Launch plan" });
    expect(chip.textContent).toContain("Launch plan");
    fireEvent.click(chip);

    // openPage loads the tree but does not navigate — both calls are required.
    expect(actions.openPage).toHaveBeenCalledWith("p1");
    expect(actions.setScreen).toHaveBeenCalledWith("pages");
  });

  it("degrades to the raw id when the page is unknown (deleted, or not hydrated yet)", () => {
    withStore(<RichText blocks={para({ text: "[Ghost](duck://page/ghost)" })} names={{}} />, {
      pages: [],
    });

    const chip = screen.getByRole("button", { name: "Open page ghost" });
    expect(chip.textContent).toContain("ghost");
    expect(chip.textContent).not.toContain("Untitled");
  });

  it("leaves a ref inside a code block literal", () => {
    withStore(
      <RichText
        blocks={[{ code: { text: "[p](duck://page/p1)", lang: null } }]}
        names={{}}
      />,
      { pages: [page("p1", "Launch plan")] },
    );

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("[p](duck://page/p1)")).toBeTruthy();
  });

  it("leaves a non-ref markdown link literal", () => {
    withStore(<RichText blocks={para({ text: "[not a page](https://x.example)" })} names={{}} />, {
      pages: [page("p1", "Launch plan")],
    });

    // an https link is an external <a>, never a page chip.
    expect(screen.queryByRole("button", { name: /Open page/ })).toBeNull();
  });
});

describe("CommentText (plain-text comment bodies)", () => {
  it("chips a ref inside otherwise plain text", () => {
    const actions = withStore(
      <CommentText text="ties into [Launch plan](duck://page/p1) here" names={{}} />,
      { pages: [page("p1", "Launch plan")] },
    );

    fireEvent.click(screen.getByRole("button", { name: "Open page Launch plan" }));

    expect(actions.openPage).toHaveBeenCalledWith("p1");
  });

  it("passes text with no refs through untouched", () => {
    const { container } = render(
      <CommentText text="no refs here" names={{}} />,
    );

    expect(container.textContent).toBe("no refs here");
    expect(screen.queryByRole("button")).toBeNull();
  });

  // The wire stores plain text: @tokens are re-resolved at render through the
  // SAME grammar the submit path used, so highlight == what the module saw.
  it("resolves a rostered agent's @token into a live mention", () => {
    const actions = withStore(<CommentText text="ping @quackbot please" names={{}} />, {
      agents: [ROSTERED],
    });

    const button = screen.getByRole("button", { name: /Open agent quackbot/ });
    expect(button.textContent).toBe("@Quackbot");
    fireEvent.click(button);

    expect(actions.openAgent).toHaveBeenCalledWith("quackbot");
  });

  it("resolves a member's handle into a Members-opening mention", () => {
    const actions = withStore(<CommentText text="cc @jess-example" names={{ [ACCOUNT_HEX]: "Jess Example" }} />, {
      nodeUsers: { node1: { accountId: ACCOUNT_HEX, name: "Jess Example" } },
    });

    fireEvent.click(screen.getByRole("button", { name: /Open Jess Example in Members/ }));

    expect(actions.openMember).toHaveBeenCalledWith(ACCOUNT_HEX);
  });

  it("leaves an @word nobody claims tinted but inert", () => {
    withStore(<CommentText text="hi @nobody" names={{}} />, {
      agents: [ROSTERED],
    });

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("@nobody")).toBeTruthy();
  });

  it("renders a page ref and a mention together in a comment", () => {
    withStore(<CommentText text="[Launch plan](duck://page/p1) @quackbot ping" names={{}} />, {
      agents: [ROSTERED],
      pages: [page("p1", "Launch plan")],
    });

    expect(screen.getByRole("button", { name: "Open page Launch plan" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Open agent quackbot/ })).toBeTruthy();
  });

  it("ignores a mid-word @ (emails are not mentions)", () => {
    withStore(<CommentText text="mail quackbot@example.com" names={{}} />, {
      agents: [ROSTERED],
    });

    expect(screen.queryByRole("button")).toBeNull();
  });
});

describe("inline code", () => {
  it("renders a backtick run literal — refs inside are source, not chips", () => {
    withStore(
      <RichText blocks={para({ text: "see `[x](duck://page/p1)` now" })} names={{}} />,
      { pages: [page("p1", "Launch plan")] },
    );

    expect(screen.getByText("[x](duck://page/p1)")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Open page/ })).toBeNull();
  });

  it("leaves an unclosed backtick as plain text", () => {
    const { container } = render(<RichText blocks={para({ text: "a ` b" })} names={{}} />);

    expect(container.textContent).toBe("a ` b");
  });

  it("leaves a bare ``` run as plain text — not a one-backtick chip", () => {
    const { container } = render(<RichText blocks={para({ text: "a ``` b" })} names={{}} />);

    expect(container.textContent).toBe("a ``` b");
    expect(container.querySelector("code")).toBeNull();
  });
});

describe("fenced code highlighting", () => {
  it("a known fence tag lazily gains shiki tokens; the text stays intact", async () => {
    const { container } = render(
      <RichText blocks={[{ code: { lang: "rust", text: 'fn main() { println!("hi"); }' } }]} names={{}} />,
    );

    // plain first paint, tokens after the lazy highlighter resolves
    expect(container.textContent).toContain("fn main()");
    await waitFor(() => expect(container.querySelector(".code-tok")).toBeTruthy(), {
      timeout: 5000,
    });
    expect(container.textContent).toContain('fn main() { println!("hi"); }');
  });
});

describe("duck://files attachment chips (markdown refs)", () => {
  const ATTACH = "[report.pdf](duck://files/shared/attachments/uuid-1/report.pdf)";
  const IMG = "![photo.png](duck://files/shared/attachments/uuid-2/photo.png)";

  // A store WITH a transport whose files reads are stubbed — enough to prove
  // the chip drives a read against the CONFINED path and nothing else.
  const withTransport = (node: ReactNode) => {
    const filesStat = vi.fn().mockResolvedValue(null);
    const filesRead = vi.fn().mockResolvedValue({ b64: "", eof: true });
    const transport = { filesStat, filesRead } as unknown as NodeTransport;
    const actions = { openAgent: vi.fn(), openMember: vi.fn(), openPage: vi.fn(), setScreen: vi.fn() } as unknown as ConsoleActions;
    render(
      <ConsoleContext.Provider value={{ state: createInitialState(), actions, transport }}>
        {node}
      </ConsoleContext.Provider>,
    );
    return { filesStat, filesRead };
  };

  it("chips a file ref (shows the name, never the raw markdown/URI text)", () => {
    withStore(<RichText blocks={para({ text: `see ${ATTACH}` })} names={{}} />);
    expect(screen.getByText("report.pdf")).toBeTruthy();
    expect(screen.queryByText(ATTACH)).toBeNull();
  });

  it("renders a file ref AND a page ref in the same message", () => {
    withStore(
      <RichText blocks={para({ text: `${ATTACH} for [Launch plan](duck://page/p1)` })} names={{}} />,
      { pages: [page("p1", "Launch plan")] },
    );
    expect(screen.getByText("report.pdf")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open page Launch plan" })).toBeTruthy();
  });

  it("leaves a duck://files ref OUTSIDE the attachments root as literal text", () => {
    const home = "[k](duck://files/home/ext:aa/secret.txt)";
    withStore(<RichText blocks={para({ text: home })} names={{}} />);
    expect(screen.getByText(home)).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("a non-image download reads the CONFINED path, nothing else", () => {
    const { filesRead } = withTransport(<RichText blocks={para({ text: ATTACH })} names={{}} />);
    fireEvent.click(screen.getByRole("button", { name: /Download attachment report.pdf/ }));
    expect(filesRead).toHaveBeenCalledWith(
      expect.objectContaining({ path: "/shared/attachments/uuid-1/report.pdf" }),
    );
  });

  it("an image embed stats the CONFINED path before fetching bytes", async () => {
    const { filesStat } = withTransport(<RichText blocks={para({ text: IMG })} names={{}} />);
    await waitFor(() =>
      expect(filesStat).toHaveBeenCalledWith(
        expect.objectContaining({ path: "/shared/attachments/uuid-2/photo.png" }),
      ),
    );
  });
});
