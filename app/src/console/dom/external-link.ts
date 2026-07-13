// Open an external link in the system browser.
//
// The embedded engines this app ships in (CEF on Linux, WKWebView on macOS)
// have no browser chrome, so a plain `target="_blank"` anchor navigates the
// webview itself (or does nothing) instead of handing the URL to the OS. The
// desktop shell routes through the opener plugin (capability-scoped to open-url
// only); the web build falls back to a real new tab.
//
// Only http/https is honored — file:, javascript:, mailto: and friends are
// rejected outright so an untrusted href can never reach the OS opener.

import { isTauri } from "../../domain/node-bootstrap";

export function openExternal(url: string): void {
  if (!/^https?:\/\//i.test(url)) return;
  if (isTauri()) {
    void import("@tauri-apps/plugin-opener")
      .then(({ openUrl }) => openUrl(url))
      .catch(() => {});
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
