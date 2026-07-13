// The explicit-accounts audience picker for the Gateway route editor. The owner
// is never implicit, so this is a plain checklist of known accounts plus a raw
// hex escape hatch and a one-click "include my account" affordance.

import { useState } from "react";

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

  const toggle = (id: string, on: boolean): void => {
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
    if (!selected.includes(id)) onChange([...selected, id]);
  };

  return (
    <div aria-label="Explicit accounts" style={{ marginTop: 13, display: "grid", gap: 8 }}>
      <div style={{ color: color.muted, font: `500 9.5px/1.45 ${font.sans}` }}>
        Your own account is not included automatically. {selected.length}/32 selected.
      </div>
      {ownerAccountId && (
        <button
          type="button"
          disabled={ownerIncluded}
          onClick={() => toggle(ownerAccountId, true)}
          aria-label="Include my account"
          style={{ ...buttonStyle(ownerIncluded), textAlign: "center" }}
        >
          {ownerIncluded ? "Your account is included" : "Include my account"}
        </button>
      )}
      {rows.length > 0 && (
        <div style={{ display: "grid", gap: 4 }}>
          {rows.map((id) => (
            <label key={id} style={{ display: "flex", alignItems: "center", gap: 6, color: color.inkSoft, font: `500 10px ${font.sans}`, overflowWrap: "anywhere" }}>
              <input type="checkbox" checked={selected.includes(id)} onChange={(event) => toggle(id, event.target.checked)} aria-label={`Include ${label(id)}`} />
              {label(id)}
            </label>
          ))}
        </div>
      )}
      <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 6 }}>
        <input aria-label="Account id hex" value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="account id hex" spellCheck={false} style={fieldStyle} />
        <button type="button" disabled={!draft.trim()} onClick={addDraft} aria-label="Add account id" style={buttonStyle(!draft.trim())}>Add</button>
      </div>
      {error && <span role="alert" style={{ color: color.danger, font: `500 9.5px ${font.sans}` }}>{error}</span>}
    </div>
  );
}
