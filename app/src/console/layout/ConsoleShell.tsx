// Sidebar + the routed screen body, plus the error strip. Routing is the
// registry lookup: state.screen is a module id or the shell-owned "settings".
// Cross-module search is not a route — it's the ⌘K palette the shell overlays.

import { useEffect } from "react";

import { moduleById } from "../modules/registry";
import { useDucktape } from "../store/use-ducktape";
import { color, font } from "../theme/tokens";
import { HuddleDock } from "../views/chat/Huddle";
import { SearchModal } from "../views/search/SearchModal";
import { SettingsView } from "../views/settings/SettingsView";
import { CHANNEL_RAIL_WIDTH } from "../views/chat/ChatView";
import { Sidebar, SIDEBAR_ICON_RAIL_WIDTH } from "./Sidebar";

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
  const { state, actions } = useDucktape();
  const Screen = resolveScreen(state.screen);

  // ⌘K / Ctrl-K opens the command palette from anywhere. Escape and backdrop
  // clicks close it from within the modal.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && (event.key === "k" || event.key === "K")) {
        event.preventDefault();
        actions.openSearch();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [actions]);

  return (
    <div style={{ display: "flex", flex: 1, minHeight: 0, position: "relative" }}>
      <Sidebar />
      <div style={{ flex: 1, minWidth: 0, display: "flex" }}>
        <Screen />
      </div>
      {/* the live-huddle session card floats above EVERY screen — a hot mic
          must never lose its mute/leave controls to navigation. Sized to sit
          INSIDE the chat screen's channel rail (the dock's own 8px margins
          inset the card within this span). */}
      <div
        style={{
          position: "absolute",
          left: SIDEBAR_ICON_RAIL_WIDTH,
          bottom: 6,
          width: CHANNEL_RAIL_WIDTH,
          zIndex: 25,
        }}
      >
        <HuddleDock />
      </div>
      <ErrorStrip />
      {state.searchOpen && <SearchModal />}
    </div>
  );
}
