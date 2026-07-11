// DANGER ZONE — the active workspace's destructive lifecycle: on-chain leave,
// guarded local forget, and the force override for a node that won't start.

import { useState } from "react";
import type { ReactNode } from "react";

import { normalizeKey } from "../../../domain/names";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton, SectionLabel } from "./parts";

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
        border: `1px solid ${color.dangerBorder}`,
        background: color.dangerSoft,
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
        hoverBg={`color-mix(in srgb, ${color.red} 82%, #000)`}
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

export function DangerZone() {
  const { state, actions } = useDucktape();
  const [pendingAction, setPendingAction] = useState<"leave" | "forget" | "forceForget" | null>(null);
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
  // the workspace card's validatorCount fallback) so the enable-state is
  // coherent before the roster arrives.
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
    setPendingAction("leave");
  };

  const forget = (): void => {
    setPendingAction("forget");
  };

  // Revealed only after a guarded forget couldn't confirm the node left its
  // valset — i.e. the node won't come up (a bricked recovery). Force skips that
  // liveness check so a workspace whose node can never start is still removable.
  // The backend still refuses to force-tear-down a node it CAN reach and that
  // proves it's a live multi-member validator, so this can't silently halt a
  // healthy network — but for a node that may still be one elsewhere, the honest
  // warning puts the call in the user's hands.
  const forceForget = (): void => {
    setPendingAction("forceForget");
  };

  const confirmAction = () => {
    if (pendingAction === "leave") actions.requestLeaveWorkspace();
    else if (pendingAction === "forget") actions.forgetWorkspace();
    else if (pendingAction === "forceForget") actions.forgetWorkspace(true);
    setPendingAction(null);
  };

  const name = state.workspace?.name ?? "this workspace";

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
                check and deletes the workspace (directory, node key, registry
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
      {pendingAction && (
        <ConfirmDialog
          title={
            pendingAction === "leave"
              ? `Request to leave ${name}?`
              : pendingAction === "forget"
                ? `Forget ${name}?`
                : `Force-forget ${name}?`
          }
          confirmLabel={
            pendingAction === "leave"
              ? "Request leave"
              : pendingAction === "forget"
                ? "Forget workspace"
                : "Force forget"
          }
          onCancel={() => setPendingAction(null)}
          onConfirm={confirmAction}
        >
          {pendingAction === "leave" ? (
            <>
              This submits an on-chain self-removal and casts this node's yes
              ballot. Your node keeps running until a strict majority of remaining
              members approve.
            </>
          ) : pendingAction === "forget" ? (
            <>
              This stops this node and deletes the workspace locally. It is refused
              while this node is still a current validator of a network with other
              members.
            </>
          ) : (
            <>
              This skips the liveness confirmation and deletes the workspace,
              including directory, node key, and registry entry. Only use this
              for a solo or defunct network.
            </>
          )}
        </ConfirmDialog>
      )}
    </>
  );
}
