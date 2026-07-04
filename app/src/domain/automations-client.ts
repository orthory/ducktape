// Typed client for the node's `automations` module — the TS mirror of
// `crates/apps/automations-interface`. Rules pair a Trigger (a chat post or a
// memory publish that matches) with an Action (post a message, create a task, or
// deliver an inbox note). The module fires rules from chat/memory hook events;
// the app only defines, toggles, deletes, and inspects them.
//
// Trigger/Action/*Msg/*Query are externally-tagged serde enums, so each crosses
// the wire as a single-key object (`{ MessagePosted: {...} }`) — the discriminated
// unions below mirror that shape verbatim. camelCase params in, verbatim wire out,
// pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (verbatim serde shapes) ──────────────────

/** What makes a rule fire. Every `null` field is a wildcard. */
export type Trigger =
  | {
      MessagePosted: {
        /** exact channel id, or null for any channel */
        channel_id: string | null;
        /** substring tested against each mention's display form, or null */
        mention: string | null;
        /** case-sensitive substring over the post's concatenated text, or null */
        text_contains: string | null;
      };
    }
  | {
      MemoryPublished: {
        /** a memory subtree prefix (segment-aware), or null for any path */
        prefix: string | null;
        /** matches `event.meta["kind"]`, or null */
        meta_kind: string | null;
        /** case-sensitive substring over the memory event author, or null */
        author_contains: string | null;
      };
    };

/** What a firing rule does. */
export type Action =
  | { PostMessage: { channel_id: string; template: string } }
  | { CreateTask: { task_id_prefix: string; title_template: string } }
  | {
      DeliverInbox: {
        member_template: string;
        kind: string;
        body_template: string;
      };
    };

export interface Rule {
  rule_id: string;
  enabled: boolean;
  trigger: Trigger;
  action: Action;
  created_at: number;
  /** successful fires only */
  fire_count: number;
}

/** One entry in the module's bounded global run-history ring. */
export interface RunRecord {
  rule_id: string;
  /** triggering channel id (chat) or triggering memory path (memory) */
  channel_id: string;
  seq: number;
  height: number;
  /** true when an action was emitted; false for a skipped/over-budget fire */
  action_ok: boolean;
  detail: string;
}

const TARGET = "automations";

/** The kinds a Trigger/Action can take — small helpers so the view builder does
 *  not hand-assemble wire objects. */
export type TriggerKind = "MessagePosted" | "MemoryPublished";
export type ActionKind = "PostMessage" | "CreateTask" | "DeliverInbox";

// ── Msgs (writes) ───────────────────────────────────────

export const createRule = (
  transport: NodeTransport,
  params: { ruleId: string; trigger: Trigger; action: Action },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    CreateRule: {
      rule_id: params.ruleId,
      trigger: params.trigger,
      action: params.action,
    },
  });

export const setEnabled = (
  transport: NodeTransport,
  params: { ruleId: string; enabled: boolean },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    SetEnabled: { rule_id: params.ruleId, enabled: params.enabled },
  });

export const deleteRule = (
  transport: NodeTransport,
  ruleId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { DeleteRule: { rule_id: ruleId } });

// ── Queries (reads over committed state) ────────────────

export const listRules = (transport: NodeTransport): Promise<Rule[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "ListRules"))
    .then((reply) => replyVariant<Rule[]>(reply, "Rules"));

export const getRule = (
  transport: NodeTransport,
  ruleId: string,
): Promise<Rule | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { GetRule: { rule_id: ruleId } }))
    .then((reply) => replyVariant<Rule | null>(reply, "Rule"));

/** The most recent `limit` run-history records for `ruleId`, oldest-first. */
export const runHistory = (
  transport: NodeTransport,
  params: { ruleId: string; limit: number },
): Promise<RunRecord[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        RunHistory: { rule_id: params.ruleId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<RunRecord[]>(reply, "History"));
