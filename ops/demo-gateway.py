#!/usr/bin/env python3
"""Publish two gateway routes on the running demo node, to showcase both hosting
modes the gateway supports:

  • site  — a NETWORK-hosted static web app (RouteTarget::DuckFs): the html/css
            live in the node's DuckFS and are served, hash-verified, by consensus.
  • app   — a USER-hosted web app (RouteTarget::LoopbackHttp): the route proxies
            to a node-local loopback HTTP server the user runs themselves.

Publishing a route is an authenticated, member-signed ceremony (an account must
own the publisher node; the route statement is signed by an account member key).
This reuses the node's OWN signing CLIs (`user-sign-bind`, `user-sign-gateway-
route`) and the frameless /v1/submit lane, which stamps the node's validator key
as the op origin — so the local daemon publishes as itself, exactly as the app does.

Usage: demo-gateway.py <http-url> <node-bin> <workdir> <chain-id> [handle]
"""
import base64, hashlib, json, re, subprocess, sys, urllib.error, urllib.request

URL, NODE_BIN, WORKDIR, CHAIN = sys.argv[1:5]
HANDLE = sys.argv[5] if len(sys.argv) > 5 else "demo"
USER_KEY = f"{WORKDIR}/user.key"

# A self-contained bouncing-"DVD"-logo screensaver. Pure HTML+CSS (no JS, no
# external assets) so it renders under any CSP, and it's obviously ALIVE — proof
# the DuckFS route is really serving. The box travels at constant velocity and
# reflects off all four walls (two `alternate` keyframes at coprime periods),
# hue-cycling as it goes, just like the classic.
INDEX_HTML = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Ducktape DVD</title><style>
  :root{color-scheme:dark}
  html,body{margin:0;height:100%;background:#05070a;overflow:hidden}
  .stage{position:fixed;inset:0}
  .dvd{position:absolute;top:0;left:0;box-sizing:border-box;width:210px;height:70px;
       display:flex;align-items:center;justify-content:center;gap:.45rem;border-radius:16px;
       font:800 28px/1 system-ui,sans-serif;color:#fff;white-space:nowrap;user-select:none;
       background:radial-gradient(130% 150% at 28% 18%,#7c5cff,#00d4ff);
       box-shadow:0 0 26px rgba(120,150,255,.75),inset 0 0 0 2px rgba(255,255,255,.28);
       animation:mx 6.1s linear infinite alternate, my 8.3s linear infinite alternate, hue 5s linear infinite;
       will-change:left,top,filter}
  .dvd .duck{font-size:34px}
  .dvd small{font-weight:700;font-size:15px;opacity:.9;letter-spacing:.18em}
  @keyframes mx{from{left:0}to{left:calc(100vw - 210px)}}
  @keyframes my{from{top:0}to{top:calc(100vh - 70px)}}
  @keyframes hue{from{filter:hue-rotate(0deg)}to{filter:hue-rotate(360deg)}}
  .tag{position:fixed;left:0;right:0;bottom:16px;text-align:center;
       font:12px/1 system-ui,sans-serif;color:#59616f;letter-spacing:.09em}
  @media (prefers-reduced-motion:reduce){
    .dvd{animation:hue 5s linear infinite;left:calc(50vw - 105px);top:calc(50vh - 35px)}}
</style></head>
<body>
  <div class="stage"><div class="dvd"><span class="duck">\U0001F986</span>DUCK<small>DVD</small></div></div>
  <div class="tag">network-hosted &middot; served from DuckFS by consensus &middot; make demo-seed</div>
</body></html>
"""


def _post(path, body):
    req = urllib.request.Request(URL + path, data=json.dumps(body).encode(),
                                 headers={"content-type": "application/json"})
    try:
        return json.load(urllib.request.urlopen(req))
    except urllib.error.HTTPError as e:
        sys.exit(f"[gateway] POST {path} failed [{e.code}]: {e.read().decode()[:400]}")


def submit(target, payload): return _post("/v1/submit", {"target": target, "payload": payload})
def query(target, q):        return _post("/v1/query", {"target": target, "query": q})


def sign(cli_args):
    """Run a `user-sign-*` CLI and return its last stdout line (the ready JSON)."""
    out = subprocess.run([NODE_BIN, *cli_args], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"[gateway] {cli_args[0]} failed: {out.stderr.strip() or out.stdout.strip()}")
    return out.stdout.strip().splitlines()[-1]


# 1. the node's own validator key (the sole validator of this solo network).
node_bytes = query("valset", "validators")["validators"][0]
node_hex = bytes(node_bytes).hex()

# 2. bind an account to the node — creates the account (id = the user key) and
#    binds this node to it, the desktop's auto-bind path. The member signature
#    is minted by the node's user-key CLI (generated on first use, plaintext).
bind = json.loads(sign(["user-sign-bind", "--key", USER_KEY, "--chain-id", CHAIN,
                        "--node-pub", node_hex, "--nonce", "0"]))
account_id = bind["bind_node"]["authorizer"]["key"]   # list[int] — also the account id
submit("identity", bind)

# 2b. give the account a human handle so its routes get a resolvable .duck
#     address (site.<handle>.duck / app.<handle>.duck). Origin is the bound node,
#     so DuckDNS attaches the handle to this account. Skipped if the id isn't a
#     legal DNS label (DuckDNS handles are lowercase [a-z0-9-], not "net").
if re.fullmatch(r"[a-z0-9]([a-z0-9-]*[a-z0-9])?", HANDLE) and HANDLE != "net":
    submit("duckdns", {"set_handle": {"handle": HANDLE}})
else:
    HANDLE = None
    print(f"[gateway] skipped .duck handle (workspace id is not a legal DNS label)")

# 3. stage the static site into the node's DuckFS gateway root for route "site":
#    /home/ext:<node>/.duck/gateway/<route>/<file> — where serve_duckfs reads it.
site = {"index.html": ("text/html", INDEX_HTML)}
changes, content_files = [], []
for path in sorted(site):                     # DuckFS content must be path-sorted
    mime, text = site[path]
    data = text.encode()
    changes.append({"put": {
        "path": f"/home/ext:{node_hex}/.duck/gateway/site/{path}",
        "exec": False, "meta": {},
        "content": {"inline": {"b64": base64.b64encode(data).decode()}}}})
    content_files.append({"path": path, "mime": mime, "size": len(data),
                          "sha256": hashlib.sha256(data).hexdigest()})
_post("/v1/files/commit", {"base_snapshot": None, "message": "seed: gateway site",
                           "changes": changes})


def publish(statement):
    msg = json.loads(sign(["user-sign-gateway-route", "--key", USER_KEY,
                           "--statement", json.dumps(statement)]))
    submit("gateway", msg)


# 4. the network-hosted static route (DuckFs). Content routes are GET/HEAD only.
publish({"version": 1, "chain_id": CHAIN, "account_id": account_id,
         "name": {"label": "site"}, "publisher_node": node_bytes, "revision": 1,
         "route": {"target": {"kind": "duck_fs",
                              "content": {"default_path": "index.html", "files": content_files}},
                   "policy": {"audience": {"kind": "owner"}, "methods": ["get", "head"],
                              "max_request_bytes": 0, "max_response_bytes": 1 << 20,
                              "allow_authorization": False}}})

# 5. the user-hosted route (LoopbackHttp) — proxies to a node-local HTTP server
#    the user runs. Published here so it shows in the gateway UI; serving it
#    needs a loopback server running (that's the "user-hosted" part).
publish({"version": 1, "chain_id": CHAIN, "account_id": account_id,
         "name": {"label": "app"}, "publisher_node": node_bytes, "revision": 1,
         "route": {"target": {"kind": "loopback_http"},
                   "policy": {"audience": {"kind": "owner"},
                              "methods": ["get", "head", "post"],
                              "max_request_bytes": 1 << 20, "max_response_bytes": 1 << 20,
                              "allow_authorization": True}}})

routes = query("gateway", {"list": {"account_id": account_id}})
n = len(routes.get("routes", []))
if HANDLE:
    print(f"[gateway] published {n} routes on {HANDLE}.duck: "
          f"site.{HANDLE}.duck (DuckFS static), app.{HANDLE}.duck (loopback)")
else:
    print(f"[gateway] published {n} routes on account {bytes(account_id).hex()[:12]}… "
          f"(no .duck handle — reach them via the Gateway view)")
