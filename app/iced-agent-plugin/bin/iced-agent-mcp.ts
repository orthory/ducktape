#!/usr/bin/env bun
// iced-agent stdio MCP server — zero npm deps, hand-rolled JSON-RPC.
// Transport: plain newline-delimited JSON on stdin/stdout (MCP stdio, no LSP framing).
// Each tools/call is a single TCP round-trip to the loopback bridge.

import { DEFAULT_APP_ID, send } from "./client.ts";

const APP_ID = process.env.ICED_AGENT_APP_ID || DEFAULT_APP_ID;

// Reusable JSON-schema fragments mirroring the protocol field shapes.
const TARGET = {
  type: "object",
  properties: {
    ref: { type: "string", description: "an @ref from a prior tree/find" },
    x: { type: "number" },
    y: { type: "number" },
  },
} as const;
const COND = {
  type: "object",
  description: "either {node:{role,name,exists}} or {state_path:{path,equals}}",
} as const;
const win = { type: "string", description: "target window (default main)" };

interface Tool {
  name: string;
  description: string;
  inputSchema: object;
}

const TOOLS: Tool[] = [
  { name: "iced_tree", description: "Dump the semantic tree of a window.", inputSchema: { type: "object", properties: { window: win } } },
  {
    name: "iced_find",
    description: "Find semantic nodes by role / name substring / text (all optional).",
    inputSchema: { type: "object", properties: { window: win, role: { type: "string" }, name: { type: "string" }, text: { type: "string" } } },
  },
  { name: "iced_click", description: "Click a node (@ref) or a point.", inputSchema: { type: "object", properties: { target: TARGET }, required: ["target"] } },
  { name: "iced_type", description: "Type text into the focused widget.", inputSchema: { type: "object", properties: { text: { type: "string" } }, required: ["text"] } },
  {
    name: "iced_press",
    description: "Press a named key with optional modifiers (ctrl/shift/alt/cmd).",
    inputSchema: { type: "object", properties: { key: { type: "string" }, modifiers: { type: "array", items: { type: "string" } } }, required: ["key"] },
  },
  { name: "iced_hover", description: "Move the cursor over a target.", inputSchema: { type: "object", properties: { target: TARGET }, required: ["target"] } },
  {
    name: "iced_scroll",
    description: "Wheel-scroll at a target by dx/dy pixels.",
    inputSchema: { type: "object", properties: { target: TARGET, dx: { type: "number" }, dy: { type: "number" } }, required: ["target"] },
  },
  { name: "iced_drag", description: "Drag from one target to another.", inputSchema: { type: "object", properties: { from: TARGET, to: TARGET }, required: ["from", "to"] } },
  { name: "iced_state", description: "Read a curated state projection by dot path.", inputSchema: { type: "object", properties: { path: { type: "string" } } } },
  {
    name: "iced_intent",
    description: 'Inject a curated intent: "toggle_theme", or {section:{name}} / {navigate:{url}} / {search:{query}}.',
    inputSchema: { type: "object", properties: { intent: { description: "an Intent value" } }, required: ["intent"] },
  },
  { name: "iced_shot", description: "Screenshot a window; returns {png_base64}.", inputSchema: { type: "object", properties: { window: win } } },
  { name: "iced_logs", description: "Read (or clear) the tracing log ring.", inputSchema: { type: "object", properties: { clear: { type: "boolean" } } } },
  {
    name: "iced_wait",
    description: "Poll until a condition holds or timeout_ms elapses (default 5000).",
    inputSchema: { type: "object", properties: { cond: COND, timeout_ms: { type: "number" } }, required: ["cond"] },
  },
  { name: "iced_expect", description: "Assert a condition holds right now.", inputSchema: { type: "object", properties: { cond: COND }, required: ["cond"] } },
  { name: "iced_windows", description: "List windows and their bounds.", inputSchema: { type: "object", properties: {} } },
  { name: "iced_a11y", description: "Dump the AccessKit tree pushed to the OS adapter.", inputSchema: { type: "object", properties: { window: win } } },
];

const TOOL_NAMES = new Set(TOOLS.map((t) => t.name));

function write(msg: object) {
  process.stdout.write(JSON.stringify(msg) + "\n");
}

async function callTool(params: any) {
  const name: string = params?.name;
  const args = params?.arguments ?? {};
  if (!TOOL_NAMES.has(name)) {
    return { content: [{ type: "text", text: `unknown tool: ${name}` }], isError: true };
  }
  // Tool args mirror the protocol fields directly → pure passthrough.
  const cmd = { cmd: name.slice("iced_".length), ...args };
  try {
    const resp = await send(APP_ID, cmd);
    if (resp.ok) return { content: [{ type: "text", text: JSON.stringify(resp.result) }] };
    return { content: [{ type: "text", text: resp.error ?? "bridge error" }], isError: true };
  } catch (e: any) {
    return { content: [{ type: "text", text: String(e?.message ?? e) }], isError: true };
  }
}

async function dispatch(method: string, params: any): Promise<object> {
  switch (method) {
    case "initialize":
      return {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "iced-agent", version: "0.1.0" },
      };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call":
      return await callTool(params);
    default:
      throw { code: -32601, message: `method not found: ${method}` };
  }
}

async function handleLine(line: string) {
  let msg: any;
  try {
    msg = JSON.parse(line);
  } catch {
    return; // ignore unparseable input
  }
  const { id, method } = msg;
  // Notifications (no id) get no response; notifications/initialized is ignored.
  if (id === undefined || id === null) return;
  try {
    const result = await dispatch(method, msg.params);
    write({ jsonrpc: "2.0", id, result });
  } catch (e: any) {
    const code = typeof e?.code === "number" ? e.code : -32603;
    write({ jsonrpc: "2.0", id, error: { code, message: String(e?.message ?? e) } });
  }
}

async function main() {
  const decoder = new TextDecoder();
  let buf = "";
  for await (const chunk of Bun.stdin.stream()) {
    buf += decoder.decode(chunk, { stream: true });
    let nl: number;
    while ((nl = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (line) await handleLine(line);
    }
  }
}

main();
