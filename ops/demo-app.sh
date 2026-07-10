#!/usr/bin/env bash
# make demo-app — serve the "user-hosted" web app behind the demo's
# app.<id>.duck gateway route.
#
# demo-seed publishes a `loopback_http` route named "app"; the gateway proxies
# it to a node-local loopback port recorded in the workspace's
# gateway-routes.json. This maps that route to a port and runs a small live
# server on it — the "user-hosted" half of the demo (a plain process you own,
# vs. the network-hosted static site that lives in consensus).
#
# Foreground: Ctrl-C stops it. The demo node must be running (open the app on
# the "$ID" workspace) for app.<id>.duck to resolve to this server.
set -uo pipefail

ID="${DEMO_WORKSPACE_ID:-demo}"
WSDIR="$HOME/.ducktape/workspaces/$ID"
RJSON="$WSDIR/gateway-routes.json"

log(){ printf '\033[36m[demo-app]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-app] %s\033[0m\n' "$*" >&2; exit 1; }

command -v python3 >/dev/null || die "python3 is required"
[ -d "$WSDIR" ] || die "no '$ID' workspace — run 'make demo-seed' first"

# reuse the port already mapped for route "app", else allocate a fresh one
PORT="$(python3 - "$RJSON" <<'PY'
import json, os, socket, sys
path = sys.argv[1]; port = None
if os.path.exists(path):
    try:
        for r in json.load(open(path)).get("routes", []):
            if r.get("name", {}).get("label") == "app":
                port = r["port"]
    except Exception:
        pass
if not port:
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
print(port)
PY
)"

# record route "app" -> this loopback port for the node's gateway proxy
python3 - "$RJSON" "$PORT" <<'PY'
import json, sys
path, port = sys.argv[1], int(sys.argv[2])
json.dump({"version": 1, "routes": [{"name": {"label": "app"}, "port": port}]},
          open(path, "w"), indent=2)
PY

log "route app.$ID.duck -> 127.0.0.1:$PORT  (Ctrl-C to stop)"
log "open the app on the '$ID' workspace, then browse to app.$ID.duck"

# a small LIVE server — the timestamp ticks on reload, proving it's a running
# process, not static consensus bytes.
exec python3 - "$PORT" "$ID" <<'PY'
import sys
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
port = int(sys.argv[1]); wid = sys.argv[2]
PAGE = """<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>User-hosted app</title><style>
body{{font:16px/1.6 system-ui,sans-serif;margin:0;min-height:100vh;display:grid;
place-items:center;background:#0e1116;color:#e6edf3}}main{{max-width:34rem;padding:2rem}}
h1{{margin:0 0 1rem;font-size:1.6rem}}code{{background:#1b2029;padding:.1em .35em;border-radius:4px}}
</style></head><body><main>
<h1>\U0001F5A5️ User-hosted web app</h1>
<p>This page is served <strong>live by a process on your machine</strong> and reached
through the gateway's <code>loopback_http</code> route <code>app.{wid}.duck</code> —
contrast <code>site.{wid}.duck</code>, which is static bytes in consensus.</p>
<p>Server time: <strong>{now}</strong> — reload and it ticks, proof it's a live process.</p>
</main></body></html>"""
class H(BaseHTTPRequestHandler):
    def _send(self, body=b""):
        self.send_response(200)
        self.send_header("content-type", "text/html; charset=utf-8")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        if body:
            self.wfile.write(body)
    def do_GET(self):
        now = datetime.now(timezone.utc).strftime("%H:%M:%S UTC")
        self._send(PAGE.format(wid=wid, now=now).encode())
    def do_HEAD(self):
        self._send()
    def log_message(self, *a):
        pass
print(f"serving on 127.0.0.1:{port}")
HTTPServer(("127.0.0.1", port), H).serve_forever()
PY
