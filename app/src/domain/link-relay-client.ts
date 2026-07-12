// Thin client over the desktop's device-link LAN relay commands
// (link_relay.rs) — the QR path of the link-device ceremony.
//
// Inviter half: linkRelayStart binds an ephemeral, token-gated LAN server
// serving a freshly-minted challenge blob and returns the URL to render as a
// QR / short address; linkRelayPoll returns the new device's response blob
// once it posts one; linkRelayCancel tears the server down. New-device half:
// linkFetchChallenge/linkSendResponse run the exchange against a typed link
// address (the shell does the HTTP — no webview cross-origin surface). All
// desktop-only (Tauri invoke).

import { invoke } from "@tauri-apps/api/core";

export interface LinkRelayStart {
  url: string;
}

/** Serve `challengeBlob` over the LAN; returns the QR / short-address URL. */
export const linkRelayStart = (challengeBlob: string): Promise<LinkRelayStart> =>
  invoke<LinkRelayStart>("link_relay_start", { challenge: challengeBlob });

/** The new device's response blob once it has posted one, else null. */
export const linkRelayPoll = (): Promise<string | null> =>
  invoke<string | null>("link_relay_poll");

/** Tear the relay down (on approve, cancel, or leaving the panel). */
export const linkRelayCancel = (): Promise<void> => invoke<void>("link_relay_cancel");

/** NEW device: fetch the inviter's challenge blob from a link address. */
export const linkFetchChallenge = (url: string): Promise<string> =>
  invoke<string>("link_fetch_challenge", { url });

/** NEW device: post the signed response blob back to the inviter's relay. */
export const linkSendResponse = (url: string, responseBlob: string): Promise<void> =>
  invoke<void>("link_send_response", { url, response: responseBlob });
