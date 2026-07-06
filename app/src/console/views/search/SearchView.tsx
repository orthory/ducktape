// Cross-module search over the node's derived-index views: one text fans out
// to the materialized views of chat and docs (the `pages` module — the
// console's docs surface), each its own /v1/index/{module}/view endpoint, and
// the hits come back grouped, newest first. Canonical state cannot serve this
// shape — hashed-key qmdb has no scans — which is exactly why the derived tier
// exists.
//
// Click-through hands off to the owning module's surface: a chat hit opens its
// channel, a doc hit opens the page.

import { useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { ChatSearchHit } from "../../../domain/chat-client";
import type { PageSearchHit } from "../../../domain/pages-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

// ── Styles ──────────────────────────────────────────────

const searchInput: CSSProperties = {
  flex: 1,
  minWidth: 0,
  height: 38,
  padding: "0 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 13.5px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const hitRow: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 3,
  width: "100%",
  textAlign: "left",
  padding: "9px 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderSoft}`,
  background: color.paper,
  cursor: "pointer",
  font: `400 13px ${font.sans}`,
  color: color.ink,
};

const hitMeta: CSSProperties = {
  font: `500 10.5px ${font.sans}`,
  color: color.muted2,
  textTransform: "none",
};

// ── Result groups ───────────────────────────────────────

function Group({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  return (
    <section style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <h3 style={{ margin: 0, font: `600 11px ${font.sans}`, color: color.muted2, letterSpacing: 0.4, textTransform: "uppercase" }}>
        {title} · {count}
      </h3>
      {children}
    </section>
  );
}

function ChatHit({ hit, onOpen }: { hit: ChatSearchHit; onOpen: () => void }) {
  return (
    <button type="button" style={hitRow} onClick={onOpen}>
      <span style={hitMeta}>#{hit.channelId} · {hit.author}{hit.edited ? " · edited" : ""}</span>
      <span>{hit.text}</span>
    </button>
  );
}

function DocHit({ hit, onOpen }: { hit: PageSearchHit; onOpen: () => void }) {
  return (
    <button type="button" style={hitRow} onClick={onOpen}>
      <span style={hitMeta}>{hit.pageId} · {hit.kind}</span>
      <span>{hit.text}</span>
    </button>
  );
}

// ── The surface ─────────────────────────────────────────

export function SearchView() {
  const { state, actions } = useDucktape();
  const [text, setText] = useState(state.search?.query ?? "");
  const results = state.search;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    actions.runSearch(text);
  };

  const total = results ? results.chat.length + results.docs.length : 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 18, padding: 24, maxWidth: 760 }}>
      <form onSubmit={submit} style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <Icon name="search" size={20} color={color.muted2} />
        <input
          style={searchInput}
          value={text}
          autoFocus
          placeholder="Search chat and docs…"
          onChange={(event) => setText(event.target.value)}
        />
      </form>

      {state.searchPending && (
        <p style={{ margin: 0, font: `400 12.5px ${font.sans}`, color: color.muted2 }}>Searching…</p>
      )}

      {results && !state.searchPending && (
        <div style={{ display: "flex", flexDirection: "column", gap: 18 }}>
          {total === 0 && (
            <p style={{ margin: 0, font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
              Nothing matches “{results.query}”.
            </p>
          )}
          {results.chat.length > 0 && (
            <Group title="Chat" count={results.chat.length}>
              {results.chat.map((hit) => (
                <ChatHit
                  key={`${hit.channelId}/${hit.seq}`}
                  hit={hit}
                  onOpen={() => {
                    actions.selectChannel(hit.channelId);
                    actions.setScreen("chat");
                  }}
                />
              ))}
            </Group>
          )}
          {results.docs.length > 0 && (
            <Group title="Docs" count={results.docs.length}>
              {results.docs.map((hit) => (
                <DocHit
                  key={hit.blockId}
                  hit={hit}
                  onOpen={() => {
                    actions.openPage(hit.pageId);
                    actions.setScreen("pages");
                  }}
                />
              ))}
            </Group>
          )}
        </div>
      )}
    </div>
  );
}
