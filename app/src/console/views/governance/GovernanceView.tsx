// Approvals — the operator-facing surface over the `governance` module. Lists
// every proposal (GovQuery::Proposals, projected into state.proposals per block)
// with its action, status, proposer, and running tally, and drives the three
// member-gated writes: Propose (a Signal), Vote (approve / reject), and Execute
// (settle a decidable proposal). Governance is pure majority rule over the
// current validator set — one member, one vote, and NO genesis/founder
// privilege — so the surface never implies any special authority.

import { useMemo, useState, type CSSProperties, type FormEvent, type ReactNode } from "react";

import { Icon } from "../../components/Icon";
import {
  actionKeyHex,
  actionLabel,
  actionText,
  majorityOf,
  proposerHex,
  tally,
  type ProposalStatus,
  type ProposalView,
} from "../../../domain/governance-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

type FilterId = "all" | "open" | "settled";

const FILTER_TABS: ReadonlyArray<{ id: FilterId; label: string }> = [
  { id: "all", label: "All" },
  { id: "open", label: "Open" },
  { id: "settled", label: "Settled" },
];

const STATUS_PILLS: Record<ProposalStatus, { text: string; bg: string; border: string }> = {
  Open: { text: color.amber, bg: "#fbf4e6", border: "#ecdcae" },
  Passed: { text: "#5f9e74", bg: "#eef5f0", border: "#cfe3d7" },
  Rejected: { text: color.danger, bg: color.dangerSoft, border: color.dangerBorder },
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

const normalizeKey = (key: string | null | undefined): string =>
  (key ?? "").trim().replace(/^0x/i, "").toLowerCase();

const sameKey = (left: string | null | undefined, right: string | null | undefined): boolean =>
  Boolean(normalizeKey(left)) && normalizeKey(left) === normalizeKey(right);

const shortKey = (hex: string, start = 10, end = 6): string => {
  const clean = hex.trim();
  if (!clean) return "—";
  return clean.length > start + end + 1
    ? `${clean.slice(0, start)}…${clean.slice(-end)}`
    : clean;
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
  /** Our own ballot on this proposal, or null when this node has not voted. */
  myVote: boolean | null;
  /** A strict majority of members has already voted one way — decidable now. */
  decided: boolean;
}

function makeProposal(
  proposal: ProposalView,
  localKey: string | null,
  memberCount: number,
): ProposalVM {
  const counts = tally(proposal);
  const threshold = majorityOf(memberCount);
  const keyText = actionKeyHex(proposal.action);
  const signalText = actionText(proposal.action);
  const label = actionLabel(proposal.action);
  const detail = signalText ?? (keyText ? shortKey(keyText) : "—");
  const proposer = proposerHex(proposal.proposer);
  const myBallot = proposal.votes.find(([voter]) =>
    sameKey(proposerHex(voter), localKey),
  );
  return {
    id: proposal.proposal_id,
    title: label,
    detail,
    status: proposal.status,
    proposerHex: proposer,
    proposerShort: shortKey(proposer),
    proposerIsLocal: sameKey(proposer, localKey),
    yes: counts.yes,
    no: counts.no,
    total: counts.total,
    threshold,
    myVote: myBallot ? myBallot[1] : null,
    decided: counts.yes >= threshold || counts.no >= threshold,
  };
}

function inFilter(proposal: ProposalVM, filter: FilterId): boolean {
  switch (filter) {
    case "all":
      return true;
    case "open":
      return proposal.status === "Open";
    case "settled":
      return proposal.status !== "Open";
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
  const accent = approve ? "#5f9e74" : reject ? color.danger : color.borderStrong;
  const activeBg = approve ? "#eef5f0" : reject ? color.dangerSoft : color.titlebar;
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
              ? "#38362e"
              : color.dark
            : active || hover
              ? activeBg
              : "transparent",
        color: disabled
          ? color.muted2
          : dark
            ? color.onDark
            : approve
              ? "#3f7d54"
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
}: {
  workspace: { founder: boolean; member: boolean } | null;
}) {
  const canVote = Boolean(workspace?.founder || workspace?.member);
  return (
    <Pill
      label={canVote ? "Voting member" : "Read only"}
      pill={canVote ? STATUS_PILLS.Passed : STATUS_PILLS.Open}
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
        <span style={{ width: `${yesPct}%`, background: "#7cc08f" }} />
        <span style={{ flex: 1, background: "#e4b0a8" }} />
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
        <span style={{ color: "#3f7d54" }}>approve {proposal.yes}</span>
        <span style={{ color: color.danger }}>reject {proposal.no}</span>
        <span>· {proposal.total} cast</span>
        <span style={{ marginLeft: "auto" }}>needs {proposal.threshold} to pass</span>
      </div>
    </div>
  );
}

function ProposalCard({
  proposal,
  canVote,
  onVote,
  onExecute,
}: {
  proposal: ProposalVM;
  canVote: boolean;
  onVote: (approve: boolean) => void;
  onExecute: () => void;
}) {
  const open = proposal.status === "Open";
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
  onPropose,
}: {
  canPropose: boolean;
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
        <Icon name="approvals" size={13} color={color.muted2} />
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
            Put a question to the validator set. Passing binds the signal; it has
            no on-chain effect of its own.
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
            <label style={{ flex: 1, minWidth: 0 }}>
              <span style={{ display: "none" }}>Proposal text</span>
              <input
                aria-label="Proposal text"
                name="proposal-text"
                autoComplete="off"
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
            border: `1px dashed ${STATUS_PILLS.Open.border}`,
            borderRadius: radius.lg,
            background: STATUS_PILLS.Open.bg,
            color: STATUS_PILLS.Open.text,
            padding: "11px 13px",
            display: "flex",
            alignItems: "center",
            gap: 9,
            font: `500 12px ${font.sans}`,
          }}
        >
          <Icon name="node" size={15} />
          Only an admitted validator can open or vote on proposals.
        </div>
      )}
    </section>
  );
}

