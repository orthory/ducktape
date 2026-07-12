// The NEW device's half of the device-link ceremony (account-console spec
// §3): take the challenge the account's existing device minted — as the QR's
// http:// address (fetched over the LAN, link_relay.rs) or as a pasted blob —
// sign this machine's possession proof locally (no node connection needed,
// every input the preimage takes rides in the challenge), then deliver the
// response: posted straight back over the LAN on the address path, shown as a
// code to carry back on the paste path (and as the fallback when the
// post-back fails). The other half (minting the challenge, approving the
// response) lives in the Home layer's DevicesCard. Renders bare content: the
// caller owns the surrounding card.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { linkFetchChallenge, linkSendResponse } from "../../../domain/link-relay-client";
import { identityState } from "../../../domain/user-identity-client";
import { color, font } from "../../theme/tokens";
import { decodeLinkChallenge, encodeLinkResponse, isLinkUrl } from "../account/link-device";
import type { LinkChallenge } from "../account/link-device";
import {
  errMessage,
  errorTextStyle,
  inputStyle,
  linkButtonStyle,
  primaryButtonStyle,
  secondaryButtonStyle,
} from "./IdentityGateForms";

const blobStyle: React.CSSProperties = {
  ...inputStyle,
  resize: "vertical",
  font: `500 10.5px ${font.mono}`,
};

const hintStyle: React.CSSProperties = {
  font: `500 11px ${font.sans}`,
  color: color.muted,
  lineHeight: 1.5,
};

export function LinkDeviceFlow({
  onDone,
  doneLabel = "Continue",
}: {
  onDone: () => void;
  doneLabel?: string;
}) {
  const [challengeText, setChallengeText] = useState("");
  const [label, setLabel] = useState("");
  const [response, setResponse] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  const [accountName, setAccountName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  /** Sign this machine's possession over `challenge` and encode the response
   *  blob — shared by both input paths. */
  const signResponse = (challenge: LinkChallenge): Promise<string> =>
    Promise.resolve()
      .then(() => identityState())
      .then((report) => {
        if (!report.pubkey) throw new Error("this device has no account key yet");
        const pubkey = report.pubkey;
        return invoke<string>("user_sign_possession", {
          chainId: challenge.chainId,
          accountId: challenge.accountId,
          nonce: challenge.nonce,
        }).then((possession) => {
          setAccountName(challenge.name);
          return encodeLinkResponse({
            pubkey,
            kind: "ed25519",
            possession,
            label: label.trim() || null,
          });
        });
      });

  // The QR path: the input names the inviter's LAN relay — fetch the
  // challenge, sign, post the reply straight back. A reply that can't be
  // delivered (panel closed there, network changed) is not lost: it falls
  // back to the manual response screen with the reason inline.
  const runAddress = (url: string): Promise<void> =>
    Promise.resolve()
      .then(() => linkFetchChallenge(url))
      .then((blob) => {
        const challenge = decodeLinkChallenge(blob);
        if (!challenge) {
          throw new Error("the other device sent a malformed link code — update both apps and retry");
        }
        return signResponse(challenge);
      })
      .then((encoded) =>
        linkSendResponse(url, encoded).then(
          () => setSent(true),
          (err) => {
            setResponse(encoded);
            setError(
              `couldn't send the reply back automatically — paste it on your other device instead (${errMessage(err)})`,
            );
          },
        ),
      );

  const runPaste = (): Promise<void> =>
    Promise.resolve().then(() => {
      const challenge = decodeLinkChallenge(challengeText);
      if (!challenge) {
        throw new Error(
          "that doesn't look like a link code — paste the code (or type the http:// address) from your other device",
        );
      }
      return signResponse(challenge).then(setResponse);
    });

  const generate = () => {
    setBusy(true);
    setError(null);
    (isLinkUrl(challengeText) ? runAddress(challengeText.trim()) : runPaste())
      .catch((err) => {
        const message = errMessage(err);
        setError(
          message === "identity-locked"
            ? "this device's key is locked — unlock it with its password first, then retry"
            : message,
        );
      })
      .finally(() => setBusy(false));
  };

  const copy = () => {
    if (!response) return;
    void navigator.clipboard?.writeText(response).then(
      () => setCopied(true),
      () => {},
    );
  };

  if (sent) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <span style={hintStyle}>
          Reply sent{accountName ? ` to ${accountName}'s account` : ""} —
          approve the link on your other device. This device joins the account
          once that lands — you can continue in the meantime.
        </span>
        <button onClick={onDone} style={primaryButtonStyle(false)}>
          {doneLabel}
        </button>
      </div>
    );
  }

  if (response) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <span style={hintStyle}>
          Paste this on your other device
          {accountName ? ` (${accountName}'s account)` : ""} and approve the
          link there. This device joins the account once that lands — you can
          continue in the meantime.
        </span>
        <textarea
          readOnly
          aria-label="Link response code"
          value={response}
          rows={4}
          style={blobStyle}
        />
        {error && <span style={errorTextStyle}>{error}</span>}
        <button onClick={copy} style={secondaryButtonStyle}>
          {copied ? "Copied" : "Copy to clipboard"}
        </button>
        <button onClick={onDone} style={primaryButtonStyle(false)}>
          {doneLabel}
        </button>
      </div>
    );
  }

  const viaAddress = isLinkUrl(challengeText);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <textarea
        aria-label="Link challenge code"
        value={challengeText}
        onChange={(event) => {
          setChallengeText(event.target.value);
          setError(null);
        }}
        placeholder="Paste the link code from your other device — or type the http:// address shown under its QR"
        rows={4}
        style={blobStyle}
      />
      <input
        value={label}
        onChange={(event) => setLabel(event.target.value)}
        placeholder="Device label (optional, e.g. work laptop)"
        style={inputStyle}
      />
      {error && <span style={errorTextStyle}>{error}</span>}
      <button onClick={generate} disabled={busy} style={primaryButtonStyle(busy)}>
        {busy
          ? viaAddress
            ? "Linking…"
            : "Signing…"
          : viaAddress
            ? "Link over the network"
            : "Generate link code"}
      </button>
      {/* Linking and proceeding are independent (spec §1): a user without the
          other device at hand must never be trapped here. The link-pending
          flag stays set, auto-bind keeps deferring, and this same wizard
          re-opens from Account → Link this device. */}
      <button onClick={onDone} style={linkButtonStyle}>
        I'll finish this later
      </button>
    </div>
  );
}
