// Nodes the account owns on the CONNECTED network (identity module), with each
// node's valset standing and the lost-device Unbind affordance — the only UI
// consumer of `user_sign_unbind`. Account data is chain-scoped, so this only
// knows the active workspace's chain and renders nothing when disconnected. The
// "workspaces on this machine" list that used to sit beside this now lives in
// WorkspacesTable, so this card is network-nodes only.

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

export function NetworkNodesCard({ accountId }: { accountId: string | undefined }) {
  const { state, actions } = useDucktape();
  const [pendingUnbind, setPendingUnbind] = useState<string | null>(null);
  const [unbindError, setUnbindError] = useState<string | null>(null);

  // Nothing to show until an account is resolved on the connected network.
  if (!accountId) return null;

  const workspace = state.workspace;
  const validators = new Set(state.members.map(normalizeKey));
  const residents = new Set(state.residents.map(normalizeKey));

  const networkNodes = Object.entries(state.nodeUsers)
    .filter(([, owner]) => owner.accountId === accountId)
    .map(([nodeHex]) => nodeHex)
    .sort();

  const standingOf = (nodeHex: string): "Validator" | "Resident" | "No seat" =>
    validators.has(normalizeKey(nodeHex))
      ? "Validator"
      : residents.has(normalizeKey(nodeHex))
        ? "Resident"
        : "No seat";

  return (
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

      {pendingUnbind && (
        <ConfirmDialog
          title={`Unbind node ${shortKey(pendingUnbind)}?`}
          confirmLabel="Unbind node"
          onCancel={() => setPendingUnbind(null)}
          onConfirm={() => {
            const target = pendingUnbind;
            setPendingUnbind(null);
            actions.accountUnbindNode(target).catch((err) => setUnbindError(errMessage(err)));
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
