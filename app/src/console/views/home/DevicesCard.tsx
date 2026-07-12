// Devices & keys — the account's collected member keys (any scheme), with the
// two halves of the device-link ceremony, the phone QR enrollment (a P-256
// key minted on a phone over the LAN — see src-tauri/src/enroll.rs), and
// member-key removal. Linked: list keys, mint/approve link codes (shown as a
// QR + LAN address via link_relay.rs, with the copy/paste blobs as the
// no-network fallback), enroll a phone, drop keys. Unlinked: offer the
// NEW-device side of the ceremony (LinkDeviceFlow) so a device that skipped
// linking during onboarding can still join an account from here.

import { useEffect, useState } from "react";
import { renderSVG } from "uqr";

import { keyHex } from "../../../domain/chat-client";
import { enrollCancel, enrollPoll } from "../../../domain/enroll-client";
import {
  linkRelayCancel,
  linkRelayPoll,
  linkRelayStart,
} from "../../../domain/link-relay-client";
import type { KeyKind, MemberKeyView } from "../../../domain/identity-client";
import { shortKey } from "../../../domain/names";
import {
  touchidAvailable,
  touchidDisable,
  touchidEnroll,
  touchidEnrolled,
} from "../../../domain/touchid-client";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import type { PhoneEnrollment } from "../../store/account-ops";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import {
  errMessage,
  errorTextStyle,
  inputStyle,
  PasswordForm,
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
import { decodeLinkResponse, encodeLinkChallenge } from "../account/link-device";
import type { LinkChallenge } from "../account/link-device";
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

/** The inviter side of the ceremony: show a freshly-minted challenge as a QR
 *  + LAN address (link_relay.rs) with the blob as fallback, take the new
 *  device's response — auto-filled by the relay or pasted — and approve. The
 *  challenge object is held in state — the possession proof inside the
 *  response is pinned to ITS nonce. */
function LinkInviterPanel({ onDone }: { onDone: () => void }) {
  const { actions } = useDucktape();
  const [challenge, setChallenge] = useState<LinkChallenge | null>(null);
  const [relayUrl, setRelayUrl] = useState<string | null>(null);
  const [response, setResponse] = useState("");
  const [delivered, setDelivered] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Mint once on mount — the panel unmounts on Cancel, so re-opening mints a
  // fresh challenge (and a fresh nonce) by construction. The relay rides the
  // same lifetime: down on unmount, never left listening.
  useEffect(() => {
    let alive = true;
    Promise.resolve()
      .then(() => actions.accountLinkChallenge())
      .then((minted) => {
        if (!alive) return;
        setChallenge(minted);
        // The QR/LAN relay is an enhancement over the paste path — a failed
        // bind (no LAN, sandboxing) degrades to blob-only, not to an error.
        return linkRelayStart(encodeLinkChallenge(minted)).then(
          (started) => {
            if (alive) setRelayUrl(started.url);
          },
          () => {},
        );
      })
      .catch((err) => {
        if (alive) setError(errMessage(err));
      });
    return () => {
      alive = false;
      void linkRelayCancel().catch(() => {});
    };
  }, [actions]);

  // Poll for the new device's reply while the QR is up — it lands in the same
  // response field a manual paste would fill. Best-effort: a missed tick just
  // means the next one picks it up. `delivered` latches polling off for good:
  // clearing the box to paste a different reply must not refill it.
  useEffect(() => {
    if (!relayUrl || delivered || response.length > 0) return;
    const timer = setInterval(() => {
      linkRelayPoll().then(
        (blob) => {
          if (blob) {
            setDelivered(true);
            setResponse(blob);
          }
        },
        () => {},
      );
    }, 1200);
    return () => clearInterval(timer);
  }, [relayUrl, delivered, response]);

  if (error && !challenge) {
    return <span style={errorTextStyle}>{error}</span>;
  }
  if (!challenge) {
    return <span style={hintStyle}>Minting a link code…</span>;
  }

  const encoded = encodeLinkChallenge(challenge);
  const parsedReply = decodeLinkResponse(response);
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
        Link this device)
        {relayUrl ? ", then type the address under this QR:" : " and paste this code:"}
      </span>
      {relayUrl && (
        <>
          {/* uqr emits pure path geometry, but an img data-URI keeps the SVG
              out of the document's DOM entirely — no innerHTML anywhere. */}
          <img
            alt="Link QR code"
            width={160}
            height={160}
            style={{ alignSelf: "center", background: "#fff", padding: 8, borderRadius: 8 }}
            src={`data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(relayUrl))}`}
          />
          <span
            aria-label="Link address"
            style={{ ...monoValue, maxWidth: "100%", textAlign: "center" }}
            title={relayUrl}
          >
            {relayUrl}
          </span>
          <span style={{ ...hintStyle, marginBottom: 0 }}>
            …or paste this code there instead:
          </span>
        </>
      )}
      <textarea readOnly aria-label="Link challenge code" value={encoded} rows={3} style={blobStyle} />
      <button onClick={copy} style={secondaryButtonStyle}>
        {copied ? "Copied" : "Copy to clipboard"}
      </button>
      <span style={{ ...hintStyle, marginBottom: 0 }}>
        2 ·{" "}
        {relayUrl
          ? "The reply arrives here automatically over your network — or paste it, then approve:"
          : "Paste the reply code it generates, then approve:"}
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
      {/* The human check the ceremony's security rests on: the approval must
          be of THIS key, verified against the fingerprint the new device
          shows — a LAN interloper's reply won't match it. */}
      {parsedReply && (
        <>
          <span style={{ ...hintStyle, marginBottom: 0 }}>
            Approve only if the new device shows this same key:
          </span>
          <span style={{ ...monoValue, maxWidth: "100%" }} title={parsedReply.pubkey}>
            {KIND_LABEL[parsedReply.kind]} · {shortKey(parsedReply.pubkey)}
            {parsedReply.label ? ` · ${parsedReply.label}` : ""}
          </span>
        </>
      )}
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

/** The phone QR enrollment panel: stand the LAN server up on mount (tear it
 *  down on unmount — the server's lifetime is exactly this panel's), render
 *  the QR, poll for the phone's candidate key, and let the user approve it.
 *  The desktop stays the authority: nothing lands until Approve signs the
 *  add-member authorizer, and the possession is pinned to the nonce the QR
 *  was minted at (approve refuses on drift). */
function PhoneEnrollPanel({ onDone }: { onDone: () => void }) {
  const { actions } = useDucktape();
  const [enrollment, setEnrollment] = useState<PhoneEnrollment | null>(null);
  const [candidate, setCandidate] = useState<{ newKey: string; sig: string } | null>(null);
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Start on mount; cancel on unmount — leaving the panel (or the screen)
  // must never leave the LAN listener up.
  useEffect(() => {
    let alive = true;
    actions
      .accountPhoneEnrollStart()
      .then((started) => {
        if (alive) setEnrollment(started);
      })
      .catch((err) => {
        if (alive) setError(errMessage(err));
      });
    return () => {
      alive = false;
      void enrollCancel().catch(() => {});
    };
  }, [actions]);

  // Poll for the phone's proof while the QR is up. Best-effort: a missed
  // tick just means the next one picks it up.
  useEffect(() => {
    if (!enrollment || candidate) return;
    const timer = setInterval(() => {
      enrollPoll().then(
        (result) => {
          if (result) setCandidate({ newKey: result[0], sig: result[1] });
        },
        () => {},
      );
    }, 1200);
    return () => clearInterval(timer);
  }, [enrollment, candidate]);

  if (error && !enrollment) {
    return <span style={errorTextStyle}>{error}</span>;
  }
  if (!enrollment) {
    return <span style={hintStyle}>Starting the enrollment…</span>;
  }

  if (candidate) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <span style={{ ...hintStyle, marginBottom: 0 }}>
          Your phone created a key. Approving adds it to your account:
        </span>
        <span style={{ ...monoValue, maxWidth: "100%" }} title={candidate.newKey}>
          Security key · {shortKey(candidate.newKey)}
        </span>
        <input
          value={label}
          onChange={(event) => setLabel(event.target.value)}
          placeholder="Key label (optional, e.g. my phone)"
          style={inputStyle}
        />
        {error && <span style={errorTextStyle}>{error}</span>}
        <button
          onClick={() => {
            setBusy(true);
            setError(null);
            actions
              .accountPhoneEnrollApprove(
                enrollment,
                candidate.newKey,
                candidate.sig,
                label.trim() || null,
              )
              .then(() => enrollCancel().catch(() => {}))
              .then(onDone)
              .catch((err) => setError(errMessage(err)))
              .finally(() => setBusy(false));
          }}
          disabled={busy}
          style={primaryButtonStyle(busy)}
        >
          {busy ? "Approving…" : "Approve key"}
        </button>
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <span style={{ ...hintStyle, marginBottom: 0 }}>
        Scan with your phone on the same Wi-Fi. The key is generated on the
        phone and nothing leaves your network — you approve it here.
      </span>
      {/* uqr emits pure path geometry, but an img data-URI keeps the SVG out
          of the document's DOM entirely — no innerHTML anywhere. */}
      <img
        alt="Enrollment QR code"
        width={190}
        height={190}
        style={{ alignSelf: "center", background: "#fff", padding: 8, borderRadius: 8 }}
        src={`data:image/svg+xml;utf8,${encodeURIComponent(renderSVG(enrollment.url))}`}
      />
      <span
        aria-label="Enrollment URL"
        style={{ ...monoValue, maxWidth: "100%", textAlign: "center" }}
        title={enrollment.url}
      >
        {enrollment.url}
      </span>
      <span style={{ ...hintStyle, marginBottom: 0 }}>Waiting for the phone…</span>
    </div>
  );
}

