// The gateway v2 reference app (spec §8): a team kanban board published at
// board.<handle>.duck. It exercises the whole protocol surface —
//
//   - static SPA shell + a dynamic loopback backend (this file),
//   - AUTH BY CALLER-ACCOUNT: every request the gateway forwards carries the
//     mesh-verified `x-duck-caller-account`; there is no login. The board reads
//     it to attribute a card move and authorize writes,
//   - realtime over the WebSocket side door (live card moves broadcast to every
//     open board),
//
// so "it runs" means the protocol is sufficient. Run standalone for a backend
// self-check: `bun ops/demo-kanban.mjs --self-check`. In the demo it is the
// loopback upstream for the `board` route (allow_upgrade), reached through the
// gateway; the caller account is injected by the publisher node.

const PORT = Number(process.env.KANBAN_PORT ?? process.argv[2] ?? 0) || 0;

/** @typedef {{ id: string, title: string, column: string, movedBy: string }} Card */
const columns = ["todo", "doing", "done"];
/** @type {Map<string, Card>} */
const cards = new Map();
let nextId = 1;

const sockets = new Set();

const caller = (request) =>
  request.headers.get("x-duck-caller-account") ?? "anonymous";

function seed() {
  for (const [title, column] of [
    ["Design the wire", "done"],
    ["Ship the WS tunnel", "doing"],
    ["Write the kanban", "todo"],
  ]) {
    const id = String(nextId++);
    cards.set(id, { id, title, column, movedBy: "seed" });
  }
}

function broadcast(event) {
  const payload = JSON.stringify(event);
  for (const socket of sockets) {
    try {
      socket.send(payload);
    } catch {
      sockets.delete(socket);
    }
  }
}

function board() {
  return { columns, cards: [...cards.values()] };
}

/** Apply one board command; returns the changed card or an error string. */
function apply(command, who) {
  if (command.kind === "create") {
    const title = String(command.title ?? "").slice(0, 200).trim();
    if (!title) return { error: "title required" };
    const id = String(nextId++);
    const card = { id, title, column: "todo", movedBy: who };
    cards.set(id, card);
    return { card };
  }
  if (command.kind === "move") {
    const card = cards.get(String(command.id));
    if (!card) return { error: "no such card" };
    if (!columns.includes(command.column)) return { error: "bad column" };
    card.column = command.column;
    card.movedBy = who;
    return { card };
  }
  return { error: "unknown command" };
}

const INDEX_HTML = `<!doctype html><html><head><meta charset="utf-8">
<title>Duck Board</title><style>
 body{font:14px system-ui;margin:0;background:#0f1115;color:#e6e6e6}
 header{padding:10px 16px;background:#161a22;border-bottom:1px solid #262b36}
 main{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;padding:16px}
 .col{background:#161a22;border:1px solid #262b36;border-radius:8px;padding:8px;min-height:200px}
 .col h2{font-size:12px;text-transform:uppercase;color:#8a93a6;margin:4px 6px}
 .card{background:#1e2430;border:1px solid #2c3444;border-radius:6px;padding:8px;margin:6px 0;cursor:grab}
 .by{color:#6f7a8d;font-size:11px;margin-top:4px}
 input{background:#0f1115;border:1px solid #2c3444;color:#e6e6e6;border-radius:6px;padding:6px;width:70%}
 button{background:#2b6cff;border:0;color:#fff;border-radius:6px;padding:6px 10px;cursor:pointer}
</style></head><body>
<header>Duck Board — you are <b id="me">…</b> <span id="live"></span></header>
<form id="new"><input id="title" placeholder="new card…" autocomplete="off"><button>Add</button></form>
<main id="board"></main>
<script>
const api = (p,b)=>fetch(p,b?{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(b)}:{}).then(r=>r.json());
let me="…";
// Card titles come from other accounts, so build every card with textContent —
// never innerHTML with card data — to keep untrusted content inert.
function render(state){
 const board=document.getElementById("board");board.replaceChildren();
 for(const col of state.columns){
  const d=document.createElement("div");d.className="col";
  const h=document.createElement("h2");h.textContent=col;d.appendChild(h);
  for(const c of state.cards.filter(c=>c.column===col)){
   const e=document.createElement("div");e.className="card";e.draggable=true;e.dataset.id=c.id;
   e.textContent=c.title;
   const by=document.createElement("div");by.className="by";by.textContent="by "+String(c.movedBy).slice(0,10);
   e.appendChild(by);
   e.ondragstart=ev=>ev.dataTransfer.setData("id",c.id);
   d.appendChild(e);
  }
  d.ondragover=ev=>ev.preventDefault();
  d.ondrop=ev=>{ev.preventDefault();api("/move",{kind:"move",id:ev.dataTransfer.getData("id"),column:col});};
  board.appendChild(d);
 }
}
document.getElementById("new").onsubmit=ev=>{ev.preventDefault();const t=document.getElementById("title");api("/cards",{kind:"create",title:t.value});t.value="";};
async function boot(){const s=await api("/board");me=s.me||"?";document.getElementById("me").textContent=me;render(s);
 try{const ws=new WebSocket((location.protocol==="https:"?"wss":"ws")+"://"+location.host+"/.duck/ws");
  ws.onopen=()=>document.getElementById("live").textContent="● live";
  ws.onmessage=async()=>render(await api("/board"));
 }catch{}}
boot();
</script></body></html>`;

function handle(request, server) {
  const url = new URL(request.url);
  if (url.pathname === "/.duck/ws") {
    if (server.upgrade(request)) return undefined;
    return new Response("expected a websocket", { status: 426 });
  }
  if (url.pathname === "/" || url.pathname === "/index.html") {
    return new Response(INDEX_HTML, { headers: { "content-type": "text/html" } });
  }
  if (url.pathname === "/board" && request.method === "GET") {
    return Response.json({ ...board(), me: caller(request) });
  }
  if ((url.pathname === "/cards" || url.pathname === "/move") && request.method === "POST") {
    return request.json().then((command) => {
      const result = apply(command, caller(request));
      if (result.error) return Response.json({ error: result.error }, { status: 400 });
      broadcast({ kind: "changed", card: result.card });
      return Response.json({ ok: true, card: result.card });
    });
  }
  return new Response("not found", { status: 404 });
}

// A tiny in-process self-check of the board logic — no gateway, no browser.
if (process.argv.includes("--self-check")) {
  cards.clear();
  nextId = 1;
  const created = apply({ kind: "create", title: "hello" }, "alice");
  if (!created.card || created.card.column !== "todo" || created.card.movedBy !== "alice") {
    throw new Error("create failed");
  }
  const moved = apply({ kind: "move", id: created.card.id, column: "done" }, "bob");
  if (!moved.card || moved.card.column !== "done" || moved.card.movedBy !== "bob") {
    throw new Error("move failed");
  }
  if (!apply({ kind: "move", id: created.card.id, column: "nope" }, "bob").error) {
    throw new Error("bad column accepted");
  }
  if (!apply({ kind: "create", title: "   " }, "alice").error) {
    throw new Error("empty title accepted");
  }
  console.log("[kanban] self-check ok: create/move/attribution/validation");
  process.exit(0);
}

seed();
const server = Bun.serve({
  port: PORT,
  fetch: handle,
  websocket: {
    open(ws) {
      sockets.add(ws);
    },
    close(ws) {
      sockets.delete(ws);
    },
    message() {},
  },
});
console.log(`[kanban] board backend on http://127.0.0.1:${server.port}`);
