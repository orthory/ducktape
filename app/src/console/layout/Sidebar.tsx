// The 74px icon rail: brand, one entry per registered module, settings. The
// rail knows no module by name — modules appear by registering.

import { Icon } from "../components/Icon";
import { MODULES } from "../modules/registry";
import { useDucktape } from "../store/use-ducktape";
import { color, font } from "../theme/tokens";

const navBg = (active: boolean) => (active ? "#e9e9e9" : "transparent");
const navFg = (active: boolean) => (active ? "#3a3934" : "#959595");
const navIc = (active: boolean) => (active ? "#26251f" : color.iconIdle);

export function Sidebar() {
  const { state, actions } = useDucktape();
  const rail = [...MODULES].sort((a, b) => a.nav.order - b.nav.order);

  return (
    <div
      style={{
        width: 74,
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
            <Icon name={mod.nav.icon} size={19} color={navIc(active)} />
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
