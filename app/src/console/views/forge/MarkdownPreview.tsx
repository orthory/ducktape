import type { CSSProperties, ReactNode } from "react";
import Markdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { color, font, radius } from "../../theme/tokens";

// Rendered preview for .md / .mdx files. react-markdown emits React elements
// (no innerHTML), styled here with the console's tokens. MDX-specific JSX isn't
// executed — the prose, code, tables and lists render, which is the intent of a
// read-only preview. Links don't navigate the webview; images show a caption.

const heading = (size: number, mt: number): CSSProperties => ({
  font: `600 ${size}px ${font.sans}`,
  color: color.ink,
  lineHeight: 1.3,
  margin: `${mt}px 0 10px`,
});

const codeFont = `400 12.5px ${font.mono}`;

const COMPONENTS: Components = {
  h1: ({ children }) => (
    <h1 style={{ ...heading(24, 26), borderBottom: `1px solid ${color.borderSoft}`, paddingBottom: 8 }}>
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 style={{ ...heading(19, 24), borderBottom: `1px solid ${color.borderSoft}`, paddingBottom: 6 }}>
      {children}
    </h2>
  ),
  h3: ({ children }) => <h3 style={heading(16, 20)}>{children}</h3>,
  h4: ({ children }) => <h4 style={heading(14, 18)}>{children}</h4>,
  h5: ({ children }) => <h5 style={heading(13, 16)}>{children}</h5>,
  h6: ({ children }) => <h6 style={{ ...heading(12, 16), color: color.muted }}>{children}</h6>,
  p: ({ children }) => <p style={{ margin: "0 0 13px", lineHeight: 1.7 }}>{children}</p>,
  a: ({ children, href }) => (
    <a
      href={href}
      onClick={(e) => e.preventDefault()}
      title={href}
      style={{ color: color.accentAlt1, textDecoration: "none", cursor: "pointer", borderBottom: `1px solid ${color.border}` }}
    >
      {children}
    </a>
  ),
  ul: ({ children }) => (
    <ul style={{ margin: "0 0 13px", paddingLeft: 22, lineHeight: 1.7, listStyleType: "disc" }}>{children}</ul>
  ),
  ol: ({ children }) => (
    <ol style={{ margin: "0 0 13px", paddingLeft: 22, lineHeight: 1.7, listStyleType: "decimal" }}>{children}</ol>
  ),
  li: ({ children }) => <li style={{ margin: "3px 0", display: "list-item" }}>{children}</li>,
  blockquote: ({ children }) => (
    <blockquote
      style={{
        margin: "0 0 13px",
        padding: "2px 14px",
        borderLeft: `3px solid ${color.borderStrong}`,
        color: color.muted,
      }}
    >
      {children}
    </blockquote>
  ),
  hr: () => <hr style={{ border: 0, borderTop: `1px solid ${color.borderSoft}`, margin: "20px 0" }} />,
  strong: ({ children }) => <strong style={{ fontWeight: 650, color: color.ink }}>{children}</strong>,
  em: ({ children }) => <em style={{ fontStyle: "italic" }}>{children}</em>,
  code: ({ className, children }) => {
    const isBlock = /language-/.test(className ?? "");
    if (isBlock) {
      return (
        <code style={{ font: codeFont, color: color.inkSoft, whiteSpace: "pre", display: "block" }}>{children}</code>
      );
    }
    return (
      <code
        style={{
          font: codeFont,
          background: color.sunken,
          border: `1px solid ${color.borderSoft}`,
          borderRadius: radius.sm,
          padding: "1px 5px",
          color: color.accent,
        }}
      >
        {children}
      </code>
    );
  },
  pre: ({ children }) => (
    <pre
      style={{
        margin: "0 0 14px",
        padding: "12px 14px",
        background: color.sidebar,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        overflowX: "auto",
        lineHeight: 1.6,
      }}
    >
      {children}
    </pre>
  ),
  table: ({ children }) => (
    <div style={{ overflowX: "auto", margin: "0 0 14px" }}>
      <table style={{ borderCollapse: "collapse", font: `400 12.5px ${font.sans}` }}>{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th style={{ border: `1px solid ${color.border}`, padding: "6px 11px", background: color.sunken, textAlign: "left", fontWeight: 600 }}>
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td style={{ border: `1px solid ${color.border}`, padding: "6px 11px" }}>{children}</td>
  ),
  img: ({ alt, src }) => (
    <span
      title={typeof src === "string" ? src : undefined}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "3px 9px",
        border: `1px dashed ${color.border}`,
        borderRadius: radius.sm,
        font: `400 11.5px ${font.mono}`,
        color: color.muted2,
      }}
    >
      {alt ? `image: ${alt}` : "image"}
    </span>
  ),
};

export function MarkdownPreview({ text }: { text: string }): ReactNode {
  return (
    <div
      style={{
        padding: "20px 28px 44px",
        maxWidth: 860,
        font: `400 14px ${font.sans}`,
        color: color.inkSoft,
        wordBreak: "break-word",
      }}
    >
      <Markdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
        {text}
      </Markdown>
    </div>
  );
}
