// Validator directory over the committed `valset` module. The node exposes
// validator keys plus profile display names; liveness/presence is intentionally
// shown as unavailable until the backing data exists.

import { useMemo, useState, type CSSProperties, type FormEvent, type ReactNode } from "react";

import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

type FilterId = "all" | "validators" | "founder" | "local";

interface MemberVM {
  key: string;
  keyNorm: string;
  displayName: string;
  profileName: string | null;
  initials: string;
  shortKey: string;
  role: "founder validator" | "member validator" | "validator";
  kind: "validator key";
  status: "in validator set";
  isFounder: boolean;
  isLocal: boolean;
  searchText: string;
}

const FILTER_TABS: ReadonlyArray<{ id: FilterId; label: string }> = [
  { id: "all", label: "All" },
  { id: "validators", label: "Validators" },
  { id: "founder", label: "Founder" },
  { id: "local", label: "This Node" },
];

const STATUS_PILLS = {
  validator: { text: "#5f9e74", bg: "#eef5f0", border: "#cfe3d7" },
  founder: { text: color.onDark, bg: color.dark, border: color.dark },
  local: { text: color.accentAlt2, bg: "#eef5f0", border: "#cfe3d7" },
  muted: { text: color.muted3, bg: color.paper, border: color.borderStrong },
  unavailable: { text: color.amber, bg: "#fbf4e6", border: "#ecdcae" },
} as const;

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

function makeMembers(
  members: string[],
  authorNames: Record<string, string>,
  workspace: { pubkey: string; founder: boolean; member: boolean } | null,
): MemberVM[] {
  const localKey = workspace?.pubkey ?? null;
  return members.map((key) => {
    const keyNorm = normalizeKey(key);
    const profileName = authorNames[key] ?? authorNames[keyNorm] ?? null;
    const isLocal = sameKey(key, localKey);
    const isFounder = Boolean(workspace?.founder && isLocal);
    const role = isFounder
      ? "founder validator"
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
      role,
      kind: "validator key",
      status: "in validator set",
      isFounder,
      isLocal,
      searchText: `${displayName} ${key} ${role}`.toLowerCase(),
    };
  });
}

function roleForFilter(member: MemberVM, filter: FilterId): boolean {
  switch (filter) {
    case "all":
    case "validators":
      return true;
    case "founder":
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
    ? "Founder"
    : workspace?.member
      ? "Admitted"
      : "Read Only";
  const pill = workspace?.founder
    ? STATUS_PILLS.founder
    : workspace?.member
      ? STATUS_PILLS.local
      : STATUS_PILLS.unavailable;
  return <Pill label={label} pill={pill} />;
}

function Pill({
  label,
  pill,
  mono,
}: {
  label: string;
  pill: { text: string; bg: string; border: string };
  mono?: boolean;
}) {
  return (
    <span
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

function Avatar({ member, size = 32 }: { member: MemberVM; size?: number }) {
  const bg = member.isFounder ? color.dark : member.isLocal ? "#dfeee4" : color.chip;
  const fg = member.isFounder ? color.onDark : member.isLocal ? color.accentAlt2 : color.muted3;
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
              ? "#38362e"
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
}: {
  member: MemberVM;
  selected: boolean;
  onOpen: () => void;
  /** Show the removal control for this row (admin, and not this node itself). */
  canRemove: boolean;
  onRemove: () => void;
}) {
  const [hover, setHover] = useState(false);
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
          {member.isFounder ? <Pill label="Founder" pill={STATUS_PILLS.founder} mono /> : null}
          <Pill label="Validator" pill={STATUS_PILLS.validator} />
        </div>
      </button>
      {canRemove ? (
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

function AdminActions({
  canAdmin,
  inviteBlob,
  onRevealInvite,
  onAdmit,
}: {
  canAdmin: boolean;
  inviteBlob: string | null;
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
                  Reveal the workspace invite blob for sharing.
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
                  autoComplete="off"
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
            <Pill label="In validator set" pill={STATUS_PILLS.validator} />
            {member.isFounder ? <Pill label="Founder" pill={STATUS_PILLS.founder} mono /> : null}
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
          <InfoRow label="founder" value={member.isFounder ? "yes" : "no"} />
          <InfoRow label="this node" value={member.isLocal ? "yes" : "no"} />
          <InfoRow label="presence" value="not exposed by this node" />
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

  const rows = useMemo(
    () => makeMembers(state.members, state.authorNames, state.workspace),
    [state.authorNames, state.members, state.workspace],
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

  const requestRemove = (member: MemberVM): void => {
    // Never remove your own node — that would drop this workspace out of the
    // set it is driving governance through.
    if (member.isLocal) return;
    const ok = window.confirm(
      `Remove ${member.displayName} from the validator set?\n\n` +
        `This opens a removal proposal and casts THIS node's yes-ballot. ` +
        `It only takes effect once a strict majority of members (n / 2 + 1) ` +
        `approve — every other member must run the same removal.`,
    );
    if (!ok) return;
    actions.demoteMember(member.key);
    if (selectedKey && normalizeKey(selectedKey) === member.keyNorm) {
      setSelectedKey(null);
    }
  };

  return (
    <div
      data-screen-label="Members"
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
              autoComplete="off"
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
          onRevealInvite={actions.revealInvite}
          onAdmit={actions.admitMember}
        />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "6px 12px",
            background: "#fcfcfc",
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
              {visibleRows.map((member) => (
                <MemberRow
                  key={member.keyNorm || member.key}
                  member={member}
                  selected={selected?.keyNorm === member.keyNorm}
                  onOpen={() => setSelectedKey(member.key)}
                  canRemove={canAdmin && !member.isLocal}
                  onRemove={() => requestRemove(member)}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {selected ? (
        <MemberDetailPane member={selected} onClose={() => setSelectedKey(null)} />
      ) : null}
    </div>
  );
}
