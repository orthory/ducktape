// The identity account at the top of the Account view. Identity owns the
// canonical display name and AccountId; DuckDNS contributes only an optional
// `<handle>.duck` account name. Workspace membership and account identity do
// not depend on registering that alias.

import { useEffect, useState, type FormEvent } from "react";

import { handleError, normalizeHandle } from "../../../domain/duckdns-client";
import { shortKey } from "../../../domain/names";
import { FinalizationMark } from "../../components/FinalizationMark";
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
  const registered = accountId ? state.accountHandles[accountId] : undefined;
  const [draft, setDraft] = useState(registered ?? "");
  const [validation, setValidation] = useState<string | null>(null);

  useEffect(() => {
    setDraft(registered ?? "");
    setValidation(null);
  }, [accountId, registered]);

  const accountLine = accountId
    ? `${shortKey(accountId, 14, 8)} · account id`
    : "not linked to an account yet";
  const pending = state.ops[opKey.duckHandle()]?.phase === "pending";

  const submitHandle = (event: FormEvent) => {
    event.preventDefault();
    const handle = normalizeHandle(draft);
    const error = handleError(handle);
    setDraft(handle);
    setValidation(error);
    if (!error && accountId) actions.setDuckHandle(handle);
  };

  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        padding: 15,
        background: color.paper,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 13 }}>
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
          <div style={{ display: "flex", alignItems: "center", gap: 7, flexWrap: "wrap" }}>
            <input
              aria-label="Display name"
              value={state.author}
              disabled={!accountId}
              onChange={(event) => actions.setAuthor(event.target.value)}
              onBlur={(event) => accountId && actions.setDisplayName(event.currentTarget.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
              }}
              style={{
                all: "unset",
                width: Math.max(58, Math.min(230, state.author.length * 8 + 12)),
                font: `600 13.5px ${font.sans}`,
                color: accountId ? color.ink : color.muted,
              }}
            />
            <FinalizationMark op={state.ops[opKey.accountName()]} />
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

      <div
        style={{
          marginTop: 14,
          paddingTop: 13,
          borderTop: `1px solid ${color.border}`,
        }}
      >
        <div style={{ display: "flex", alignItems: "baseline", gap: 7, flexWrap: "wrap" }}>
          <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>Duck name</span>
          <span style={{ font: `400 11px ${font.sans}`, color: color.muted }}>
            Optional account name — your identity works without one.
          </span>
          <FinalizationMark op={state.ops[opKey.duckHandle()]} />
        </div>

        <form
          onSubmit={submitHandle}
          style={{ display: "flex", alignItems: "center", gap: 7, marginTop: 8, flexWrap: "wrap" }}
        >
          <label
            style={{
              display: "flex",
              alignItems: "center",
              border: `1px solid ${validation ? color.danger : color.border}`,
              borderRadius: radius.md,
              background: color.sunken,
              overflow: "hidden",
            }}
          >
            <input
              aria-label="Duck name"
              value={draft}
              disabled={!accountId || pending}
              placeholder="your-name"
              onChange={(event) => {
                setDraft(event.target.value);
                setValidation(null);
              }}
              style={{
                width: 150,
                border: 0,
                outline: 0,
                padding: "7px 8px",
                background: "transparent",
                font: `500 12px ${font.mono}`,
                color: color.ink,
              }}
            />
            <span
              style={{
                paddingRight: 8,
                font: `500 12px ${font.mono}`,
                color: color.muted,
              }}
            >
              .duck
            </span>
          </label>
          <button
            type="submit"
            disabled={!accountId || pending || !draft.trim()}
            aria-label={registered ? "Update Duck name" : "Register Duck name"}
            style={{
              ...outlineButton,
              cursor: !accountId || pending || !draft.trim() ? "not-allowed" : "pointer",
              opacity: !accountId || pending || !draft.trim() ? 0.55 : 1,
            }}
          >
            {registered ? "Update" : "Register"}
          </button>
          {registered && (
            <HoverButton
              ariaLabel="Remove Duck name"
              onClick={() => actions.setDuckHandle(null)}
              disabled={pending}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Remove
            </HoverButton>
          )}
        </form>
        {validation && (
          <div role="alert" style={{ marginTop: 5, font: `500 10.5px ${font.sans}`, color: color.danger }}>
            {validation}
          </div>
        )}
      </div>
    </div>
  );
}
