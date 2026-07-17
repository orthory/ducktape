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
import { ErrorBoundary } from "./console/layout/ErrorBoundary";
import { installHistoryButtons } from "./console/dom/history-buttons";
import { installAutocompleteDefault } from "./console/dom/suppress-autocomplete";

// No autocomplete/autocorrect/autocapitalize on inputs by default (kills the
// history dropdown and macOS WKWebView's as-you-type completion) — see the
// module. Runs before render so the observer is live for every field React
// mounts. Applies to whichever surface this window is (console / tray / huddle).
installAutocompleteDefault();

// Mouse back/forward buttons + Alt+Arrow → history traversal. The embedded
// engines don't wire these browser-chrome inputs themselves; the entries they
// traverse are the console store's nav slice (see store/nav-history.ts).
installHistoryButtons();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* Outermost boundary: catches a throw in the provider/frame itself (above
        WindowFrame's inner boundary) and funnels global unhandled errors /
        rejections — so nothing ends in a blank white window or a silent drop. */}
    <ErrorBoundary global>
      <DucktapeConsole />
    </ErrorBoundary>
  </React.StrictMode>,
);
