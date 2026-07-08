// Shared forge-surface primitives: the label/tone/pill/tab/note building
// blocks ForgeView established, extracted so the issue/PR tracker files reuse
// the exact same styling instead of duplicating it.

import { useEffect, useState, type CSSProperties, type ReactNode } from "react";

import { forgeDiff, type CommitInfo, type FileDiff } from "../../../domain/forge-git-client";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";

export const panelLabel: CSSProperties = {
  font: `700 9px ${font.mono}`,
  letterSpacing: ".08em",
  color: color.muted2,
};

export const statusTone = {
  success: { text: color.green, bg: "#eef5f0", border: "#cfe3d7" },
  warning: { text: color.amber, bg: "#fbf4e6", border: "#ecdcae" },
  neutral: { text: color.purple, bg: "#f1edf5", border: "#ddd2e6" },
  info: { text: color.blue, bg: "#f1f4f8", border: "#d7e0eb" },
  danger: { text: color.red, bg: "#fbeeec", border: "#eccfc9" },
} as const;

export const inputStyle: CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "9px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12.5px ${font.sans}`,
  color: color.ink,
};

export function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function shortHash(value: string | null | undefined): string {
  return value ? `${value.slice(0, 10)}...` : "unborn";
}

export function relTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  // The node's commit time is genesis-relative today (not wall-clock), so a
  // small value would render an absurd "20637d ago". Omit it until the node
  // stamps real time (> 2001); ordering/history are unaffected.
  if (seconds <= 978_307_200) return "";
  const diff = Math.max(0, Date.now() - seconds * 1000);
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  if (diff < minute) return "now";
  if (diff < hour) return `${Math.floor(diff / minute)}m ago`;
  if (diff < day) return `${Math.floor(diff / hour)}h ago`;
  return `${Math.floor(diff / day)}d ago`;
}

export function StatusPill({ label, tone }: { label: string; tone: keyof typeof statusTone }) {
  const styles = statusTone[tone];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        height: 20,
        padding: "0 8px",
        borderRadius: radius.sm,
        border: `1px solid ${styles.border}`,
        background: styles.bg,
        color: styles.text,
        font: `700 9px ${font.mono}`,
        letterSpacing: ".06em",
        textTransform: "uppercase",
      }}
    >
      {label}
    </span>
  );
}

export function TabButton({
  label,
  active,
  onClick,
  badge,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 7,
        font: `600 13px ${font.sans}`,
        color: active ? color.ink : color.muted2,
        padding: "10px 0",
        borderBottom: `2px solid ${active ? color.dark : "transparent"}`,
        marginBottom: -1,
      }}
    >
      {label}
      {badge !== undefined && (
        <span
          aria-hidden="true"
          style={{
            font: `600 10px ${font.mono}`,
            color: color.muted2,
            background: color.panel,
            borderRadius: 9,
            padding: "1px 7px",
          }}
        >
          {badge}
        </span>
      )}
    </button>
  );
}

export function SegButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        all: "unset",
        cursor: "pointer",
        padding: "3px 9px",
        font: `600 10px ${font.mono}`,
        letterSpacing: ".04em",
        textTransform: "uppercase",
        color: active ? color.ink : color.muted2,
        background: active ? color.panel : "transparent",
      }}
    >
      {label}
    </button>
  );
}

export function CenterNote({ title, detail }: { title: string; detail?: string }) {
  return (
    <div
      style={{
        height: "100%",
        minHeight: 180,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        textAlign: "center",
        padding: 24,
      }}
    >
      <div style={{ font: `600 12.5px ${font.sans}`, color: color.muted2 }}>{title}</div>
      {detail && <div style={{ marginTop: 5, font: `400 11.5px ${font.sans}`, color: color.muted2, maxWidth: 360 }}>{detail}</div>}
    </div>
  );
}

export function InlineNote({ children }: { children: ReactNode }) {
  return <div style={{ padding: "9px 16px", font: `400 11px ${font.sans}`, color: color.muted2 }}>{children}</div>;
}

export function ErrorNote({ message, padded = false }: { message: string; padded?: boolean }) {
  return (
    <div style={{ padding: padded ? 18 : "8px 14px" }}>
      <div
        style={{
          border: `1px solid ${statusTone.danger.border}`,
          borderRadius: radius.sm,
          background: statusTone.danger.bg,
          color: statusTone.danger.text,
          font: `500 11px ${font.sans}`,
          padding: "7px 9px",
          wordBreak: "break-word",
        }}
      >
        {message}
      </div>
    </div>
  );
}

export function CommitRow({
  commit,
  selected = false,
  onOpen,
}: {
  commit: CommitInfo;
  selected?: boolean;
  onOpen?: () => void;
}) {
  const [hover, setHover] = useState(false);
  const content = (
    <>
      <span
        style={{
          width: 24,
          height: 24,
          borderRadius: radius.sm,
          background: statusTone.info.bg,
          border: `1px solid ${statusTone.info.border}`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          marginTop: 1,
        }}
      >
        <Icon name="forge" size={13} color={statusTone.info.text} strokeWidth={1.7} />
      </span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ font: `600 14px ${font.sans}`, color: color.ink }}>{commit.summary}</div>
        <div style={{ marginTop: 4, font: `400 11px ${font.mono}`, color: color.muted2 }}>
          {[shortHash(commit.id), commit.author, relTime(commit.time)].filter(Boolean).join(" · ")}
        </div>
      </div>
    </>
  );
  const style: CSSProperties = {
    display: "flex",
    gap: 13,
    width: "100%",
    boxSizing: "border-box",
    padding: "13px 0",
    borderBottom: `1px solid ${color.borderSoft}`,
    background: selected ? color.hover : hover && onOpen ? color.sunken : "transparent",
  };
  if (!onOpen) {
    return (
      <div title={commit.id} style={style}>
        {content}
      </div>
    );
  }
  return (
    <button
      type="button"
      title={commit.id}
      aria-label={`${commit.summary} commit details`}
      onClick={onOpen}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        ...style,
        cursor: "pointer",
      }}
    >
      {content}
    </button>
  );
}

