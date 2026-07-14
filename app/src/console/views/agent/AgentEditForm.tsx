// The inline edit composer inside the agent detail pane. The curated skill set
// REPLACES wholesale on save (that is the module's update semantics), so the
// field is seeded from the record and always sent back.

import { useState } from "react";
import type { FormEvent } from "react";

import type { AgentRecord, ResourceCaps, SkillRef } from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import { accentVar, color, font, radius } from "../../theme/tokens";
import {
  ACTION_HINT,
  ACTION_LABEL,
  CapCheckbox,
  FieldLabel,
  inputStyle,
  monoInputStyle,
  parsePagesWrite,
  primaryButton,
  secondaryButton,
  SectionLabel,
  statusTone,
} from "./parts";
import { RunsOnPicker } from "./RunsOnPicker";
import { canReadLibrary, withLibraryRead } from "./skill-library";
import { cleanSkills } from "./skills";
import { SkillsField } from "./SkillsField";

export function AgentEditForm({
  agent,
  capabilities,
  capabilitiesStatus,
  pending,
  onUpdate,
  onClose,
}: {
  agent: AgentRecord;
  capabilities: string[];
  capabilitiesStatus: "loading" | "ready" | "error";
  pending: boolean;
  onUpdate: (params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    allowedActions?: string[];
    caps?: ResourceCaps;
    skills?: SkillRef[];
  }) => Promise<boolean>;
  onClose: () => void;
}) {
  const [displayName, setDisplayName] = useState(agent.display_name);
  const [capability, setCapability] = useState(agent.capability);
  const [skills, setSkills] = useState<SkillRef[]>(agent.skills ?? []);
  const [allowedActions, setAllowedActions] = useState<string[]>(agent.allowed_actions);
  const [pagesWrite, setPagesWrite] = useState(
    (agent.caps?.pages_write ?? []).join(" "),
  );
  // Seeded from the record: an agent registered without the grant gains it here
  // (and one that has it can lose it) — the caps are what the tool plane
  // enforces and what the run's context document is assembled against.
  const [libraryRead, setLibraryRead] = useState(canReadLibrary(agent.caps));
  const [submitting, setSubmitting] = useState(false);
  const blocked = pending || submitting;

  const toggle = (name: string) =>
    setAllowedActions((prev) =>
      prev.includes(name) ? prev.filter((action) => action !== name) : [...prev, name],
    );

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (blocked) return;
    setSubmitting(true);
    const committed = await onUpdate({
      agentId: agent.agent_id,
      displayName: displayName.trim(),
      capability: capability.trim(),
      allowedActions,
      // caps REPLACE wholesale on update: send the record's current caps with
      // only the two fields this form owns swapped, so every other grant
      // survives the edit.
      caps: withLibraryRead(
        { ...agent.caps, pages_write: parsePagesWrite(pagesWrite) },
        libraryRead,
      ),
      skills: cleanSkills(skills),
    });
    setSubmitting(false);
    if (!committed) return;
    onClose();
  };

  return (
    <form
      onSubmit={submit}
      aria-label="Edit agent"
      style={{
        marginTop: 15,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.sidebar,
        padding: 14,
      }}
    >
      <SectionLabel>EDIT AGENT</SectionLabel>
      <div
        style={{
          marginTop: 9,
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 150px), 1fr))",
          gap: 9,
        }}
      >
        <div>
          <FieldLabel htmlFor="agent-edit-display-name">Edit display name</FieldLabel>
          <input
            id="agent-edit-display-name"
            name="agent-edit-display-name"
            type="text"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            style={inputStyle}
          />
        </div>
        <div>
          <FieldLabel htmlFor="agent-edit-capability">Runs on</FieldLabel>
          <RunsOnPicker
            id="agent-edit-capability"
            value={capability}
            capabilities={capabilities}
            registryStatus={capabilitiesStatus}
            onChange={setCapability}
          />
        </div>
      </div>

      <fieldset
        style={{
          margin: "12px 0 0",
          padding: 0,
          border: 0,
          display: "flex",
          flexDirection: "column",
          gap: 7,
        }}
      >
        <legend
          style={{
            marginBottom: 2,
            padding: 0,
            font: `600 10px ${font.mono}`,
            letterSpacing: ".05em",
            color: color.muted2,
          }}
        >
          CAPABILITIES
        </legend>
        <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
          {KNOWN_ACTIONS.map((name) => {
            const checked = allowedActions.includes(name);
            return (
              <label
                key={name}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 7,
                  border: `1px solid ${checked ? statusTone.agent.border : color.border}`,
                  borderRadius: radius.sm,
                  background: checked ? statusTone.agent.bg : color.paper,
                  padding: "6px 9px",
                  cursor: "pointer",
                  font: `600 10.5px ${font.sans}`,
                  color: checked ? accentVar : color.muted3,
                }}
              >
                <input
                  type="checkbox"
                  name="agent-edit-capability"
                  checked={checked}
                  onChange={() => toggle(name)}
                  style={{ margin: 0 }}
                />
                <span>{ACTION_HINT[name] ?? ACTION_LABEL[name] ?? name}</span>
              </label>
            );
          })}
        </div>
        <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
          <CapCheckbox
            id="agent-edit-library-read"
            label="Can search the global skill library"
            checked={libraryRead}
            onChange={setLibraryRead}
          />
        </div>
        <div>
          <FieldLabel htmlFor="agent-edit-pages-write">Page write access</FieldLabel>
          <input
            id="agent-edit-pages-write"
            name="agent-edit-pages-write"
            type="text"
            spellCheck={false}
            value={pagesWrite}
            onChange={(event) => setPagesWrite(event.target.value)}
            placeholder="page ids, or * for all"
            style={monoInputStyle}
          />
          <div
            style={{
              marginTop: 5,
              font: `400 10.5px ${font.sans}`,
              color: color.muted2,
            }}
          >
            Pages the agent may comment on or check off. Space-separated ids; * grants
            every page.
          </div>
        </div>
      </fieldset>

      <SkillsField
        idPrefix="agent-edit"
        agentId={agent.agent_id}
        displayName={displayName}
        skills={skills}
        onChange={setSkills}
      />

      <div
        style={{
          marginTop: 12,
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          gap: 8,
        }}
      >
        <button type="button" onClick={onClose} style={secondaryButton}>
          Cancel
        </button>
        <button
          type="submit"
          disabled={blocked}
          style={primaryButton(!blocked)}
        >
          Save changes
        </button>
      </div>
    </form>
  );
}
