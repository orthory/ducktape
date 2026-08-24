#!/usr/bin/env bun
// Publish two authenticated gateway routes on the running demo node:
//   site — network-hosted static bytes in DuckFS
//   app  — user-hosted loopback HTTP

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";

// Consensus rejects these handles — a seed that asks for one aborts the demo.
// Mirrors RESERVED_ROOT_LABELS in crates/duckdns/src/wire.rs (the source
// of truth); app/src/domain/duckdns-client.test.ts pins all three copies.
const RESERVED_ROOT_LABELS = ["net", "agents"];

const INDEX_HTML = `<!doctype html>
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
  <div class="stage"><div class="dvd"><span class="duck">🦆</span>DUCK<small>DVD</small></div></div>
  <div class="tag">network-hosted &middot; served from DuckFS by consensus &middot; make demo-seed</div>
</body></html>
`;

async function main() {
  const [url, nodeBin, workdir, chain, requestedHandle = "demo", userKeyArg, password = ""] = Bun.argv.slice(2);
  if (!url || !nodeBin || !workdir || !chain) {
    throw new Error("usage: demo-gateway.mjs <http-url> <node-bin> <workdir> <chain-id> [handle] <user-key> [password]");
  }
  // The signing key is the local user identity demo-seed provisioned
  // (encrypted) — its password unlocks each sign-* call over stdin.
  if (!userKeyArg) {
    throw new Error("usage: demo-gateway.mjs <http-url> <node-bin> <workdir> <chain-id> [handle] <user-key> [password]");
  }
  const userKey = userKeyArg;

  async function post(path, body) {
    const response = await fetch(`${url}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    const text = await response.text();
    if (!response.ok) throw new Error(`[gateway] POST ${path} failed [${response.status}]: ${text.slice(0, 400)}`);
    return JSON.parse(text);
  }

  const submit = (target, payload) => post("/v1/submit", { target, payload });
  const query = (target, query) => post("/v1/query", { target, query });
  function sign(args) {
    // every `user sign-*` verb unlocks the encrypted key by reading its
    // password as the first (only) stdin line — see load_user_signer.
    const result = spawnSync(nodeBin, args, { encoding: "utf8", input: `${password}\n` });
    if (result.status !== 0) {
      const detail = (result.stderr || result.stdout || result.error?.message || "unknown error").trim();
      throw new Error(`[gateway] ${args[0]} failed: ${detail}`);
    }
    return result.stdout.trim().split(/\r?\n/).at(-1);
  }

  const nodeBytes = (await query("valset", "validators")).validators[0];
  const nodeHex = Buffer.from(nodeBytes).toString("hex");

  const bind = JSON.parse(sign([
    "user",
    "sign-bind",
    "--key", userKey,
    "--chain-id", chain,
    "--node-pub", nodeHex,
    "--nonce", "0",
  ]));
  const accountId = bind.bind_node.authorizer.key;
  try {
    await submit("identity", bind);
  } catch (error) {
    // Same wire-drift family as the gateway component: a module rejecting a
    // map where its (stale or changed) type wants a sequence. Route
    // publishing is a demo garnish — skip it loudly, never die over it.
    if (!error.message.includes("invalid type: map, expected a sequence")) throw error;
    console.error("[gateway] SKIPPED all routes: identity bind rejected with a map-vs-sequence wire drift — see the gateway-component regen note below");
    process.exitCode = 78;
    return;
  }

  let handle = requestedHandle;
  if (/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(handle) && !RESERVED_ROOT_LABELS.includes(handle)) {
    await submit("gateway", { set_handle: { handle } });
  } else {
    handle = null;
    console.log("[gateway] skipped .duck handle (workspace id is not a legal DNS label)");
  }

  const data = Buffer.from(INDEX_HTML);
  // The file table lives off consensus in .manifest.json; the signed route
  // binds only the manifest's SHA-256.
  const manifest = {
    default_path: "index.html",
    files: [{
      path: "index.html",
      mime: "text/html",
      size: data.byteLength,
      sha256: createHash("sha256").update(data).digest("hex"),
    }],
  };
  const manifestBytes = Buffer.from(JSON.stringify(manifest));
  const manifestSha256 = createHash("sha256").update(manifestBytes).digest("hex");
  const put = (path, bytes) => ({
    put: {
      path: `/home/ext:${nodeHex}/.duck/gateway/site/${path}`,
      exec: false,
      meta: {},
      content: { inline: { b64: bytes.toString("base64") } },
    },
  });
  await post("/v1/files/commit", {
    base_snapshot: null,
    message: "seed: gateway site + manifest",
    changes: [put("index.html", data), put(".manifest.json", manifestBytes)],
  });

  async function publish(statement) {
    const message = JSON.parse(sign([
      "user",
      "sign-gateway-route",
      "--key", userKey,
      "--statement", JSON.stringify(statement),
    ]));
    await submit("gateway", message);
  }

  const site = {
    chain_id: chain,
    account_id: accountId,
    name: { label: "site" },
    publisher_node: nodeBytes,
    revision: 1,
    route: {
      target: { kind: "duck_fs", manifest_sha256: manifestSha256 },
      policy: {
        audience: { kind: "owner" },
        methods: ["get", "head"],
        max_request_bytes: 0,
        max_response_bytes: 1 << 20,
        allow_authorization: false,
        allow_upgrade: false,
      },
    },
  };
  // A stale embedded gateway component is a build error, not a condition to
  // survive: a publish rejection fails the demo loudly.
  await publish(site);

  await publish({
    chain_id: chain,
    account_id: accountId,
    name: { label: "app" },
    publisher_node: nodeBytes,
    revision: 1,
    route: {
      target: { kind: "loopback_http" },
      policy: {
        audience: { kind: "owner" },
        methods: ["get", "head", "post"],
        max_request_bytes: 1 << 20,
        max_response_bytes: 1 << 20,
        allow_authorization: true,
        allow_upgrade: true,
      },
    },
  });

  // The reference app (spec §8): board.<handle>.duck — served by
  // ops/demo-kanban.mjs, reachable by any admitted member, WebSocket-realtime.
  await publish({
    chain_id: chain,
    account_id: accountId,
    name: { label: "board" },
    publisher_node: nodeBytes,
    revision: 1,
    route: {
      target: { kind: "loopback_http" },
      policy: {
        audience: { kind: "network" },
        methods: ["get", "head", "post"],
        max_request_bytes: 1 << 20,
        max_response_bytes: 1 << 20,
        allow_authorization: false,
        allow_upgrade: true,
      },
    },
  });

  const routes = await query("gateway", { list: { account_id: accountId } });
  const count = routes.routes?.length ?? 0;
  if (handle) {
    console.log(`[gateway] published ${count} routes on ${handle}.duck: site.${handle}.duck (static), app.${handle}.duck (loopback), board.${handle}.duck (kanban, WS)`);
  } else {
    console.log(`[gateway] published ${count} routes on account ${Buffer.from(accountId).toString("hex").slice(0, 12)}… (no .duck handle — reach them via the Gateway view)`);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
