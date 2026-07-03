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
    <DucktapeConsole />
  </React.StrictMode>,
);
