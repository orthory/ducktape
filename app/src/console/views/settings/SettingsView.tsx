// Local console preferences plus the active workspace identity surface. This
// view stays wired only to the console store facade: views read
// useDucktape() -> { state, actions } and never reach around it.

import { useState, type CSSProperties, type ReactNode } from "react";

import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { Toggle } from "./Toggle";

const ACCENTS = [
  color.accent,
  color.accentAlt1,
  color.accentAlt2,
  color.purple,
  color.red,
] as const;

const monoValue: CSSProperties = {
  font: `400 12px ${font.mono}`,
  color: color.muted,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  maxWidth: 330,
};

const smallMono: CSSProperties = {
  font: `400 10.5px ${font.mono}`,
  color: color.muted2,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const copyText = (text: string): void => {
  void navigator.clipboard?.writeText(text).catch(() => {});
};

const workspaceDataDir = (id: string): string => `~/.ducktape/workspaces/${id}`;

const quorumText = (count: number): string => {
  if (count <= 0) return "not exposed";
  const threshold = Math.floor((count * 2) / 3) + 1;
  return `${threshold} of ${count} validator${count === 1 ? "" : "s"}`;
};

const initialsOf = (name: string): string => {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "?";
  return parts
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
};

function workspaceRole(workspace: {
  founder: boolean;
  member: boolean;
} | null) {
  if (workspace?.founder) {
    return {
      role: "genesis validator",
      title: "Genesis validator",
      tier: "GENESIS",
      fg: color.onDark,
      bg: color.dark,
      bd: color.dark,
    } as const;
  }
  if (workspace?.member) {
    return {
      role: "member validator",
      title: "Member validator",
      tier: "MEMBER",
      fg: color.accentAlt2,
      bg: "#eef5f0",
      bd: "#cfe3d7",
    } as const;
  }
  return {
    role: "guest",
    title: "Guest",
    tier: "GUEST",
    fg: color.amber,
    bg: "#fbf4e6",
    bd: "#ecdcae",
  } as const;
}

function SectionLabel({
  children,
  danger,
  marginTop = 20,
}: {
  children: ReactNode;
  danger?: boolean;
  marginTop?: number;
}) {
  return (
    <div
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".11em",
        color: danger ? "#c79a8a" : color.muted2,
        marginTop,
      }}
    >
      {children}
    </div>
  );
}

function GroupCard({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        overflow: "hidden",
        background: color.paper,
      }}
    >
      {children}
    </div>
  );
}

function InfoRow({
  label,
  value,
  last,
}: {
  label: string;
  value: ReactNode;
  last?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "13px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <span style={{ font: `500 12.5px ${font.sans}`, color: color.inkSoft }}>
        {label}
      </span>
      <span style={{ marginLeft: "auto", minWidth: 0, textAlign: "right" }}>
        {value}
      </span>
    </div>
  );
}

function ControlRow({
  title,
  desc,
  control,
  last,
}: {
  title: string;
  desc: string;
  control: ReactNode;
  last?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "13px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ font: `500 12.5px ${font.sans}`, color: color.inkSoft }}>
          {title}
        </div>
        <div
          style={{
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            marginTop: 1,
            lineHeight: 1.35,
          }}
        >
          {desc}
        </div>
      </div>
      <div style={{ marginLeft: "auto", flexShrink: 0 }}>{control}</div>
    </div>
  );
}

function HoverButton({
  onClick,
  style,
  hoverBg,
  children,
  ariaLabel,
  disabled,
}: {
  onClick: () => void;
  style: CSSProperties;
  hoverBg: string;
  children: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        ...style,
        cursor: disabled ? "not-allowed" : style.cursor,
        opacity: disabled ? 0.55 : style.opacity,
        background: !disabled && hover ? hoverBg : style.background,
      }}
    >
      {children}
    </button>
  );
}

const outlineButton: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `500 11.5px ${font.sans}`,
  color: color.muted3,
  border: `1px solid ${color.borderStrong}`,
  borderRadius: 8,
  padding: "7px 13px",
};

