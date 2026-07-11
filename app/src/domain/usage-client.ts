// Typed client for the saga module's USAGE LEDGER view — the node-local
// derived index over the saga op stream (crates/system/saga/src/index.rs).
// "Whose subscription carried how much": each row aggregates finalized
// attempts per (executor node key, capability tag, outcome). Executor→account
// resolution happens HERE (identity's OfNode), because the fold can't do a
// cross-module join. Durations are BLOCK deltas, never seconds — label them
// as blocks/ticks.

import { accountOfNode, type AccountView } from "./identity-client";
import { bytesToHex } from "./gateway-client";
import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

export const TARGET = "saga";

/** One ledger line, camelCase verbatim from the index wire (`UsageRow`). */
export interface TokenUsage {
  inputTokens: number;
  cachedInputTokens: number;
  cacheWriteInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
}

export interface UsageRow extends TokenUsage {
  executorHex: string;
  capability: string;
  outcomeOk: boolean;
  runs: number;
  totalDurationBlocks: number;
}

/** The raw aggregated rows: `{"usage": {"sinceHeight": N}}` → `{"usage": [...]}`.
 *  Omit `sinceHeight` for the all-time ledger. */
export const usageRows = (
  transport: NodeTransport,
  params: { sinceHeight?: number } = {},
): Promise<UsageRow[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, { usage: { sinceHeight: params.sinceHeight } }),
    )
    .then((reply) => replyVariant<UsageRow[]>(reply, "usage"));

export interface CapabilityUsage extends TokenUsage {
  capability: string;
  runs: number;
  failed: number;
  totalDurationBlocks: number;
}

/** One account's ledger: totals plus the per-capability-tag breakdown. An
 *  executor node bound to no account groups under its own key hex. */
export interface AccountUsage extends TokenUsage {
  /** Display name when the executor resolved to a named account, else the
   *  grouping key hex (account id, or the bare executor key when unbound). */
  label: string;
  accountIdHex: string | null;
  runs: number;
  failed: number;
  totalDurationBlocks: number;
  byCapability: CapabilityUsage[];
}

/** Fetch the ledger and group it per account: resolve each distinct executor
 *  key via identity's OfNode, then fold rows into account totals with a
 *  per-capability breakdown. Sorted by runs desc. */
export const accountUsage = async (
  transport: NodeTransport,
  params: { sinceHeight?: number } = {},
): Promise<AccountUsage[]> => {
  const rows = await usageRows(transport, params);
  const executors = [...new Set(rows.map((row) => row.executorHex))];
  const resolved = new Map<string, AccountView | null>();
  await Promise.all(
    executors.map(async (hex) => {
      resolved.set(hex, await accountOfNode(transport, hex).catch(() => null));
    }),
  );

  const groups = new Map<string, AccountUsage & { caps: Map<string, CapabilityUsage> }>();
  for (const row of rows) {
    const account = resolved.get(row.executorHex) ?? null;
    const key = account ? bytesToHex(account.account_id) : row.executorHex;
    let group = groups.get(key);
    if (!group) {
      group = {
        label: account?.display_name ?? key,
        accountIdHex: account ? key : null,
        runs: 0,
        failed: 0,
        totalDurationBlocks: 0,
        inputTokens: 0,
        cachedInputTokens: 0,
        cacheWriteInputTokens: 0,
        outputTokens: 0,
        reasoningOutputTokens: 0,
        byCapability: [],
        caps: new Map(),
      };
      groups.set(key, group);
    }
    group.runs += row.runs;
    if (!row.outcomeOk) group.failed += row.runs;
    group.totalDurationBlocks += row.totalDurationBlocks;
    group.inputTokens += row.inputTokens;
    group.cachedInputTokens += row.cachedInputTokens;
    group.cacheWriteInputTokens += row.cacheWriteInputTokens;
    group.outputTokens += row.outputTokens;
    group.reasoningOutputTokens += row.reasoningOutputTokens;
    let cap = group.caps.get(row.capability);
    if (!cap) {
      cap = {
        capability: row.capability,
        runs: 0,
        failed: 0,
        totalDurationBlocks: 0,
        inputTokens: 0,
        cachedInputTokens: 0,
        cacheWriteInputTokens: 0,
        outputTokens: 0,
        reasoningOutputTokens: 0,
      };
      group.caps.set(row.capability, cap);
    }
    cap.runs += row.runs;
    if (!row.outcomeOk) cap.failed += row.runs;
    cap.totalDurationBlocks += row.totalDurationBlocks;
    cap.inputTokens += row.inputTokens;
    cap.cachedInputTokens += row.cachedInputTokens;
    cap.cacheWriteInputTokens += row.cacheWriteInputTokens;
    cap.outputTokens += row.outputTokens;
    cap.reasoningOutputTokens += row.reasoningOutputTokens;
  }

  return [...groups.values()]
    .map(({ caps, ...group }) => ({
      ...group,
      byCapability: [...caps.values()].sort((a, b) => b.runs - a.runs),
    }))
    .sort((a, b) => b.runs - a.runs);
};
