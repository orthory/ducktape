// Pure helpers behind the composer's @mention typeahead and the structured
// mention marks it emits on send. No React, no store — everything here is a
// plain function over text/blocks so it unit-tests without a DOM.

import type { AgentRecord } from "../../../domain/agent-client";
import { keyBytes, type AuthorRef, type ChatBlock, type Span } from "../../../domain/chat-client";

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

/** One mentionable workspace user, derived from identity's node->user map. */
export interface UserMentionCandidate {
  kind: "user";
  /** lowercase hex of the durable user key — the mention mark's bytes. */
  userKeyHex: string;
  /** the @token inserted into the composer; matches chat-input's charset [a-z0-9._-]. */
  handle: string;
  /** display label (chosen name or short hex). */
  label: string;
}

export interface AgentMentionCandidate {
  kind: "agent";
  agent: AgentRecord;
}

export type MentionCandidate = UserMentionCandidate | AgentMentionCandidate;

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

const slugifyUserHandle = (name: string): string =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");

const uniqueHandle = (base: string, used: Set<string>): string => {
  let handle = base;
  let suffix = 2;
  while (used.has(handle)) {
    handle = `${base}-${suffix}`;
    suffix += 1;
  }
  return handle;
};

/** Distinct users from state.nodeUsers (dedupe by userKey). handle = display
 *  name slugified (lowercase, [^a-z0-9._-]+ -> "-", trimmed of leading/
 *  trailing "-"), falling back to userKeyHex.slice(0, 8) when empty; a handle
 *  colliding with an earlier user's or any agent_id gets "-2", "-3", ... */
export const mentionableUsers = (
  nodeUsers: Record<string, { userKey: string; name: string | null }>,
  agents: AgentRecord[],
): UserMentionCandidate[] => {
  const seenUserKeys = new Set<string>();
  const usedHandles = new Set(agents.map((agent) => agent.agent_id.toLowerCase()));
  const users: UserMentionCandidate[] = [];

  for (const nodeUser of Object.values(nodeUsers)) {
    const userKeyHex = nodeUser.userKey.toLowerCase();
    if (seenUserKeys.has(userKeyHex)) continue;
    seenUserKeys.add(userKeyHex);

    const shortKey = userKeyHex.slice(0, 8);
    const name = nodeUser.name?.trim() ?? "";
    const baseHandle = slugifyUserHandle(name) || shortKey;
    const handle = uniqueHandle(baseHandle, usedHandles);
    usedHandles.add(handle);
    users.push({
      kind: "user",
      userKeyHex,
      handle,
      label: name || shortKey,
    });
  }

  return users;
};

const agentIsPrefixed = (agent: AgentRecord, query: string): boolean =>
  agent.agent_id.toLowerCase().startsWith(query) ||
  agent.display_name.toLowerCase().startsWith(query);

const userMatches = (user: UserMentionCandidate, query: string): boolean =>
  user.handle.toLowerCase().includes(query) || user.label.toLowerCase().includes(query);

const userIsPrefixed = (user: UserMentionCandidate, query: string): boolean =>
  user.handle.toLowerCase().startsWith(query) || user.label.toLowerCase().startsWith(query);

/** Agents-and-users matching `query` (case-insensitive, prefix-ranked like the
 *  existing agent-only filter). Agents keep their existing relative order and
 *  rank rules; users interleave by the same prefix-first rule, ties agents-first. */
export const mentionCandidatesAll = (
  agents: AgentRecord[],
  users: UserMentionCandidate[],
  query: string,
): MentionCandidate[] => {
  const q = query.toLowerCase();
  const agentHits = mentionCandidates(agents, query);
  const userHits = users
    .map((user, index) => ({ user, index, prefixed: userIsPrefixed(user, q) }))
    .filter(({ user }) => userMatches(user, q))
    .sort((a, b) => {
      const rank = Number(b.prefixed) - Number(a.prefixed);
      return rank !== 0 ? rank : a.index - b.index;
    });

  const prefixedAgents = agentHits.filter((agent) => agentIsPrefixed(agent, q));
  const otherAgents = agentHits.filter((agent) => !agentIsPrefixed(agent, q));
  const prefixedUsers = userHits.filter((hit) => hit.prefixed).map((hit) => hit.user);
  const otherUsers = userHits.filter((hit) => !hit.prefixed).map((hit) => hit.user);

  return [
    ...prefixedAgents.map((agent) => ({ kind: "agent" as const, agent })),
    ...prefixedUsers,
    ...otherAgents.map((agent) => ({ kind: "agent" as const, agent })),
    ...otherUsers,
  ];
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
export const mentionResolverOf = (
  agents: AgentRecord[],
  users?: UserMentionCandidate[],
): Map<string, AuthorRef> => {
  const resolver = new Map<string, AuthorRef>();
  for (const agent of agents) {
    if (agent.status === "active") {
      resolver.set(agent.agent_id, { agent: { module: "runs", agent_id: agent.agent_id } });
    }
  }
  for (const user of users ?? []) {
    if (!resolver.has(user.handle)) {
      resolver.set(user.handle, { user: keyBytes(user.userKeyHex) });
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
