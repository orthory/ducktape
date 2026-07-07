// File-type icon pack for the forge tree: maps a filename to one of the
// console's monochrome line icons plus a category accent color, so the tree
// reads like an IDE while staying inside the design's icon language.

import type { IconName } from "../../components/Icon";
import { color } from "../../theme/tokens";

export interface FileIcon {
  icon: IconName;
  color: string;
}

// category tints drawn from the shared palette.
const CODE = color.accentAlt1; // blue — general source
const RUST = color.accent; // warm — the repo's primary language
const DATA = color.accentAlt1; // blue — structured data
const CONFIG = color.amber; // amber — config / lockfiles
const DOC = color.accentAlt2; // green — prose / markdown
const IMAGE = color.purple; // purple — assets
const MUTED = color.muted2;

// exact filenames that beat the extension map (dotfiles, lockfiles, ...).
const BY_NAME: Record<string, FileIcon> = {
  "cargo.toml": { icon: "settings", color: RUST },
  "cargo.lock": { icon: "settings", color: CONFIG },
  "package.json": { icon: "braces", color: DOC },
  "bun.lockb": { icon: "settings", color: CONFIG },
  "tsconfig.json": { icon: "braces", color: CONFIG },
  dockerfile: { icon: "settings", color: CONFIG },
  makefile: { icon: "settings", color: CONFIG },
  ".gitignore": { icon: "settings", color: MUTED },
  "readme.md": { icon: "document", color: DOC },
  "license": { icon: "document", color: MUTED },
};

const BY_EXT: Record<string, FileIcon> = {
  // code
  ts: { icon: "code", color: CODE },
  tsx: { icon: "code", color: CODE },
  mts: { icon: "code", color: CODE },
  cts: { icon: "code", color: CODE },
  js: { icon: "code", color: CONFIG },
  jsx: { icon: "code", color: CONFIG },
  mjs: { icon: "code", color: CONFIG },
  cjs: { icon: "code", color: CONFIG },
  rs: { icon: "code", color: RUST },
  go: { icon: "code", color: CODE },
  py: { icon: "code", color: CODE },
  c: { icon: "code", color: CODE },
  h: { icon: "code", color: CODE },
  cpp: { icon: "code", color: CODE },
  java: { icon: "code", color: CODE },
  rb: { icon: "code", color: RUST },
  sh: { icon: "code", color: DOC },
  bash: { icon: "code", color: DOC },
  css: { icon: "code", color: CODE },
  scss: { icon: "code", color: CODE },
  html: { icon: "code", color: RUST },
  // data
  json: { icon: "braces", color: DATA },
  // config
  toml: { icon: "settings", color: CONFIG },
  yaml: { icon: "settings", color: CONFIG },
  yml: { icon: "settings", color: CONFIG },
  ini: { icon: "settings", color: CONFIG },
  env: { icon: "settings", color: CONFIG },
  conf: { icon: "settings", color: CONFIG },
  lock: { icon: "settings", color: CONFIG },
  // prose
  md: { icon: "document", color: DOC },
  mdx: { icon: "document", color: DOC },
  markdown: { icon: "document", color: DOC },
  txt: { icon: "document", color: MUTED },
  // images / assets
  png: { icon: "image", color: IMAGE },
  jpg: { icon: "image", color: IMAGE },
  jpeg: { icon: "image", color: IMAGE },
  gif: { icon: "image", color: IMAGE },
  svg: { icon: "image", color: IMAGE },
  webp: { icon: "image", color: IMAGE },
  ico: { icon: "image", color: IMAGE },
};

const DEFAULT_FILE: FileIcon = { icon: "document", color: MUTED };

/** Icon + tint for a file by its name (exact name wins over extension). */
export function fileIcon(name: string): FileIcon {
  const lower = name.toLowerCase();
  if (BY_NAME[lower]) return BY_NAME[lower];
  const ext = lower.includes(".") ? lower.slice(lower.lastIndexOf(".") + 1) : "";
  return BY_EXT[ext] ?? DEFAULT_FILE;
}
