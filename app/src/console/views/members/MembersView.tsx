// Membership directory over the committed `valset` module — BOTH tiers:
// validators (the consensus quorum) and residents (staged admission — mesh +
// statesync standing, no quorum seat). Approving a join request grants
// resident standing; the deliberate second step, Promote, seats the warm
// resident as a validator, and Revoke drops its standing (its node parks
// again). The node exposes the keys plus profile display names;
// liveness/presence is intentionally shown as unavailable until the backing
// data exists.

import {
  useEffect,
  useMemo,
  useState,
  type CSSProperties,
  type FormEvent,
  type ReactNode,
} from "react";

import {
  providersOf,
  type ProviderGroup,
} from "../../../domain/capability-client";
import {
  displayNameForKey,
  normalizeKey,
  sameKey,
  shortKey,
} from "../../../domain/names";
import {
  isDesktop,
  joinRequests as fetchJoinRequests,
  type JoinRequest,
} from "../../../domain/workspace-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";

type FilterId = "all" | "validators" | "genesis" | "local";

// Provenance, not privilege: the genesis (founding) node created the network at
// genesis. It carries NO special governance authority — membership is pure
// majority rule, one member one vote — so this tooltip rides every genesis tag.
const GENESIS_TOOLTIP =
  "Founding node — created the network at genesis. This is provenance only: it confers no special governance authority (one member, one vote).";

interface MemberVM {
  key: string;
  keyNorm: string;
  displayName: string;
  profileName: string | null;
  initials: string;
  shortKey: string;
  /** Which valset tier the key sits in — the tiers never overlap. */
  tier: "validator" | "resident";
  role: "genesis validator" | "member validator" | "validator" | "resident";
  kind: "validator key" | "resident key";
  status: "in validator set" | "resident standing";
  isFounder: boolean;
  isLocal: boolean;
  searchText: string;
  /** Executor tags this node announced to the capability registry. */
  capabilities: string[];
}

type MemberConfirm =
  | { kind: "remove"; member: MemberVM }
  | { kind: "promote"; member: MemberVM }
  | { kind: "revoke"; member: MemberVM };

const FILTER_TABS: ReadonlyArray<{ id: FilterId; label: string }> = [
  { id: "all", label: "All" },
  { id: "validators", label: "Validators" },
  { id: "genesis", label: "Genesis" },
  { id: "local", label: "This Node" },
];

// Tinted from status hues so the chips re-skin with the theme (was a set of
// baked-in pale pastels that stayed bright in dark mode).
const STATUS_PILLS = {
  validator: tint(color.green),
  // amber: standing granted but not seated — the warming, in-between tier.
  resident: tint(color.amber),
  genesis: { text: color.onDark, bg: color.dark, border: color.dark },
  local: tint(color.accentAlt2),
  muted: { text: color.muted3, bg: color.paper, border: color.borderStrong },
  unavailable: tint(color.amber),
} as const;

const initialsOf = (name: string): string => {
  // Drop parenthetical qualifiers ("eddy (joined node)" → "eddy") and keep only
  // words that START alphanumeric, so an initial never becomes "(" or other
  // punctuation. Fall back to the first two alnum chars (e.g. a hex key → "4C").
  const trimmed = name.replace(/\s*\([^)]*\)\s*/g, " ").trim();
  if (!trimmed) return "?";
  const words = trimmed.split(/\s+/).filter((w) => /^[\p{L}\p{N}]/u.test(w));
  if (words.length >= 2) return `${words[0][0]}${words[1][0]}`.toUpperCase();
  const alnum = (words[0] ?? trimmed).replace(/[^\p{L}\p{N}]/gu, "");
  return alnum.slice(0, 2).toUpperCase() || "?";
};

