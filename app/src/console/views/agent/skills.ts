// The agent's soul, as pure functions. A "skill" is a duckfs DIRECTORY holding
// a `SKILL.md`; the agent record curates an ordered list of them, each marked
// `always` (inlined into the assembled context of every run — the persona) or
// `on_demand` (indexed by name, read from the run's skill mount when relevant).
//
// Nothing here talks to a transport: the form composes these, the Files surface
// stores the documents.

import type { LoadMode, SkillRef } from "../../../domain/agent-client";

/** The file every skill directory carries — the assembler reads its body for an
 *  `always` skill and its frontmatter description for an `on_demand` one. */
export const SKILL_DOC = "SKILL.md";

/** Where an agent's own documents live by default. Nothing enforces this — a
 *  skill can be any duckfs prefix, including one shared between agents. */
export const skillsRoot = (agentId: string): string => `/shared/agents/${agentId}`;

/** The default persona prefix for a new agent. */
export const personaPrefix = (agentId: string): string => `${skillsRoot(agentId)}/persona`;

/** Normalize an operator-typed prefix: absolute, no trailing slash. */
export const cleanPrefix = (raw: string): string => {
  const trimmed = raw.trim().replace(/\/+$/, "");
  if (trimmed === "") return "";
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
};

/** The document a skill prefix resolves to. */
export const skillDocPath = (prefix: string): string => `${cleanPrefix(prefix)}/${SKILL_DOC}`;

/** The starter document written by the form's "create doc" affordance. The
 *  frontmatter `description` is what an `on_demand` skill shows in the run's
 *  index; the body is what an `always` skill inlines. */
export const skillTemplate = (params: {
  name: string;
  displayName: string;
  load: LoadMode;
}): string => {
  const always = params.load === "always";
  const description = always
    ? `Who ${params.displayName} is and how it works.`
    : `What ${params.displayName} does when this skill applies.`;
  const body = always
    ? `Write ${params.displayName}'s persona here: its voice, its judgment, the standing rules it carries into every run. This whole document is loaded into every run.`
    : `Write the procedure here. ${params.displayName} reads this document only when the task calls for it, so say up front when that is.`;
  return `---\nname: ${params.name}\ndescription: ${description}\n---\n\n# ${params.name}\n\n${body}\n`;
};

/** A blank curated row — new skills default to on-demand, the cheap mode. */
export const newSkill = (agentId: string, load: LoadMode = "on_demand"): SkillRef =>
  load === "always"
    ? { name: "persona", source_prefix: personaPrefix(agentId), load }
    : { name: "", source_prefix: `${skillsRoot(agentId)}/`, load };

/** Drop empty rows and normalize prefixes — what the form actually submits. */
export const cleanSkills = (skills: SkillRef[]): SkillRef[] =>
  skills
    .map((skill) => ({
      ...skill,
      name: skill.name.trim(),
      source_prefix: cleanPrefix(skill.source_prefix),
    }))
    .filter((skill) => skill.name !== "" && skill.source_prefix !== "");

/** One line an operator can read: what this agent always carries, and what it
 *  can reach for. */
export const skillsSummary = (skills: SkillRef[]): string => {
  const always = skills.filter((skill) => skill.load === "always").length;
  const onDemand = skills.length - always;
  if (skills.length === 0) return "none";
  return `${always} always · ${onDemand} on demand`;
};
