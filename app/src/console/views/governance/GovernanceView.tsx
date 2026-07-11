// Governance — the operator-facing surface over the `governance` module. Lists
// every proposal (GovQuery::Proposals, projected into state.proposals per block)
// with its action, status, proposer, and running tally. Validators may make a
// one-way transition to non-transferable Identity-account shares; proposal
// cards then render their frozen weighted electorate and decision rule.

import { useMemo, useState, type CSSProperties, type FormEvent, type ReactNode } from "react";

import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import {
  actionKeyHex,
  actionLabel,
  actionText,
  canSettleEarly,
  decisionThreshold,
  proposerHex,
  tally,
  type SharesView,
  type ProposalStatus,
  type ProposalView,
} from "../../../domain/governance-client";
import { sameKey, shortKey } from "../../../domain/names";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";

type FilterId = "all" | "open" | "settled";

const FILTER_TABS: ReadonlyArray<{ id: FilterId; label: string }> = [
  { id: "all", label: "All" },
  { id: "open", label: "Open" },
  { id: "settled", label: "Settled" },
];

const STATUS_PILLS: Record<ProposalStatus, { text: string; bg: string; border: string }> = {
  open: tint(color.amber),
  passed: tint(color.green),
  rejected: { text: color.danger, bg: color.dangerSoft, border: color.dangerBorder },
};

const buttonBase: CSSProperties = {
  appearance: "none",
  boxSizing: "border-box",
  border: "0",
  margin: 0,
  font: "inherit",
  cursor: "pointer",
  touchAction: "manipulation",
};

const sectionLabel: CSSProperties = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
};

interface ProposalVM {
  id: string;
  title: string;
  detail: string;
  status: ProposalStatus;
  proposerHex: string;
  proposerShort: string;
  proposerIsLocal: boolean;
  yes: number;
  no: number;
  total: number;
  threshold: number;
  thresholdLabel: string;
  /** Our own ballot on this proposal, or null when this node has not voted. */
  myVote: boolean | null;
  eligible: boolean;
  /** The proposal's frozen rule can no longer be reversed before its deadline. */
  decided: boolean;
}

function makeProposal(
  proposal: ProposalView,
  localKey: string | null,
  localAccount: string | null,
  memberCount: number,
  legacyCanVote: boolean,
): ProposalVM {
  const counts = tally(proposal);
  const threshold = decisionThreshold(proposal, memberCount);
  const keyText = actionKeyHex(proposal.action);
  const signalText = actionText(proposal.action);
  const label = actionLabel(proposal.action);
  const detail = signalText ?? (keyText ? shortKey(keyText) : "—");
  const proposer = proposerHex(proposal.proposer);
  const localPrincipal = proposal.voter_kind === "account" ? localAccount : localKey;
  const myBallot = proposal.votes.find(([voter]) =>
    sameKey(proposerHex(voter), localPrincipal),
  );
  return {
    id: proposal.proposal_id,
    title: label,
    detail,
    status: proposal.status,
    proposerHex: proposer,
    proposerShort: shortKey(proposer),
    proposerIsLocal: sameKey(proposer, localPrincipal),
    yes: counts.yes,
    no: counts.no,
    total: counts.total,
    threshold,
    thresholdLabel:
      typeof proposal.voting_rule === "object" &&
      "participating_majority" in proposal.voting_rule
        ? `quorum ${threshold} + majority`
        : `needs ${threshold} approve`,
    myVote: myBallot ? myBallot[1] : null,
    eligible:
      proposal.electorate.length > 0
        ? proposal.electorate.some(([principal]) =>
            sameKey(proposerHex(principal), localPrincipal),
          )
        : proposal.voter_kind === "validator_node" && legacyCanVote,
    decided: canSettleEarly(proposal, memberCount),
  };
}

function inFilter(proposal: ProposalVM, filter: FilterId): boolean {
  switch (filter) {
    case "all":
      return true;
    case "open":
      return proposal.status === "open";
    case "settled":
      return proposal.status !== "open";
  }
}

