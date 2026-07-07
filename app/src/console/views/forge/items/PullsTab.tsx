// The repo's Pull requests tab: the same filtered list shape as Issues, plus
// the "New pull request" form — source branch from state.forgeBranches (main
// excluded), target fixed to main for v1.

import { useState } from "react";

import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { ActionButton, CenterNote, inputStyle } from "../ui";
import { ItemDetailPanel } from "./ItemDetailPanel";
import { ItemRow, StateFilterTabs } from "./shared";

const TARGET_BRANCH = "main";

export function PullsTab({ repo }: { repo: string }) {
  const { state, actions } = useDucktape();
  const [filter, setFilter] = useState<"open" | "closed">("open");
  const [openNumber, setOpenNumber] = useState<number | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [sourceBranch, setSourceBranch] = useState("");
  const [busy, setBusy] = useState(false);

  const pulls = state.forgeItems.filter((item) => item.kind === "pr");
  const openCount = pulls.filter((item) => item.state === "open").length;
  const closedCount = pulls.length - openCount;
  const visible = pulls
    .filter((item) => (filter === "open" ? item.state === "open" : item.state !== "open"))
    .sort((a, b) => b.number - a.number);
  const sourceBranches = state.forgeBranches.filter((branch) => branch.name !== TARGET_BRANCH);

  if (openNumber !== null) {
    return (
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", borderTop: `1px solid ${color.borderSoft}`, padding: "16px 24px" }}>
        <ItemDetailPanel repo={repo} number={openNumber} backLabel="Pull requests" onBack={() => setOpenNumber(null)} />
      </div>
    );
  }

  const submit = () => {
    if (busy || !title.trim() || !sourceBranch) return;
    setBusy(true);
    // Promise.resolve guards a test-harness action stub that returns void.
    Promise.resolve(
      actions.openForgePr({
        repo,
        title: title.trim(),
        body,
        sourceBranch,
        targetBranch: TARGET_BRANCH,
      }),
    )
      .then(() => {
        setTitle("");
        setBody("");
        setSourceBranch("");
        setShowForm(false);
      })
      .finally(() => setBusy(false));
  };

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto", borderTop: `1px solid ${color.borderSoft}`, padding: "16px 24px 24px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <StateFilterTabs filter={filter} openCount={openCount} closedCount={closedCount} onFilter={setFilter} />
        <span style={{ marginLeft: "auto" }}>
          <ActionButton label="New pull request" onClick={() => setShowForm((v) => !v)} strong />
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
          <div style={{ font: `600 13px ${font.sans}`, color: color.ink }}>New pull request</div>
          {sourceBranches.length === 0 ? (
            <div style={{ marginTop: 8, font: `400 12px ${font.sans}`, color: color.muted }}>
              No source branches — push a branch besides {TARGET_BRANCH} to open a pull request.
            </div>
          ) : (
            <>
              <div style={{ marginTop: 10, display: "flex", alignItems: "center", gap: 9 }}>
                <label
                  htmlFor="forge-pr-source"
                  style={{ font: `600 11px ${font.sans}`, color: color.muted3, flexShrink: 0 }}
                >
                  Merge
                </label>
                <select
                  id="forge-pr-source"
                  name="forge-pr-source"
                  value={sourceBranch}
                  onChange={(e) => setSourceBranch(e.target.value)}
                  style={{ ...inputStyle, width: "auto", minWidth: 160, cursor: "pointer", font: `500 12px ${font.mono}` }}
                >
                  <option value="" disabled>
                    Choose a branch…
                  </option>
                  {sourceBranches.map((branch) => (
                    <option key={branch.name} value={branch.name}>
                      {branch.name}
                    </option>
                  ))}
                </select>
                <span style={{ font: `600 11px ${font.sans}`, color: color.muted3 }}>into</span>
                <span
                  style={{
                    font: `500 12px ${font.mono}`,
                    color: color.ink,
                    border: `1px solid ${color.border}`,
                    borderRadius: radius.sm,
                    background: color.paper,
                    padding: "6px 10px",
                  }}
                >
                  {TARGET_BRANCH}
                </span>
              </div>
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
                  label={busy ? "Opening..." : "Open pull request"}
                  onClick={submit}
                  disabled={busy || !title.trim() || !sourceBranch}
                  strong
                />
              </div>
            </>
          )}
        </div>
      )}

      <div style={{ marginTop: 12, border: `1px solid ${color.border}`, borderRadius: radius.md, overflow: "hidden", background: color.paper }}>
        {visible.length === 0 ? (
          pulls.length === 0 ? (
            <CenterNote
              title="No pull requests yet"
              detail={`Push a branch and open a pull request to propose changes to ${repo}.`}
            />
          ) : (
            <CenterNote title={filter === "open" ? "No open pull requests" : "No closed pull requests"} />
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
