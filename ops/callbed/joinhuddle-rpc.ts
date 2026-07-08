// L2 — Prove the CONSENSUS glue behind a huddle call: a `join_huddle` chat op
// submitted on ONE node makes the OTHER node's chat channel carry the joiner's
// 32-byte mesh key in its huddle roster, so the call's fan-out recipients
// populate from FINALIZED consensus state — not from a client hand-pushing
// peer keys over the call ws.
//
// This drives the LIVE two-node callbed over the daemon's real HTTP surface
// (the exact plane app/src/domain/transport.ts dials):
//   POST /v1/submit  { target, payload, origin }   -> BlockEvent
//   POST /v1/query   { target, query }             -> module reply json
//
// The op payload is the chat module's `ChatMsg` serde form, verbatim what
// chat-client.ts's joinHuddle() sends: { join_huddle: { channel_id, node } }.
// `node` is a member's status.publicKey decoded to 32 raw bytes.
//
// Then it asserts the roster on BOTH nodes carries BOTH keys, and runs the
// REAL huddleRecipients() from app/src/domain/voice-session.ts over each
// node's roster to prove the call's fan-out set derives correctly from it.
//
//   bun joinhuddle-rpc.ts <HOST:PORT_A> <HOST:PORT_B>

import { huddleRecipients } from "../../app/src/domain/voice-session";
import type { Channel, HuddleMember } from "../../app/src/domain/chat-client";

const [httpA, httpB] = process.argv.slice(2);
if (!httpA || !httpB) {
  console.error("usage: bun joinhuddle-rpc.ts <HOST:PORT_A> <HOST:PORT_B>");
  process.exit(2);
}

const TARGET = "chat";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function hexToBytes(hex: string): number[] {
  const out: number[] = [];
  for (let i = 0; i < hex.length; i += 2) out.push(parseInt(hex.slice(i, i + 2), 16));
  return out;
}

async function statusKey(http: string): Promise<string> {
  const r = await fetch(`http://${http}/v1/status`);
  const j: any = await r.json();
  if (!j.publicKey) throw new Error(`/v1/status from ${http} has no publicKey`);
  return String(j.publicKey).toLowerCase();
}

async function submit(http: string, payload: unknown, origin: string): Promise<any> {
  const r = await fetch(`http://${http}/v1/submit`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ target: TARGET, payload, origin }),
  });
  const j: any = await r.json();
  if (!r.ok) throw new Error(`submit ${http} -> ${r.status}: ${JSON.stringify(j)}`);
  return j;
}

async function query(http: string, q: unknown): Promise<any> {
  const r = await fetch(`http://${http}/v1/query`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ target: TARGET, query: q }),
  });
  const j: any = await r.json();
  if (!r.ok) throw new Error(`query ${http} -> ${r.status}: ${JSON.stringify(j)}`);
  return j;
}

async function channelOf(http: string, channelId: string): Promise<Channel | null> {
  const reply = await query(http, { channel: { channel_id: channelId } });
  // ChatReply::Channel(Option<Channel>) -> { "channel": <Channel|null> }
  return (reply?.channel ?? null) as Channel | null;
}

function rosterHexes(ch: Channel | null): string[] {
  const h: HuddleMember[] = ch?.huddle ?? [];
  return h.map((m) => m.node.map((b) => b.toString(16).padStart(2, "0")).join(""));
}

