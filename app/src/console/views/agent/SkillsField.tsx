// The agent's soul, as a form field. An agent is its curated skills: each row
// points at a duckfs directory holding a SKILL.md, and the "always load" toggle
// decides whether that document is inlined into every run (the persona) or only
// indexed for the agent to read when the task calls for it.
//
// Curation happens against the global skill library (/shared/skills) — the
// picker is the easy path in, but the typed prefix stays, because an
// agent-private skill (a persona) lives outside the library.
//
// The documents themselves live in Files. This field never holds their text —
// it points at them, opens them in the files browser, and can seed a starter
// SKILL.md through the ordinary duckfs client (files-client's uploadFile), the
// same door the files browser writes through.

import { useState } from "react";

import { stat, uploadFile } from "../../../domain/files-client";
import type { LoadMode, SkillRef } from "../../../domain/agent-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { errMsg } from "../files/files-format";
import { FieldLabel, monoInputStyle, inputStyle, secondaryButton, statusTone } from "./parts";
import { inLibrary, LIBRARY_ROOT, libraryPrefix, type LibrarySkill } from "./skill-library";
import { SkillLibraryPicker } from "./SkillLibraryPicker";
import { cleanPrefix, newSkill, skillDocPath, skillTemplate } from "./skills";

const smallButton = {
  ...secondaryButton,
  minHeight: 24,
  padding: "2px 8px",
  font: `600 10.5px ${font.sans}`,
};