function EmptyState({ filter }: { filter: FilterId }) {
  const label = FILTER_TABS.find((tab) => tab.id === filter)?.label.toLowerCase() ?? "proposals";
  return (
    <div style={{ padding: "36px 12px", textAlign: "center", color: color.muted2 }}>
      <Icon name="approvals" size={26} color={color.iconIdle} />
      <div style={{ marginTop: 10, font: `500 12.5px ${font.sans}` }}>
        No {filter === "all" ? "" : `${label} `}proposals to show.
      </div>
      <div style={{ marginTop: 4, font: `400 11px ${font.sans}` }}>
        Proposals appear here once a validator opens one.
      </div>
    </div>
  );
}

export function GovernanceView() {
  const { state, actions } = useDucktape();
  const [filter, setFilter] = useState<FilterId>("all");

  const localKey = state.workspace?.pubkey ?? null;
  const memberCount = state.members.length;
  const rows = useMemo(
    () =>
      [...state.proposals]
        .map((proposal) => makeProposal(proposal, localKey, memberCount))
        .sort((a, b) => {
          // Open first, then by id so the ordering is stable across refreshes.
          if (a.status === b.status) return a.id.localeCompare(b.id);
          return a.status === "Open" ? -1 : b.status === "Open" ? 1 : 0;
        }),
    [state.proposals, localKey, memberCount],
  );
  const visibleRows = useMemo(
    () => rows.filter((proposal) => inFilter(proposal, filter)),
    [rows, filter],
  );
  const canVote = Boolean(state.workspace?.founder || state.workspace?.member);
  const openCount = rows.filter((proposal) => proposal.status === "Open").length;

  return (
    <div
      data-screen-label="Approvals"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        background: "#fcfcfc",
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
            Approvals
          </span>
          <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
            {openCount} open · {rows.length}
          </span>
          <span style={{ marginLeft: "auto" }}>
            <HeaderRole workspace={state.workspace} />
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

        <ProposeForm canPropose={canVote} onPropose={actions.proposeSignal} />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "12px",
            background: "#fcfcfc",
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
                canVote={canVote}
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