const copyText = (text: string): void => {
  if (!text) return;
  void navigator.clipboard?.writeText(text).catch(() => {});
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

// mirrors identity's MAX_DISPLAY_NAME_LEN — consensus rejects a longer name.
const MAX_NAME_LEN = 64;

function makeMembers(
  members: string[],
  residents: string[],
  authorNames: Record<string, string>,
  workspace: { pubkey: string; founder: boolean; member: boolean } | null,
  capabilitiesByNode: Map<string, string[]>,
): MemberVM[] {
  const localKey = workspace?.pubkey ?? null;
  const toVM = (key: string, tier: MemberVM["tier"]): MemberVM => {
    const keyNorm = normalizeKey(key);
    const profileName = displayNameForKey(key, authorNames);
    const isLocal = sameKey(key, localKey);
    // A founder is by definition seated — resident standing never applies.
    const isFounder = Boolean(workspace?.founder && isLocal);
    const role =
      tier === "resident"
        ? "resident"
        : isFounder
          ? "genesis validator"
          : isLocal && workspace?.member
            ? "member validator"
            : "validator";
    const displayName = profileName ?? shortKey(key);
    return {
      key,
      keyNorm,
      displayName,
      profileName,
      initials: initialsOf(displayName),
      shortKey: shortKey(key),
      tier,
      role,
      kind: tier === "resident" ? "resident key" : "validator key",
      status: tier === "resident" ? "resident standing" : "in validator set",
      isFounder,
      isLocal,
      searchText: `${displayName} ${key} ${role}`.toLowerCase(),
      capabilities: capabilitiesByNode.get(keyNorm) ?? [],
    };
  };
  // Validators first (the seated quorum), then the warming resident tier.
  return [
    ...members.map((key) => toVM(key, "validator")),
    ...residents.map((key) => toVM(key, "resident")),
  ];
}

/** A run of node rows that share a bound user, within one tier. Sections
 *  (validators/residents) never merge — see groupMembersByUser. */
interface MemberGroup {
  key: string;
  accountId: string;
  name: string;
  members: MemberVM[];
}

type MemberItem =
  | { kind: "member"; member: MemberVM }
  | { kind: "group"; group: MemberGroup };

/** Fold rows onto one header per (tier, bound user) — keyed on tier so a user
 *  with nodes in BOTH the validator and resident tiers gets a separate header
 *  in each (the tiers are disjoint valset standings, never merged). An
 *  unbound key passes through untouched, in place, with no group wrapper.
 *  A bound group with exactly ONE node also collapses flat: post auto-bind
 *  most users are single-device, and the row's authorNames-resolved label
 *  already carries the user name (the provider overlays it), so a header
 *  would only repeat the name directly above its own row. */
function groupMembersByUser(
  members: MemberVM[],
  nodeUsers: Record<string, { accountId: string; name: string | null }>,
): MemberItem[] {
  const items: MemberItem[] = [];
  const groups = new Map<string, MemberGroup>();
  for (const member of members) {
    const bound = nodeUsers[member.keyNorm];
    if (!bound) {
      items.push({ kind: "member", member });
      continue;
    }
    const groupKey = `${member.tier}:${bound.accountId}`;
    let group = groups.get(groupKey);
    if (!group) {
      group = {
        key: groupKey,
        accountId: bound.accountId,
        name: bound.name ?? shortKey(bound.accountId),
        members: [],
      };
      groups.set(groupKey, group);
      items.push({ kind: "group", group });
    }
    group.members.push(member);
  }
  return items.map((item) =>
    item.kind === "group" && item.group.members.length === 1
      ? { kind: "member", member: item.group.members[0] }
      : item,
  );
}

/** A grouped node row labels by its DEVICE key: the header above it already
 *  carries the who (the user), so the row carries the which-device. Copy,
 *  never mutate — the detail pane and search both read the original VM. */
const asDeviceRow = (member: MemberVM): MemberVM => ({
  ...member,
  displayName: member.shortKey,
  initials: initialsOf(member.shortKey),
});

function MemberGroupHeader({ group }: { group: MemberGroup }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "9px 14px 3px",
      }}
    >
      <span style={{ font: `600 11.5px ${font.sans}`, color: color.inkSoft }}>
        {group.name}
      </span>
      <span style={{ font: `500 9.5px ${font.mono}`, color: color.muted2 }}>
        {group.members.length} device{group.members.length === 1 ? "" : "s"}
      </span>
    </div>
  );
}

function roleForFilter(member: MemberVM, filter: FilterId): boolean {
  switch (filter) {
    case "all":
      return true;
    case "validators":
      return member.tier === "validator";
    case "genesis":
      return member.isFounder;
    case "local":
      return member.isLocal;
  }
}

