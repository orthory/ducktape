import React from "react";
import ReactDOM from "react-dom/client";

// self-hosted fonts — the desktop build must render offline
import "@fontsource/geist-sans/400.css";
import "@fontsource/geist-sans/500.css";
import "@fontsource/geist-sans/600.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/500.css";
import "@fontsource/geist-mono/600.css";
import "@fontsource/ibm-plex-sans-kr/400.css";
import "@fontsource/ibm-plex-sans-kr/500.css";

import "./console/theme/global.css";
import { DucktapeConsole } from "./console/DucktapeConsole";
import { HuddleWindow } from "./console/views/huddle/HuddleWindow";
import { TrayPopover } from "./console/views/tray/TrayPopover";

// Auxiliary windows pick their surface via `?view=`: the frameless menu-bar
// popover (macOS, `view=tray`) and the popped-out huddle card (`view=huddle`).
// Every other window is the full console.
const view = new URLSearchParams(window.location.search).get("view");

// dev-only: connect the tauri-plugin-mcp guest bindings so the socket helper
// (app/scripts/tauri-debug.mjs) can run JS / inspect the DOM in this webview.
// screenshots work without it; the DOM/JS commands need it. never in release.
if (import.meta.env.DEV) {
  void import("tauri-plugin-mcp")
    .then(({ setupPluginListeners }) => setupPluginListeners())
    .catch(() => {});
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {view === "tray" ? <TrayPopover /> : view === "huddle" ? <HuddleWindow /> : <DucktapeConsole />}
  </React.StrictMode>,
);
