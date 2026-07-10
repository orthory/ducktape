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

Usage: demo-gateway.py <http-url> <node-bin> <workdir> <chain-id>
"""
import base64, hashlib, json, subprocess, sys, urllib.error, urllib.request

URL, NODE_BIN, WORKDIR, CHAIN = sys.argv[1:5]
USER_KEY = f"{WORKDIR}/user.key"

INDEX_HTML = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Ducktape Demo Site</title><link rel="stylesheet" href="style.css"></head>
<body><main>
<h1>\U0001F986 Network-hosted web app</h1>
<p>This page is static content stored in the node's <strong>DuckFS</strong> and
served straight from consensus state — every byte is hash-verified against a
signed route. No server process, no origin host.</p>
<p>Published by <code>make demo-seed</code>.</p>
</main></body></html>
"""

STYLE_CSS = ("body{font:16px/1.6 system-ui,sans-serif;margin:0;min-height:100vh;"
             "display:grid;place-items:center;background:#0e1116;color:#e6edf3}"
             "main{max-width:34rem;padding:2rem}h1{margin:0 0 1rem;font-size:1.6rem}"
             "code{background:#1b2029;padding:.1em .35em;border-radius:4px}\n")


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

# 3. stage the static site into the node's DuckFS gateway root for route "site":
#    /home/ext:<node>/.duck/gateway/<route>/<file> — where serve_duckfs reads it.
site = {"index.html": ("text/html", INDEX_HTML), "style.css": ("text/css", STYLE_CSS)}
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
print(f"[gateway] account {bytes(account_id).hex()[:12]}… published "
      f"{len(routes.get('routes', []))} routes: site (DuckFS static), app (loopback)")
