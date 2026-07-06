import { useEffect, useState } from "react";

import { color, font } from "../../theme/tokens";
import type { HlToken } from "./highlight";

// github-light's default foreground — text outside any token, kept in sync with
// the shiki theme used by ./highlight. Inlined so this module doesn't statically
// pull in shiki (see the dynamic import below).
const CODE_FG = "#24292e";

// A read-only code pane: syntax-highlighted (shiki, lazy-loaded on first file
// view) with a line-number gutter, falling back to plain monochrome text until
// highlighting resolves or for languages we don't highlight. Layout mirrors the
// viewer's prior line loop.
export function CodeView({ text, filename }: { text: string; filename: string | null }) {
  const [highlighted, setHighlighted] = useState<HlToken[][] | null>(null);

  useEffect(() => {
    let alive = true;
    setHighlighted(null);
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
  const lines: HlToken[][] = highlighted ?? text.split("\n").map((line) => [{ content: line }]);

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
              color: highlighted ? CODE_FG : color.inkSoft,
              paddingLeft: 13,
              paddingRight: 24,
            }}
          >
            {tokens.length === 0 ? (
              " "
            ) : (
              tokens.map((token, tokenIndex) => (
                <span key={tokenIndex} style={token.color ? { color: token.color } : undefined}>
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
