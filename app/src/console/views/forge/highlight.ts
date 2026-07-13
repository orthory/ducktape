// Lazy, WASM-free syntax highlighter for the forge file viewer.
//
// Uses shiki's JavaScript RegExp engine (no oniguruma WASM), so it works in the
// offline Tauri webview with no extra asset, and only bundles the languages
// imported below. Returns per-line colored tokens that drop straight into the
// viewer's line-number gutter.

import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import githubDark from "@shikijs/themes/github-dark";
import githubLight from "@shikijs/themes/github-light";
import bash from "@shikijs/langs/bash";
import css from "@shikijs/langs/css";
import go from "@shikijs/langs/go";
import html from "@shikijs/langs/html";
import javascript from "@shikijs/langs/javascript";
import json from "@shikijs/langs/json";
import jsx from "@shikijs/langs/jsx";
import markdown from "@shikijs/langs/markdown";
import rust from "@shikijs/langs/rust";
import toml from "@shikijs/langs/toml";
import tsx from "@shikijs/langs/tsx";
import typescript from "@shikijs/langs/typescript";
import yaml from "@shikijs/langs/yaml";

// Tokenize against BOTH github themes at once. With `defaultColor: false` shiki
// emits no baked-in `color`; each token instead carries `--shiki-light` and
// `--shiki-dark` custom properties, and the `.code-tok` rule in global.css picks
// one by the live `data-theme` on <html>. So a theme flip re-paints instantly —
// no re-tokenize, and no second source of truth for "what theme are we in".
const THEMES = { light: "github-light", dark: "github-dark" } as const;

// extension (lowercase, no dot) -> a loaded shiki language id. An extension not
// here renders as plain text — never an error.
const LANG_BY_EXT: Record<string, string> = {
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "tsx",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  rs: "rust",
  toml: "toml",
  json: "json",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  md: "markdown",
  mdx: "markdown",
  markdown: "markdown",
  css: "css",
  scss: "css",
  html: "html",
  htm: "html",
  yaml: "yaml",
  yml: "yaml",
  go: "go",
};

let highlighterPromise: Promise<HighlighterCore> | null = null;

function highlighter(): Promise<HighlighterCore> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighterCore({
      themes: [githubLight, githubDark],
      langs: [
        typescript,
        tsx,
        javascript,
        jsx,
        rust,
        toml,
        json,
        bash,
        markdown,
        css,
        html,
        yaml,
        go,
      ],
      // forgiving: a grammar regex the JS engine can't compile degrades to a
      // partial highlight instead of throwing.
      engine: createJavaScriptRegexEngine({ forgiving: true }),
    });
  }
  return highlighterPromise;
}

/** The shiki language id for a filename, or null if we don't highlight it. */
export function langForFilename(name: string): string | null {
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
  return LANG_BY_EXT[ext] ?? null;
}

/** The shiki language id for a code-fence tag (```rust, ```ts, …), or null.
 *  Accepts the extension aliases above and the loaded language ids themselves. */
export function langForTag(tag: string): string | null {
  const t = tag.toLowerCase();
  if (LANG_BY_EXT[t]) return LANG_BY_EXT[t];
  return Object.values(LANG_BY_EXT).includes(t) ? t : null;
}

export interface HlToken {
  content: string;
  /** Per-theme colors as CSS custom properties (`--shiki-light` / `--shiki-dark`),
   *  applied inline; `.code-tok` in global.css resolves them per `data-theme`. */
  style?: Record<string, string>;
}

/** Tokenize `code` into per-line colored tokens for `lang`, or null on any
 *  failure (caller falls back to plain text). Never throws. */
export async function highlightLines(code: string, lang: string): Promise<HlToken[][] | null> {
  try {
    const hl = await highlighter();
    const { tokens } = hl.codeToTokens(code, {
      lang,
      themes: THEMES,
      defaultColor: false,
      includeExplanation: false,
    });
    return tokens.map((line) => line.map((t) => ({ content: t.content, style: t.htmlStyle })));
  } catch {
    return null;
  }
}