export function SkillsField({
  idPrefix,
  agentId,
  displayName,
  skills,
  onChange,
}: {
  /** Distinguishes the register form's inputs from the edit form's. */
  idPrefix: string;
  agentId: string;
  displayName: string;
  skills: SkillRef[];
  onChange: (skills: SkillRef[]) => void;
}) {
  const { transport, actions } = useDucktape();
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);

  const patch = (index: number, next: Partial<SkillRef>) =>
    onChange(skills.map((skill, i) => (i === index ? { ...skill, ...next } : skill)));
  const add = (load: LoadMode) => onChange([...skills, newSkill(agentId || "agent", load)]);
  const remove = (index: number) => onChange(skills.filter((_, i) => i !== index));

  const hasPersona = skills.some((skill) => skill.load === "always");

  /** Seed the skill's SKILL.md from a template — a plain duckfs commit. Never
   *  clobbers: an existing document is reported, not overwritten (editing its
   *  text happens in Files, not here). A prefix under the library root seeds the
   *  shared wording: the document belongs to no one agent. */
  const createDoc = async (skill: SkillRef) => {
    const prefix = cleanPrefix(skill.source_prefix);
    const name = skill.name.trim();
    if (!transport || !prefix || !name) {
      setNote("Give the skill a name and a duckfs path first.");
      return;
    }
    const path = skillDocPath(prefix);
    setBusy(path);
    setNote(null);
    try {
      if (await stat(transport, { path })) {
        setNote(`${path} already exists — open it in Files to edit it.`);
        return;
      }
      await uploadFile(transport, {
        path,
        bytes: new TextEncoder().encode(
          skillTemplate({
            name,
            displayName: displayName.trim() || agentId,
            load: skill.load,
            shared: inLibrary(prefix),
          }),
        ),
        message: `create skill doc ${path}`,
      });
      setNote(`Created ${path}. Edit it in Files.`);
    } catch (err) {
      setNote(errMsg(err));
    } finally {
      setBusy(null);
    }
  };

  /** Curate a library skill: its prefix, its frontmatter name, on demand (the
   *  cheap mode — the operator promotes it with "Always load"). */
  const pickFromLibrary = (skill: LibrarySkill) => {
    onChange([...skills, { name: skill.name, source_prefix: skill.prefix, load: "on_demand" }]);
    setPicking(false);
  };

  /** Publish a new library skill and curate it in the same click: the row goes
   *  in, and the ordinary create-doc path seeds its SKILL.md. */
  const publishToLibrary = (name: string) => {
    const skill: SkillRef = {
      name,
      source_prefix: libraryPrefix(name),
      load: "on_demand",
    };
    onChange([...skills, skill]);
    setPicking(false);
    void createDoc(skill);
  };

  return (
    <fieldset style={{ margin: "12px 0 0", padding: 0, border: 0 }}>
      <legend
        style={{
          marginBottom: 2,
          padding: 0,
          font: `600 10px ${font.mono}`,
          letterSpacing: ".05em",
          color: color.muted2,
        }}
      >
        SKILLS
      </legend>
      {/* The three tiers, one line each — curation is about what the agent LEADS
          WITH, not about what it can reach. */}
      <ul
        style={{
          margin: "0 0 8px",
          padding: 0,
          listStyle: "none",
          display: "flex",
          flexDirection: "column",
          gap: 2,
          font: `400 10.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.5,
        }}
      >
        <li>
          <b style={{ color: color.muted3 }}>Always</b> — pasted into every run: the agent's
          persona, and a cost paid on every single run.
        </li>
        <li>
          <b style={{ color: color.muted3 }}>On demand</b> — the agent is told the skill exists
          and reads the document itself when the job calls for it.
        </li>
        <li>
          <b style={{ color: color.muted3 }}>Everything else</b> — the rest of{" "}
          <span translate="no" style={{ font: `400 10px ${font.mono}` }}>
            {LIBRARY_ROOT}
          </span>{" "}
          stays reachable at runtime through the agent's file tools, curated or not.
        </li>
      </ul>

      {skills.length === 0 && (
        <div
          style={{
            padding: "10px 12px",
            borderRadius: radius.sm,
            border: `1px dashed ${color.border}`,
            background: color.sunken,
            font: `400 11px ${font.sans}`,
            color: color.muted2,
          }}
        >
          No skills yet. Without an always-loaded skill this agent has no persona — it
          runs on the task instructions alone.
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {skills.map((skill, index) => {
          const always = skill.load === "always";
          const path = skillDocPath(skill.source_prefix);
          return (
            <div
              key={index}
              style={{
                border: `1px solid ${always ? statusTone.agent.border : color.border}`,
                borderRadius: radius.sm,
                background: always ? statusTone.agent.bg : color.paper,
                padding: 9,
                display: "flex",
                flexDirection: "column",
                gap: 7,
              }}
            >
              <div style={{ display: "flex", alignItems: "flex-end", gap: 8 }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <FieldLabel htmlFor={`${idPrefix}-skill-name-${index}`}>
                    Skill name
                  </FieldLabel>
                  <input
                    id={`${idPrefix}-skill-name-${index}`}
                    name={`${idPrefix}-skill-name-${index}`}
                    type="text"
                    value={skill.name}
                    onChange={(event) => patch(index, { name: event.target.value })}
                    placeholder="persona, triage, release…"
                    style={inputStyle}
                  />
                </div>
                <label
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 6,
                    minHeight: 30,
                    padding: "0 4px",
                    cursor: "pointer",
                    font: `600 10.5px ${font.sans}`,
                    color: always ? statusTone.agent.text : color.muted3,
                    whiteSpace: "nowrap",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={always}
                    aria-label={`Always load ${skill.name || `skill ${index + 1}`}`}
                    onChange={(event) =>
                      patch(index, { load: event.target.checked ? "always" : "on_demand" })
                    }
                    style={{ margin: 0 }}
                  />
                  Always load
                </label>
                <button
                  type="button"
                  onClick={() => remove(index)}
                  aria-label={`Remove ${skill.name || `skill ${index + 1}`}`}
                  style={{ ...smallButton, minHeight: 30 }}
                >
                  Remove
                </button>
              </div>

              <div>
                <FieldLabel htmlFor={`${idPrefix}-skill-prefix-${index}`}>
                  Document folder (duckfs)
                </FieldLabel>
                <input
                  id={`${idPrefix}-skill-prefix-${index}`}
                  name={`${idPrefix}-skill-prefix-${index}`}
                  type="text"
                  spellCheck={false}
                  value={skill.source_prefix}
                  onChange={(event) => patch(index, { source_prefix: event.target.value })}
                  placeholder="/shared/agents/…/persona"
                  style={monoInputStyle}
                />
                <div
                  style={{
                    marginTop: 5,
                    display: "flex",
                    alignItems: "center",
                    gap: 7,
                    flexWrap: "wrap",
                  }}
                >
                  <span
                    translate="no"
                    style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}
                  >
                    {path}
                  </span>
                  <button
                    type="button"
                    onClick={() => actions.openFiles(cleanPrefix(skill.source_prefix))}
                    disabled={cleanPrefix(skill.source_prefix) === ""}
                    style={smallButton}
                  >
                    Open in Files
                  </button>
                  <button
                    type="button"
                    onClick={() => void createDoc(skill)}
                    disabled={busy !== null}
                    style={smallButton}
                  >
                    {busy === path ? "Creating…" : "Create doc"}
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div
        style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}
      >
        <button
          type="button"
          onClick={() => setPicking((open) => !open)}
          aria-expanded={picking}
          style={smallButton}
        >
          + From library
        </button>
        {!hasPersona && (
          <button type="button" onClick={() => add("always")} style={smallButton}>
            + Persona (always loaded)
          </button>
        )}
        <button type="button" onClick={() => add("on_demand")} style={smallButton}>
          + Skill (on demand)
        </button>
      </div>

      {picking && (
        <SkillLibraryPicker
          curated={skills.map((skill) => cleanPrefix(skill.source_prefix))}
          onPick={pickFromLibrary}
          onCreate={publishToLibrary}
          onClose={() => setPicking(false)}
        />
      )}

      {note && (
        <div
          role="status"
          style={{
            marginTop: 7,
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            wordBreak: "break-all",
          }}
        >
          {note}
        </div>
      )}
    </fieldset>
  );
}
