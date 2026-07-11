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
import { installAutocompleteDefault } from "./console/dom/suppress-autocomplete";

// Auxiliary windows pick their surface via `?view=`: the frameless menu-bar
// popover (macOS, `view=tray`) and the popped-out huddle card (`view=huddle`).
// Every other window is the full console.
const view = new URLSearchParams(window.location.search).get("view");

// dev-only: install the tauri-agent guest instrumentation so the `tauri-agent`
// CLI / MCP server can snapshot the semantic tree, drive input, capture logs,
// and render DOM-SVG screenshots in this webview. One instance per window,
// labelled by the real Tauri window label. Never in release.
if (import.meta.env.DEV || import.meta.env.VITE_TAURI_AGENT === "1") {
  void (async () => {
    const [{ WebviewAgentInstrumentation }, { getCurrentWindow }] = await Promise.all([
      import("@byeongsu-hong/tauri-agent-plugin"),
      import("@tauri-apps/api/window"),
    ]);
    new WebviewAgentInstrumentation({ windowLabel: getCurrentWindow().label }).install();
  })().catch(() => {});
}

// No autocomplete/autocorrect/autocapitalize on inputs by default (kills the
// history dropdown and macOS WKWebView's as-you-type completion) — see the
// module. Runs before render so the observer is live for every field React
// mounts. Applies to whichever surface this window is (console / tray / huddle).
installAutocompleteDefault();

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
