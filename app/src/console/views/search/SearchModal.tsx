// The ⌘K command palette: a centered overlay the shell owns (see ConsoleShell),
// reachable from either rail. One text fans out four ways —
//   • chat + docs  — the node's derived-index views (state.search, async);
//   • members + files — an instant client-side filter over already-loaded state
// — and every hit navigates to the owning surface, closing the palette.
//
// It is mounted only while open (ConsoleShell gates on state.searchOpen), so
// each open is a fresh instance: the query resets, the input autofocuses, and
// the Escape listener attaches for exactly the palette's lifetime.

import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";

import type { ChatSearchHit } from "../../../domain/chat-client";
import { basename } from "../../../domain/files-client";
import type { FileEntry } from "../../../domain/files-client";
import type { PageSearchHit } from "../../../domain/pages-client";
import { displayNameForKey, shortKey } from "../../../domain/names";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

const RESULT_CAP = 8;
// debounce the node round-trip (chat/docs) so keystrokes don't each fan out.
const SEARCH_DEBOUNCE_MS = 180;

// ── Styles ──────────────────────────────────────────────

const backdrop: CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 50,
  display: "flex",
  justifyContent: "center",
  alignItems: "flex-start",
  paddingTop: "12vh",
  background: "rgba(24,23,20,0.32)",
  animation: "ik-fade .12s ease-out",
};

const panel: CSSProperties = {
  width: "min(640px, 92vw)",
  maxHeight: "68vh",
  display: "flex",
  flexDirection: "column",
  background: color.paper,
  borderRadius: radius.md,
  border: `1px solid ${color.borderStrong}`,
  boxShadow: shadow.pop,
  overflow: "hidden",
};

const inputRow: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "12px 16px",
  borderBottom: `1px solid ${color.borderSoft}`,
};

const searchInput: CSSProperties = {
  flex: 1,
  minWidth: 0,
  height: 26,
  border: "none",
  background: "transparent",
  font: `400 15px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const kbdHint: CSSProperties = {
  flexShrink: 0,
  padding: "2px 6px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderSoft}`,
  font: `600 10px ${font.mono}`,
  color: color.muted2,
};

const scroll: CSSProperties = {
  overflowY: "auto",
  padding: "8px 8px 12px",
  display: "flex",
  flexDirection: "column",
  gap: 14,
};

const hitRow: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
  width: "100%",
  textAlign: "left",
  padding: "8px 10px",
  borderRadius: radius.sm,
  border: "none",
  background: "transparent",
  cursor: "pointer",
  font: `400 13px ${font.sans}`,
  color: color.ink,
};

const hitMeta: CSSProperties = {
  font: `500 10.5px ${font.sans}`,
  color: color.muted2,
};

// ── Result groups ───────────────────────────────────────