const darkButton: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 11.5px ${font.sans}`,
  color: color.onDark,
  background: color.dark,
  borderRadius: 8,
  padding: "8px 14px",
};

function AccentPicker({
  value,
  onPick,
}: {
  value: string;
  onPick: (accent: string) => void;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
      {ACCENTS.map((accent) => (
        <button
          key={accent}
          type="button"
          aria-label={`Set accent ${accent}`}
          title={accent}
          onClick={() => onPick(accent)}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 22,
            height: 22,
            borderRadius: "50%",
            background: accent,
            boxShadow:
              value === accent
                ? `0 0 0 2px ${color.paper}, 0 0 0 4px ${accent}`
                : `0 0 0 1px ${color.borderStrong}`,
          }}
        />
      ))}
    </div>
  );
}

function IdentityCard() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const role = workspaceRole(workspace);
  const key = workspace?.pubkey ?? "";
  const keyLine = key
    ? `${key} · key on this device`
    : "no workspace key loaded";

  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        padding: 15,
        display: "flex",
        alignItems: "center",
        gap: 13,
        background: color.paper,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 40,
          height: 40,
          borderRadius: "50%",
          background: "#cdcdcd",
          color: color.muted3,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          font: `600 15px ${font.sans}`,
        }}
      >
        {initialsOf(state.author)}
      </span>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          <input
            aria-label="Display name"
            value={state.author}
            onChange={(event) => actions.setAuthor(event.target.value)}
            onBlur={(event) => actions.setDisplayName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            style={{
              all: "unset",
              width: Math.max(58, Math.min(230, state.author.length * 8 + 12)),
              font: `600 13.5px ${font.sans}`,
              color: color.ink,
            }}
          />
          <span
            title={
              workspace?.founder
                ? "Founding node — created the network at genesis. Provenance only; it confers no special governance authority."
                : undefined
            }
            style={{
              font: `600 9px ${font.mono}`,
              color: role.fg,
              background: role.bg,
              border: `1px solid ${role.bd}`,
              borderRadius: 4,
              padding: "2px 6px",
              letterSpacing: ".04em",
            }}
          >
            {role.tier}
          </span>
        </div>
        <div style={{ ...smallMono, marginTop: 3 }} title={keyLine}>
          {keyLine}
        </div>
      </div>

      <HoverButton
        ariaLabel="Copy key"
        onClick={() => copyText(key)}
        hoverBg={color.titlebar}
        disabled={!key}
        style={outlineButton}
      >
        Copy key
      </HoverButton>
    </div>
  );
}

function InviteBlob({ value }: { value: string }) {
  return (
    <div
      style={{
        padding: "10px 15px 13px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.sunken,
      }}
    >
      <textarea
        readOnly
        rows={2}
        value={value}
        onFocus={(event) => event.currentTarget.select()}
        style={{
          width: "100%",
          boxSizing: "border-box",
          padding: "9px 10px",
          borderRadius: radius.sm,
          border: `1px solid ${color.borderStrong}`,
          background: color.paper,
          font: `500 10.5px ${font.mono}`,
          color: color.inkSoft,
          resize: "vertical",
        }}
      />
    </div>
  );
}

function AdmitControl() {
  const { actions } = useDucktape();
  const [pubkey, setPubkey] = useState("");
  return (
    <div style={{ display: "flex", gap: 7 }}>
      <input
        aria-label="Joiner pubkey"
        value={pubkey}
        placeholder="joiner pubkey"
        onChange={(event) => setPubkey(event.target.value)}
        style={{
          width: 160,
          boxSizing: "border-box",
          padding: "7px 9px",
          borderRadius: radius.sm,
          border: `1px solid ${color.borderStrong}`,
          background: color.sunken,
          font: `500 11px ${font.mono}`,
          color: color.ink,
        }}
      />
      <HoverButton
        onClick={() => {
          actions.admitMember(pubkey);
          setPubkey("");
        }}
        hoverBg={color.titlebar}
        style={outlineButton}
      >
        Admit
      </HoverButton>
    </div>
  );
}

function NetworkSection() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const role = workspaceRole(workspace);
  const validatorCount = state.members.length || (workspace?.member ? 1 : 0);
  const portLine = workspace
    ? `p2p ${workspace.ports.listen} · http ${workspace.ports.http} · rpc ${workspace.ports.rpc}`
    : "not available";

  return (
    <>
      <SectionLabel marginTop={18}>NETWORK</SectionLabel>
      <GroupCard>
        <InfoRow
          label="Network name"
          value={
            <span style={{ font: `500 12px ${font.mono}`, color: color.inkSofter }}>
              {workspace?.name ?? "Remote node"}
            </span>
          }
        />
        <InfoRow
          label="Network ID"
          value={
            <span style={monoValue} title={workspace?.chainId}>
              {workspace?.chainId ?? "not available"}
            </span>
          }
        />
        <InfoRow
          label="Data dir"
          value={
            <span style={monoValue}>
              {workspace ? workspaceDataDir(workspace.id) : "not available"}
            </span>
          }
        />
        <InfoRow label="Ports" value={<span style={monoValue}>{portLine}</span>} />
        <InfoRow
          label="Quorum threshold"
          value={<span style={monoValue}>{quorumText(validatorCount)}</span>}
        />
        <InfoRow
          label="Node role"
          value={<span style={monoValue}>{role.role}</span>}
        />
        <ControlRow
          title="Switch workspace"
          desc="Create, join, or select another local workspace."
          last={!LIVE_JOIN_SUPPORTED}
          control={
            <HoverButton
              ariaLabel="Workspaces"
              onClick={actions.newWorkspace}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Workspaces
            </HoverButton>
          }
        />

        {LIVE_JOIN_SUPPORTED && (
          <>
            <ControlRow
              title="Invite a member"
              desc="Reveal a fresh invite blob for this network."
              control={
                <HoverButton
                  onClick={actions.revealInvite}
                  hoverBg="#38362e"
                  disabled={!workspace}
                  style={darkButton}
                >
                  {state.inviteBlob ? "Refresh invite" : "Reveal invite"}
                </HoverButton>
              }
            />
            {state.inviteBlob && <InviteBlob value={state.inviteBlob} />}
            <ControlRow
              title="Admit a joiner"
              desc="Promote a waiting workspace by its public key."
              last
              control={<AdmitControl />}
            />
          </>
        )}
      </GroupCard>
    </>
  );
}

function PreferencesSection() {
  const { state, actions } = useDucktape();
  return (
    <>
      <SectionLabel>PREFERENCES</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Local node"
          desc={
            state.managed
              ? "Start or stop the desktop-managed daemon."
              : "Remote nodes are controlled outside this console."
          }
          control={
            <Toggle
              on={state.connected}
              disabled={!state.managed}
              label="Local node"
              onToggle={() => {
                if (state.connected) actions.stopNode();
                else actions.startNode();
              }}
            />
          }
        />
        <ControlRow
          title="Accent"
          desc="Used for active navigation, focus, and primary controls."
          last
          control={<AccentPicker value={state.accent} onPick={actions.setAccent} />}
        />
      </GroupCard>
    </>
  );
}

const normalizeKey = (key: string | null | undefined): string =>
  (key ?? "").trim().replace(/^0x/i, "").toLowerCase();

function DangerRow({
  title,
  detail,
  buttonLabel,
  ariaLabel,
  onClick,
  disabled,
}: {
  title: string;
  detail: ReactNode;
  buttonLabel: string;
  ariaLabel: string;
  onClick: () => void;
  disabled: boolean;
}) {
  return (
    <div
      style={{
        border: "1px solid #ecd6d0",
        background: "#fdf6f4",
        borderRadius: radius.lg,
        padding: 15,
        display: "flex",
        alignItems: "center",
        gap: 13,
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>
          {title}
        </div>
        <div
          style={{
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            marginTop: 2,
            lineHeight: 1.4,
          }}
        >
          {detail}
        </div>
      </div>
      <HoverButton
        ariaLabel={ariaLabel}
        onClick={onClick}
        hoverBg="#8f463d"
        disabled={disabled}
        style={{
          all: "unset",
          cursor: disabled ? "not-allowed" : "pointer",
          font: `600 11.5px ${font.sans}`,
          color: color.onDark,
          background: color.red,
          borderRadius: 8,
          padding: "8px 15px",
          opacity: disabled ? 0.5 : 1,
          whiteSpace: "nowrap",
        }}
      >
        {buttonLabel}
      </HoverButton>
    </div>
  );
}

function DangerZone() {
  const { state, actions } = useDucktape();
  const base = !state.workspace || !state.managed;

  // Is THIS node still a current validator, and how big is the set? Leaving is
  // an on-chain self-removal that needs a strict majority of the OTHER members;
  // forgetting is a local teardown that must not run while we're still a current
  // validator of a set of two-or-more (it would halt quorum). A solo node
  // (validators = 1) can't remove the last validator — it just forgets.
  const pubkey = state.workspace?.pubkey ?? null;
  // Before the first roster query hydrates state.members it is []; deriving
  // membership straight from it would read as "not in the set / 0 validators"
  // and lock a real validator out of BOTH request-leave and forget during the
  // cold-start window. Fall back to this node's own membership flag (mirrors
  // NetworkSection's validatorCount fallback) so the enable-state is coherent
  // before the roster arrives.
  const hasRoster = state.members.length > 0;
  const inSet = hasRoster
    ? state.members.some((m) => normalizeKey(m) === normalizeKey(pubkey))
    : Boolean(state.workspace?.member);
  const validatorCount = state.members.length || (state.workspace?.member ? 1 : 0);
  // With a known roster we still hide request-leave for a provably-solo set
  // (forget instead). Before the roster hydrates we can't know the set size, so
  // we enable it for a member and let the node's last-validator guard refuse a
  // solo leave honestly — never a silent lock-out.
  const soloKnown = hasRoster && validatorCount < 2;
  const canRequestLeave = !base && inSet && !soloKnown;

  const requestLeave = (): void => {
    const name = state.workspace?.name ?? "this network";
    const ok = window.confirm(
      `Request to leave "${name}"?\n\n` +
        `This submits an ON-CHAIN self-removal of this node and casts its ` +
        `yes-ballot. Your node KEEPS RUNNING: in a set of two or more members ` +
        `the removal stays PENDING until a strict majority (n / 2 + 1) of the ` +
        `remaining members approve — the node must stay up through its own ` +
        `pending removal or the network can't finalize it. Once you're removed ` +
        `(you drop out of the validator set), you can forget the workspace.`,
    );
    if (!ok) return;
    actions.requestLeaveWorkspace();
  };

  const forget = (): void => {
    const name = state.workspace?.name ?? "this workspace";
    const ok = window.confirm(
      `Forget "${name}"?\n\n` +
        `This stops this node and deletes the workspace locally — its ` +
        `directory and registry entry are removed. This is refused while this ` +
        `node is still a current validator of a network with other members ` +
        `(forgetting it then would halt the network's quorum). Safe once you've ` +
        `been removed, or for a solo network only this node runs.`,
    );
    if (!ok) return;
    actions.forgetWorkspace();
  };

  // Revealed only after a guarded forget couldn't confirm the node left its
  // valset — i.e. the node won't come up (a bricked recovery). Force skips that
  // liveness check so a workspace whose node can never start is still removable.
  // The backend still refuses to force-tear-down a node it CAN reach and that
  // proves it's a live multi-member validator, so this can't silently halt a
  // healthy network — but for a node that may still be one elsewhere, the honest
  // warning puts the call in the user's hands.
  const forceForget = (): void => {
    const name = state.workspace?.name ?? "this workspace";
    const ok = window.confirm(
      `Force-forget "${name}"?\n\n` +
        `The node couldn't confirm it has left its validator set — usually ` +
        `because it can't start (a corrupt / bricked local state). Forcing ` +
        `deletes the workspace WITHOUT that confirmation: its directory, ` +
        `identity key, and registry entry are removed for good.\n\n` +
        `Only do this for a network you know is solo or defunct. If this node ` +
        `is still a validator of a network with OTHER live members, destroying ` +
        `its identity can PERMANENTLY halt that network. This cannot be undone.`,
    );
    if (!ok) return;
    actions.forgetWorkspace(true);
  };

  return (
    <>
      <SectionLabel danger>DANGER ZONE</SectionLabel>
      <div
        style={{ marginTop: 9, display: "flex", flexDirection: "column", gap: 9 }}
      >
        <DangerRow
          title="Leave this network"
          detail={
            <>
              Submits an on-chain self-removal (pending a strict majority of the
              remaining members). Your node keeps running until they approve;
              once removed you can forget the workspace.
              {inSet && soloKnown ? (
                <> A solo node can’t remove the last validator — forget it below.</>
              ) : null}
            </>
          }
          buttonLabel="Request leave"
          ariaLabel="Request leave"
          onClick={requestLeave}
          disabled={!canRequestLeave}
        />
        <DangerRow
          title="Forget this workspace"
          detail={
            <>
              Stops this node and deletes the workspace locally (directory +
              registry entry). Guarded: refused while this node is still a
              current validator of a network with other members.
            </>
          }
          buttonLabel="Forget workspace"
          ariaLabel="Forget workspace"
          onClick={forget}
          disabled={base}
        />
        {state.forgetNeedsForce && !base ? (
          <DangerRow
            title="Force-forget (node won’t start)"
            detail={
              <>
                The guarded forget couldn’t confirm this node has left its
                validator set — usually because it can’t start. Force skips that
                check and deletes the workspace (directory, identity key, registry
                entry). Only for a solo or defunct network — if this node is still
                a live validator elsewhere, this can permanently halt it.
              </>
            }
            buttonLabel="Force forget"
            ariaLabel="Force forget workspace"
            onClick={forceForget}
            disabled={base}
          />
        ) : null}
      </div>
    </>
  );
}

export function SettingsView() {
  return (
    <div
      data-screen-label="Settings"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: "#fcfcfc",
        padding: 22,
        overflowY: "auto",
      }}
    >
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
        Settings
      </div>

      <div style={{ maxWidth: 600 }}>
        <NetworkSection />

        <SectionLabel>YOUR IDENTITY</SectionLabel>
        <IdentityCard />

        <PreferencesSection />
        <DangerZone />

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
