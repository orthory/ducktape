// The 74px icon rail: brand, a USER ⇄ NODE OPERATOR mode toggle, one entry per
// module of the active rail, and settings. The rail knows no module by name —
// modules appear by registering, and the toggle picks which section's group of
// modules the rail shows (see registry.ts / module-def.ts). Neither rail confers
// authority; the toggle is purely which surfaces are on screen.

import { Icon, type IconName } from "../components/Icon";
import { modulesInSection } from "../modules/registry";
import type { NavSection } from "../modules/module-def";
import { useDucktape } from "../store/use-ducktape";
import { color, font, radius } from "../theme/tokens";

const navBg = (active: boolean) => (active ? "#e9e9e9" : "transparent");
const navFg = (active: boolean) => (active ? "#3a3934" : "#959595");
const navIc = (active: boolean) => (active ? "#26251f" : color.iconIdle);

const MODES: ReadonlyArray<{
  id: NavSection;
  icon: IconName;
  label: string;
  title: string;
}> = [
  { id: "user", icon: "members", label: "USER", title: "User apps" },
  { id: "operator", icon: "node", label: "NODE", title: "Node operator" },
];

function ModeToggle({
  mode,
  onSelect,
}: {
  mode: NavSection;
  onSelect: (mode: NavSection) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label="View mode"
      style={{
        width: 58,
        display: "flex",
        flexDirection: "column",
        gap: 3,
        padding: 3,
        borderRadius: radius.md,
        background: color.sunken,
        border: `1px solid ${color.borderSoft}`,
        marginBottom: 8,
      }}
    >
      {MODES.map((entry) => {
        const active = mode === entry.id;
        return (
          <button
            key={entry.id}
            role="tab"
            aria-selected={active}
            title={entry.title}
            onClick={() => onSelect(entry.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              boxSizing: "border-box",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 5,
              padding: "6px 0",
              borderRadius: radius.sm,
              background: active ? color.dark : "transparent",
              transition: "background .12s",
            }}
          >
            <Icon name={entry.icon} size={13} color={active ? color.onDark : color.iconIdle} />
            <span
              style={{
                font: `700 8.5px ${font.mono}`,
                letterSpacing: ".08em",
                color: active ? color.onDark : "#959595",
              }}
            >
              {entry.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** The icon rail's fixed width — ConsoleShell offsets the floating huddle
 *  dock by exactly this, so the two must agree. */
export const SIDEBAR_ICON_RAIL_WIDTH = 74;

export function Sidebar() {
  const { state, actions } = useDucktape();
  const rail = modulesInSection(state.viewMode);

  return (
    <div
      style={{
        width: SIDEBAR_ICON_RAIL_WIDTH,
        flexShrink: 0,
        borderRight: `1px solid ${color.borderSoft}`,
        background: color.sidebar,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        padding: "13px 0",
        gap: 4,
        color: color.iconIdle,
      }}
    >
      <div
        style={{
          width: 30,
          height: 30,
          borderRadius: 9,
          background: color.dark,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          font: `600 14px ${font.mono}`,
          color: color.onDark,
          marginBottom: 9,
        }}
      >
        D
      </div>

      <ModeToggle mode={state.viewMode} onSelect={actions.setViewMode} />

      {rail.map((mod) => {
        const active = state.screen === mod.id;
        return (
          <button
            key={mod.id}
            onClick={() => actions.setScreen(mod.id)}
            style={{
              all: "unset",
              cursor: "pointer",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 4,
              width: 58,
              padding: "8px 0",
              borderRadius: 10,
              background: navBg(active),
              transition: "background .12s",
            }}
          >
            <span style={{ position: "relative", display: "flex" }}>
              <Icon name={mod.nav.icon} size={19} color={navIc(active)} />
            </span>
            <span style={{ font: `600 9.5px ${font.sans}`, color: navFg(active) }}>
              {mod.nav.label}
            </span>
          </button>
        );
      })}

      <div style={{ flex: 1 }} />

      <button
        onClick={() => actions.setScreen("settings")}
        title="Settings"
        style={{
          all: "unset",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: 34,
          height: 34,
          borderRadius: 9,
          background: navBg(state.screen === "settings"),
          color: navIc(state.screen === "settings"),
        }}
      >
        <Icon name="settings" size={18} color={navIc(state.screen === "settings")} />
      </button>
    </div>
  );
}
