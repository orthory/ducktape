// The shared issue/PR detail shell: header (title, #n, state badge, edit /
// close-reopen), the markdown body, then the kind-specific surfaces — an
// issue goes straight to its Discussion; a PR adds the Conversation | Commits
// | Files changed sub-tabs with the merge box on the conversation.

import { useCallback, useEffect, useState } from "react";

import type { ForgeItemDetail } from "../../../../domain/forge-client";
import { forgeCompare, isForgeGitAvailable } from "../../../../domain/forge-git-client";
import type { CommitInfo } from "../../../../domain/forge-git-client";
import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { MarkdownPreview } from "../MarkdownPreview";
import {
  ActionButton,
  CenterNote,
  CommitDetails,
  CommitRow,
  ErrorNote,
  errMsg,
  inputStyle,
  panelLabel,
  relTime,
  SegButton,
} from "../ui";
import { Discussion } from "./Discussion";
import { MergeBox } from "./MergeBox";
import { PrFilesTab } from "./PrFilesTab";
import { isSelfAuthor, itemNumber, StateBadge, useAuthorName } from "./shared";

// Mirrors ForgeView's markdown guard: remark is superlinear on pathological
// docs, so an oversized body renders as plain text instead of freezing the
// webview. Same 200 kB line as the file previewer.
const MARKDOWN_PREVIEW_MAX_BYTES = 200_000;

type PrTab = "conversation" | "commits" | "files";

export function ItemDetailPanel({
  repo,
  number,
  onBack,
  backLabel,
  messageId,
  messageSeq,
}: {
  repo: string;
  number: number;
  onBack: () => void;
  backLabel: string;
  messageId?: string;
  messageSeq?: number;
}) {
  const { state, actions } = useDucktape();
  const [detail, setDetail] = useState<ForgeItemDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [prTab, setPrTab] = useState<PrTab>("conversation");
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const [editBody, setEditBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(() => {
    return Promise.resolve(actions.getForgeItem(repo, number)).then((item) => {
      setDetail(item ?? null);
      setLoading(false);
    });
    // actions is a stable facade; repo/number are the real identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo, number]);

  useEffect(() => {
    setDetail(null);
    setLoading(true);
    setPrTab("conversation");
    setEditing(false);
    void refetch();
  }, [refetch]);

  useEffect(() => {
    if (messageId || messageSeq) setPrTab("conversation");
  }, [messageId, messageSeq]);

  const authorLabel = useAuthorName(detail?.author ?? "system");

  if (loading) return <CenterNote title="Loading item..." />;
  if (!detail) {
    return (
      <div>
        <BackLink label={backLabel} onBack={onBack} />
        <CenterNote title="Item not found" detail={`${itemNumber(number)} names nothing in ${repo}.`} />
      </div>
    );
  }

  const mine = isSelfAuthor(detail.author, state.author);
  const isPr = detail.kind === "pr";
  const bodyText = detail.body;
  const sourceHead =
    detail.source_branch !== null
      ? state.forgeBranches.find((b) => b.name === detail.source_branch)?.head ?? null
      : null;

  const toggleState = () => {
    if (busy || detail.state === "merged") return;
    setBusy(true);
    Promise.resolve(
      actions.setForgeItemState({ repo, number, open: detail.state !== "open" }),
    )
      .then(() => refetch())
      .catch((e) => setError(errMsg(e)))
      .finally(() => setBusy(false));
  };

  const startEdit = () => {
    setEditTitle(detail.title);
    setEditBody(detail.body);
    setEditing(true);
  };

  const saveEdit = () => {
    if (busy || !editTitle.trim()) return;
    setBusy(true);
    Promise.resolve(
      actions.editForgeItem({
        repo,
        number,
        title: editTitle.trim() === detail.title ? null : editTitle.trim(),
        body: editBody === detail.body ? null : editBody,
      }),
    )
      .then(() => {
        setEditing(false);
        return refetch();
      })
      .catch((e) => setError(errMsg(e)))
      .finally(() => setBusy(false));
  };

  const body = (
    <div style={{ marginTop: 14, border: `1px solid ${color.border}`, borderRadius: radius.md, background: color.paper }}>
      {bodyText.trim() === "" ? (
        <div style={{ padding: "12px 15px", font: `400 12px ${font.sans}`, color: color.muted2, fontStyle: "italic" }}>
          No description provided.
        </div>
      ) : bodyText.length <= MARKDOWN_PREVIEW_MAX_BYTES ? (
        <MarkdownPreview text={bodyText} />
      ) : (
        <pre
          style={{
            margin: 0,
            padding: "12px 15px",
            font: `400 12px ${font.mono}`,
            color: color.inkSoft,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {bodyText}
        </pre>
      )}
    </div>
  );

  return (
    <div style={{ paddingBottom: 30 }}>
      <BackLink label={backLabel} onBack={onBack} />

      {editing ? (
        <div style={{ marginTop: 10, display: "flex", flexDirection: "column", gap: 9, maxWidth: 720 }}>
          <input
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            placeholder="Title"
            style={inputStyle}
          />
          <textarea
            value={editBody}
            onChange={(e) => setEditBody(e.target.value)}
            placeholder="Description (markdown)"
            rows={7}
            style={{ ...inputStyle, resize: "vertical" }}
          />
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <ActionButton label="Cancel" onClick={() => setEditing(false)} disabled={busy} />
            <ActionButton label={busy ? "Saving..." : "Save"} onClick={saveEdit} disabled={busy || !editTitle.trim()} strong />
          </div>
        </div>
      ) : (
        <>
          <div style={{ marginTop: 10, display: "flex", alignItems: "flex-start", gap: 10 }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ font: `600 19px ${font.sans}`, color: color.ink, lineHeight: 1.3, wordBreak: "break-word" }}>
                {detail.title}{" "}
                <span style={{ font: `400 19px ${font.sans}`, color: color.muted2 }}>{itemNumber(detail.number)}</span>
              </div>
            </div>
            {mine && <ActionButton label="Edit" onClick={startEdit} disabled={busy} />}
            {detail.state !== "merged" && (
              <ActionButton
                label={detail.state === "open" ? (isPr ? "Close" : "Close issue") : "Reopen"}
                onClick={toggleState}
                disabled={busy}
                tone={detail.state === "open" ? "danger" : "default"}
              />
            )}
          </div>
          <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 9, flexWrap: "wrap" }}>
            <StateBadge state={detail.state} />
            <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted }}>
              <span style={{ fontWeight: 600, color: color.muted3 }}>{authorLabel}</span>
              {" opened this "}
              {isPr ? "pull request" : "issue"}
              {relTime(detail.created_at) ? ` ${relTime(detail.created_at)}` : ""}
            </span>
            {isPr && detail.source_branch && (
              <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted3 }}>
                {detail.source_branch} → {detail.target_branch || "main"}
              </span>
            )}
          </div>
        </>
      )}

      {error && <div style={{ marginTop: 10 }}><ErrorNote message={error} /></div>}

      {!isPr && (
        <>
          {body}
          <Discussion channelId={detail.channel_id} messageId={messageId} messageSeq={messageSeq} />
        </>
      )}

      {isPr && (
        <>
          <div
            style={{
              marginTop: 14,
              display: "inline-flex",
              border: `1px solid ${color.border}`,
              borderRadius: radius.sm,
              overflow: "hidden",
            }}
          >
            <SegButton label="Conversation" active={prTab === "conversation"} onClick={() => setPrTab("conversation")} />
            <SegButton label="Commits" active={prTab === "commits"} onClick={() => setPrTab("commits")} />
            <SegButton label="Files changed" active={prTab === "files"} onClick={() => setPrTab("files")} />
          </div>

          {prTab === "conversation" && (
            <>
              {body}
              <MergeBox repo={repo} detail={detail} onChanged={() => void refetch()} />
              <Discussion channelId={detail.channel_id} messageId={messageId} messageSeq={messageSeq} />
            </>
          )}
          {prTab === "commits" && <PrCommits repo={repo} detail={detail} />}
          {prTab === "files" && (
            <div style={{ marginTop: 12 }}>
              <PrFilesTab repo={repo} detail={detail} sourceHead={sourceHead} onReviewed={() => void refetch()} />
            </div>
          )}
        </>
      )}
    </div>
  );
}

