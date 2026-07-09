// Node → Logs tab: a live stream of the node logs. Managed desktop nodes also
// use the existing daemon.log tail as backfill and show local runtime facts.

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { RuntimeFacts } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";
import {
  filterLines,
  levelCounts,
  LOG_LEVELS,
  splitLines,
  type LogLevel,
} from "./log-lines";
import { useLogStream } from "./use-log-stream";

/** How often the tail is re-read. Fast enough to feel live, slow enough that a
 *  64 KB read every tick is negligible. Mirrors the Overview's metrics poller. */
const LOG_POLL_MS = 1_500;
/** Runtime facts change slowly (pid is fixed; uptime ticks) — poll lazily. */
const FACTS_POLL_MS = 5_000;

const LEVEL_META: Record<LogLevel, { label: string; tag: string; text: string }> = {
  error: { label: "ERR", tag: color.red, text: color.red },
  warn: { label: "WARN", tag: color.amber, text: color.inkSoft },
  info: { label: "INFO", tag: color.accentAlt2, text: color.inkSoft },
  debug: { label: "DBG", tag: color.muted2, text: color.muted3 },
  trace: { label: "TRC", tag: color.muted2, text: color.muted2 },
  other: { label: "·", tag: color.borderStrong, text: color.inkSofter },
};

const sectionLabelStyle = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
} as const;

/** Human uptime: `45s`, `12m 05s`, `3h 12m`, `2d 03h`. `—` when unknown. */
function formatUptime(secs: number | null): string {
  if (secs === null || secs < 0) return "—";
  if (secs < 60) return `${secs}s`;
  const d = Math.floor(secs / 86_400);
  const h = Math.floor(secs / 3_600) % 24;
  const m = Math.floor(secs / 60) % 60;
  const s = secs % 60;
  if (d > 0) return `${d}d ${String(h).padStart(2, "0")}h`;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  return `${m}m ${String(s).padStart(2, "0")}s`;
}

/** Poll the daemon log + runtime facts while this tab is mounted and the node
 *  is managed. Both keep their last good frame on a failed read (a node stopped
 *  mid-view), and reset when the active workspace changes. */
