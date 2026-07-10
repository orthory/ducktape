// Build the token-routed VNC WebSocket URL. The console is served by the same
// fleet server that proxies VNC, so we derive host + scheme from where the browser
// actually reached us — that transparently handles the tailscale IP, a MagicDNS
// name, and a future TLS front, without threading host/port through the app.

export interface LocationLike {
  protocol: string;
  host: string;
}

export function wsUrlFromLocation(loc: LocationLike, token: string): string {
  const proto = loc.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${loc.host}/websockify?token=${encodeURIComponent(token)}`;
}
