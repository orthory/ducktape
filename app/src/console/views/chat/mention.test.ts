import { describe, expect, it } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import type { ChatBlock } from "../../../domain/chat-client";
import {
  hasAgentMention,
  insertMention,
  mentionCandidates,
  mentionResolverOf,
  mentionTokenAt,
} from "./mention";

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

describe("mentionTokenAt", () => {
  it("opens on @ at the start of the text", () => {
    expect(mentionTokenAt("@qu", 3)).toEqual({ start: 0, query: "qu" });
  });

  it("opens on @ after whitespace, with an empty query right after @", () => {
    expect(mentionTokenAt("hey @", 5)).toEqual({ start: 4, query: "" });
    expect(mentionTokenAt("line one\n@bot", 13)).toEqual({ start: 9, query: "bot" });
  });

  it("does NOT open mid-word (emails stay emails)", () => {
    expect(mentionTokenAt("mail me a@b", 11)).toBeNull();
  });

  it("closes once whitespace ends the token", () => {
    expect(mentionTokenAt("@quackbot done", 14)).toBeNull();
  });

  it("only sees the token the caret is inside", () => {
    // caret in the middle of "@qua|ckbot" — query is what's typed so far
    expect(mentionTokenAt("hey @quackbot", 8)).toEqual({ start: 4, query: "qua" });
  });

  it("a second @ kills the token", () => {
    expect(mentionTokenAt("@a@b", 4)).toBeNull();
  });
});

describe("mentionCandidates", () => {
  const roster = [
    agent("quackbot", "Quackbot"),
    agent("scribe", "Scribe the Writer"),
    agent("paused-bot", "Paused Bot", "paused"),
  ];

  it("lists only Active agents, matching agent_id case-insensitively", () => {
    const hits = mentionCandidates(roster, "QUACK");
    expect(hits.map((a) => a.agent_id)).toEqual(["quackbot"]);
  });

  it("matches display_name too", () => {
    const hits = mentionCandidates(roster, "writer");
    expect(hits.map((a) => a.agent_id)).toEqual(["scribe"]);
  });

  it("never surfaces a paused agent, even on exact id", () => {
    expect(mentionCandidates(roster, "paused-bot")).toEqual([]);
  });

  it("empty query lists every Active agent, prefix matches first on a typed one", () => {
    expect(mentionCandidates(roster, "").map((a) => a.agent_id)).toEqual([
      "quackbot",
      "scribe",
    ]);
    const ranked = mentionCandidates([agent("abc-s", "abc s"), agent("s-bot", "S Bot")], "s");
    expect(ranked.map((a) => a.agent_id)).toEqual(["s-bot", "abc-s"]);
  });
});

describe("insertMention", () => {
  it("replaces the typed fragment with @agent_id plus a trailing space", () => {
    const next = insertMention("hey @qu can you", { start: 4, query: "qu" }, 7, "quackbot");
    expect(next.text).toBe("hey @quackbot  can you");
    expect(next.caret).toBe("hey @quackbot ".length);
  });

  it("works on a bare @ at the end of the draft", () => {
    const next = insertMention("hey @", { start: 4, query: "" }, 5, "scribe");
    expect(next.text).toBe("hey @scribe ");
    expect(next.caret).toBe(12);
  });
});

describe("mentionResolverOf", () => {
  it("maps Active agents to runs-module AuthorRefs and skips paused ones", () => {
    const resolver = mentionResolverOf([
      agent("quackbot", "Quackbot"),
      agent("paused-bot", "Paused Bot", "paused"),
    ]);
    // module MUST be "runs": the runs module rejects tags whose module isn't itself.
    expect(resolver.get("quackbot")).toEqual({
      agent: { module: "runs", agent_id: "quackbot" },
    });
    expect(resolver.has("paused-bot")).toBe(false);
  });
});

describe("hasAgentMention", () => {
  const mentionSpan = {
    text: "@quackbot",
    marks: [{ mention: { agent: { module: "runs", agent_id: "quackbot" } } }],
  };

  it("finds an agent mention in paragraphs and quotes", () => {
    expect(hasAgentMention([{ paragraph: [mentionSpan] }] as ChatBlock[])).toBe(true);
    expect(hasAgentMention([{ quote: [mentionSpan] }] as ChatBlock[])).toBe(true);
  });

  it("ignores plain text, user mentions, code, and dividers", () => {
    const blocks: ChatBlock[] = [
      { paragraph: [{ text: "@quackbot as plain text", marks: [] }] },
      { paragraph: [{ text: "@eddy", marks: [{ mention: { user: [1, 2] } }] }] },
      { code: { lang: null, text: "@quackbot" } },
      "divider",
    ];
    expect(hasAgentMention(blocks)).toBe(false);
  });
});
