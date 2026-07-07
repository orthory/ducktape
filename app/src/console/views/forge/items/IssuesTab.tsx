// The repo's Issues tab: open/closed filter, the item list from
// state.forgeItems (loaded per-screen by ForgeView), an inline "New issue"
// form, and the shared detail panel on row click.

import { useState } from "react";

import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { ActionButton, CenterNote, inputStyle } from "../ui";
import { ItemDetailPanel } from "./ItemDetailPanel";
import { ItemRow, StateFilterTabs } from "./shared";

export function IssuesTab({ repo }: { repo: string }) {
  const { state, actions } = useDucktape();
  const [filter, setFilter] = useState<"open" | "closed">("open");
  const [openNumber, setOpenNumber] = useState<number | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  const issues = state.forgeItems.filter((item) => item.kind === "issue");
  const openCount = issues.filter((item) => item.state === "open").length;
  const closedCount = issues.length - openCount;
  const visible = issues
    .filter((item) => (filter === "open" ? item.state === "open" : item.state !== "open"))
    .sort((a, b) => b.number - a.number);

  if (openNumber !== null) {
    return (
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", borderTop: `1px solid ${color.borderSoft}`, padding: "16px 24px" }}>
        <ItemDetailPanel repo={repo} number={openNumber} backLabel="Issues" onBack={() => setOpenNumber(null)} />
      </div>
    );
  }

  const submit = () => {
    if (busy || !title.trim()) return;
    setBusy(true);
    // Promise.resolve guards a test-harness action stub that returns void.
    Promise.resolve(actions.openForgeIssue({ repo, title: title.trim(), body }))
      .then(() => {
        setTitle("");
        setBody("");
        setShowForm(false);
      })
      .finally(() => setBusy(false));
  };

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto", borderTop: `1px solid ${color.borderSoft}`, padding: "16px 24px 24px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <StateFilterTabs filter={filter} openCount={openCount} closedCount={closedCount} onFilter={setFilter} />
        <span style={{ marginLeft: "auto" }}>
          <ActionButton label="New issue" onClick={() => setShowForm((v) => !v)} strong />
        </span>
      </div>

      {showForm && (
        <div
          style={{
            marginTop: 12,
            border: `1px solid ${color.borderStrong}`,
            borderRadius: radius.md,
            background: color.sidebar,
            padding: "13px 15px",
            maxWidth: 720,
          }}
        >
          <div style={{ font: `600 13px ${font.sans}`, color: color.ink }}>New issue</div>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Title"
            style={{ ...inputStyle, marginTop: 9 }}
          />
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder="Description (markdown)"
            rows={5}
            style={{ ...inputStyle, marginTop: 8, resize: "vertical" }}
          />
          <div style={{ marginTop: 10, display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <ActionButton label="Cancel" onClick={() => setShowForm(false)} disabled={busy} />
            <ActionButton
              label={busy ? "Opening..." : "Open issue"}
              onClick={submit}
              disabled={busy || !title.trim()}
              strong
            />
          </div>
        </div>
      )}

      <div style={{ marginTop: 12, border: `1px solid ${color.border}`, borderRadius: radius.md, overflow: "hidden", background: color.paper }}>
        {visible.length === 0 ? (
          issues.length === 0 ? (
            <CenterNote title="No issues yet" detail={`Open the first issue to start tracking work in ${repo}.`} />
          ) : (
            <CenterNote title={filter === "open" ? "No open issues" : "No closed issues"} />
          )
        ) : (
          visible.map((item) => (
            <ItemRow key={item.number} item={item} onOpen={() => setOpenNumber(item.number)} />
          ))
        )}
      </div>
    </div>
  );
}
