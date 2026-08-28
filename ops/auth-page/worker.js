// The result relay. A phone that scanned the app's QR runs the ceremony on
// this origin and POSTs the result to /r/<id>; the app, which the phone cannot
// reach, polls the same path. KV holds a result for five minutes and a GET
// hands it out exactly once. Every other path is the static page.
//
// Contract pin: README.md §Relay. Tests: test.mjs (a Map stands in for KV).
const ID = /^\/r\/([A-Za-z0-9_-]{43})$/;
const TTL_SECONDS = 300;
const MAX_BODY = 16 * 1024;

const DONE_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>ducktape</title>
<style>
  :root { color-scheme: light dark; --fg: #1b1b1f; --bg: #f3f3f6; --card: #ffffff; --muted: #6b6b76; --line: #e2e2e8; }
  @media (prefers-color-scheme: dark) { :root { --fg: #ececf1; --bg: #111114; --card: #1b1b20; --muted: #9a9aa6; --line: #2a2a33; } }
  body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: var(--bg); color: var(--fg); font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
  main { width: min(26rem, calc(100vw - 2rem)); background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 2rem; }
  .brand { font-size: .8rem; font-weight: 600; letter-spacing: .06em; text-transform: uppercase; color: var(--muted); }
  h1 { font-size: 1.35rem; margin: 1rem 0 .4rem; }
  p { margin: .3rem 0; color: var(--muted); }
</style></head>
<body><main><div class="brand">🦆 ducktape</div><h1>Done</h1><p>You can close this and return to ducktape.</p></main></body></html>`;

export async function handle(request, env) {
  const url = new URL(request.url);
  const m = url.pathname.match(ID);
  if (url.pathname.startsWith("/r/") && !m) return new Response("no such ceremony", { status: 404 });
  if (!m) return env.ASSETS.fetch(request);
  const id = m[1];
  if (request.method === "POST") {
    const body = await request.text();
    if (body.length > MAX_BODY) return new Response("too large", { status: 413 });
    const result = new URLSearchParams(body).get("result");
    if (result === null) return new Response("no result", { status: 400 });
    await env.CEREMONIES.put(id, result, { expirationTtl: TTL_SECONDS });
    return new Response(DONE_PAGE, { status: 200, headers: { "content-type": "text/html; charset=utf-8" } });
  }
  if (request.method === "GET") {
    const result = await env.CEREMONIES.get(id);
    if (result === null) return new Response(null, { status: 204 });
    await env.CEREMONIES.delete(id);
    return new Response(result, { status: 200, headers: { "content-type": "application/json" } });
  }
  return new Response("method", { status: 405 });
}

export default { fetch: handle };