async function main() {
  const keyA = await statusKey(httpA);
  const keyB = await statusKey(httpB);
  console.log(`node A ${httpA}  mesh key ${keyA}`);
  console.log(`node B ${httpB}  mesh key ${keyB}`);
  if (keyA === keyB) throw new Error("both nodes report the SAME mesh key — not a real 2-node mesh");

  // a FRESH channel per run so the roster assertion is clean (no stale members
  // from earlier runs). PostPolicy::Open so external origins may join.
  const channelId = `callbed-huddle-${Date.now()}`;
  console.log(`\nchannel: ${channelId}  (post_policy=open)`);

  // create on node A; consensus replicates it to node B.
  const c1 = await submit(
    httpA,
    { create_channel: { channel_id: channelId, name: "callbed", post_policy: "open" } },
    "callbed-founder",
  );
  console.log(`create_channel on A -> included at height ${c1.height}, appHash ${c1.appHash?.slice(0, 12)}…`);

  // member A joins on node A carrying A's OWN mesh key; member B joins on node
  // B carrying B's OWN mesh key. Each op's `node` is that member's node key,
  // exactly as chat-client.joinHuddle sends it. Different origins => distinct
  // AuthorRef::User rows.
  const jA = await submit(
    httpA,
    { join_huddle: { channel_id: channelId, node: hexToBytes(keyA) } },
    "callbed-alice",
  );
  console.log(`join_huddle(A key) on A  -> height ${jA.height}`);
  const jB = await submit(
    httpB,
    { join_huddle: { channel_id: channelId, node: hexToBytes(keyB) } },
    "callbed-bob",
  );
  console.log(`join_huddle(B key) on B  -> height ${jB.height}`);

  // poll BOTH nodes until each one's committed roster carries BOTH mesh keys —
  // i.e. B's join (submitted on B) is visible in A's chat state and vice versa.
  let chA: Channel | null = null;
  let chB: Channel | null = null;
  let converged = false;
  for (let i = 0; i < 80; i++) {
    chA = await channelOf(httpA, channelId);
    chB = await channelOf(httpB, channelId);
    const ra = rosterHexes(chA);
    const rb = rosterHexes(chB);
    if (ra.includes(keyA) && ra.includes(keyB) && rb.includes(keyA) && rb.includes(keyB)) {
      converged = true;
      break;
    }
    await sleep(250);
  }

  const ra = rosterHexes(chA);
  const rb = rosterHexes(chB);
  console.log(`\nnode A roster (${ra.length}): ${ra.join(", ")}`);
  console.log(`node B roster (${rb.length}): ${rb.join(", ")}`);

  // ── assertions ──────────────────────────────────────────────────────────
  const checks: [string, boolean][] = [
    ["rosters converged across BOTH nodes", converged],
    ["A's roster contains B's mesh key (B's join crossed consensus to A)", ra.includes(keyB)],
    ["A's roster contains A's own mesh key", ra.includes(keyA)],
    ["B's roster contains A's mesh key (A's join crossed consensus to B)", rb.includes(keyA)],
    ["B's roster contains B's own mesh key", rb.includes(keyB)],
    ["both rosters are byte-identical (same consensus state)", ra.slice().sort().join() === rb.slice().sort().join()],
  ];

  // ── the REAL derivation the call uses: huddleRecipients() from the app,
  //    run over the CONSENSUS roster (not a hand-built peer list). ──
  const recipFromA = huddleRecipients(chA?.huddle ?? [], keyA); // self = A
  const recipFromB = huddleRecipients(chB?.huddle ?? [], keyB); // self = B
  console.log(`\nhuddleRecipients(A roster, self=A) -> [${recipFromA.join(", ")}]`);
  console.log(`huddleRecipients(B roster, self=B) -> [${recipFromB.join(", ")}]`);
  checks.push(
    ["huddleRecipients over A's consensus roster (self=A) == [B]", recipFromA.length === 1 && recipFromA[0] === keyB],
    ["huddleRecipients over B's consensus roster (self=B) == [A]", recipFromB.length === 1 && recipFromB[0] === keyA],
  );

  console.log("\n── assertions ──");
  let allPass = true;
  for (const [label, ok] of checks) {
    console.log(`${ok ? "PASS" : "FAIL"}  ${label}`);
    if (!ok) allPass = false;
  }

  console.log(`\n${allPass ? "OVERALL PASS" : "OVERALL FAIL"}`);
  if (!allPass) process.exit(1);
}

main().catch((e) => {
  console.error("ERROR:", e?.message ?? e);
  process.exit(1);
});