function usePolledDaemon(managed: boolean, workspaceId: string | null) {
  const { actions } = useDucktape();
  const [tail, setTail] = useState<string | null>(null);
  const [facts, setFacts] = useState<RuntimeFacts | null>(null);

  useEffect(() => {
    setTail(null);
    if (!managed) return;
    let cancelled = false;
    const poll = () => {
      void actions.readDaemonLog().then((log) => {
        if (cancelled || !log) return;
        setTail(log.tail);
      });
    };
    poll();
    const timer = setInterval(poll, LOG_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [managed, workspaceId, actions]);

  useEffect(() => {
    setFacts(null);
    if (!managed) return;
    let cancelled = false;
    const poll = () => {
      void actions.readRuntimeFacts().then((f) => {
        if (!cancelled && f) setFacts(f);
      });
    };
    poll();
    const timer = setInterval(poll, FACTS_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [managed, workspaceId, actions]);

  return { tail, facts };
}

function Fact({
  label,
  value,
  copyable = false,
  tone,
}: {
  label: string;
  value: string;
  copyable?: boolean;
  tone?: string;
}) {
  const [copied, setCopied] = useState(false);
  const has = value !== "—";
  const copy = () => {
    if (!copyable || !has) return;
    setCopied(true);
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(value).catch(() => {});
    }
    globalThis.setTimeout(() => setCopied(false), 1200);
  };
  return (
    <button
      type="button"
      onClick={copy}
      disabled={!copyable || !has}
      title={copyable && has ? value : undefined}
      style={{
        all: "unset",
        cursor: copyable && has ? "pointer" : "default",
        border: `1px solid ${copied ? "#cfe3d7" : color.border}`,
        background: copied ? "#eef5f0" : color.paper,
        borderRadius: radius.md,
        padding: "8px 11px",
        minWidth: 0,
        boxSizing: "border-box",
      }}
    >
      <div
        style={{
          font: `700 8px ${font.mono}`,
          letterSpacing: ".08em",
          color: color.muted2,
        }}
      >
        {copied ? "COPIED" : label}
      </div>
      <div
        style={{
          font: `600 11.5px ${font.mono}`,
          color: has ? (tone ?? color.inkSoft) : color.muted2,
          marginTop: 3,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {value}
      </div>
    </button>
  );
}

function RuntimeFactsRow({
  facts,
  version,
}: {
  facts: RuntimeFacts | null;
  version: string | null;
}) {
  const pid =
    facts?.pid != null
      ? facts.alive === false
        ? `${facts.pid} (exited)`
        : String(facts.pid)
      : "—";
  return (
    <div>
      <div style={sectionLabelStyle}>RUNTIME</div>
      <div
        style={{
          marginTop: 8,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
          gap: 8,
        }}
      >
        <Fact
          label="PID"
          value={pid}
          tone={facts?.alive === false ? color.red : undefined}
        />
        <Fact label="UPTIME" value={formatUptime(facts?.uptimeSecs ?? null)} />
        <Fact label="NODE VERSION" value={version ? `v${version}` : "—"} />
        <Fact label="BINARY" value={facts?.binaryPath ?? "—"} copyable />
        <Fact label="LOG PATH" value={facts?.logPath ?? "—"} copyable />
        <Fact label="DATA DIR" value={facts?.dataDir ?? "—"} copyable />
      </div>
    </div>
  );
}

function LevelChip({
  level,
  active,
  count,
  onToggle,
}: {
  level: LogLevel;
  active: boolean;
  count: number;
  onToggle: () => void;
}) {
  const meta = LEVEL_META[level];
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={active}
      aria-label={`${meta.label} lines`}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".04em",
        color: active ? meta.tag : color.muted2,
        background: active ? color.paper : color.sunken,
        border: `1px solid ${active ? color.borderStrong : color.borderSoft}`,
        borderRadius: radius.sm,
        padding: "4px 8px",
        opacity: active ? 1 : 0.6,
      }}
    >
      <span
        style={{
          width: 5,
          height: 5,
          borderRadius: "50%",
          background: meta.tag,
        }}
      />
      {meta.label}
      <span style={{ color: color.muted2 }}>{count}</span>
    </button>
  );
}

/** The log toolbar: follow status, search, level chips, copy. */
function Toolbar({
  following,
  matchInfo,
  query,
  setQuery,
  counts,
  enabled,
  toggleLevel,
  onCopy,
  onJump,
}: {
  following: boolean;
  matchInfo: string;
  query: string;
  setQuery: (q: string) => void;
  counts: Record<LogLevel, number>;
  enabled: ReadonlySet<LogLevel>;
  toggleLevel: (l: LogLevel) => void;
  onCopy: () => void;
  onJump: () => void;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <span
          style={{ display: "inline-flex", alignItems: "center", gap: 6, minWidth: 0 }}
        >
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: "50%",
              background: following ? "#5f9e74" : color.amber,
              animation: following ? "ik-pulse 1.6s ease-in-out infinite" : undefined,
            }}
          />
          <span style={{ font: `600 11px ${font.mono}`, color: color.inkSoft }}>
            node logs
          </span>
          <span style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
            {following ? "following" : "paused"}
          </span>
        </span>

        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search…"
          aria-label="Search daemon log"
          style={{
            flex: 1,
            minWidth: 120,
            font: `500 11.5px ${font.mono}`,
            color: color.inkSoft,
            background: color.paper,
            border: `1px solid ${color.border}`,
            borderRadius: radius.sm,
            padding: "6px 10px",
            outline: "none",
          }}
        />
        <span style={{ font: `500 10px ${font.mono}`, color: color.muted2 }}>
          {matchInfo}
        </span>
        <button
          type="button"
          onClick={onCopy}
          style={{
            all: "unset",
            cursor: "pointer",
            font: `600 10.5px ${font.sans}`,
            color: color.inkSoft,
            background: color.paper,
            border: `1px solid ${color.borderStrong}`,
            borderRadius: radius.sm,
            padding: "6px 11px",
          }}
        >
          Copy
        </button>
        {!following && (
          <button
            type="button"
            onClick={onJump}
            style={{
              all: "unset",
              cursor: "pointer",
              font: `600 10.5px ${font.sans}`,
              color: color.onDark,
              background: color.dark,
              border: `1px solid ${color.dark}`,
              borderRadius: radius.sm,
              padding: "6px 11px",
            }}
          >
            Jump to latest
          </button>
        )}
      </div>

      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {LOG_LEVELS.map((level) => (
          <LevelChip
            key={level}
            level={level}
            active={enabled.has(level)}
            count={counts[level]}
            onToggle={() => toggleLevel(level)}
          />
        ))}
      </div>
    </div>
  );
}

