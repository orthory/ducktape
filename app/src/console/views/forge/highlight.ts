// Lazy, WASM-free syntax highlighter for the forge file viewer.
//
// Uses shiki's JavaScript RegExp engine (no oniguruma WASM), so it works in the
// offline Tauri webview with no extra asset, and only bundles the languages
// imported below. Returns per-line colored tokens that drop straight into the
// viewer's line-number gutter.

import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
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

const THEME = "github-light";

/** github-light's default foreground — used for text outside any token so the
 *  gutter body stays consistent with the highlighted spans. */
export const CODE_FG = "#24292e";

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
      themes: [githubLight],
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

export interface HlToken {
  content: string;
  color?: string;
}

/** Tokenize `code` into per-line colored tokens for `lang`, or null on any
 *  failure (caller falls back to plain text). Never throws. */
export async function highlightLines(code: string, lang: string): Promise<HlToken[][] | null> {
  try {
    const hl = await highlighter();
    const { tokens } = hl.codeToTokens(code, {
      lang,
      theme: THEME,
      includeExplanation: false,
    });
    return tokens.map((line) => line.map((t) => ({ content: t.content, color: t.color })));
  } catch {
    return null;
  }
}
