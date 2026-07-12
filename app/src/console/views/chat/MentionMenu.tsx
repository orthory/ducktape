// The composer's typeahead popovers — the Pages SlashMenu idiom (role="listbox"
// / role="option", activeIndex highlight, mousedown-to-pick). Both the @mention
// menu and the `[[` page-ref menu are the same listbox over different rows, so
// they share one component and differ only in what they list.
// Opens UPWARD (bottom: 100%): the composer sits at the bottom of the pane, so
// a downward menu would render off-screen.

import type { PageMeta } from "../../../domain/pages-client";
import { color, font, radius, shadow } from "../../theme/tokens";
import { mentionCandidateToken, type MentionCandidate } from "./mention";

/** One row: `label` is the human name, `hint` the literal token it inserts. */
interface Row {
  key: string;
  label: string;
  hint: string;
  token: string;
}

function TypeaheadMenu({
  ariaLabel,
  rows,
  activeIndex,
  onPick,
  drop = "up",
}: {
  ariaLabel: string;
  rows: Row[];
  activeIndex: number;
  onPick: (token: string) => void;
  /** "up" (default — the chat composer sits at the pane bottom) opens above
   *  the anchor; "down" below it (a comment composer near the window top). */
  drop?: "up" | "down";
}) {
  if (rows.length === 0) return null;
  return (
    <div
      role="listbox"
      aria-label={ariaLabel}
      style={{
        position: "absolute",
        zIndex: 20,
        ...(drop === "up"
          ? { bottom: "100%", marginBottom: 6 }
          : { top: "100%", marginTop: 6 }),
        left: 0,
        width: 260,
        maxHeight: 240,
        overflowY: "auto",
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.paper,
        boxShadow: shadow.pop,
        padding: 4,
      }}
    >
      {rows.map((row, i) => (
        <button
          key={row.key}
          type="button"
          role="option"
          aria-selected={i === activeIndex}
          onMouseDown={(event) => {
            // mousedown, not click: the textarea must not blur first.
            event.preventDefault();
            onPick(row.token);
          }}
          style={{
            all: "unset",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: 8,
            width: "100%",
            boxSizing: "border-box",
            padding: "6px 9px",
            borderRadius: radius.sm,
            background: i === activeIndex ? color.hover : "transparent",
          }}
        >
          <span
            style={{
              font: `600 12px ${font.sans}`,
              color: color.ink,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {row.label}
          </span>
          <span
            style={{
              marginLeft: "auto",
              font: `400 10.5px ${font.mono}`,
              color: color.muted2,
              flexShrink: 0,
            }}
          >
            {row.hint}
          </span>
        </button>
      ))}
    </div>
  );
}

export function MentionMenu({
  candidates,
  activeIndex,
  onPick,
  drop,
}: {
  candidates: MentionCandidate[];
  activeIndex: number;
  onPick: (token: string) => void;
  drop?: "up" | "down";
}) {
  const rows = candidates.map((candidate): Row => {
    const token = mentionCandidateToken(candidate);
    return {
      key: candidate.kind === "user" ? candidate.userKeyHex : candidate.agent.agent_id,
      label:
        candidate.kind === "user"
          ? candidate.label
          : candidate.agent.display_name || candidate.agent.agent_id,
      hint: `@${token}`,
      token,
    };
  });
  return (
    <TypeaheadMenu
      ariaLabel="Mention a person or agent"
      rows={rows}
      activeIndex={activeIndex}
      onPick={onPick}
      drop={drop}
    />
  );
}

/** The `[[` page-ref menu. The token it picks is the page ID — the composer
 *  wraps it into `[[page:<id>]]`; the id is what the wire and the runs module
 *  read, the title is only ever a display. */
export function PageMenu({
  pages,
  activeIndex,
  onPick,
}: {
  pages: PageMeta[];
  activeIndex: number;
  onPick: (pageId: string) => void;
}) {
  const rows = pages.map(
    (page): Row => ({
      key: page.id,
      label: page.title.trim() || "Untitled",
      hint: page.id,
      token: page.id,
    }),
  );
  return (
    <TypeaheadMenu ariaLabel="Link a page" rows={rows} activeIndex={activeIndex} onPick={onPick} />
  );
}
