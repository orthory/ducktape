// The global skill library: the shared pool every agent composes from. It is a
// CONVENTION over duckfs, not a new plane — one directory per skill under
// /shared/skills, each holding the same `SKILL.md` any curated skill holds. The
// picker reads those documents' frontmatter (`name`, `description`) to list the
// library; a document without frontmatter is still a skill, it just shows up
// under its directory name.
//
// Pure functions only — SkillLibraryPicker does the duckfs reads through the
// ordinary files-client.

import { basename } from "../../../domain/files-client";
import { slug } from "./parts";

/** Where the shared pool lives. An agent-private skill (a persona) lives
 *  outside it, so this is a default, never a fence. */
export const LIBRARY_ROOT = "/shared/skills";

/** Whether a curated prefix points into the shared pool. */
export const inLibrary = (prefix: string): boolean => prefix.startsWith(`${LIBRARY_ROOT}/`);

/** The prefix a newly published skill takes: one slug segment under the root. */
export const libraryPrefix = (name: string): string => `${LIBRARY_ROOT}/${slug(name)}`;

/** One skill as the library lists it. `description` is null when the document
 *  carries no frontmatter (or none at all) — name-only, never an error. */
export interface LibrarySkill {
  /** The skill's duckfs directory. */
  prefix: string;
  /** The frontmatter `name`, else the directory name. */
  name: string;
  description: string | null;
}

/** Strip one layer of matching YAML quotes. */
const unquote = (value: string): string =>
  /^"(.*)"$/.test(value) || /^'(.*)'$/.test(value) ? value.slice(1, -1) : value;

/** The `name`/`description` a SKILL.md's YAML frontmatter declares — the two
 *  keys the run assembler's skill index reads. A document with no frontmatter,
 *  an unterminated block, or neither key degrades to nulls; nothing here throws.
 *
 *  ponytail: a line scanner, not a YAML parser — flat `key: value` frontmatter
 *  is all the assembler's index reads. Reach for a parser if the format grows
 *  block scalars or nesting. */
export const parseSkillMeta = (
  text: string,
): { name: string | null; description: string | null } => {
  const meta: { name: string | null; description: string | null } = {
    name: null,
    description: null,
  };
  const lines = text.replace(/^\uFEFF/, "").split(/\r?\n/);
  if (lines[0]?.trim() !== "---") return meta;
  for (const line of lines.slice(1)) {
    if (line.trim() === "---") break;
    const match = /^(name|description)[ \t]*:[ \t]*(.*)$/.exec(line);
    if (!match) continue;
    const value = unquote(match[2].trim());
    if (value !== "") meta[match[1] as "name" | "description"] = value;
  }
  return meta;
};

/** A library row from its directory and (optionally read) SKILL.md head. A
 *  missing or unreadable document is still listed — under its folder name. */
export const librarySkill = (prefix: string, doc: string | null): LibrarySkill => {
  const meta = doc === null ? { name: null, description: null } : parseSkillMeta(doc);
  return {
    prefix,
    name: meta.name ?? basename(prefix),
    description: meta.description,
  };
};

/** The picker's search: every whitespace-separated term must appear somewhere in
 *  the row (name, description, or path), case-insensitively. Library order is
 *  preserved — matching never reranks. */
export const filterLibrary = (skills: LibrarySkill[], query: string): LibrarySkill[] => {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return skills;
  return skills.filter((skill) => {
    const haystack = `${skill.name} ${skill.description ?? ""} ${skill.prefix}`.toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
};
