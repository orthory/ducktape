// The NEW device's half of the device-link ceremony (account-console spec
// §3): paste the challenge code the account's existing device minted, sign
// this machine's possession proof locally — no node connection needed, every
// input the preimage takes (chain id, account id, nonce) rides in the
// challenge — and show the response code to carry back. The other half
// (minting the challenge, approving the response) lives in the Account view's
// DeviceKeysCard. Renders bare content: the caller owns the surrounding card.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { identityState } from "../../../domain/user-identity-client";
import { color, font } from "../../theme/tokens";
import { decodeLinkChallenge, encodeLinkResponse } from "../account/link-device";
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
  const [accountName, setAccountName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const generate = () => {
    const challenge = decodeLinkChallenge(challengeText);
    if (!challenge) {
      setError("that doesn't look like a link code — paste the code from your other device");
      return;
    }
    setBusy(true);
    setError(null);
    identityState()
      .then((report) => {
        if (!report.pubkey) throw new Error("this device has no account key yet");
        const pubkey = report.pubkey;
        return invoke<string>("user_sign_possession", {
          chainId: challenge.chainId,
          accountId: challenge.accountId,
          nonce: challenge.nonce,
        }).then((possession) => {
          setAccountName(challenge.name);
          setResponse(
            encodeLinkResponse({
              pubkey,
              kind: "ed25519",
              possession,
              label: label.trim() || null,
            }),
          );
        });
      })
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
        <button onClick={copy} style={secondaryButtonStyle}>
          {copied ? "Copied" : "Copy to clipboard"}
        </button>
        <button onClick={onDone} style={primaryButtonStyle(false)}>
          {doneLabel}
        </button>
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <textarea
        aria-label="Link challenge code"
        value={challengeText}
        onChange={(event) => {
          setChallengeText(event.target.value);
          setError(null);
        }}
        placeholder="Paste the link code from your other device (ducktape-link-challenge-…)"
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
        {busy ? "Signing…" : "Generate link code"}
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