function Pill({
  label,
  pill,
  mono,
  title,
}: {
  label: string;
  pill: { text: string; bg: string; border: string };
  mono?: boolean;
  title?: string;
}) {
  return (
    <span
      title={title}
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: radius.sm,
        border: `1px solid ${pill.border}`,
        background: pill.bg,
        color: pill.text,
        padding: "3px 8px",
        font: `600 ${mono ? "9.5px" : "10.5px"} ${mono ? font.mono : font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

function HoverButton({
  children,
  onClick,
  ariaLabel,
  variant = "outline",
  disabled,
  type = "button",
  active,
}: {
  children: ReactNode;
  onClick?: () => void;
  ariaLabel?: string;
  variant?: "outline" | "dark" | "approve" | "reject";
  disabled?: boolean;
  type?: "button" | "submit";
  active?: boolean;
}) {
  const [hover, setHover] = useState(false);
  const dark = variant === "dark";
  const approve = variant === "approve";
  const reject = variant === "reject";
  const accent = approve ? tint(color.green).border : reject ? color.danger : color.borderStrong;
  const activeBg = approve ? tint(color.green).bg : reject ? color.dangerSoft : color.titlebar;
  return (
    <button
      type={type}
      aria-label={ariaLabel}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        ...buttonBase,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 7,
        borderRadius: radius.sm,
        border: dark ? `1px solid ${color.dark}` : `1px solid ${accent}`,
        background: disabled
          ? color.sunken
          : dark
            ? hover
              ? color.filledHover
              : color.dark
            : active || hover
              ? activeBg
              : "transparent",
        color: disabled
          ? color.muted2
          : dark
            ? color.onDark
            : approve
              ? color.accentAlt2
              : reject
                ? color.danger
                : color.inkSoft,
        padding: "7px 12px",
        font: `600 11.5px ${font.sans}`,
        opacity: disabled ? 0.58 : 1,
        cursor: disabled ? "not-allowed" : "pointer",
      }}
    >
      {children}
    </button>
  );
}

function HeaderRole({
  workspace,
  shares,
  localAccount,
}: {
  workspace: { founder: boolean; member: boolean } | null;
  shares: SharesView;
  localAccount: string | null;
}) {
  if (shares.active) {
    const own = shares.allocations.find((allocation) =>
      sameKey(proposerHex(allocation.account_id), localAccount),
    );
    return (
      <Pill
        label={
          own
            ? `Shareholder ${own.shares}/${shares.total} (${formatSharePercent(own.shares, shares.total)})`
            : "Read only"
        }
        pill={own ? STATUS_PILLS.passed : STATUS_PILLS.open}
        title={
          own
            ? "This Identity account may propose and vote with its frozen share power."
            : "This node's Identity account holds no governance shares."
        }
      />
    );
  }
  const canVote = Boolean(workspace?.founder || workspace?.member);
  return (
    <Pill
      label={canVote ? "Voting member" : "Read only"}
      pill={canVote ? STATUS_PILLS.passed : STATUS_PILLS.open}
      title={
        canVote
          ? "This node is a current validator: it may propose, vote, and settle."
          : "This node is not an admitted validator, so it can watch but not vote."
      }
    />
  );
}

function TallyBar({ proposal }: { proposal: ProposalVM }) {
  const denom = Math.max(proposal.yes + proposal.no, 1);
  const yesPct = (proposal.yes / denom) * 100;
  return (
    <div style={{ marginTop: 10 }}>
      <div
        style={{
          height: 6,
          borderRadius: 3,
          background: color.sunken,
          border: `1px solid ${color.borderSoft}`,
          overflow: "hidden",
          display: "flex",
        }}
      >
        <span style={{ width: `${yesPct}%`, background: `color-mix(in srgb, ${color.green} 50%, var(--c-paper))` }} />
        <span style={{ flex: 1, background: `color-mix(in srgb, ${color.red} 50%, var(--c-paper))` }} />
      </div>
      <div
        style={{
          marginTop: 6,
          display: "flex",
          alignItems: "center",
          gap: 12,
          font: `500 10.5px ${font.mono}`,
          color: color.muted3,
          flexWrap: "wrap",
        }}
      >
        <span style={{ color: color.accentAlt2 }}>approve {proposal.yes}</span>
        <span style={{ color: color.danger }}>reject {proposal.no}</span>
        <span>· {proposal.total} cast</span>
        <span style={{ marginLeft: "auto" }}>{proposal.thresholdLabel}</span>
      </div>
    </div>
  );
}

function ProposalCard({
  proposal,
  op,
  canVote,
  onVote,
  onExecute,
}: {
  proposal: ProposalVM;
  /** The proposal row's finalization record — propose, this node's ballots,
   *  and settles all key here, so the row shows its latest write's state. */
  op: OpRecord | undefined;
  canVote: boolean;
  onVote: (approve: boolean) => void;
  onExecute: () => void;
}) {
  const open = proposal.status === "open";
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        padding: "14px 15px",
        boxShadow: shadow.card,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 9 }}>
        <span style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>
          {proposal.title}
        </span>
        <span style={{ marginLeft: "auto" }}>
          <Pill label={proposal.status} pill={STATUS_PILLS[proposal.status]} />
        </span>
      </div>

      <div
        title={proposal.detail}
        style={{
          marginTop: 5,
          font: `400 12px ${font.sans}`,
          color: color.inkSoft,
          overflowWrap: "anywhere",
        }}
      >
        {proposal.detail}
      </div>

      <div
        style={{
          marginTop: 8,
          display: "flex",
          alignItems: "center",
          gap: 8,
          font: `400 10.5px ${font.mono}`,
          color: color.muted2,
          flexWrap: "wrap",
        }}
      >
        <span title={proposal.proposerHex}>by {proposal.proposerShort}</span>
        {proposal.proposerIsLocal ? (
          <span style={{ color: color.muted3 }}>· this node</span>
        ) : null}
        <span title={proposal.id}>· {shortKey(proposal.id)}</span>
        <FinalizationMark op={op} />
      </div>

      <TallyBar proposal={proposal} />

      {open ? (
        <div
          style={{
            marginTop: 12,
            display: "flex",
            alignItems: "center",
            gap: 8,
            flexWrap: "wrap",
          }}
        >
          <HoverButton
            variant="approve"
            active={proposal.myVote === true}
            disabled={!canVote}
            ariaLabel={`Approve proposal ${proposal.title}`}
            onClick={() => onVote(true)}
          >
            <Icon name="check" size={13} />
            Approve
          </HoverButton>
          <HoverButton
            variant="reject"
            active={proposal.myVote === false}
            disabled={!canVote}
            ariaLabel={`Reject proposal ${proposal.title}`}
            onClick={() => onVote(false)}
          >
            <Icon name="close" size={13} />
            Reject
          </HoverButton>
          <span style={{ marginLeft: "auto" }}>
            <HoverButton
              variant="dark"
              ariaLabel={`Settle proposal ${proposal.title}`}
              onClick={onExecute}
            >
              {proposal.decided ? "Settle · ready" : "Settle"}
            </HoverButton>
          </span>
        </div>
      ) : null}
    </div>
  );
}

function ProposeForm({
  canPropose,
  shareMode,
  onPropose,
}: {
  canPropose: boolean;
  shareMode: boolean;
  onPropose: (text: string) => void;
}) {
  const [text, setText] = useState("");
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const body = text.trim();
    if (!body) return;
    onPropose(body);
    setText("");
  };

  return (
    <section
      aria-label="Open a proposal"
      style={{
        flexShrink: 0,
        padding: "13px 22px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <div style={{ ...sectionLabel, display: "flex", alignItems: "center", gap: 7 }}>
        <Icon name="governance" size={13} color={color.muted2} />
        OPEN A PROPOSAL
      </div>

      {canPropose ? (
        <form
          aria-label="Signal proposal"
          onSubmit={submit}
          style={{
            marginTop: 9,
            border: `1px solid ${color.border}`,
            borderRadius: radius.lg,
            background: color.paper,
            padding: "12px 13px",
          }}
        >
          <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>
            Signal proposal
          </div>
          <div style={{ marginTop: 2, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
            Put a question to the {shareMode ? "shareholders" : "validator set"}. Passing binds the signal; it has
            no on-chain effect of its own.
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
            <label style={{ flex: 1, minWidth: 0 }}>
              <span style={{ display: "none" }}>Proposal text</span>
              <input
                aria-label="Proposal text"
                name="proposal-text"
                spellCheck
                value={text}
                placeholder="Describe what the set should signal…"
                onChange={(event) => setText(event.target.value)}
                style={{
                  width: "100%",
                  boxSizing: "border-box",
                  border: `1px solid ${color.borderStrong}`,
                  borderRadius: radius.sm,
                  background: color.sunken,
                  color: color.ink,
                  font: `500 11.5px ${font.sans}`,
                  padding: "8px 9px",
                }}
              />
            </label>
            <HoverButton type="submit" variant="dark" ariaLabel="Open proposal">
              <Icon name="plus" size={13} />
              Propose
            </HoverButton>
          </div>
        </form>
      ) : (
        <div
          style={{
            marginTop: 9,
            border: `1px dashed ${STATUS_PILLS.open.border}`,
            borderRadius: radius.lg,
            background: STATUS_PILLS.open.bg,
            color: STATUS_PILLS.open.text,
            padding: "11px 13px",
            display: "flex",
            alignItems: "center",
            gap: 9,
            font: `500 12px ${font.sans}`,
          }}
        >
          <Icon name="node" size={15} />
          Only an eligible {shareMode ? "shareholder account" : "validator"} can open or vote on proposals.
        </div>
      )}
    </section>
  );
}

export function parseShareAllocations(
  text: string,
): Array<{ accountId: string; shares: number }> | null {
  const allocations: Array<{ accountId: string; shares: number }> = [];
  const seen = new Set<string>();
  for (const line of text.split("\n").map((value) => value.trim()).filter(Boolean)) {
    const [rawAccount, rawShares, extra] = line.split(/\s+/);
    const accountId = rawAccount?.toLowerCase() ?? "";
    const shares = Number(rawShares);
    if (
      extra !== undefined ||
      accountId.length === 0 ||
      accountId.length % 2 !== 0 ||
      !/^[0-9a-f]+$/.test(accountId) ||
      !Number.isSafeInteger(shares) ||
      shares <= 0 ||
      seen.has(accountId)
    ) {
      return null;
    }
    seen.add(accountId);
    allocations.push({ accountId, shares });
  }
  return allocations.length > 0 ? allocations : null;
}

export function formatSharePercent(shares: number, total: number): string {
  if (total <= 0) return "0%";
  return `${Number(((shares / total) * 100).toFixed(2))}%`;
}

function SharesPanel({
  shares,
  knownAccounts,
  canPropose,
  onAdopt,
  onSet,
}: {
  shares: SharesView;
  knownAccounts: string[];
  canPropose: boolean;
  onAdopt: (allocations: Array<{ accountId: string; shares: number }>) => void;
  onSet: (accountId: string, shares: number) => void;
}) {
  const [allocationText, setAllocationText] = useState("");
  const [accountId, setAccountId] = useState("");
  const [shareText, setShareText] = useState("");
  const [error, setError] = useState<string | null>(null);

  const adopt = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const allocations = parseShareAllocations(allocationText);
    if (!allocations) {
      setError("Use one unique ‘account-hex shares’ row per account.");
      return;
    }
    setError(null);
    onAdopt(allocations);
  };
  const set = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalized = accountId.trim().toLowerCase();
    const power = Number(shareText);
    if (
      normalized.length === 0 ||
      normalized.length % 2 !== 0 ||
      !/^[0-9a-f]+$/.test(normalized) ||
      !Number.isSafeInteger(power) ||
      power < 0
    ) {
      setError("Enter an account hex id and a non-negative integer share value.");
      return;
    }
    setError(null);
    onSet(normalized, power);
  };

  return (
    <section
      aria-label="Governance shares"
      style={{
        flexShrink: 0,
        padding: "12px 22px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <div style={{ ...sectionLabel, display: "flex", alignItems: "center", gap: 7 }}>
        GOVERNANCE SHARES
        <Pill
          label={shares.active ? `${shares.total} active` : "validator ballots"}
          pill={shares.active ? STATUS_PILLS.passed : STATUS_PILLS.open}
        />
      </div>

      {shares.active ? (
        <>
          <div
            style={{
              display: "flex",
              gap: 6,
              flexWrap: "wrap",
              marginTop: 9,
              font: `500 10.5px ${font.mono}`,
            }}
          >
            {shares.allocations.map((allocation) => {
              const hex = proposerHex(allocation.account_id);
              return (
                <span key={hex} title={hex} style={{ color: color.inkSoft }}>
                  {shortKey(hex)} · {allocation.shares} · {formatSharePercent(allocation.shares, shares.total)}
                </span>
              );
            })}
          </div>
          <form onSubmit={set} style={{ display: "flex", gap: 8, marginTop: 10 }}>
            <input
              aria-label="Share account id"
              value={accountId}
              placeholder="Account hex"
              onChange={(event) => setAccountId(event.target.value)}
              style={{ flex: 1, minWidth: 0, padding: "7px 8px", font: `500 10.5px ${font.mono}` }}
            />
            <input
              aria-label="Account shares"
              value={shareText}
              placeholder="Shares (0 removes)"
              inputMode="numeric"
              onChange={(event) => setShareText(event.target.value)}
              style={{ width: 145, padding: "7px 8px", font: `500 10.5px ${font.mono}` }}
            />
            <HoverButton type="submit" variant="dark" disabled={!canPropose}>
              Propose change
            </HoverButton>
          </form>
        </>
      ) : (
        <form onSubmit={adopt} style={{ marginTop: 9 }}>
          <div style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
            One-way activation. Existing Identity accounts: {knownAccounts.length > 0
              ? knownAccounts.map((account) => shortKey(account)).join(", ")
              : "none bound yet"}.
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 8, alignItems: "stretch" }}>
            <textarea
              aria-label="Initial share allocations"
              value={allocationText}
              placeholder={`${knownAccounts[0] ?? "account-hex"} 100`}
              onChange={(event) => setAllocationText(event.target.value)}
              rows={Math.max(2, Math.min(knownAccounts.length, 4))}
              style={{ flex: 1, minWidth: 0, padding: "7px 8px", font: `500 10.5px ${font.mono}` }}
            />
            <HoverButton type="submit" variant="dark" disabled={!canPropose}>
              Propose adoption
            </HoverButton>
          </div>
        </form>
      )}
      {error ? (
        <div role="alert" style={{ marginTop: 7, color: color.danger, font: `500 10.5px ${font.sans}` }}>
          {error}
        </div>
      ) : null}
    </section>
  );
}

function EmptyState({ filter }: { filter: FilterId }) {
  const label = FILTER_TABS.find((tab) => tab.id === filter)?.label.toLowerCase() ?? "proposals";
  return (
    <div style={{ padding: "36px 12px", textAlign: "center", color: color.muted2 }}>
      <Icon name="governance" size={26} color={color.iconIdle} />
      <div style={{ marginTop: 10, font: `500 12.5px ${font.sans}` }}>
        No {filter === "all" ? "" : `${label} `}proposals to show.
      </div>
      <div style={{ marginTop: 4, font: `400 11px ${font.sans}` }}>
        Proposals appear here once an eligible voter opens one.
      </div>
    </div>
  );
}

export function GovernanceView() {
  const { state, actions } = useDucktape();
  const [filter, setFilter] = useState<FilterId>("all");

  const localKey = state.workspace?.pubkey ?? null;
  const localAccount = localKey
    ? state.nodeUsers[localKey.toLowerCase()]?.accountId ?? null
    : null;
  const memberCount = state.members.length;
  const legacyCanVote = Boolean(state.workspace?.founder || state.workspace?.member);
  const knownAccounts = useMemo(
    () => [...new Set(Object.values(state.nodeUsers).map((user) => user.accountId))].sort(),
    [state.nodeUsers],
  );
  const rows = useMemo(
    () =>
      [...state.proposals]
        .map((proposal) =>
          makeProposal(proposal, localKey, localAccount, memberCount, legacyCanVote),
        )
        .sort((a, b) => {
          // Open first, then by id so the ordering is stable across refreshes.
          if (a.status === b.status) return a.id.localeCompare(b.id);
          return a.status === "open" ? -1 : b.status === "open" ? 1 : 0;
        }),
    [state.proposals, localKey, localAccount, memberCount, legacyCanVote],
  );
  const visibleRows = useMemo(
    () => rows.filter((proposal) => inFilter(proposal, filter)),
    [rows, filter],
  );
  const canPropose = state.governanceShares.active
    ? state.governanceShares.allocations.some((allocation) =>
        sameKey(proposerHex(allocation.account_id), localAccount),
      )
    : legacyCanVote;
  const openCount = rows.filter((proposal) => proposal.status === "open").length;

  return (
    <div
      data-screen-label="Governance"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        background: color.canvas,
        overflow: "hidden",
      }}
    >
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <div
          style={{
            height: 56,
            flexShrink: 0,
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "0 22px",
            borderBottom: `1px solid ${color.borderSoft}`,
            background: color.paper,
          }}
        >
          <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
            Governance
          </span>
          <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
            {openCount} open · {rows.length}
          </span>
          <span style={{ marginLeft: "auto" }}>
            <HeaderRole
              workspace={state.workspace}
              shares={state.governanceShares}
              localAccount={localAccount}
            />
          </span>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            flexWrap: "wrap",
            gap: 12,
            padding: "12px 22px",
            borderBottom: `1px solid ${color.borderSoft}`,
            flexShrink: 0,
            background: color.paper,
          }}
        >
          <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
            {FILTER_TABS.map((tab) => {
              const active = filter === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setFilter(tab.id)}
                  style={{
                    ...buttonBase,
                    color: active ? color.ink : color.muted2,
                    background: active ? color.chip : "transparent",
                    borderRadius: radius.sm,
                    padding: "5px 11px",
                    font: `500 11.5px ${font.sans}`,
                  }}
                >
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>

        <SharesPanel
          shares={state.governanceShares}
          knownAccounts={knownAccounts}
          canPropose={canPropose}
          onAdopt={actions.proposeAdoptShares}
          onSet={actions.proposeSetShares}
        />

        <ProposeForm
          canPropose={canPropose}
          shareMode={state.governanceShares.active}
          onPropose={actions.proposeSignal}
        />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "12px",
            background: color.canvas,
            display: "flex",
            flexDirection: "column",
            gap: 10,
          }}
        >
          {visibleRows.length === 0 ? (
            <EmptyState filter={filter} />
          ) : (
            visibleRows.map((proposal) => (
              <ProposalCard
                key={proposal.id}
                proposal={proposal}
                op={state.ops[opKey.proposal(proposal.id)]}
                canVote={proposal.eligible}
                onVote={(approve) => actions.voteProposal(proposal.id, approve)}
                onExecute={() => actions.executeProposal(proposal.id)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