function LogBody({
  ready,
  lines,
  scrollRef,
  onScroll,
}: {
  ready: boolean;
  lines: ReturnType<typeof filterLines>;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onScroll: () => void;
}) {
  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      role="log"
      aria-label="Daemon log output"
      aria-live="off"
      style={{
        flex: 1,
        minHeight: 220,
        overflow: "auto",
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: "#fbfaf7",
        padding: "10px 12px",
        boxShadow: shadow.card,
      }}
    >
      {!ready ? (
        <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
          Reading daemon.log…
        </div>
      ) : lines.length === 0 ? (
        <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
          No matching log lines.
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column" }}>
          {lines.map((line) => {
            const meta = LEVEL_META[line.level];
            return (
              <div
                key={line.n}
                style={{
                  display: "grid",
                  gridTemplateColumns: "34px 1fr",
                  gap: 8,
                  padding: "1px 0",
                  font: `500 11px ${font.mono}`,
                  lineHeight: 1.5,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                <span
                  style={{
                    color: meta.tag,
                    font: `700 8.5px ${font.mono}`,
                    textAlign: "right",
                    paddingTop: 2,
                    userSelect: "none",
                  }}
                >
                  {meta.label}
                </span>
                <span style={{ color: meta.text }}>{line.text || " "}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function LogsTab(): ReactNode {
  const { state, transport } = useDucktape();
  const managed = state.managed;
  const version = state.status?.version ?? null;
  const { tail, facts } = usePolledDaemon(managed, state.workspace?.id ?? null);
  const streamed = useLogStream(transport, managed ? tail : null);

  const [query, setQuery] = useState("");
  const [enabled, setEnabled] = useState<Set<LogLevel>>(() => new Set(LOG_LEVELS));
  const [following, setFollowing] = useState(true);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  const lines = useMemo(() => splitLines(streamed.text), [streamed.text]);
  const counts = useMemo(() => levelCounts(lines), [lines]);
  const filtered = useMemo(
    () => filterLines(lines, { query, levels: enabled }),
    [lines, query, enabled],
  );

  // Auto-scroll to the newest line whenever content changes while following.
  useEffect(() => {
    if (!following) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [filtered, following]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    // Following == parked at the bottom. Scrolling up pauses; scrolling back
    // to the bottom (or Jump to latest) resumes — derived, so there is no
    // separate toggle to keep in sync.
    setFollowing(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
  };

  const jumpToLatest = () => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
    setFollowing(true);
  };

  const toggleLevel = (level: LogLevel) =>
    setEnabled((prev) => {
      const next = new Set(prev);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });

  const copyVisible = () => {
    const text = filtered.map((l) => l.text).join("\n");
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(text).catch(() => {});
    }
  };

  const matchInfo =
    query.trim() !== "" ? `${filtered.length}/${lines.length} match` : `${lines.length} lines`;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, minHeight: 0, flex: 1 }}>
      {managed && <RuntimeFactsRow facts={facts} version={version} />}
      <Toolbar
        following={following}
        matchInfo={matchInfo}
        query={query}
        setQuery={setQuery}
        counts={counts}
        enabled={enabled}
        toggleLevel={toggleLevel}
        onCopy={copyVisible}
        onJump={jumpToLatest}
      />
      <LogBody
        ready={streamed.ready}
        lines={filtered}
        scrollRef={scrollRef}
        onScroll={onScroll}
      />
    </div>
  );
}
