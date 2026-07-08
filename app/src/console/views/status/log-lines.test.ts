import { describe, expect, it } from "vitest";

import {
  filterLines,
  levelCounts,
  parseLevel,
  splitLines,
  stripAnsi,
  type LogLevel,
} from "./log-lines";

const ESC = String.fromCharCode(27);
// The exact shape a live node writes to daemon.log (tracing fmt with ANSI).
const COLORED_ERROR = `${ESC}[2m2026-07-08T17:19:37Z${ESC}[0m ${ESC}[31mERROR${ESC}[0m ${ESC}[2mdefguard_boringtun${ESC}[0m: CONNECTION_EXPIRED`;

describe("parseLevel", () => {
  it("recognizes tracing-style level tokens", () => {
    expect(parseLevel("2026-07-09T12:03:01.123Z  INFO ducktape_node: view 41")).toBe("info");
    expect(parseLevel("2026-07-09T12:03:02Z  WARN peer slow ack 812ms")).toBe("warn");
    expect(parseLevel("2026-07-09T12:03:03Z ERROR bind 127.0.0.1:8844 in use")).toBe("error");
    expect(parseLevel("12:03 DEBUG reactor drained 3 ops")).toBe("debug");
    expect(parseLevel("12:03 TRACE frame 0xdead")).toBe("trace");
  });

  it("matches bracketed and lowercase forms", () => {
    expect(parseLevel("[error] could not open storage")).toBe("error");
    expect(parseLevel("warning: deprecated flag")).toBe("warn");
  });

  it("treats panics and FATAL as errors", () => {
    expect(parseLevel("thread 'main' panicked at src/main.rs:1")).toBe("error");
    expect(parseLevel("FATAL not admitted after 900 attempts")).toBe("error");
  });

  it("is severity-ordered when a line names two levels", () => {
    // an ERROR line that also contains the word info is still an error
    expect(parseLevel("ERROR info-channel closed unexpectedly")).toBe("error");
  });

  it("falls back to other for unrecognized lines", () => {
    expect(parseLevel("Listening on http://127.0.0.1:8844")).toBe("other");
    expect(parseLevel("")).toBe("other");
  });

  it("does not match level substrings inside words", () => {
    // "information" / "traceroute" should not trip info/trace via \b boundaries…
    expect(parseLevel("reformatting")).toBe("other");
    // …but a standalone token still matches
    expect(parseLevel("info")).toBe("info");
  });
});

describe("ANSI-colorized logs (real daemon.log format)", () => {
  it("strips SGR color sequences", () => {
    expect(stripAnsi(COLORED_ERROR)).toBe(
      "2026-07-08T17:19:37Z ERROR defguard_boringtun: CONNECTION_EXPIRED",
    );
  });

  it("leaves legitimate bracket text alone (anchored on ESC, not bare '[..m')", () => {
    expect(stripAnsi("[node ab] parked [500ms] later")).toBe("[node ab] parked [500ms] later");
  });

  it("splitLines strips ANSI and then classifies the cleaned line", () => {
    const [line] = splitLines(`${COLORED_ERROR}\n`);
    expect(line.text).toBe(
      "2026-07-08T17:19:37Z ERROR defguard_boringtun: CONNECTION_EXPIRED",
    );
    // the color code no longer fuses to ERROR, so the level is recovered
    expect(line.level).toBe("error");
  });
});

describe("splitLines", () => {
  it("returns [] for empty input", () => {
    expect(splitLines("")).toEqual([]);
    expect(splitLines("\n")).toEqual([]);
  });

  it("drops a single trailing newline but keeps interior blanks", () => {
    const lines = splitLines("a\n\nb\n");
    expect(lines.map((l) => l.text)).toEqual(["a", "", "b"]);
    expect(lines.map((l) => l.n)).toEqual([1, 2, 3]);
  });

  it("classifies each line", () => {
    const lines = splitLines("INFO up\nERROR down\n");
    expect(lines.map((l) => l.level)).toEqual(["info", "error"]);
  });
});

const enabled = (...levels: LogLevel[]): Set<LogLevel> => new Set(levels);
const ALL = enabled("error", "warn", "info", "debug", "trace", "other");

describe("filterLines", () => {
  const lines = splitLines("INFO alpha\nWARN beta\nERROR gamma\nplain delta\n");

  it("keeps everything with an empty query and all levels", () => {
    expect(filterLines(lines, { query: "", levels: ALL })).toHaveLength(4);
  });

  it("filters by case-insensitive substring", () => {
    const hit = filterLines(lines, { query: "BETA", levels: ALL });
    expect(hit.map((l) => l.text)).toEqual(["WARN beta"]);
  });

  it("hides lines whose level is not enabled", () => {
    const hit = filterLines(lines, { query: "", levels: enabled("error") });
    expect(hit.map((l) => l.text)).toEqual(["ERROR gamma"]);
  });

  it("applies query and level together", () => {
    const hit = filterLines(lines, { query: "a", levels: enabled("info", "warn") });
    // "INFO alpha" and "WARN beta" both contain 'a'; ERROR/plain excluded by level
    expect(hit.map((l) => l.text)).toEqual(["INFO alpha", "WARN beta"]);
  });

  it("routes unrecognized lines to the 'other' chip", () => {
    const hit = filterLines(lines, { query: "", levels: enabled("other") });
    expect(hit.map((l) => l.text)).toEqual(["plain delta"]);
  });
});

describe("levelCounts", () => {
  it("tallies each level", () => {
    const lines = splitLines("INFO a\nINFO b\nWARN c\nplain d\n");
    expect(levelCounts(lines)).toEqual({
      error: 0,
      warn: 1,
      info: 2,
      debug: 0,
      trace: 0,
      other: 1,
    });
  });
});
