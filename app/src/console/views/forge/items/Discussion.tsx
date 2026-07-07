// The embedded discussion of one issue/PR: the item's HIDDEN chat channel
// (`forge:<repo>:<n>`) rendered as a lightweight timeline — Module("forge")
// authors are the tracker's own "closed this"/"merged this" markers (muted
// system lines), everything else is a user comment. Posting goes straight
// through chat-client's postMessage against the injected transport (the store's
// sendMessage only targets the ACTIVE chat channel); reads re-pull on every
// finalized block via state.lastBlock.

import { useEffect, useState } from "react";

import {
  authorName,
  blocksText,
  latestMessages,
  postMessage,
} from "../../../../domain/chat-client";
import type { MessageView } from "../../../../domain/chat-client";
import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { errMsg, panelLabel, relTime } from "../ui";

/** ~100 latest messages is plenty for a tracker thread's v1. */
const DISCUSSION_LIMIT = 100;

export function Discussion({ channelId }: { channelId: string }) {
  const { state, transport } = useDucktape();
  const [messages, setMessages] = useState<MessageView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [posting, setPosting] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);

  const live = transport ?? null;

  useEffect(() => {
    if (!live || !channelId) return;
    let alive = true;
    latestMessages(live, channelId, DISCUSSION_LIMIT)
      .then((next) => {
        if (!alive) return;
        setError(null);
        setMessages(
          next
            .filter((m) => !m.head.deleted)
            .sort((a, b) => a.seq - b.seq),
        );
      })
      .catch((e) => {
        if (alive) setError(errMsg(e));
      });
    return () => {
      alive = false;
    };
    // state.lastBlock is the cheap per-finalized-block hook: any committed
    // write (comments, close/merge markers) lands a block, so this stays live
    // while the panel is open without a bespoke poller.
  }, [live, channelId, state.lastBlock, reloadToken]);

  const post = () => {
    const text = draft.trim();
    if (!live || !text || posting) return;
    setPosting(true);
    postMessage(live, {
      channelId,
      messageId: crypto.randomUUID(),
      blocks: [{ paragraph: [{ text, marks: [] }] }],
      origin: state.author,
    })
      .then(() => {
        setDraft("");
        setReloadToken((t) => t + 1);
      })
      .catch((e) => setError(errMsg(e)))
      .finally(() => setPosting(false));
  };

  return (
    <div style={{ marginTop: 20 }}>
      <div style={{ ...panelLabel, marginBottom: 9 }}>DISCUSSION</div>
      {error && (
        <div style={{ font: `500 11px ${font.sans}`, color: color.red, marginBottom: 8 }}>{error}</div>
      )}
      {messages === null && !error && (
        <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, padding: "8px 0" }}>
          Loading discussion...
        </div>
      )}
      {messages !== null && messages.length === 0 && (
        <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, padding: "8px 0" }}>
          No comments yet — start the discussion below.
        </div>
      )}
      {messages !== null && messages.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {messages.map((message) => (
            <DiscussionRow
              key={`${message.channel_id}:${message.seq}`}
              message={message}
              names={state.authorNames}
            />
          ))}
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 8 }}>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={live ? "Leave a comment" : "Connect a node to comment"}
          rows={3}
          disabled={!live || posting}
          style={{
            width: "100%",
            boxSizing: "border-box",
            resize: "vertical",
            padding: "9px 11px",
            borderRadius: radius.sm,
            border: `1px solid ${color.borderStrong}`,
            background: color.paper,
            font: `400 12.5px ${font.sans}`,
            color: color.ink,
          }}
        />
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button
            type="button"
            onClick={post}
            disabled={!live || posting || !draft.trim()}
            style={{
              all: "unset",
              boxSizing: "border-box",
              cursor: !live || posting || !draft.trim() ? "default" : "pointer",
              opacity: !live || posting || !draft.trim() ? 0.45 : 1,
              padding: "6px 14px",
              borderRadius: radius.sm,
              border: `1px solid ${color.dark}`,
              background: color.dark,
              color: color.onDark,
              font: `600 11.5px ${font.sans}`,
            }}
          >
            {posting ? "Posting..." : "Comment"}
          </button>
        </div>
      </div>
    </div>
  );
}

function DiscussionRow({
  message,
  names,
}: {
  message: MessageView;
  names: Record<string, string>;
}) {
  const author = message.head.author;
  const text = blocksText(message.head.blocks);
  const time = relTime(message.head.created_at);
  const isMarker = author !== "system" && typeof author === "object" && "module" in author;

  // The tracker's own lifecycle markers ("closed this", "merged this pull
  // request") post as Module("forge") — render them as muted system lines, not
  // comment cards.
  if (isMarker) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          gap: 7,
          padding: "2px 2px",
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
        }}
      >
        <span style={{ font: `600 10px ${font.mono}`, color: color.muted2 }}>
          {authorName(author, names)}
        </span>
        <span style={{ fontStyle: "italic" }}>{text}</span>
        {time && <span style={{ marginLeft: "auto", font: `400 10px ${font.mono}` }}>{time}</span>}
      </div>
    );
  }

  return (
    <div
      style={{
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        padding: "9px 12px",
      }}
    >
      <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
        <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>
          {authorName(author, names)}
        </span>
        {time && <span style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>{time}</span>}
      </div>
      <div
        style={{
          marginTop: 5,
          font: `400 12.5px ${font.sans}`,
          color: color.inkSoft,
          lineHeight: 1.55,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {text}
      </div>
    </div>
  );
}