function BackLink({ label, onBack }: { label: string; onBack: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onBack}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        font: `600 11.5px ${font.sans}`,
        color: hover ? color.ink : color.muted,
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
      }}
    >
      ← {label}
    </button>
  );
}

/** The PR's "Commits" sub-tab: the compare's head-only commits, rendered with
 *  the same rows as the repo's commit history. */
function PrCommits({ repo, detail }: { repo: string; detail: ForgeItemDetail }) {
  const desktop = isForgeGitAvailable();
  const [commits, setCommits] = useState<CommitInfo[] | null>(null);
  const [selectedCommitId, setSelectedCommitId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const targetBranch = detail.target_branch || "main";
  const sourceBranch = detail.source_branch ?? "";

  useEffect(() => {
    if (!desktop || !sourceBranch) return;
    let alive = true;
    setSelectedCommitId(null);
    forgeCompare(repo, targetBranch, sourceBranch)
      .then((result) => {
        if (alive) setCommits(result.commits);
      })
      .catch((e) => {
        if (alive) setError(errMsg(e));
      });
    return () => {
      alive = false;
    };
  }, [desktop, repo, targetBranch, sourceBranch, detail.updated_at]);

  if (!desktop) {
    return <CenterNote title="Desktop app required" detail="Commit listing reads the local git repository through the desktop bridge." />;
  }
  if (error) return <ErrorNote message={error} padded />;
  if (commits === null) return <CenterNote title="Loading commits..." />;
  if (commits.length === 0) {
    return <CenterNote title="No commits" detail={`${sourceBranch} has no commits ahead of ${targetBranch}.`} />;
  }
  const selectedCommit = commits.find((commit) => commit.id === selectedCommitId) ?? null;
  return (
    <div style={{ marginTop: 8 }}>
      <div style={{ ...panelLabel, margin: "12px 0 2px" }}>
        COMMITS - {commits.length}
      </div>
      {commits.map((commit) => (
        <div key={commit.id}>
          <CommitRow
            commit={commit}
            selected={commit.id === selectedCommitId}
            onOpen={() => setSelectedCommitId((current) => (current === commit.id ? null : commit.id))}
          />
          {selectedCommit?.id === commit.id && <CommitDetails repo={repo} commit={selectedCommit} />}
        </div>
      ))}
    </div>
  );
}