function HeaderRole({
  workspace,
}: {
  workspace: { founder: boolean; member: boolean } | null;
}) {
  const label = workspace?.founder
    ? "Genesis"
    : workspace?.member
      ? "Admitted"
      : "Read Only";
  const pill = workspace?.founder
    ? STATUS_PILLS.genesis
    : workspace?.member
      ? STATUS_PILLS.local
      : STATUS_PILLS.unavailable;
  return <Pill label={label} pill={pill} title={workspace?.founder ? GENESIS_TOOLTIP : undefined} />;
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

/** A node runs one tag per model×effort combo, so a busy node announces dozens.
 *  The row only needs to say WHICH providers it runs: one pill per provider,
 *  with a badge counting its distinct models when there's more than one. The
 *  model list rides the native tooltip, one hover away. */
function ProviderPill({ group }: { group: ProviderGroup }) {
  const count = group.models.length;
  return (
    <span
      title={(count ? group.models : group.tags).join("\n")}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        padding: count > 1 ? "2px 5px 2px 9px" : "2px 9px",
        borderRadius: 999,
        background: color.sunken,
        border: `1px solid ${color.border}`,
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ font: `600 10.5px ${font.sans}`, color: color.inkSoft }}>
        {group.label}
      </span>
      {count > 1 && (
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            minWidth: 16,
            height: 15,
            padding: "0 4px",
            borderRadius: 999,
            background: color.paper,
            border: `1px solid ${color.borderSoft}`,
            font: `600 9px ${font.mono}`,
            color: color.muted3,
          }}
        >
          {count}
        </span>
      )}
    </span>
  );
}

function Avatar({ member, size = 32 }: { member: MemberVM; size?: number }) {
  const bg = member.isFounder ? color.dark : member.isLocal ? STATUS_PILLS.local.bg : color.chip;
  const fg = member.isFounder ? color.onDark : member.isLocal ? STATUS_PILLS.local.text : color.muted3;
  return (
    <span
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        background: bg,
        color: fg,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 ${size > 40 ? 18 : 12}px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {member.initials}
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
}: {
  children: ReactNode;
  onClick?: () => void;
  ariaLabel?: string;
  variant?: "outline" | "dark" | "ghost";
  disabled?: boolean;
  type?: "button" | "submit";
}) {
  const [hover, setHover] = useState(false);
  const dark = variant === "dark";
  const ghost = variant === "ghost";
  return (
    <button
      type={type}
      aria-label={ariaLabel}
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
        border: ghost ? "0" : `1px solid ${dark ? color.dark : color.borderStrong}`,
        background: disabled
          ? color.sunken
          : hover
            ? dark
              ? color.filledHover
              : color.titlebar
            : dark
              ? color.dark
              : "transparent",
        color: disabled ? color.muted2 : dark ? color.onDark : color.inkSoft,
        padding: ghost ? 6 : "7px 12px",
        font: `600 11.5px ${font.sans}`,
        opacity: disabled ? 0.58 : 1,
        cursor: disabled ? "not-allowed" : "pointer",
      }}
    >
      {children}
    </button>
  );
}

