// Devices & keys — the account's collected member keys (any scheme), with the
// two halves of the device-link ceremony and member-key removal. Linked: list
// keys, mint/approve link codes (the inviter side), drop keys. Unlinked: offer
// the NEW-device side of the ceremony (LinkDeviceFlow) so a device that
// skipped linking during onboarding can still join an account from here.

import { useEffect, useState } from "react";

import { keyHex } from "../../../domain/chat-client";
import type { KeyKind, MemberKeyView } from "../../../domain/identity-client";
import { shortKey } from "../../../domain/names";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import {
  errMessage,
  errorTextStyle,
  inputStyle,
  primaryButtonStyle,
  secondaryButtonStyle,
} from "../onboarding/IdentityGateForms";
import { LinkDeviceFlow } from "../onboarding/LinkDeviceFlow";
import {
  ControlRow,
  GroupCard,
  HoverButton,
  InfoRow,
  monoValue,
  outlineButton,
  SectionLabel,
} from "../settings/parts";
import { encodeLinkChallenge } from "./link-device";
import type { LinkChallenge } from "./link-device";
import { CustodyPanel } from "./CustodyCard";

/** Human label for a member key's scheme. */
export const KIND_LABEL: Record<KeyKind, string> = {
  ed25519: "Seed key",
  p256: "Security key",
  webauthn_p256: "Passkey",
};

const blobStyle: React.CSSProperties = {
  ...inputStyle,
  resize: "vertical",
  font: `500 10.5px ${font.mono}`,
};

const hintStyle: React.CSSProperties = {
  font: `500 11px ${font.sans}`,
  color: color.muted,
  lineHeight: 1.5,
  display: "block",
  marginBottom: 8,
};

/** The inviter side of the ceremony: show a freshly-minted challenge, take
 *  the new device's response, approve. The challenge object is held in state —
 *  the possession proof inside the response is pinned to ITS nonce. */
