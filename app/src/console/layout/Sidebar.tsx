// The 74px icon rail: brand, the view-mode toggle (only while node control is
// available — ADR A5/A6: the NODE rail is a conditional surface, absent for
// clients and non-owners, not disabled), one entry per module of the active
// rail, and settings. Within a rail, in-view role checks own op-level
// authority.

import { Icon, type IconName } from "../components/Icon";
import { moduleFilterOf, modulesInSection } from "../modules/registry";
import type { NavSection } from "../modules/module-def";
import { nodeControlAvailable } from "../store/state";
import { useDucktape } from "../store/use-ducktape";
import { color, font, radius } from "../theme/tokens";
const navBg = (active: boolean) => (active ? color.hover : "transparent");
const navFg = (active: boolean) => (active ? color.inkSoft : color.muted);
const navIc = (active: boolean) => (active ? color.ink : color.iconIdle);

type ModeEntry = {
  id: NavSection;
  icon: IconName;
  label: string;
  title: string;
};

const LOCAL_MODES: ReadonlyArray<ModeEntry> = [
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
      {LOCAL_MODES.map((entry) => {
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
                color: active ? color.onDark : color.muted,
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
  const canControl = nodeControlAvailable(state);
  // Without node control any persisted "operator" mode falls back to the
  // account rail — the NODE surface is absent, so it cannot be selected.
  const mode = canControl ? state.viewMode : "user";
  const rail = modulesInSection(mode, moduleFilterOf(state));

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

      {canControl && <ModeToggle mode={mode} onSelect={actions.setViewMode} />}

      {rail.map((mod) => {
        // at Home the layer covers the routed screen, so no rail entry is the
        // visible surface — only the avatar below highlights.
        const active = !state.atHome && state.screen === mod.id;
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

      {/* The account (me) chip lives on the far-left network rail now; this
          column is pure module nav + node/app chrome (settings, theme). */}
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
          background: navBg(!state.atHome && state.screen === "settings"),
          color: navIc(!state.atHome && state.screen === "settings"),
        }}
      >
        <Icon
          name="settings"
          size={18}
          color={navIc(!state.atHome && state.screen === "settings")}
        />
      </button>

      {/* Light/dark switch — a peer of the gear, not a screen. The sun/moon
          glyph keeps it distinct from Settings (which now wears a real cog). */}
      <button
        onClick={actions.toggleTheme}
        title={state.theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        aria-label="Toggle light/dark theme"
        style={{
          all: "unset",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: 34,
          height: 34,
          borderRadius: 9,
          marginTop: 2,
          background: "transparent",
          color: color.iconIdle,
        }}
      >
        <Icon name={state.theme === "dark" ? "moon" : "sun"} size={18} color={color.iconIdle} />
      </button>
    </div>
  );
}