function MemberRow({
  member,
  selected,
  onOpen,
  canRemove,
  onRemove,
  canGovernResident,
  onPromote,
  onRevoke,
  canRename,
  onRename,
}: {
  member: MemberVM;
  selected: boolean;
  onOpen: () => void;
  /** Show the removal control for this row (admin, and not this node itself). */
  canRemove: boolean;
  onRemove: () => void;
  /** Show the resident controls for this row (admin, resident-tier row). */
  canGovernResident: boolean;
  onPromote: () => void;
  onRevoke: () => void;
  /** Show the rename control for this row — the local node's own entry, the
   *  only account name an origin-gated `SetAccountName` may write. */
  canRename: boolean;
  onRename: (name: string) => void;
}) {
  const [hover, setHover] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  // Inline self-rename: the same origin-gated identity write Account uses,
  // surfaced on your own row. Enter/Save commits, Escape/Cancel discards; an
  // empty name is treated as no change (Settings owns clearing).
  if (canRename && editing) {
    const commit = () => {
      const next = draft.trim();
      if (next && next !== member.profileName) onRename(next);
      setEditing(false);
    };
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "10px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
          borderRadius: radius.md,
          background: color.sunken,
        }}
      >
        <Avatar member={member} />
        <input
          autoFocus
          aria-label="Edit your display name"
          value={draft}
          maxLength={MAX_NAME_LEN}
          placeholder={member.shortKey}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commit();
            } else if (event.key === "Escape") {
              event.preventDefault();
              setEditing(false);
            }
          }}
          style={{
            flex: 1,
            minWidth: 0,
            height: 30,
            padding: "0 10px",
            borderRadius: radius.sm,
            border: `1px solid ${color.borderStrong}`,
            background: color.paper,
            font: `500 13.5px ${font.sans}`,
            color: color.ink,
            outline: "none",
          }}
        />
        <HoverButton variant="dark" ariaLabel="Save display name" onClick={commit}>
          <Icon name="check" size={13} />
          Save
        </HoverButton>
        <HoverButton
          variant="ghost"
          ariaLabel="Cancel rename"
          onClick={() => setEditing(false)}
        >
          <Icon name="close" size={13} />
        </HoverButton>
      </div>
    );
  }
  // A container div (not a button) so the removal control is a SIBLING of the
  // open button — nesting interactive controls inside a button is invalid.
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        borderBottom: `1px solid ${color.borderSoft}`,
        borderRadius: radius.md,
        background: hover ? color.sidebar : selected ? color.sunken : "transparent",
      }}
    >
      <button
        type="button"
        aria-label={`Open member ${member.displayName}`}
        aria-pressed={selected}
        onClick={onOpen}
        style={{
          ...buttonBase,
          flex: 1,
          minWidth: 0,
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "12px 14px",
          borderRadius: radius.md,
          background: "transparent",
          textAlign: "left",
        }}
      >
        <Avatar member={member} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 7,
              minWidth: 0,
            }}
          >
            <span
              style={{
                font: `600 13.5px ${font.sans}`,
                color: color.ink,
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {member.displayName}
            </span>
            {member.isLocal ? (
              <span style={{ font: `500 9.5px ${font.sans}`, color: color.muted2 }}>
                this node
              </span>
            ) : null}
          </div>
          <div
            title={member.key}
            style={{
              marginTop: 3,
              font: `400 10.5px ${font.mono}`,
              color: color.muted2,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {member.shortKey} · {member.status}
          </div>
          {member.capabilities.length > 0 && (
            <div style={{ marginTop: 6, display: "flex", flexWrap: "wrap", gap: 5 }}>
              {providersOf(member.capabilities).map((group) => (
                <ProviderPill key={group.provider} group={group} />
              ))}
            </div>
          )}
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            flexShrink: 0,
            flexWrap: "wrap",
            justifyContent: "flex-end",
          }}
        >
          {member.isFounder ? (
            <Pill label="Genesis" pill={STATUS_PILLS.genesis} mono title={GENESIS_TOOLTIP} />
          ) : null}
          {member.tier === "resident" ? (
            <Pill label="Resident" pill={STATUS_PILLS.resident} />
          ) : (
            <Pill label="Validator" pill={STATUS_PILLS.validator} />
          )}
        </div>
      </button>
      {canRename ? (
        <div style={{ flexShrink: 0, paddingRight: 12 }}>
          <HoverButton
            variant="ghost"
            ariaLabel="Rename yourself"
            onClick={() => {
              setDraft(member.profileName ?? "");
              setEditing(true);
            }}
          >
            <Icon name="edit" size={14} />
          </HoverButton>
        </div>
      ) : null}
      {canGovernResident ? (
        <div style={{ flexShrink: 0, paddingRight: 12, display: "flex", gap: 7 }}>
          <HoverButton
            variant="dark"
            ariaLabel={`Promote ${member.displayName} into the validator set`}
            onClick={onPromote}
          >
            <Icon name="check" size={13} />
            Promote
          </HoverButton>
          <HoverButton
            variant="outline"
            ariaLabel={`Revoke resident standing from ${member.displayName}`}
            onClick={onRevoke}
          >
            <Icon name="close" size={13} />
            Revoke
          </HoverButton>
        </div>
      ) : canRemove ? (
        <div style={{ flexShrink: 0, paddingRight: 12 }}>
          <HoverButton
            variant="outline"
            ariaLabel={`Remove ${member.displayName} from validator set`}
            onClick={onRemove}
          >
            <Icon name="close" size={13} />
            Remove
          </HoverButton>
        </div>
      ) : null}
    </div>
  );
}

function EmptyState({ filter }: { filter: FilterId }) {
  const label = FILTER_TABS.find((tab) => tab.id === filter)?.label.toLowerCase() ?? "members";
  return (
    <div
      style={{
        padding: "36px 12px",
        textAlign: "center",
        color: color.muted2,
      }}
    >
      <Icon name="members" size={26} color={color.iconIdle} />
      <div style={{ marginTop: 10, font: `500 12.5px ${font.sans}` }}>
        No {filter === "all" ? "validators" : label} to show.
      </div>
      <div style={{ marginTop: 4, font: `400 11px ${font.sans}` }}>
        This view only lists keys reported by the valset module.
      </div>
    </div>
  );
}