export function DevicesCard({
  accountId,
  identity,
}: {
  accountId: string | undefined;
  identity: IdentityStateReport | null;
}) {
  const { state, actions } = useDucktape();
  const [panel, setPanel] = useState<"none" | "invite" | "phone" | "linkSelf">("none");
  const [pendingRemove, setPendingRemove] = useState<MemberKeyView | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);

  // Touch ID is machine-scoped (a Keychain item on THIS Mac), not part of the
  // account's member keys — its own probe + enable/disable, gated on macOS
  // availability. `touchidEnroll` needs the vault passphrase, so Enable opens a
  // password confirm; a Touch-ID-created account is already enrolled at signup.
  const [tidOk, setTidOk] = useState(false);
  const [tidOn, setTidOn] = useState(false);
  const [tidEnable, setTidEnable] = useState(false);
  const [tidBusy, setTidBusy] = useState(false);
  const [tidError, setTidError] = useState<string | null>(null);
  const [tidConfirmDisable, setTidConfirmDisable] = useState(false);
  useEffect(() => {
    touchidAvailable().then(setTidOk).catch(() => setTidOk(false));
    touchidEnrolled().then(setTidOn).catch(() => setTidOn(false));
  }, []);

  const members = accountId ? (state.accountKeys[accountId] ?? []) : [];
  const devicePubkey = identity?.pubkey?.toLowerCase();

  const touchidSection = tidOk ? (
    <>
      <SectionLabel>THIS DEVICE</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Touch ID"
          desc={
            tidOn
              ? "Unlock your account on this Mac with Touch ID."
              : "Unlock your account on this Mac with Touch ID instead of your password."
          }
          last={!tidEnable && !tidError}
          control={
            tidOn ? (
              <HoverButton
                ariaLabel="Disable Touch ID"
                onClick={() => setTidConfirmDisable(true)}
                hoverBg={color.dangerSoft}
                style={{ ...outlineButton, color: color.red }}
              >
                Disable Touch ID
              </HoverButton>
            ) : tidEnable ? (
              <HoverButton
                ariaLabel="Cancel enable Touch ID"
                onClick={() => {
                  setTidEnable(false);
                  setTidError(null);
                }}
                hoverBg={color.titlebar}
                style={outlineButton}
              >
                Cancel
              </HoverButton>
            ) : (
              <HoverButton
                onClick={() => {
                  setTidError(null);
                  setTidEnable(true);
                }}
                hoverBg={color.titlebar}
                style={outlineButton}
              >
                Enable Touch ID
              </HoverButton>
            )
          }
        />
        {tidEnable && !tidOn && (
          <CustodyPanel last={!tidError}>
            <PasswordForm
              mode="confirm"
              busy={tidBusy}
              error={tidError}
              submitLabel={tidBusy ? "Enabling…" : "Enable Touch ID"}
              onSubmit={(password) => {
                setTidBusy(true);
                setTidError(null);
                touchidEnroll(password)
                  .then(() => {
                    setTidEnable(false);
                    setTidOn(true);
                  })
                  .catch((err) => setTidError(errMessage(err)))
                  .finally(() => setTidBusy(false));
              }}
            />
          </CustodyPanel>
        )}
        {tidError && !tidEnable && (
          <CustodyPanel last>
            <span style={errorTextStyle}>{tidError}</span>
          </CustodyPanel>
        )}
      </GroupCard>
      {tidConfirmDisable && (
        <ConfirmDialog
          title="Disable Touch ID on this device?"
          confirmLabel="Disable Touch ID"
          onCancel={() => setTidConfirmDisable(false)}
          onConfirm={() => {
            setTidConfirmDisable(false);
            touchidDisable()
              .then(() => setTidOn(false))
              .catch((err) => setTidError(errMessage(err)));
          }}
        >
          Your account is unaffected — you&apos;ll unlock with your recovery
          phrase or password instead. You can re-enable Touch ID here anytime.
        </ConfirmDialog>
      )}
    </>
  ) : null;

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
        {touchidSection}
      </>
    );
  }

  return (
    <>
      {touchidSection}
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
                      hoverBg={color.dangerSoft}
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
          <CustodyPanel>
            <LinkInviterPanel onDone={() => setPanel("none")} />
          </CustodyPanel>
        )}
        <ControlRow
          title="Add a key from your phone"
          desc="Scan a QR over the LAN — the phone mints a security key you approve here."
          last={panel !== "phone" && !removeError}
          control={
            <HoverButton
              onClick={() => setPanel(panel === "phone" ? "none" : "phone")}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              {panel === "phone" ? "Cancel" : "Show QR"}
            </HoverButton>
          }
        />
        {panel === "phone" && (
          <CustodyPanel last={!removeError}>
            <PhoneEnrollPanel onDone={() => setPanel("none")} />
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
