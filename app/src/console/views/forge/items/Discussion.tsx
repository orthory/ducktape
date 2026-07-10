// The embedded discussion of one issue/PR: the item's HIDDEN chat channel
// (`forge:<repo>:<n>`) rendered as a lightweight timeline — Module("forge")
// authors are the tracker's own "closed this"/"merged this" markers (muted
// system lines), everything else is a user comment. Posting rides the store's
// channel-aware `postInChannel`, so a comment gets the SAME mention engine as
// chat (typeahead via the shared Composer, mention marks, and the
// first-agent-mention watch that routes the engagement to the runs module).
// Reads re-pull on every finalized block via state.lastBlock.

import { useEffect, useState } from "react";

import {
  authorName,
  blocksText,
  latestMessages,
} from "../../../../domain/chat-client";
import type { MessageView } from "../../../../domain/chat-client";
import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { Composer } from "../../chat/Composer";
import { errMsg, panelLabel, relTime } from "../ui";

/** ~100 latest messages is plenty for a tracker thread's v1. */
const DISCUSSION_LIMIT = 100;

export function Discussion({ channelId }: { channelId: string }) {
  const { state, actions, transport } = useDucktape();
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
    // submit errors surface through the store's ops ledger / error banner —
    // same contract as the chat composer.
    void actions
      .postInChannel(channelId, draft)
      .then(() => {
        setDraft("");
        setReloadToken((t) => t + 1);
      })
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

      <div style={{ marginTop: 12 }}>
        {live ? (
          <Composer
            value={draft}
            onChange={setDraft}
            onSend={post}
            placeholder="Leave a comment — @mention an agent to hand it this item"
          />
        ) : (
          <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, padding: "8px 0" }}>
            Connect a node to comment.
          </div>
        )}
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
