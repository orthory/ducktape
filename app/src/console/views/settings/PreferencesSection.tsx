// Local console preferences. Daemon start/stop lives on the Node view — the
// operator surface that owns it — not here.

import { useDucktape } from "../../store/use-ducktape";
import type { NotifyPrefs } from "../../store/state";
import { color } from "../../theme/tokens";
import { ControlRow, GroupCard, SectionLabel } from "./parts";

const ACCENTS = [
  color.accent,
  color.accentAlt1,
  color.accentAlt2,
  color.purple,
  color.red,
] as const;

type NotifyCategory = Exclude<keyof NotifyPrefs, "enabled" | "mutedChannels">;

const NOTIFICATION_CATEGORIES: ReadonlyArray<{
  key: NotifyCategory;
  title: string;
  desc: string;
}> = [
  {
    key: "mentions",
    title: "Mentions",
    desc: "When someone mentions you in a channel.",
  },
  {
    key: "replies",
    title: "Replies",
    desc: "When someone replies to one of your messages.",
  },
  {
    key: "huddles",
    title: "Huddles",
    desc: "When a channel huddle starts.",
  },
  {
    key: "runs",
    title: "Agent runs",
    desc: "When an agent run completes or needs attention.",
  },
  {
    key: "forge",
    title: "Forge",
    desc: "For Forge activity that needs your attention.",
  },
  {
    key: "governance",
    title: "Governance",
    desc: "For governance proposals and voting activity.",
  },
];

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

function Toggle({
  checked,
  name,
  accent,
  onToggle,
  disabled = false,
}: {
  checked: boolean;
  name: string;
  accent: string;
  onToggle: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={`Toggle ${name} notifications`}
      disabled={disabled}
      onClick={disabled ? undefined : onToggle}
      style={{
        all: "unset",
        position: "relative",
        display: "block",
        cursor: disabled ? "not-allowed" : "pointer",
        width: 32,
        height: 18,
        borderRadius: 999,
        background: checked ? accent : color.paper,
        boxShadow: checked
          ? `0 0 0 1px ${accent}`
          : `0 0 0 1px ${color.borderStrong}`,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          position: "absolute",
          top: 3,
          left: checked ? 17 : 3,
          width: 12,
          height: 12,
          borderRadius: "50%",
          background: checked ? color.paper : color.muted2,
          transition: "left 120ms ease",
        }}
      />
    </button>
  );
}

export function PreferencesSection() {
  const { state, actions } = useDucktape();
  const prefs = state.notifyPrefs;
  const activeChannel = state.activeChannel;
  const channelMuted =
    activeChannel !== null && prefs.mutedChannels.includes(activeChannel);

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

      <SectionLabel>NOTIFICATIONS</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Enable notifications"
          desc="Allow Ducktape to send native desktop notifications."
          control={
            <Toggle
              name="all"
              checked={prefs.enabled}
              accent={state.accent}
              onToggle={() =>
                actions.setNotifyPrefs({
                  ...prefs,
                  enabled: !prefs.enabled,
                })
              }
            />
          }
        />

        {NOTIFICATION_CATEGORIES.map(({ key, title, desc }) => (
          <div key={key} style={{ opacity: prefs.enabled ? 1 : 0.55 }}>
            <ControlRow
              title={title}
              desc={desc}
              last={key === "governance" && activeChannel === null}
              control={
                <Toggle
                  name={title}
                  checked={prefs[key]}
                  accent={state.accent}
                  disabled={!prefs.enabled}
                  onToggle={() =>
                    actions.setNotifyPrefs({
                      ...prefs,
                      [key]: !prefs[key],
                    })
                  }
                />
              }
            />
          </div>
        ))}

        {activeChannel !== null && (
          <ControlRow
            title={`${channelMuted ? "Unmute" : "Mute"} #${activeChannel}`}
            desc={
              channelMuted
                ? "Resume notifications from the current channel."
                : "Suppress notifications from the current channel."
            }
            last
            control={
              <Toggle
                name={`#${activeChannel}`}
                checked={!channelMuted}
                accent={state.accent}
                onToggle={() => actions.toggleChannelMute(activeChannel)}
              />
            }
          />
        )}
      </GroupCard>
    </>
  );
}
