// Local console preferences. Daemon start/stop lives on the Node view — the
// operator surface that owns it — not here.

import { useDucktape } from "../../store/use-ducktape";
import { color } from "../../theme/tokens";
import { ControlRow, GroupCard, SectionLabel } from "./parts";

const ACCENTS = [
  color.accent,
  color.accentAlt1,
  color.accentAlt2,
  color.purple,
  color.red,
] as const;

function AccentPicker({
  value,
  onPick,
}: {
  value: string;
  onPick: (accent: string) => void;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
      {ACCENTS.map((accent) => (
        <button
          key={accent}
          type="button"
          aria-label={`Set accent ${accent}`}
          title={accent}
          onClick={() => onPick(accent)}
          style={{
            all: "unset",
            cursor: "pointer",
            width: 22,
            height: 22,
            borderRadius: "50%",
            background: accent,
            boxShadow:
              value === accent
                ? `0 0 0 2px ${color.paper}, 0 0 0 4px ${accent}`
                : `0 0 0 1px ${color.borderStrong}`,
          }}
        />
      ))}
    </div>
  );
}

export function PreferencesSection() {
  const { state, actions } = useDucktape();
  return (
    <>
      <SectionLabel>PREFERENCES</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Accent"
          desc="Used for active navigation, focus, and primary controls."
          last
          control={<AccentPicker value={state.accent} onPick={actions.setAccent} />}
        />
      </GroupCard>
    </>
  );
}
