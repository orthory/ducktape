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
import { ErrorBoundary } from "./console/layout/ErrorBoundary";

// Auxiliary windows pick their surface via `?view=`: the frameless menu-bar
// popover (macOS, `view=tray`) and the popped-out huddle card (`view=huddle`).
// Every other window is the full console.
const view = new URLSearchParams(window.location.search).get("view");

// dev-only: install the tauri-agent guest instrumentation so the `tauri-agent`
// CLI / MCP server can snapshot the semantic tree, drive input, capture logs,
// and render DOM-SVG screenshots in this webview. One instance per window,
// labelled by the real Tauri window label. Never in release.
if (import.meta.env.DEV) {
  void (async () => {
    const [{ WebviewAgentInstrumentation }, { getCurrentWindow }] = await Promise.all([
      import("@byeongsu-hong/tauri-plugin-agent"),
      import("@tauri-apps/api/window"),
    ]);
    new WebviewAgentInstrumentation({ windowLabel: getCurrentWindow().label }).install();
  })().catch(() => {});
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* Outermost boundary: catches a throw in the provider/frame itself (above
        WindowFrame's inner boundary) and funnels global unhandled errors /
        rejections — so nothing ends in a blank white window or a silent drop. */}
    <ErrorBoundary global>
      {view === "tray" ? <TrayPopover /> : view === "huddle" ? <HuddleWindow /> : <DucktapeConsole />}
    </ErrorBoundary>
  </React.StrictMode>,
);
