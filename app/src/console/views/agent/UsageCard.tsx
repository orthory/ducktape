// The usage ledger card — "whose subscription carried how much". Reads the
// saga module's derived usage view (node-local index, no consensus) and
// groups finalized attempts per ACCOUNT (executor node key → account via
// identity's OfNode, resolved in usage-client), with a per-capability-tag
// breakdown. All-time window: block heights don't map cleanly to a wall-clock
// week, so the card says so instead of faking one. Durations are BLOCKS,
// never seconds. Read-only; quotas/enforcement are out of scope (M3).

import { useEffect, useState } from "react";

import {
  accountUsage,
  type AccountUsage,
  type TokenUsage,
} from "../../../domain/usage-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import { GroupCard, SectionLabel } from "./parts";

const shortKey = (hex: string): string =>
  hex.length > 18 ? `${hex.slice(0, 10)}…${hex.slice(-6)}` : hex;

const rowStyle = {
  display: "flex",
  alignItems: "baseline",
  gap: 10,
} as const;

const tokenCount = new Intl.NumberFormat(undefined, {
  notation: "compact",
  maximumFractionDigits: 1,
});

const tokenTitle = (usage: TokenUsage): string =>
  `reported tokens · input ${usage.inputTokens.toLocaleString()} · output ${usage.outputTokens.toLocaleString()} · cached ${usage.cachedInputTokens.toLocaleString()} · cache write ${usage.cacheWriteInputTokens.toLocaleString()} · reasoning ${usage.reasoningOutputTokens.toLocaleString()}`;

function Metric({ label, value, title }: { label: string; value: string; title?: string }) {
  return (
    <span
      title={title}
      style={{ font: `400 11px ${font.sans}`, color: color.muted2, whiteSpace: "nowrap" }}
    >
      <span style={{ font: `600 12px ${font.mono}`, color: color.ink }}>{value}</span>{" "}
      {label}
    </span>
  );
}

export function UsageCard({ refreshKey }: { refreshKey: string }) {
  const { transport } = useDucktape();
  const [rows, setRows] = useState<AccountUsage[] | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!transport) return;
    let live = true;
    accountUsage(transport)
      .then((usage) => {
        if (live) setRows(usage);
      })
      .catch(() => {
        if (live) setError(true);
      });
    return () => {
      live = false;
    };
  }, [transport, refreshKey]);

  // no transport (web preview) or a node without the view: stay quiet rather
  // than render a dead card.
  if (!transport || error) return null;

  return (
    <GroupCard style={{ marginBottom: 16, padding: 16 }}>
      <div style={{ ...rowStyle, marginBottom: 4 }}>
        <SectionLabel>USAGE LEDGER</SectionLabel>
        <span style={{ font: `400 10px ${font.sans}`, color: color.muted2, marginLeft: "auto" }}>
          all time · reported tokens · durations in blocks
        </span>
      </div>
      {rows === null ? (
        <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>Loading…</div>
      ) : rows.length === 0 ? (
        <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
          No runs executed yet.
        </div>
      ) : (
        rows.map((account) => (
          <div
            key={account.accountIdHex ?? account.label}
            style={{ padding: "8px 0", borderTop: `1px solid ${color.borderSoft}` }}
          >
            <div style={rowStyle}>
              <span
                style={{
                  font:
                    account.label === account.accountIdHex || account.accountIdHex === null
                      ? `600 12px ${font.mono}`
                      : `600 13px ${font.sans}`,
                  color: color.ink,
                }}
                title={account.accountIdHex ?? account.label}
              >
                {account.accountIdHex && account.label !== account.accountIdHex
                  ? account.label
                  : shortKey(account.label)}
              </span>
              {account.accountIdHex === null && (
                <span style={{ font: `400 10px ${font.sans}`, color: color.muted2 }}>
                  unbound node
                </span>
              )}
              <span style={{ marginLeft: "auto", display: "flex", gap: 12 }}>
                <Metric label="runs" value={String(account.runs)} />
                {account.failed > 0 && (
                  <Metric label="failed" value={String(account.failed)} />
                )}
                <Metric label="blocks" value={String(account.totalDurationBlocks)} />
                <Metric
                  label="tokens"
                  value={tokenCount.format(account.inputTokens + account.outputTokens)}
                  title={tokenTitle(account)}
                />
              </span>
            </div>
            {account.byCapability.map((cap) => (
              <div key={cap.capability} style={{ ...rowStyle, paddingLeft: 14, marginTop: 3 }}>
                <span style={{ font: `400 11px ${font.mono}`, color: color.inkSoft }}>
                  {cap.capability}
                </span>
                <span style={{ marginLeft: "auto", display: "flex", gap: 12 }}>
                  <Metric label="runs" value={String(cap.runs)} />
                  {cap.failed > 0 && <Metric label="failed" value={String(cap.failed)} />}
                  <Metric label="blocks" value={String(cap.totalDurationBlocks)} />
                  <Metric
                    label="tokens"
                    value={tokenCount.format(cap.inputTokens + cap.outputTokens)}
                    title={tokenTitle(cap)}
                  />
                </span>
              </div>
            ))}
          </div>
        ))
      )}
    </GroupCard>
  );
}
