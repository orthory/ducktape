import { useEffect, useState, type CSSProperties } from "react";

import { color, font } from "../../theme/tokens";
import type { HlToken } from "./highlight";

// shiki tokenizes synchronously on the main thread; above this size that would
// jank the webview for seconds (lockfiles, minified bundles), so we render such
// files as plain text — still fully readable, just uncolored.
const HIGHLIGHT_MAX_BYTES = 200_000;

// A read-only code pane: syntax-highlighted (shiki, lazy-loaded on first file
// view) with a line-number gutter, falling back to plain monochrome text until
// highlighting resolves or for languages we don't highlight. Layout mirrors the
// viewer's prior line loop.
export function CodeView({ text, filename }: { text: string; filename: string | null }) {
  const [highlighted, setHighlighted] = useState<HlToken[][] | null>(null);

  useEffect(() => {
    let alive = true;
    setHighlighted(null);
    if (text.length > HIGHLIGHT_MAX_BYTES) return; // too large — stay plain
    // shiki + its grammars live in a separate chunk fetched only when code is
    // first viewed, keeping them out of the app's startup bundle.
    void (async () => {
      const { highlightLines, langForFilename } = await import("./highlight");
      const lang = filename ? langForFilename(filename) : null;
      if (!lang) return;
      const lines = await highlightLines(text, lang);
      if (alive) setHighlighted(lines);
    })();
    return () => {
      alive = false;
    };
  }, [text, filename]);

  // one token array per line; plain text is a single uncolored token per line.
  // split on CRLF-or-LF so the plain fallback matches shiki's stripped output
  // (no trailing \r) and the view doesn't shift when highlighting resolves.
  const lines: HlToken[][] = highlighted ?? text.split(/\r?\n/).map((line) => [{ content: line }]);

  return (
    <div style={{ minWidth: "max-content", padding: "8px 0 20px" }}>
      {lines.map((tokens, index) => (
        <div
          key={index}
          style={{
            display: "flex",
            font: `400 12px ${font.mono}`,
            lineHeight: 1.65,
            minWidth: "max-content",
          }}
        >
          <span
            style={{
              width: 48,
              flexShrink: 0,
              textAlign: "right",
              paddingRight: 12,
              color: color.iconIdle,
              userSelect: "none",
              background: color.sidebar,
            }}
          >
            {index + 1}
          </span>
          <span
            style={{
              flex: 1,
              whiteSpace: "pre",
              // Highlighted tokens carry their own per-theme color (see below);
              // this is the color of the plain fallback, and of any token shiki
              // left unstyled — a theme token, so it inverts with the app.
              color: color.inkSoft,
              paddingLeft: 13,
              paddingRight: 24,
            }}
          >
            {tokens.length === 0 ? (
              " "
            ) : (
              tokens.map((token, tokenIndex) => (
                <span
                  key={tokenIndex}
                  className={token.style ? "code-tok" : undefined}
                  style={token.style as CSSProperties | undefined}
                >
                  {token.content}
                </span>
              ))
            )}
          </span>
        </div>
      ))}
    </div>
  );
}
