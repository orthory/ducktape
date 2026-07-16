#!/usr/bin/env bun
// iced-agent CLI — one request/response against the loopback bridge.
// Program output goes to stdout (console.log is correct here, not tracing).

import { DEFAULT_APP_ID, send } from "./client.ts";

const USAGE = `iced-agent <cmd> [args...]  — drive the native iced shell via the loopback bridge

Global flags:
  --app <id>        discovery id (default ${DEFAULT_APP_ID})
  --window <name>   target window (default main)

Commands:
  tree                                   dump the semantic tree
  find [--role R] [--name N] [--text T]  filter nodes (all nullable)
  click <@ref | --x N --y N>             click a node or point
  hover <@ref | --x N --y N>             move the cursor over a target
  scroll <@ref|--x --y> [--dx N --dy N]  wheel-scroll at a target
  drag <@from> <@to> | --from @r --to @r drag between two targets
  type <text...>                         type text into the focused widget
  press <key> [--mod ctrl ...]           press a named key with modifiers
  state [path]                           read a curated state projection (dot path)
  intent <toggle_theme|section|navigate|search> [--name|--url|--query V]
  shot [--out file.png]                  screenshot (PNG base64, or decode to --out)
  logs [--clear]                         read (or clear) the log ring
  wait  [--role R --name N [--absent]] | [--path P --equals JSON] [--timeout MS]
  expect [same cond flags as wait]       assert a condition now
  windows                                list windows and bounds
  a11y                                   dump the AccessKit tree pushed to the OS

Examples:
  iced-agent find --role button --name Forge
  iced-agent click @3
  iced-agent type "hello world"
  iced-agent press enter --mod ctrl
  iced-agent state section
  iced-agent shot --out /tmp/app.png
`;

interface Parsed {
  flags: Record<string, string>;
  mods: string[];
  positionals: string[];
  bools: Set<string>;
}

const BOOL_FLAGS = new Set(["clear", "exists", "absent"]);

function parseArgs(argv: string[]): Parsed {
  const flags: Record<string, string> = {};
  const mods: string[] = [];
  const positionals: string[] = [];
  const bools = new Set<string>();
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--mod") {
      const v = argv[++i];
      if (v !== undefined) mods.push(v);
    } else if (a.startsWith("--")) {
      const key = a.slice(2);
      if (BOOL_FLAGS.has(key)) bools.add(key);
      else flags[key] = argv[++i];
    } else {
      positionals.push(a);
    }
  }
  return { flags, mods, positionals, bools };
}

const num = (v: string | undefined): number | null => (v == null ? null : Number(v));

function target(p: Parsed): { ref: string | null; x: number | null; y: number | null } {
  const ref = p.flags.ref ?? p.positionals.find((t) => t.startsWith("@")) ?? null;
  return { ref, x: num(p.flags.x), y: num(p.flags.y) };
}

function cond(p: Parsed): unknown {
  if (p.flags.path != null) {
    let equals: unknown = null;
    if (p.flags.equals != null) {
      try {
        equals = JSON.parse(p.flags.equals);
      } catch {
        equals = p.flags.equals;
      }
    }
    return { state_path: { path: p.flags.path, equals } };
  }
  return {
    node: {
      role: p.flags.role ?? null,
      name: p.flags.name ?? null,
      exists: !p.bools.has("absent"),
    },
  };
}

function intent(p: Parsed): unknown {
  const kind = p.positionals[0];
  switch (kind) {
    case "toggle_theme":
      return "toggle_theme";
    case "section":
      return { section: { name: p.flags.name } };
    case "navigate":
      return { navigate: { url: p.flags.url } };
    case "search":
      return { search: { query: p.flags.query } };
    default:
      throw new Error(`unknown intent '${kind}' (want: toggle_theme|section|navigate|search)`);
  }
}

// Build the protocol `cmd` object from parsed args.
function buildCmd(name: string, p: Parsed): object {
  const window = p.flags.window ?? "main";
  switch (name) {
    case "tree":
      return { cmd: "tree", window };
    case "find":
      return { cmd: "find", window, role: p.flags.role ?? null, name: p.flags.name ?? null, text: p.flags.text ?? null };
    case "click":
      return { cmd: "click", target: target(p) };
    case "hover":
      return { cmd: "hover", target: target(p) };
    case "scroll":
      return { cmd: "scroll", target: target(p), dx: num(p.flags.dx) ?? 0, dy: num(p.flags.dy) ?? 0 };
    case "drag": {
      const refs = p.positionals.filter((t) => t.startsWith("@"));
      const from = { ref: p.flags.from ?? refs[0] ?? null, x: num(p.flags.fromx), y: num(p.flags.fromy) };
      const to = { ref: p.flags.to ?? refs[1] ?? null, x: num(p.flags.tox), y: num(p.flags.toy) };
      return { cmd: "drag", from, to };
    }
    case "type":
      return { cmd: "type", text: p.flags.text ?? p.positionals.join(" ") };
    case "press":
      return { cmd: "press", key: p.flags.key ?? p.positionals[0] ?? "", modifiers: p.mods };
    case "state":
      return { cmd: "state", path: p.flags.path ?? p.positionals[0] ?? "" };
    case "intent":
      return { cmd: "intent", intent: intent(p) };
    case "shot":
      return { cmd: "shot", window };
    case "logs":
      return { cmd: "logs", clear: p.bools.has("clear") };
    case "wait":
      return { cmd: "wait", cond: cond(p), timeout_ms: num(p.flags.timeout) ?? 5000 };
    case "expect":
      return { cmd: "expect", cond: cond(p) };
    case "windows":
      return { cmd: "windows" };
    case "a11y":
      return { cmd: "a11y", window };
    default:
      throw new Error(`unknown command '${name}' (see --help)`);
  }
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.length === 0 || argv[0] === "--help" || argv[0] === "-h") {
    console.log(USAGE);
    process.exit(0);
  }
  const name = argv[0];
  const p = parseArgs(argv.slice(1));
  const appId = p.flags.app ?? DEFAULT_APP_ID;

  const cmd = buildCmd(name, p);
  const resp = await send(appId, cmd);

  if (!resp.ok) {
    console.error(`error: ${resp.error ?? "unknown error"}`);
    process.exit(1);
  }

  // shot --out: decode the PNG to a file instead of dumping base64.
  if (name === "shot" && p.flags.out) {
    const b64 = (resp.result as { png_base64?: string })?.png_base64;
    if (!b64) {
      console.error("error: shot result has no png_base64");
      process.exit(1);
    }
    const bytes = Buffer.from(b64, "base64");
    await Bun.write(p.flags.out, bytes);
    console.log(JSON.stringify({ out: p.flags.out, bytes: bytes.length }));
    return;
  }

  console.log(JSON.stringify(resp.result, null, 2));
}

main().catch((e) => {
  console.error(`error: ${e?.message ?? e}`);
  process.exit(1);
});