function LinkInviterPanel({ onDone }: { onDone: () => void }) {
  const { actions } = useDucktape();
  const [challenge, setChallenge] = useState<LinkChallenge | null>(null);
  const [response, setResponse] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Mint once on mount — the panel unmounts on Cancel, so re-opening mints a
  // fresh challenge (and a fresh nonce) by construction.
  useEffect(() => {
    let alive = true;
    actions
      .accountLinkChallenge()
      .then((minted) => {
        if (alive) setChallenge(minted);
      })
      .catch((err) => {
        if (alive) setError(errMessage(err));
      });
    return () => {
      alive = false;
    };
  }, [actions]);

  if (error && !challenge) {
    return <span style={errorTextStyle}>{error}</span>;
  }
  if (!challenge) {
    return <span style={hintStyle}>Minting a link code…</span>;
  }

  const encoded = encodeLinkChallenge(challenge);
  const copy = () => {
    void navigator.clipboard?.writeText(encoded).then(
      () => setCopied(true),
      () => {},
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <span style={{ ...hintStyle, marginBottom: 0 }}>
        1 · On the new device, choose “Link device” during setup (or Account →
        Link this device) and paste this code:
      </span>
      <textarea readOnly aria-label="Link challenge code" value={encoded} rows={3} style={blobStyle} />
      <button onClick={copy} style={secondaryButtonStyle}>
        {copied ? "Copied" : "Copy to clipboard"}
      </button>
      <span style={{ ...hintStyle, marginBottom: 0 }}>
        2 · Paste the reply code it generates, then approve:
      </span>
      <textarea
        aria-label="Link response code"
        value={response}
        onChange={(event) => {
          setResponse(event.target.value);
          setError(null);
        }}
        placeholder="ducktape-link-response-…"
        rows={3}
        style={blobStyle}
      />
      {error && <span style={errorTextStyle}>{error}</span>}
      <button
        onClick={() => {
          setBusy(true);
          setError(null);
          actions
            .accountAddMember(challenge, response)
            .then(onDone)
            .catch((err) => setError(errMessage(err)))
            .finally(() => setBusy(false));
        }}
        disabled={busy || response.trim().length === 0}
        style={primaryButtonStyle(busy || response.trim().length === 0)}
      >
        {busy ? "Approving…" : "Approve link"}
      </button>
    </div>
  );
}

export function DeviceKeysCard({
  accountId,
  identity,
}: {
  accountId: string | undefined;
  identity: IdentityStateReport | null;
}) {
  const { state, actions } = useDucktape();
  const [panel, setPanel] = useState<"none" | "invite" | "linkSelf">("none");
  const [pendingRemove, setPendingRemove] = useState<MemberKeyView | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);

  const members = accountId ? (state.accountKeys[accountId] ?? []) : [];
  const devicePubkey = identity?.pubkey?.toLowerCase();

  if (!accountId) {
    // Unlinked device: offer the NEW-device half of the ceremony (needs a
    // local account key to sign possession with — the gate creates one).
    if (!identity || identity.state === "absent") return null;
    return (
      <>
        <SectionLabel>DEVICES &amp; KEYS</SectionLabel>
        <GroupCard>
          <ControlRow
            title="Link this device"
            desc="This device isn't part of an account yet. Link it to your account on another device."
            last={panel !== "linkSelf"}
            control={
              <HoverButton
                onClick={() => setPanel(panel === "linkSelf" ? "none" : "linkSelf")}
                hoverBg={color.titlebar}
                style={outlineButton}
              >
                {panel === "linkSelf" ? "Cancel" : "Link"}
              </HoverButton>
            }
          />
          {panel === "linkSelf" && (
            <CustodyPanel last>
              <LinkDeviceFlow onDone={() => setPanel("none")} doneLabel="Done" />
            </CustodyPanel>
          )}
        </GroupCard>
      </>
    );
  }

  return (
    <>
      <SectionLabel>DEVICES &amp; KEYS</SectionLabel>
      <GroupCard>
        {members.map((member) => {
          const hex = keyHex(member.pubkey).toLowerCase();
          const isThisDevice = devicePubkey !== undefined && hex === devicePubkey;
          return (
            <InfoRow
              key={hex}
              label={member.label ?? KIND_LABEL[member.kind]}
              value={
                <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                  <span style={monoValue}>
                    {KIND_LABEL[member.kind]} · {shortKey(hex)}
                    {isThisDevice ? " · this device" : ""}
                  </span>
                  {members.length > 1 && (
                    <HoverButton
                      ariaLabel={`Remove key ${shortKey(hex)}`}
                      onClick={() => {
                        setRemoveError(null);
                        setPendingRemove(member);
                      }}
                      hoverBg="#fbeeec"
                      style={{ ...outlineButton, color: color.red }}
                    >
                      Remove
                    </HoverButton>
                  )}
                </span>
              }
            />
          );
        })}
        {members.length === 0 && (
          <InfoRow
            label="Member keys"
            value={<span style={monoValue}>loading…</span>}
          />
        )}
        <ControlRow
          title="Link a device"
          desc="Bring another machine into this account — it signs as you, with its own key."
          last={panel !== "invite" && !removeError}
          control={
            <HoverButton
              onClick={() => setPanel(panel === "invite" ? "none" : "invite")}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              {panel === "invite" ? "Cancel" : "Start"}
            </HoverButton>
          }
        />
        {panel === "invite" && (
          <CustodyPanel last={!removeError}>
            <LinkInviterPanel onDone={() => setPanel("none")} />
          </CustodyPanel>
        )}
        {removeError && (
          <CustodyPanel last>
            <span style={errorTextStyle}>{removeError}</span>
          </CustodyPanel>
        )}
      </GroupCard>
      {pendingRemove && (
        <ConfirmDialog
          title="Remove this key from your account?"
          confirmLabel="Remove key"
          onCancel={() => setPendingRemove(null)}
          onConfirm={() => {
            const target = keyHex(pendingRemove.pubkey);
            setPendingRemove(null);
            actions
              .accountRemoveMember(target)
              .catch((err) => setRemoveError(errMessage(err)));
          }}
        >
          The device holding it can no longer sign as this account. The
          account itself survives — its other keys keep working. (The last
          remaining key can never be removed.)
        </ConfirmDialog>
      )}
    </>
  );
}
