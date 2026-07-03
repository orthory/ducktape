// The forge surface over the node's `forge` module: a git-backed store whose
// root is sha256 of its HEAD commit oid. The only read is the current HEAD;
// each commit (path + content) is one git commit, so HEAD — and the composed
// app-hash — advances per write. This screen shows HEAD and drives commits so
// you can watch the oid move.

import { useState } from "react";
import type { FormEvent } from "react";

import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";

const cardStyle = {
  display: "flex",
  flexDirection: "column" as const,
  gap: 9,
  padding: 15,
  borderRadius: radius.md,
  border: `1px solid ${color.border}`,
  background: color.paper,
};

const labelStyle = {
  font: `600 11px ${font.sans}`,
  color: color.muted,
  letterSpacing: ".04em",
};

const fieldStyle = {
  padding: "8px 11px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 12.5px ${font.sans}`,
  color: color.ink,
  width: "100%",
};

export function ForgeView() {
  const { state, actions } = useDucktape();
  const [path, setPath] = useState("");
  const [message, setMessage] = useState("");
  const [content, setContent] = useState("");

  const head = state.forgeHead;
  const canCommit = path.trim().length > 0 && content.length > 0;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!canCommit) return;
    actions.commitForge({ path, message, content });
    setPath("");
    setMessage("");
    setContent("");
  };

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 7,
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <Icon name="forge" size={15} color={color.muted} />
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Forge</span>
        {head ? (
          <span
            title={head}
            style={{
              marginLeft: 4,
              padding: "2px 8px",
              borderRadius: radius.sm,
              background: color.chip,
              font: `500 11px ${font.mono}`,
              color: color.muted3,
            }}
          >
            {head.slice(0, 10)}…
          </span>
        ) : (
          <span style={{ marginLeft: 4, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            unborn repo
          </span>
        )}
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 17, display: "flex", flexDirection: "column", gap: 13 }}>
        <div style={cardStyle}>
          <span style={labelStyle}>HEAD COMMIT</span>
          <div
            style={{
              font: `400 12.5px ${font.mono}`,
              color: head ? color.inkSofter : color.muted2,
              wordBreak: "break-all",
              fontStyle: head ? "normal" : "italic",
            }}
          >
            {head ?? "no commits yet — commit a file to born the repo"}
          </div>
          <p style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, margin: 0 }}>
            forge root = sha256(HEAD oid). Every commit advances HEAD and the
            module root, so the app-hash moves with it.
          </p>
        </div>

        <form onSubmit={submit} style={cardStyle}>
          <span style={labelStyle}>NEW COMMIT</span>
          <input
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="path — e.g. README.md"
            style={fieldStyle}
          />
          <input
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            placeholder="commit message (optional)"
            style={fieldStyle}
          />
          <textarea
            value={content}
            onChange={(event) => setContent(event.target.value)}
            placeholder="file content"
            rows={6}
            style={{ ...fieldStyle, resize: "vertical", font: `400 12px ${font.mono}` }}
          />
          <button
            type="submit"
            disabled={!canCommit}
            style={{
              all: "unset",
              cursor: canCommit ? "pointer" : "default",
              alignSelf: "flex-start",
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "7px 13px",
              borderRadius: radius.sm,
              background: canCommit ? accentVar : color.chip,
              color: canCommit ? "#fff" : color.muted2,
              font: `600 12px ${font.sans}`,
            }}
          >
            <Icon name="check" size={14} />
            Commit
          </button>
        </form>
      </div>
    </div>
  );
}
