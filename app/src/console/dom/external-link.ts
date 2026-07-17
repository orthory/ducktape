// Open an external link in the system browser.
//
// The static web twin opens links in a separate browser tab.
//
// Only http/https is honored — file:, javascript:, mailto: and friends are
// rejected outright so an untrusted href can never reach the OS opener.

export function openExternal(url: string): void {
  if (!/^https?:\/\//i.test(url)) return;
  window.open(url, "_blank", "noopener,noreferrer");
}