function Group({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  return (
    <section style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <h3
        style={{
          margin: "0 0 2px 10px",
          font: `600 10px ${font.sans}`,
          color: color.muted2,
          letterSpacing: 0.4,
          textTransform: "uppercase",
        }}
      >
        {title} · {count}
      </h3>
      {children}
    </section>
  );
}

// The chat index pre-renders authors as `user:<id>`, `agent:<mod>/<id>`,
// `module:<id>`, or `system`. On a networked node `<id>` is the submitter's hex
// pubkey — resolve it to a profile nickname (or a short hex handle) so a hit
// reads like that author does everywhere else, and picks up member renames for
// free. On the embedded daemon `<id>` is a claimed utf-8 name; show it as-is.
// Agent / module / system authors keep their tag, which distinguishes them.
export function resolveHitAuthor(author: string, names: Record<string, string>): string {
  if (!author.startsWith("user:")) return author;
  const id = author.slice("user:".length);
  if (/^[0-9a-f]{16,}$/.test(id) && id.length % 2 === 0) {
    return displayNameForKey(id, names) ?? shortKey(id);
  }
  return id;
}

function HitButton({ meta, text, onOpen }: { meta: string; text: string; onOpen: () => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      style={{ ...hitRow, background: hover ? color.sunken : "transparent" }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onClick={onOpen}
    >
      <span style={hitMeta}>{meta}</span>
      <span
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {text}
      </span>
    </button>
  );
}

// ── The palette ─────────────────────────────────────────

export function SearchModal() {
  const { state, actions } = useDucktape();
  const [text, setText] = useState("");
  const query = text.trim();
  const results = state.search;

  // Escape closes ONLY the palette. Capture-phase + stopPropagation so a
  // background popover's own document Escape handler (emoji picker, message
  // action menu — both bubble-phase on document) never fires alongside it.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        actions.closeSearch();
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [actions]);

  // Debounced fan-out to the node's derived index (chat + docs).
  useEffect(() => {
    if (!query) {
      actions.clearSearch();
      return;
    }
    const timer = setTimeout(() => actions.runSearch(query), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, actions]);

  // Instant client-side filters over already-loaded roster + manifests.
  const memberHits = useMemo(() => {
    const q = query.toLowerCase();
    if (!q) return [] as { key: string; name: string }[];
    return state.members
      .map((key) => ({ key, name: displayNameForKey(key, state.authorNames) ?? shortKey(key) }))
      .filter((m) => m.name.toLowerCase().includes(q) || m.key.toLowerCase().includes(q))
      .slice(0, RESULT_CAP);
  }, [query, state.members, state.authorNames]);

  const fileHits = useMemo(() => {
    const q = query.toLowerCase();
    if (!q) return [] as FileEntry[];
    return state.files.filter((f) => f.path.toLowerCase().includes(q)).slice(0, RESULT_CAP);
  }, [query, state.files]);

  // Only surface node-index hits when they belong to the CURRENT input — a
  // debounced or in-flight query means `results` still holds a prior query's
  // groups, which must not show. `searching` is exactly that gap (debounce +
  // round-trip), and drives the "Searching…" line instead of a false empty
  // state. Client-side member/file hits are always live off the input.
  const matched = query !== "" && results?.query === query;
  const chatHits: ChatSearchHit[] = matched ? (results?.chat ?? []) : [];
  const docHits: PageSearchHit[] = matched ? (results?.docs ?? []) : [];
  const searching = query !== "" && !matched;
  const total = chatHits.length + docHits.length + memberHits.length + fileHits.length;

  const openChat = (channelId: string) => {
    actions.selectChannel(channelId);
    actions.setScreen("chat");
    actions.closeSearch();
  };
  const openDoc = (pageId: string) => {
    actions.openPage(pageId);
    actions.setScreen("pages");
    actions.closeSearch();
  };
  const goto = (screen: string) => {
    actions.setScreen(screen);
    actions.closeSearch();
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (query) actions.runSearch(query);
  };

  return (
    <div style={backdrop} onMouseDown={() => actions.closeSearch()}>
      <div style={panel} onMouseDown={(event) => event.stopPropagation()}>
        <form onSubmit={submit} style={inputRow}>
          <Icon name="search" size={18} color={color.muted2} />
          <input
            style={searchInput}
            value={text}
            autoFocus
            // A search box, not prose: kill WebKit's autocorrect/-capitalize so
            // typing "test" isn't "corrected" to "Test". Matches the members
            // search input. (Autocomplete is off globally — see main.tsx.)
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            placeholder="Search chat, pages, members, files…"
            aria-label="Search"
            onChange={(event) => setText(event.target.value)}
          />
          <span style={kbdHint}>ESC</span>
        </form>

        <div style={scroll}>
          {!query && (
            <p style={{ margin: "4px 10px", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
              Type to search chat, pages, members, and files.
            </p>
          )}

          {searching && chatHits.length === 0 && docHits.length === 0 && (
            <p style={{ margin: "4px 10px", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
              Searching…
            </p>
          )}

          {!searching && query !== "" && total === 0 && (
            <p style={{ margin: "4px 10px", font: `400 12.5px ${font.sans}`, color: color.muted2 }}>
              Nothing matches “{query}”.
            </p>
          )}

          {chatHits.length > 0 && (
            <Group title="Chat" count={chatHits.length}>
              {chatHits.map((hit) => (
                <HitButton
                  key={`${hit.channelId}/${hit.seq}`}
                  meta={`#${hit.channelId} · ${resolveHitAuthor(hit.author, state.authorNames)}${hit.edited ? " · edited" : ""}`}
                  text={hit.text}
                  onOpen={() => openChat(hit.channelId)}
                />
              ))}
            </Group>
          )}

          {docHits.length > 0 && (
            <Group title="Pages" count={docHits.length}>
              {docHits.map((hit) => (
                <HitButton
                  key={hit.blockId}
                  meta={`${hit.pageId} · ${hit.kind}`}
                  text={hit.text}
                  onOpen={() => openDoc(hit.pageId)}
                />
              ))}
            </Group>
          )}

          {memberHits.length > 0 && (
            <Group title="Members" count={memberHits.length}>
              {memberHits.map((hit) => (
                <HitButton
                  key={hit.key}
                  meta={shortKey(hit.key)}
                  text={hit.name}
                  onOpen={() => goto("members")}
                />
              ))}
            </Group>
          )}

          {fileHits.length > 0 && (
            <Group title="Files" count={fileHits.length}>
              {fileHits.map((hit) => (
                <HitButton
                  key={hit.path}
                  meta={hit.path}
                  text={basename(hit.path)}
                  onOpen={() => goto("files")}
                />
              ))}
            </Group>
          )}
        </div>
      </div>
    </div>
  );
}