/** One joining node's in-flight redemption: who is joining, who invited it.
 *  Invites redeem automatically (minting was the approval), so rows clear on
 *  their own once standing lands — the button forces the manual ballot as a
 *  fallback (a token-less join, or a network mid-upgrade). */
function PendingJoinRequests({
  requests,
  onApprove,
}: {
  requests: JoinRequest[];
  onApprove: (pubkey: string) => void;
}) {
  if (requests.length === 0) return null;
  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.borderStrong}`,
        borderRadius: radius.lg,
        background: color.paper,
        overflow: "hidden",
      }}
    >
      <div
        style={{
          padding: "10px 13px 8px",
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>
          Joining Nodes
        </div>
        <span style={{ font: `500 11px ${font.mono}`, color: color.muted2 }}>
          {requests.length}
        </span>
        <span style={{ marginLeft: "auto", font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
          Invites redeem automatically into resident standing; rows clear once it lands. Approve
          forces the ballot manually.
        </span>
      </div>
      {requests.map((request) => (
        <div
          key={request.joiner}
          style={{
            borderTop: `1px solid ${color.borderSoft}`,
            padding: "9px 13px",
            display: "flex",
            alignItems: "center",
            gap: 10,
          }}
        >
          <div style={{ minWidth: 0 }}>
            <div style={{ font: `500 11.5px ${font.mono}`, color: color.ink }}>
              {shortKey(request.joiner)}
            </div>
            <div style={{ marginTop: 1, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
              invited by {shortKey(request.issuer)}
            </div>
          </div>
          <div style={{ marginLeft: "auto", flexShrink: 0 }}>
            <HoverButton
              onClick={() => onApprove(request.joiner)}
              variant="dark"
              ariaLabel={`Approve join request ${request.joiner}`}
            >
              <Icon name="check" size={13} />
              Approve
            </HoverButton>
          </div>
        </div>
      ))}
    </div>
  );
}

function AdminActions({
  canAdmin,
  inviteBlob,
  pendingJoins,
  onRevealInvite,
  onAdmit,
}: {
  canAdmin: boolean;
  inviteBlob: string | null;
  pendingJoins: JoinRequest[];
  onRevealInvite: () => void;
  onAdmit: (pubkey: string) => void;
}) {
  const [joinerKey, setJoinerKey] = useState("");
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const clean = normalizeKey(joinerKey);
    if (!clean) return;
    onAdmit(clean);
    setJoinerKey("");
  };

  return (
    <section
      aria-label="Member admin actions"
      style={{
        flexShrink: 0,
        padding: "13px 22px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <div style={{ ...sectionLabel, display: "flex", alignItems: "center", gap: 7 }}>
        <Icon name="members" size={13} color={color.muted2} />
        ADMIN ACTIONS
      </div>

      {canAdmin ? <PendingJoinRequests requests={pendingJoins} onApprove={onAdmit} /> : null}

      {canAdmin ? (
        <div
          style={{
            marginTop: 9,
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
            gap: 10,
          }}
        >
          <div
            style={{
              border: `1px solid ${color.border}`,
              borderRadius: radius.lg,
              background: color.sunken,
              overflow: "hidden",
            }}
          >
            <div style={{ padding: "12px 13px", display: "flex", gap: 12, alignItems: "center" }}>
              <div style={{ minWidth: 0 }}>
                <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>
                  Invite a Member
                </div>
                <div style={{ marginTop: 2, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
                  Reveal the workspace invite blob for sharing. One invite admits one
                  person — mint a fresh one per member.
                </div>
              </div>
              <div style={{ marginLeft: "auto", flexShrink: 0 }}>
                <HoverButton onClick={onRevealInvite} variant="dark">
                  <Icon name="plus" size={13} />
                  {inviteBlob ? "Refresh invite" : "Reveal invite"}
                </HoverButton>
              </div>
            </div>
            {inviteBlob ? (
              <div
                style={{
                  borderTop: `1px solid ${color.borderSoft}`,
                  padding: "10px 12px 12px",
                  background: color.paper,
                }}
              >
                <textarea
                  readOnly
                  aria-label="Workspace invite blob"
                  rows={2}
                  value={inviteBlob}
                  onFocus={(event) => event.currentTarget.select()}
                  style={{
                    width: "100%",
                    boxSizing: "border-box",
                    border: `1px solid ${color.borderStrong}`,
                    borderRadius: radius.sm,
                    background: color.paper,
                    color: color.inkSoft,
                    font: `500 10.5px ${font.mono}`,
                    padding: "8px 9px",
                    resize: "vertical",
                  }}
                />
                <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 7 }}>
                  <HoverButton onClick={() => copyText(inviteBlob)} ariaLabel="Copy invite">
                    Copy invite
                  </HoverButton>
                </div>
              </div>
            ) : null}
          </div>

          <form
            aria-label="Admit a joiner"
            onSubmit={submit}
            style={{
              border: `1px solid ${color.border}`,
              borderRadius: radius.lg,
              background: color.paper,
              padding: "12px 13px",
            }}
          >
            <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>
              Admit a Joiner
            </div>
            <div style={{ marginTop: 2, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
              Promote a parked workspace by its public key.
            </div>
            <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
              <label style={{ flex: 1, minWidth: 0 }}>
                <span style={{ display: "none" }}>Joiner public key</span>
                <input
                  aria-label="Joiner public key"
                  name="joiner-public-key"
                  spellCheck={false}
                  value={joinerKey}
                  placeholder="Paste joiner public key…"
                  onChange={(event) => setJoinerKey(event.target.value)}
                  style={{
                    width: "100%",
                    boxSizing: "border-box",
                    border: `1px solid ${color.borderStrong}`,
                    borderRadius: radius.sm,
                    background: color.sunken,
                    color: color.ink,
                    font: `500 11px ${font.mono}`,
                    padding: "8px 9px",
                  }}
                />
              </label>
              <HoverButton type="submit" variant="outline" ariaLabel="Admit joiner">
                <Icon name="check" size={13} />
                Admit
              </HoverButton>
            </div>
          </form>
        </div>
      ) : (
        <div
          style={{
            marginTop: 9,
            border: `1px dashed ${STATUS_PILLS.unavailable.border}`,
            borderRadius: radius.lg,
            background: STATUS_PILLS.unavailable.bg,
            color: STATUS_PILLS.unavailable.text,
            padding: "11px 13px",
            display: "flex",
            alignItems: "center",
            gap: 9,
            font: `500 12px ${font.sans}`,
          }}
        >
          <Icon name="node" size={15} />
          Invite and admission controls require an admitted workspace.
        </div>
      )}
    </section>
  );
}

function InfoRow({
  label,
  value,
  action,
}: {
  label: string;
  value: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.sm,
        background: color.paper,
        padding: "9px 11px",
        display: "grid",
        gridTemplateColumns: "82px minmax(0, 1fr)",
        gap: 10,
        alignItems: "center",
      }}
    >
      <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
        {label}
      </span>
      <div
        style={{
          minWidth: 0,
          display: "flex",
          alignItems: "center",
          gap: 8,
          justifyContent: "flex-end",
        }}
      >
        <span
          style={{
            minWidth: 0,
            textAlign: "right",
            font: `500 11px ${font.mono}`,
            color: color.inkSoft,
            overflowWrap: "anywhere",
          }}
        >
          {value}
        </span>
        {action ? <span style={{ flexShrink: 0 }}>{action}</span> : null}
      </div>
    </div>
  );
}

function MemberDetailPane({
  member,
  onClose,
}: {
  member: MemberVM;
  onClose: () => void;
}) {
  return (
    <aside
      aria-label="Member detail"
      style={{
        width: 332,
        flexShrink: 0,
        borderLeft: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          height: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          padding: "0 16px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.sidebar,
        }}
      >
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>
          Member
        </span>
        <div style={{ marginLeft: "auto" }}>
          <HoverButton onClick={onClose} ariaLabel="Close member detail" variant="ghost">
            <Icon name="close" size={16} color={color.muted2} />
          </HoverButton>
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "18px 16px" }}>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            textAlign: "center",
          }}
        >
          <Avatar member={member} size={54} />
          <h2
            style={{
              margin: "11px 0 0",
              font: `600 16px ${font.sans}`,
              color: color.dark,
              maxWidth: "100%",
              overflowWrap: "anywhere",
            }}
          >
            {member.displayName}
          </h2>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              flexWrap: "wrap",
              justifyContent: "center",
              marginTop: 7,
            }}
          >
            {member.tier === "resident" ? (
              <Pill label="Resident standing" pill={STATUS_PILLS.resident} />
            ) : (
              <Pill label="In validator set" pill={STATUS_PILLS.validator} />
            )}
            {member.isFounder ? (
              <Pill label="Genesis" pill={STATUS_PILLS.genesis} mono title={GENESIS_TOOLTIP} />
            ) : null}
          </div>
        </div>

        <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 8 }}>
          <InfoRow label="profile" value={member.profileName ?? "not available"} />
          <InfoRow
            label="public key"
            value={member.key}
            action={
              <HoverButton onClick={() => copyText(member.key)} ariaLabel="Copy public key">
                Copy
              </HoverButton>
            }
          />
          <InfoRow label="short key" value={member.shortKey} />
          <InfoRow label="role" value={member.role} />
          <InfoRow label="kind" value={member.kind} />
          <InfoRow label="status" value={member.status} />
          <InfoRow label="genesis" value={member.isFounder ? "yes" : "no"} />
          <InfoRow label="this node" value={member.isLocal ? "yes" : "no"} />
          <InfoRow label="presence" value="not exposed by this node" />
        </div>

        <div style={{ marginTop: 18 }}>
          <div style={{ ...sectionLabel, marginBottom: 9 }}>RUNS ON</div>
          {member.capabilities.length === 0 ? (
            <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
              No executors announced by this node.
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              {providersOf(member.capabilities).map((group) => (
                <div key={group.provider}>
                  <div style={{ font: `600 11.5px ${font.sans}`, color: color.inkSoft }}>
                    {group.label}
                  </div>
                  {group.models.length > 0 ? (
                    <div
                      style={{ marginTop: 6, display: "flex", flexWrap: "wrap", gap: 4 }}
                    >
                      {group.models.map((model) => (
                        <span
                          key={model}
                          translate="no"
                          style={{
                            padding: "2px 8px",
                            borderRadius: 999,
                            background: color.paper,
                            border: `1px solid ${color.border}`,
                            font: `500 10px ${font.mono}`,
                            color: color.muted3,
                          }}
                        >
                          {model}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <div style={{ marginTop: 4, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
                      default executor
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}

export function MembersView() {
  const { state, actions } = useDucktape();
  const [filter, setFilter] = useState<FilterId>("all");
  const [query, setQuery] = useState("");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [pendingConfirm, setPendingConfirm] = useState<MemberConfirm | null>(null);

  const rows = useMemo(
    () =>
      makeMembers(
        state.members,
        state.residents,
        state.authorNames,
        state.workspace,
        state.capabilitiesByNode,
      ),
    [
      state.authorNames,
      state.capabilitiesByNode,
      state.members,
      state.residents,
      state.workspace,
    ],
  );
  const queryText = query.trim().toLowerCase();
  const visibleRows = useMemo(
    () =>
      rows.filter(
        (member) =>
          roleForFilter(member, filter) &&
          (!queryText || member.searchText.includes(queryText)),
      ),
    [filter, queryText, rows],
  );
  const selected = selectedKey
    ? rows.find((member) => member.keyNorm === normalizeKey(selectedKey)) ?? null
    : null;
  const canAdmin = Boolean(state.workspace?.founder || state.workspace?.member);
  const groupedRows = useMemo(
    () => groupMembersByUser(visibleRows, state.nodeUsers),
    [visibleRows, state.nodeUsers],
  );

  // Pending join requests live on THIS member's node (delivered over the lobby
  // channel), read via the desktop registry — poll while the admin surface is
  // up. A dead node / web build degrades to an empty list, never an error.
  const [pendingJoins, setPendingJoins] = useState<JoinRequest[]>([]);
  const workspaceId = state.workspace?.id ?? null;
  useEffect(() => {
    if (!workspaceId || !canAdmin || !isDesktop()) {
      setPendingJoins([]);
      return;
    }
    let alive = true;
    const pull = () =>
      Promise.resolve()
        .then(() => fetchJoinRequests(workspaceId))
        .then((rows) => {
          if (alive) setPendingJoins(rows);
        })
        .catch(() => {
          if (alive) setPendingJoins([]);
        });
    void pull();
    const timer = window.setInterval(pull, 5000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [workspaceId, canAdmin]);

  const requestRemove = (member: MemberVM): void => {
    // Never remove your own node — that would drop this workspace out of the
    // set it is driving governance through.
    if (member.isLocal) return;
    setPendingConfirm({ kind: "remove", member });
  };

  const requestPromote = (member: MemberVM): void => {
    setPendingConfirm({ kind: "promote", member });
  };

  const requestRevoke = (member: MemberVM): void => {
    setPendingConfirm({ kind: "revoke", member });
  };

  const confirmPendingAction = () => {
    if (!pendingConfirm) return;
    const { kind, member } = pendingConfirm;
    if (kind === "remove") {
      actions.demoteMember(member.key);
      if (selectedKey && normalizeKey(selectedKey) === member.keyNorm) {
        setSelectedKey(null);
      }
    } else if (kind === "promote") {
      actions.promoteMember(member.key);
    } else {
      actions.removeResident(member.key);
      if (selectedKey && normalizeKey(selectedKey) === member.keyNorm) {
        setSelectedKey(null);
      }
    }
    setPendingConfirm(null);
  };

  // Shared row renderer — a group's nested rows and a standalone row get the
  // exact same per-node affordances (promote/demote/remove/rename), only the
  // wrapper around them differs.
  const renderMemberRow = (member: MemberVM) => (
    <MemberRow
      key={member.keyNorm || member.key}
      member={member}
      selected={selected?.keyNorm === member.keyNorm}
      onOpen={() => setSelectedKey(member.key)}
      canRemove={canAdmin && !member.isLocal && member.tier === "validator"}
      onRemove={() => requestRemove(member)}
      canGovernResident={canAdmin && member.tier === "resident"}
      onPromote={() => requestPromote(member)}
      onRevoke={() => requestRevoke(member)}
      canRename={member.isLocal}
      onRename={actions.setDisplayName}
    />
  );

  return (
    <div
      data-screen-label="Members"
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
            Members
          </span>
          <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
            {rows.length}
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
          <label style={{ marginLeft: "auto", flex: "1 1 220px", maxWidth: 260, minWidth: 140 }}>
            <span style={{ display: "none" }}>Search members</span>
            <input
              aria-label="Search members"
              name="member-search"
              spellCheck={false}
              value={query}
              placeholder="Search name or key…"
              onChange={(event) => setQuery(event.target.value)}
              style={{
                width: "100%",
                boxSizing: "border-box",
                border: `1px solid ${color.borderStrong}`,
                borderRadius: radius.sm,
                background: color.sunken,
                color: color.ink,
                font: `500 11.5px ${font.sans}`,
                padding: "7px 10px",
              }}
            />
          </label>
        </div>

        <AdminActions
          canAdmin={canAdmin}
          inviteBlob={state.inviteBlob}
          pendingJoins={pendingJoins}
          onRevealInvite={actions.revealInvite}
          onAdmit={actions.admitMember}
        />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "6px 12px",
            background: color.canvas,
          }}
        >
          {visibleRows.length === 0 ? (
            <EmptyState filter={filter} />
          ) : (
            <div
              style={{
                border: `1px solid ${color.borderSoft}`,
                borderRadius: radius.md,
                overflow: "hidden",
                background: color.paper,
                boxShadow: shadow.card,
              }}
            >
              {groupedRows.map((item) =>
                item.kind === "member" ? (
                  renderMemberRow(item.member)
                ) : (
                  <div key={item.group.key}>
                    <MemberGroupHeader group={item.group} />
                    <div style={{ paddingLeft: 14 }}>
                      {item.group.members.map((member) =>
                        renderMemberRow(asDeviceRow(member)),
                      )}
                    </div>
                  </div>
                ),
              )}
            </div>
          )}
        </div>
      </div>

      {selected ? (
        <MemberDetailPane member={selected} onClose={() => setSelectedKey(null)} />
      ) : null}
      {pendingConfirm && (
        <ConfirmDialog
          title={
            pendingConfirm.kind === "remove"
              ? `Remove ${pendingConfirm.member.displayName}?`
              : pendingConfirm.kind === "promote"
                ? `Promote ${pendingConfirm.member.displayName}?`
                : `Revoke ${pendingConfirm.member.displayName}?`
          }
          confirmLabel={
            pendingConfirm.kind === "remove"
              ? "Remove from validators"
              : pendingConfirm.kind === "promote"
                ? "Promote to validator"
                : "Revoke standing"
          }
          danger={pendingConfirm.kind !== "promote"}
          onCancel={() => setPendingConfirm(null)}
          onConfirm={confirmPendingAction}
        >
          {pendingConfirm.kind === "remove" ? (
            <>
              This opens a removal proposal and casts this node's yes ballot.
              It only takes effect once a strict majority approves.
            </>
          ) : pendingConfirm.kind === "promote" ? (
            <>
              This opens an AddValidator proposal and casts this node's yes ballot.
              The pre-synced resident joins quorum at the next epoch cutover after
              majority approval.
            </>
          ) : (
            <>
              This opens a RemoveResident proposal and casts this node's yes ballot.
              After majority approval, the key drops off the mesh at the next epoch
              cutover and its node parks again.
            </>
          )}
        </ConfirmDialog>
      )}
    </div>
  );
}
