// The person at the top of the Account view: avatar initials, the canonical
// display-name editor (setDisplayName routes bound → identity SetAccountName,
// unbound → profiles SetName), and the account id this device's node resolves
// to. The node key and workspace role deliberately do NOT render here — they
// are the Node page's facts; this card is about the account.

import { FinalizationMark } from "../../components/FinalizationMark";
import { shortKey } from "../../../domain/names";
import { opKey } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { copyText, HoverButton, outlineButton, smallMono } from "../settings/parts";

export const initialsOf = (name: string): string => {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length === 0) return "?";
  return parts
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
};

export function ProfileCard({ accountId }: { accountId: string | undefined }) {
  const { state, actions } = useDucktape();
  const accountLine = accountId
    ? `${shortKey(accountId, 14, 8)} · account id`
    : "not linked to an account yet";

  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        padding: 15,
        display: "flex",
        alignItems: "center",
        gap: 13,
        background: color.paper,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 40,
          height: 40,
          borderRadius: "50%",
          background: "#cdcdcd",
          color: color.muted3,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          font: `600 15px ${font.sans}`,
        }}
      >
        {initialsOf(state.author)}
      </span>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          <input
            aria-label="Display name"
            value={state.author}
            onChange={(event) => actions.setAuthor(event.target.value)}
            onBlur={(event) => actions.setDisplayName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            style={{
              all: "unset",
              width: Math.max(58, Math.min(230, state.author.length * 8 + 12)),
              font: `600 13.5px ${font.sans}`,
              color: color.ink,
            }}
          />
          <FinalizationMark op={state.ops[opKey.profile()]} />
        </div>
        <div style={{ ...smallMono, marginTop: 3 }} title={accountId ?? accountLine}>
          {accountLine}
        </div>
      </div>

      <HoverButton
        ariaLabel="Copy account id"
        onClick={() => accountId && copyText(accountId)}
        hoverBg={color.titlebar}
        disabled={!accountId}
        style={outlineButton}
      >
        Copy id
      </HoverButton>
    </div>
  );
}
