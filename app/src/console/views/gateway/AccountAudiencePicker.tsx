// The explicit-accounts audience picker for the Gateway route editor. The owner
// is never implicit, so this is a plain checklist of known accounts plus a raw
// hex escape hatch and a one-click "include my account" affordance.

import { useState } from "react";

import { MAX_AUDIENCE_ACCOUNTS } from "../../../domain/gateway-client";
import { normalizeKey } from "../../../domain/names";
import { color, font } from "../../theme/tokens";
import { buttonStyle, fieldStyle } from "./GatewayView";

interface AccountAudiencePickerProps {
  /** hex(account id) roster to offer as checkboxes (known accounts). */
  roster: string[];
  /** Human label for one account id hex (handle, name, or short id). */
  label: (id: string) => string;
  /** Currently selected account id hexes. */
  selected: string[];
  onChange: (next: string[]) => void;
  /** This node's owning account id hex, or "" when unbound. */
  ownerAccountId: string;
}

export function AccountAudiencePicker({ roster, label, selected, onChange, ownerAccountId }: AccountAudiencePickerProps) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  // Off-roster ids added by hex still render (and stay togglable) in the list.
  const rows = Array.from(new Set([...roster, ...selected]));
  const ownerIncluded = Boolean(ownerAccountId) && selected.includes(ownerAccountId);
  // At the cap every ADD is blocked: the builder would silently drop the
  // overflow out of a policy the operator then signs. Removal stays open.
  const atCap = selected.length >= MAX_AUDIENCE_ACCOUNTS;
  const addBlocked = !draft.trim() || atCap;

  const toggle = (id: string, on: boolean): void => {
    if (on && (atCap || selected.includes(id))) return;
    onChange(on ? [...selected, id] : selected.filter((item) => item !== id));
  };

  const addDraft = (): void => {
    const id = normalizeKey(draft);
    if (!/^(?:[0-9a-f]{2})+$/.test(id)) {
      setError("Enter an even-length hex account id.");
      return;
    }
    setError(null);
    setDraft("");
    if (!selected.includes(id) && !atCap) onChange([...selected, id]);
  };

  return (
    <div aria-label="Explicit accounts" style={{ marginTop: 13, display: "grid", gap: 8 }}>
      <div style={{ color: color.muted, font: `500 9.5px/1.45 ${font.sans}` }}>
        Your own account is not included automatically. {selected.length}/{MAX_AUDIENCE_ACCOUNTS} selected.
      </div>
      {atCap && (
        <div role="status" style={{ color: color.danger, font: `500 9.5px/1.45 ${font.sans}` }}>
          A route policy carries a maximum of {MAX_AUDIENCE_ACCOUNTS} accounts. Remove one to add another.
        </div>
      )}
      {ownerAccountId && (
        <button
          type="button"
          disabled={ownerIncluded || atCap}
          onClick={() => toggle(ownerAccountId, true)}
          aria-label="Include my account"
          style={{ ...buttonStyle(ownerIncluded || atCap), textAlign: "center" }}
        >
          {ownerIncluded ? "Your account is included" : "Include my account"}
        </button>
      )}
      {rows.length > 0 && (
        <div style={{ display: "grid", gap: 4 }}>
          {rows.map((id) => (
            <label key={id} style={{ display: "flex", alignItems: "center", gap: 6, color: color.inkSoft, font: `500 10px ${font.sans}`, overflowWrap: "anywhere" }}>
              <input type="checkbox" checked={selected.includes(id)} disabled={atCap && !selected.includes(id)} onChange={(event) => toggle(id, event.target.checked)} aria-label={`Include ${label(id)}`} />
              {label(id)}
            </label>
          ))}
        </div>
      )}
      <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 6 }}>
        <input aria-label="Account id hex" value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="account id hex" spellCheck={false} disabled={atCap} style={fieldStyle} />
        <button type="button" disabled={addBlocked} onClick={addDraft} aria-label="Add account id" style={buttonStyle(addBlocked)}>Add</button>
      </div>
      {error && <span role="alert" style={{ color: color.danger, font: `500 9.5px ${font.sans}` }}>{error}</span>}
    </div>
  );
}
