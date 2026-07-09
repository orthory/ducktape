// The composer's @mention typeahead popover — the Pages SlashMenu idiom
// (role="listbox" / role="option", activeIndex highlight, mousedown-to-pick).
// Opens UPWARD (bottom: 100%): the composer sits at the bottom of the pane,
// so a downward menu would render off-screen.

import { color, font, radius, shadow } from "../../theme/tokens";
import { mentionCandidateToken, type MentionCandidate } from "./mention";

export function MentionMenu({
  candidates,
  activeIndex,
  onPick,
}: {
  candidates: MentionCandidate[];
  activeIndex: number;
  onPick: (token: string) => void;
}) {
  if (candidates.length === 0) return null;
  return (
    <div
      role="listbox"
      aria-label="Mention a person or agent"
      style={{
        position: "absolute",
        zIndex: 20,
        bottom: "100%",
        left: 0,
        marginBottom: 6,
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
      {candidates.map((candidate, i) => {
        const key = candidate.kind === "user" ? candidate.userKeyHex : candidate.agent.agent_id;
        const label =
          candidate.kind === "user"
            ? candidate.label
            : candidate.agent.display_name || candidate.agent.agent_id;
        const token = mentionCandidateToken(candidate);
        return (
          <button
            key={key}
            type="button"
            role="option"
            aria-selected={i === activeIndex}
            onMouseDown={(event) => {
              // mousedown, not click: the textarea must not blur first.
              event.preventDefault();
              onPick(token);
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
              {label}
            </span>
            <span
              style={{
                marginLeft: "auto",
                font: `400 10.5px ${font.mono}`,
                color: color.muted2,
                flexShrink: 0,
              }}
            >
              @{token}
            </span>
          </button>
        );
      })}
    </div>
  );
}
