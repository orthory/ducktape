// Nodes — the multi-node account made visible, in two honest groups. "On this
// network": every node the account owns on the CONNECTED network (identity
// module), with its valset standing and the lost-device Unbind affordance
// (`user_sign_unbind`'s first UI consumer). "On this machine": the local
// workspace registry — other networks this box runs nodes for, switchable.
// Account data is chain-scoped, so the network group only knows the active
// workspace's chain.

import { useState } from "react";

import { normalizeKey, sameKey, shortKey } from "../../../domain/names";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, tint } from "../../theme/tokens";
import { errMessage, errorTextStyle } from "../onboarding/IdentityGateForms";
import {
  GroupCard,
  HoverButton,
  InfoRow,
  monoValue,
  outlineButton,
  SectionLabel,
} from "../settings/parts";
import { CustodyPanel } from "./CustodyCard";

function StandingChip({ standing }: { standing: "Validator" | "Resident" | "No seat" }) {
  const palette =
    standing === "Validator"
      ? { fg: color.onDark, bg: color.dark, bd: color.dark }
      : standing === "Resident"
        ? { fg: tint(color.green).text, bg: tint(color.green).bg, bd: tint(color.green).border }
        : { fg: color.muted2, bg: color.sunken, bd: color.border };
  return (
    <span
      style={{
        font: `600 9px ${font.mono}`,
        color: palette.fg,
        background: palette.bg,
        border: `1px solid ${palette.bd}`,
        borderRadius: 4,
        padding: "2px 6px",
        letterSpacing: ".04em",
        whiteSpace: "nowrap",
      }}
    >
      {standing.toUpperCase()}
    </span>
  );
}

export function NodesCard({ accountId }: { accountId: string | undefined }) {
  const { state, actions } = useDucktape();
  const [pendingUnbind, setPendingUnbind] = useState<string | null>(null);
  const [unbindError, setUnbindError] = useState<string | null>(null);

  const workspace = state.workspace;
  const validators = new Set(state.members.map(normalizeKey));
  const residents = new Set(state.residents.map(normalizeKey));

  // Every node bound to this account on the connected network.
  const networkNodes = accountId
    ? Object.entries(state.nodeUsers)
        .filter(([, owner]) => owner.accountId === accountId)
        .map(([nodeHex]) => nodeHex)
        .sort()
    : [];

  const standingOf = (nodeHex: string): "Validator" | "Resident" | "No seat" =>
    validators.has(normalizeKey(nodeHex))
      ? "Validator"
      : residents.has(normalizeKey(nodeHex))
        ? "Resident"
        : "No seat";

  return (
    <>
      {accountId && (
        <>
          <SectionLabel>NODES ON THIS NETWORK</SectionLabel>
          <GroupCard>
            {networkNodes.map((nodeHex, i) => {
              const isThisDevice = workspace ? sameKey(nodeHex, workspace.pubkey) : false;
              const last = i === networkNodes.length - 1 && !unbindError;
              return (
                <InfoRow
                  key={nodeHex}
                  label={isThisDevice ? "This device's node" : "Node"}
                  last={last}
                  value={
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                      <span style={monoValue}>{shortKey(nodeHex)}</span>
                      <StandingChip standing={standingOf(nodeHex)} />
                      <HoverButton
                        ariaLabel={`Unbind node ${shortKey(nodeHex)}`}
                        onClick={() => {
                          setUnbindError(null);
                          setPendingUnbind(nodeHex);
                        }}
                        hoverBg={color.dangerSoft}
                        style={{ ...outlineButton, color: color.red }}
                      >
                        Unbind
                      </HoverButton>
                    </span>
                  }
                />
              );
            })}
            {networkNodes.length === 0 && (
              <InfoRow
                label="Nodes"
                last={!unbindError}
                value={<span style={monoValue}>none bound yet</span>}
              />
            )}
            {unbindError && (
              <CustodyPanel last>
                <span style={errorTextStyle}>{unbindError}</span>
              </CustodyPanel>
            )}
          </GroupCard>
        </>
      )}

      {state.workspaces.length > 0 && (
        <>
          <SectionLabel>WORKSPACES ON THIS MACHINE</SectionLabel>
          <GroupCard>
            {state.workspaces.map((w, i) => {
              const active = workspace?.id === w.id;
              return (
                <InfoRow
                  key={w.id}
                  label={w.name}
                  last={i === state.workspaces.length - 1}
                  value={
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                      <span style={monoValue}>{w.chainId}</span>
                      {active ? (
                        <span
                          style={{
                            font: `600 9px ${font.mono}`,
                            color: tint(color.green).text,
                            background: tint(color.green).bg,
                            border: `1px solid ${tint(color.green).border}`,
                            borderRadius: 4,
                            padding: "2px 6px",
                            letterSpacing: ".04em",
                          }}
                        >
                          ACTIVE
                        </span>
                      ) : (
                        <HoverButton
                          ariaLabel={`Open workspace ${w.name}`}
                          onClick={() => actions.selectWorkspace(w.id)}
                          hoverBg={color.titlebar}
                          style={outlineButton}
                        >
                          Open
                        </HoverButton>
                      )}
                    </span>
                  }
                />
              );
            })}
          </GroupCard>
        </>
      )}

      {pendingUnbind && (
        <ConfirmDialog
          title={`Unbind node ${shortKey(pendingUnbind)}?`}
          confirmLabel="Unbind node"
          onCancel={() => setPendingUnbind(null)}
          onConfirm={() => {
            const target = pendingUnbind;
            setPendingUnbind(null);
            actions
              .accountUnbindNode(target)
              .catch((err) => setUnbindError(errMessage(err)));
          }}
        >
          This is for a lost or retired device: the node keeps running, but it
          stops being yours — its writes no longer resolve to this account, and
          any captured bind certificates die with the nonce bump. Its valset
          seat (if any) is separate; retire that from the Members view.
        </ConfirmDialog>
      )}
    </>
  );
}
