// Sidebar + the routed screen body, plus the error strip. Routing is the
// registry lookup: state.screen is a module id or the shell-owned "settings".

import { moduleById } from "../modules/registry";
import { useDucktape } from "../store/use-ducktape";
import { color, font } from "../theme/tokens";
import { SettingsView } from "../views/settings/SettingsView";
import { Sidebar } from "./Sidebar";

function resolveScreen(screen: string) {
  if (screen === "settings") return SettingsView;
  return moduleById(screen)?.Screen ?? SettingsView;
}

function ErrorStrip() {
  const { state, actions } = useDucktape();
  if (!state.error) return null;
  return (
    <div
      style={{
        position: "absolute",
        bottom: 13,
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: 10,
        maxWidth: 560,
        padding: "8px 13px",
        borderRadius: 9,
        background: color.dark,
        color: color.onDark,
        font: `500 11.5px ${font.mono}`,
        animation: "ik-fade .18s ease-out",
        zIndex: 20,
      }}
    >
      <span
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {state.error}
      </span>
      <button
        onClick={actions.dismissError}
        style={{
          all: "unset",
          cursor: "pointer",
          color: color.muted2,
          font: `600 11px ${font.sans}`,
        }}
      >
        dismiss
      </button>
    </div>
  );
}

export function ConsoleShell() {
  const { state } = useDucktape();
  const Screen = resolveScreen(state.screen);

  return (
    <div style={{ display: "flex", flex: 1, minHeight: 0, position: "relative" }}>
      <Sidebar />
      <div style={{ flex: 1, minWidth: 0, display: "flex" }}>
        <Screen />
      </div>
      <ErrorStrip />
    </div>
  );
}
