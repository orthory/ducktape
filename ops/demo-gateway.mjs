#!/usr/bin/env bun
// Publish two authenticated gateway routes on the running demo node:
//   site — network-hosted static bytes in DuckFS
//   app  — user-hosted loopback HTTP

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { join } from "node:path";

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
  const [url, nodeBin, workdir, chain, requestedHandle = "demo"] = Bun.argv.slice(2);
  if (!url || !nodeBin || !workdir || !chain) {
    throw new Error("usage: demo-gateway.mjs <http-url> <node-bin> <workdir> <chain-id> [handle]");
  }
  const userKey = join(workdir, "user.key");

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
    const result = spawnSync(nodeBin, args, { encoding: "utf8" });
    if (result.status !== 0) {
      const detail = (result.stderr || result.stdout || result.error?.message || "unknown error").trim();
      throw new Error(`[gateway] ${args[0]} failed: ${detail}`);
    }
    return result.stdout.trim().split(/\r?\n/).at(-1);
  }

  const nodeBytes = (await query("valset", "validators")).validators[0];
  const nodeHex = Buffer.from(nodeBytes).toString("hex");

  const bind = JSON.parse(sign([
    "user-sign-bind",
    "--key", userKey,
    "--chain-id", chain,
    "--node-pub", nodeHex,
    "--nonce", "0",
  ]));
  const accountId = bind.bind_node.authorizer.key;
  await submit("identity", bind);

  let handle = requestedHandle;
  if (/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(handle) && handle !== "net") {
    await submit("duckdns", { set_handle: { handle } });
  } else {
    handle = null;
    console.log("[gateway] skipped .duck handle (workspace id is not a legal DNS label)");
  }

  const data = Buffer.from(INDEX_HTML);
  // v2: the file table lives off consensus in .manifest.json; the signed route
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
      "user-sign-gateway-route",
      "--key", userKey,
      "--statement", JSON.stringify(statement),
    ]));
    await submit("gateway", message);
  }

  await publish({
    version: 1,
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
  });

  await publish({
    version: 1,
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

  const routes = await query("gateway", { list: { account_id: accountId } });
  const count = routes.routes?.length ?? 0;
  if (handle) {
    console.log(`[gateway] published ${count} routes on ${handle}.duck: site.${handle}.duck (DuckFS static), app.${handle}.duck (loopback)`);
  } else {
    console.log(`[gateway] published ${count} routes on account ${Buffer.from(accountId).toString("hex").slice(0, 12)}… (no .duck handle — reach them via the Gateway view)`);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