export function CommitDetails({ repo, commit }: { repo: string; commit: CommitInfo }) {
  const [files, setFiles] = useState<FileDiff[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const from = commit.parentIds?.[0] ?? null;
  const description = commitDescription(commit);

  useEffect(() => {
    let alive = true;
    setFiles(null);
    setError(null);
    forgeDiff(repo, { from, to: commit.id })
      .then((next) => {
        if (alive) setFiles(next);
      })
      .catch((e) => {
        if (alive) setError(errMsg(e));
      });
    return () => {
      alive = false;
    };
  }, [commit.id, from, repo]);

  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        margin: "10px 0 16px 37px",
        overflow: "hidden",
      }}
    >
      <div style={{ padding: "12px 14px", borderBottom: `1px solid ${color.borderSoft}`, background: color.sidebar }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap" }}>
          <span style={{ ...panelLabel, color: color.muted3 }}>COMMIT</span>
          <span style={{ font: `600 12px ${font.mono}`, color: color.ink }}>{shortHash(commit.id)}</span>
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted }}>
            {[commit.author, relTime(commit.time)].filter(Boolean).join(" · ")}
          </span>
        </div>
        <div style={{ marginTop: 8, font: `600 14px ${font.sans}`, color: color.ink }}>{commit.summary}</div>
        <div style={{ marginTop: 8 }}>
          <div style={panelLabel}>DESCRIPTION</div>
          <div
            style={{
              marginTop: 5,
              font: `400 12.5px ${font.sans}`,
              color: description ? color.inkSoft : color.muted2,
              lineHeight: 1.55,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {description || "No description provided."}
          </div>
        </div>
      </div>
      <div style={{ padding: "10px 14px 14px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div style={panelLabel}>DIFF</div>
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            {from ? `${shortHash(from)} -> ${shortHash(commit.id)}` : `root -> ${shortHash(commit.id)}`}
          </span>
        </div>
        {error && <div style={{ marginTop: 8 }}><ErrorNote message={error} /></div>}
        {!error && files === null && <CenterNote title="Loading diff..." />}
        {!error && files?.length === 0 && (
          <div style={{ marginTop: 8, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            No file changes in this commit.
          </div>
        )}
        {!error && files !== null && files.length > 0 && (
          <div style={{ marginTop: 9, display: "flex", flexDirection: "column", gap: 10 }}>
            {files.map((file) => (
              <CommitFileDiff key={file.path} file={file} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function commitDescription(commit: CommitInfo): string {
  const message = (commit.message ?? "").trim();
  if (!message) return "";
  const lines = message.split(/\r?\n/);
  if (lines[0]?.trim() === commit.summary.trim()) {
    return lines.slice(1).join("\n").trim();
  }
  return lines.join("\n").trim();
}

function CommitFileDiff({ file }: { file: FileDiff }) {
  return (
    <div style={{ border: `1px solid ${color.borderSoft}`, borderRadius: radius.sm, overflow: "hidden" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 9,
          padding: "7px 10px",
          background: color.sidebar,
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span style={{ font: `600 12px ${font.mono}`, color: color.ink, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file.path}
        </span>
        <span style={{ marginLeft: "auto", font: `600 9.5px ${font.mono}`, color: color.muted2, textTransform: "uppercase" }}>
          {file.status}
        </span>
      </div>
      {file.hunks.length === 0 ? (
        <div style={{ padding: "8px 10px", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          Binary file or empty patch.
        </div>
      ) : (
        <div style={{ overflowX: "auto" }}>
          {file.hunks.map((hunk, hunkIndex) => (
            <div key={`${file.path}:${hunkIndex}`}>
              <div
                style={{
                  padding: "4px 10px",
                  font: `600 10.5px ${font.mono}`,
                  color: statusTone.info.text,
                  background: statusTone.info.bg,
                  whiteSpace: "pre",
                }}
              >
                {hunk.header}
              </div>
              {hunk.lines.map((line, lineIndex) => {
                const text = `${line.origin}${line.content}`;
                const tone =
                  line.origin === "+"
                    ? statusTone.success
                    : line.origin === "-"
                      ? statusTone.danger
                      : null;
                return (
                  <div
                    key={`${file.path}:${hunkIndex}:${lineIndex}`}
                    style={{
                      padding: "1px 10px",
                      font: `400 11.5px ${font.mono}`,
                      lineHeight: 1.65,
                      color: color.inkSofter,
                      background: tone?.bg ?? "transparent",
                      whiteSpace: "pre",
                    }}
                  >
                    {text}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** A small hover-aware action button (the console's quiet bordered button). */
export function ActionButton({
  label,
  onClick,
  disabled = false,
  tone = "default",
  strong = false,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  tone?: "default" | "danger" | "success";
  strong?: boolean;
}) {
  const [hover, setHover] = useState(false);
  const text =
    tone === "danger" ? color.danger : tone === "success" ? color.green : strong ? color.onDark : color.ink;
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.45 : 1,
        padding: "6px 12px",
        borderRadius: radius.sm,
        border: `1px solid ${
          tone === "danger" ? color.dangerBorder : strong ? color.dark : hover && !disabled ? color.borderStrong : color.border
        }`,
        background: strong ? color.dark : tone === "danger" ? color.dangerSoft : hover && !disabled ? color.sunken : color.paper,
        font: `600 11.5px ${font.sans}`,
        color: text,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}
