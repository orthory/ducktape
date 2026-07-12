import { describe, expect, it } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import { keyBytes, type ChatBlock } from "../../../domain/chat-client";
import { parseMessageInput } from "./chat-input";
import {
  agentMentions,
  hasAgentMention,
  insertMention,
  mentionCandidates,
  mentionCandidatesAll,
  mentionableUsers,
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

describe("mentionableUsers", () => {
  const jessKey = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  const fallbackKey = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

  it("dedupes node users, slugifies names, falls back to short hex, and avoids agent handle collisions", () => {
    const users = mentionableUsers(
      {
        node1: { accountId: jessKey, name: "Jess K" },
        node2: { accountId: jessKey, name: "Jess K" },
        node3: { accountId: fallbackKey, name: null },
      },
      [],
    );

    expect(users).toEqual([
      { kind: "user", userKeyHex: jessKey, handle: "jess-k", label: "Jess K" },
      { kind: "user", userKeyHex: fallbackKey, handle: fallbackKey.slice(0, 8), label: fallbackKey.slice(0, 8) },
    ]);

    expect(
      mentionableUsers({ node1: { accountId: jessKey, name: "Jess K" } }, [agent("jess-k", "Jess Agent")]),
    ).toEqual([{ kind: "user", userKeyHex: jessKey, handle: "jess-k-2", label: "Jess K" }]);
  });

  it("suffixes handles when distinct users' display names slugify identically", () => {
    const users = mentionableUsers(
      {
        node1: { accountId: jessKey, name: "Jess K" },
        node2: { accountId: fallbackKey, name: "Jess K" },
      },
      [],
    );

    expect(users.map((user) => user.handle)).toEqual(["jess-k", "jess-k-2"]);
  });

  it("falls back to short hex when a symbols-only name has an empty slug", () => {
    const [user] = mentionableUsers(
      { node1: { accountId: fallbackKey, name: "!!!" } },
      [],
    );

    expect(user?.handle).toBe(fallbackKey.slice(0, 8));
  });
});

describe("mentionCandidatesAll", () => {
  const users = [
    { kind: "user" as const, userKeyHex: "11", handle: "jess-k", label: "Jess K" },
    { kind: "user" as const, userKeyHex: "22", handle: "casey", label: "Casey Jensen" },
  ];

  it("ranks prefix matches first, with agents before users for rank ties", () => {
    const hits = mentionCandidatesAll(
      [agent("writer", "Pocket Jensen"), agent("je-bot", "Agent")],
      users,
      "je",
    );

    expect(
      hits.map((hit) => (hit.kind === "agent" ? `agent:${hit.agent.agent_id}` : `user:${hit.handle}`)),
    ).toEqual(["agent:je-bot", "user:jess-k", "agent:writer", "user:casey"]);
  });

  it("returns agents then users on an empty query", () => {
    const hits = mentionCandidatesAll([agent("alpha", "Alpha"), agent("omega", "Omega")], users, "");

    expect(
      hits.map((hit) => (hit.kind === "agent" ? `agent:${hit.agent.agent_id}` : `user:${hit.handle}`)),
    ).toEqual(["agent:alpha", "agent:omega", "user:jess-k", "user:casey"]);
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

  it("maps user handles to user AuthorRefs while keeping active agent resolution", () => {
    const jessKey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const resolver = mentionResolverOf(
      [agent("quackbot", "Quackbot"), agent("paused-bot", "Paused Bot", "paused")],
      [{ kind: "user", userKeyHex: jessKey, handle: "jess-k", label: "Jess K" }],
    );

    expect(resolver.get("jess-k")).toEqual({ user: keyBytes(jessKey) });
    expect(resolver.get("quackbot")).toEqual({
      agent: { module: "runs", agent_id: "quackbot" },
    });
    expect(resolver.has("paused-bot")).toBe(false);
  });

  it("gives an active agent precedence when a user handle collides with its agent_id", () => {
    const jessKey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const resolver = mentionResolverOf(
      [agent("jess-k", "Jess Agent")],
      [{ kind: "user", userKeyHex: jessKey, handle: "jess-k", label: "Jess K" }],
    );

    expect(resolver.get("jess-k")).toEqual({
      agent: { module: "runs", agent_id: "jess-k" },
    });
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

  it("returns false for blocks containing only a user mention", () => {
    const blocks: ChatBlock[] = [
      {
        paragraph: [
          {
            text: "@jess-k",
            marks: [{ mention: { user: keyBytes("0123456789abcdef") } }],
          },
        ],
      },
    ];

    expect(hasAgentMention(blocks)).toBe(false);
  });
});

describe("agentMentions", () => {
  it("returns distinct structured agent refs and ignores plain/user mentions", () => {
    const ref = { agent: { module: "runs", agent_id: "quackbot" } };
    const blocks: ChatBlock[] = [
      {
        paragraph: [
          { text: "@quackbot", marks: [{ mention: ref }] },
          { text: " again", marks: [{ mention: ref }] },
          { text: " @eddy", marks: [{ mention: { user: [1, 2] } }] },
        ],
      },
      { code: { lang: null, text: "@quackbot" } },
    ];
    expect(agentMentions(blocks)).toEqual([ref]);
  });
});

describe("user mention parsing", () => {
  it("round-trips a user handle through parseMessageInput with the mention resolver", () => {
    const jessKey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const users = [{ kind: "user" as const, userKeyHex: jessKey, handle: "jess-k", label: "Jess K" }];

    expect(parseMessageInput("hi @jess-k", mentionResolverOf([], users))).toEqual([
      {
        paragraph: [
          { text: "hi ", marks: [] },
          { text: "@jess-k", marks: [{ mention: { user: keyBytes(jessKey) } }] },
        ],
      },
    ]);
  });
});
