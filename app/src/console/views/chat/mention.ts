// Pure helpers behind the composer's @mention typeahead and the structured
// mention marks it emits on send. No React, no store — everything here is a
// plain function over text/blocks so it unit-tests without a DOM.

import type { AgentRecord } from "../../../domain/agent-client";
import type { AuthorRef, ChatBlock, Span } from "../../../domain/chat-client";

// ── Caret token detection ───────────────────────────────

export interface MentionToken {
  /** Index of the `@` in the composer text. */
  start: number;
  /** What was typed after the `@` so far (may be empty). */
  query: string;
}

/** The in-progress @token ending at `caret`, or null. An `@` opens a token
 *  only at the start of the text or after whitespace (so emails/handles mid-
 *  word don't trigger the menu), and the fragment must be a single unbroken
 *  word — any whitespace or second `@` closes it. */
export const mentionTokenAt = (text: string, caret: number): MentionToken | null => {
  const upto = text.slice(0, caret);
  const start = upto.lastIndexOf("@");
  if (start === -1) return null;
  if (start > 0 && !/\s/.test(upto[start - 1]!)) return null;
  const query = upto.slice(start + 1);
  if (/[\s@]/.test(query)) return null;
  return { start, query };
};

// ── Candidate filtering ─────────────────────────────────

/** Active agents matching `query` against agent_id and display_name, case-
 *  insensitive. Prefix matches rank before mid-string ones; ties break on
 *  agent_id so the list is stable. */
export const mentionCandidates = (agents: AgentRecord[], query: string): AgentRecord[] => {
  const q = query.toLowerCase();
  const matches = (value: string) => value.toLowerCase().includes(q);
  const prefixed = (agent: AgentRecord) =>
    agent.agent_id.toLowerCase().startsWith(q) ||
    agent.display_name.toLowerCase().startsWith(q);
  return agents
    .filter((agent) => agent.status === "active")
    .filter((agent) => matches(agent.agent_id) || matches(agent.display_name))
    .sort((a, b) => {
      const rank = Number(prefixed(b)) - Number(prefixed(a));
      return rank !== 0 ? rank : a.agent_id.localeCompare(b.agent_id);
    });
};

// ── Insertion ───────────────────────────────────────────

/** Replace the typed fragment (`token.start` .. `caret`) with `@<agent_id> `
 *  and report where the caret lands afterwards. */
export const insertMention = (
  text: string,
  token: MentionToken,
  caret: number,
  agentId: string,
): { text: string; caret: number } => {
  const inserted = `@${agentId} `;
  const nextCaret = token.start + inserted.length;
  return {
    text: text.slice(0, token.start) + inserted + text.slice(caret),
    caret: nextCaret,
  };
};

// ── Resolver + parsed-block inspection (the send path) ──

/** agent_id → the AuthorRef its mention mark carries. Module is the LITERAL
 *  "runs": engagement tags route through the runs module, and it rejects any
 *  tag whose module isn't itself — "agent" (the registry) would be dropped. */
export const mentionResolverOf = (agents: AgentRecord[]): Map<string, AuthorRef> => {
  const resolver = new Map<string, AuthorRef>();
  for (const agent of agents) {
    if (agent.status === "active") {
      resolver.set(agent.agent_id, { agent: { module: "runs", agent_id: agent.agent_id } });
    }
  }
  return resolver;
};

const spanMentionsAgent = (span: Span): boolean =>
  span.marks.some(
    (mark) =>
      typeof mark === "object" &&
      "mention" in mark &&
      typeof mark.mention === "object" &&
      "agent" in mark.mention,
  );

/** Whether any span in `blocks` carries an agent mention mark — the trigger
 *  for the first-mention auto-watch. */
export const hasAgentMention = (blocks: ChatBlock[]): boolean =>
  blocks.some((block) => {
    if (block === "divider" || "code" in block) return false;
    const spans = "paragraph" in block ? block.paragraph : block.quote;
    return spans.some(spanMentionsAgent);
  });
