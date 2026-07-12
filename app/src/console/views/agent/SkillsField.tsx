// The agent's soul, as a form field. An agent is its curated skills: each row
// points at a duckfs directory holding a SKILL.md, and the "always load" toggle
// decides whether that document is inlined into every run (the persona) or only
// indexed for the agent to read when the task calls for it.
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

  const patch = (index: number, next: Partial<SkillRef>) =>
    onChange(skills.map((skill, i) => (i === index ? { ...skill, ...next } : skill)));
  const add = (load: LoadMode) => onChange([...skills, newSkill(agentId || "agent", load)]);
  const remove = (index: number) => onChange(skills.filter((_, i) => i !== index));

  const hasPersona = skills.some((skill) => skill.load === "always");

  /** Seed the skill's SKILL.md from a template — a plain duckfs commit. Never
   *  clobbers: an existing document is reported, not overwritten (editing its
   *  text happens in Files, not here). */
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
          skillTemplate({ name, displayName: displayName.trim() || agentId, load: skill.load }),
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
      <div
        style={{
          marginBottom: 8,
          font: `400 10.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.5,
        }}
      >
        Always-loaded skills are pasted into every run — together they are the agent's
        persona. The rest are listed by name, and the agent opens them from its skill
        folder only when the job calls for one.
      </div>

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

      <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 8 }}>
        {!hasPersona && (
          <button type="button" onClick={() => add("always")} style={smallButton}>
            + Persona (always loaded)
          </button>
        )}
        <button type="button" onClick={() => add("on_demand")} style={smallButton}>
          + Skill (on demand)
        </button>
      </div>

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
