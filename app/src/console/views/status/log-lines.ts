// Pure derivations for the Node → Logs viewer. The daemon writes freeform
// stdout+stderr to daemon.log (tracing/RUST_LOG lines, plus the odd panic or
// bare println), so level detection is best-effort: we scan each line for a
// standalone level token and fall back to "other" when none is present. No line
// is ever hidden by classification alone — the viewer's level chips do the
// hiding, and "other" is its own toggle.

export type LogLevel = "error" | "warn" | "info" | "debug" | "trace" | "other";

/** The ordered levels the viewer offers as filter chips (most severe first).
 *  "other" catches lines with no recognizable level token. */
export const LOG_LEVELS: readonly LogLevel[] = [
  "error",
  "warn",
  "info",
  "debug",
  "trace",
  "other",
];

/** One rendered log line: its 1-based position in the tail (a stable React
 *  key within a single tail render) and its classified level. */
export interface LogLine {
  n: number;
  text: string;
  level: LogLevel;
}

// The node's tracing fmt layer writes ANSI SGR color codes to daemon.log
// (verified against a live node: `\x1b[31mERROR\x1b[0m`). Rendered in the
// webview they'd show as literal garbage, and — worse — the trailing `m` of a
// color code fuses to the next word, breaking the \b level boundary so every
// colorized line falls to "other". Strip SGR sequences before anything reads
// the text.
const ANSI_SGR = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "g");

/** Remove ANSI SGR (color/style) escape sequences from a log line. */
export function stripAnsi(text: string): string {
  return text.replace(ANSI_SGR, "");
}

// A level token standing on its own (word-bounded), case-insensitive. tracing
// prints `INFO`, RUST_LOG env-filter and `[ERROR]` bracket forms both match.
const LEVEL_PATTERNS: ReadonlyArray<readonly [LogLevel, RegExp]> = [
  ["error", /\b(error|fatal|panic|panicked)\b/i],
  ["warn", /\b(warn|warning)\b/i],
  ["info", /\binfo\b/i],
  ["debug", /\bdebug\b/i],
  ["trace", /\btrace\b/i],
];

/** Best-effort level of a single log line. Severity-ordered: a line that names
 *  both ERROR and INFO is an error. Returns "other" when nothing matches. */
export function parseLevel(text: string): LogLevel {
  for (const [level, re] of LEVEL_PATTERNS) {
    if (re.test(text)) return level;
  }
  return "other";
}

/** Split a raw tail into classified lines. A single trailing newline (every
 *  well-formed log file has one) is dropped so it doesn't render as a blank
 *  final row; interior blank lines are kept. */
export function splitLines(tail: string): LogLine[] {
  if (!tail) return [];
  const body = tail.endsWith("\n") ? tail.slice(0, -1) : tail;
  if (!body) return [];
  return body.split("\n").map((raw, i) => {
    const text = stripAnsi(raw);
    return { n: i + 1, text, level: parseLevel(text) };
  });
}

export interface LineFilter {
  /** Case-insensitive substring; empty matches everything. */
  query: string;
  /** The set of levels to KEEP. A line whose level is absent is hidden. */
  levels: ReadonlySet<LogLevel>;
}

/** Filter lines by enabled level and a case-insensitive substring query. */
export function filterLines(lines: readonly LogLine[], filter: LineFilter): LogLine[] {
  const needle = filter.query.trim().toLowerCase();
  return lines.filter(
    (line) =>
      filter.levels.has(line.level) &&
      (needle === "" || line.text.toLowerCase().includes(needle)),
  );
}

/** Per-level counts across a set of lines — the numbers the filter chips show. */
export function levelCounts(lines: readonly LogLine[]): Record<LogLevel, number> {
  const counts: Record<LogLevel, number> = {
    error: 0,
    warn: 0,
    info: 0,
    debug: 0,
    trace: 0,
    other: 0,
  };
  for (const line of lines) counts[line.level] += 1;
  return counts;
}
