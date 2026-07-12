// The focused Add-agent pane. The agent id is derived from the display name
// by default; the Advanced disclosure reveals the override.
//
// There is no prompt textarea: an agent's persona is a duckfs document it
// curates as an always-loaded skill (see SkillsField), so this form commits
// pins, never prompt text.

import { useState } from "react";
import type { FormEvent } from "react";

import type { ResourceCaps, SkillRef } from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import { accentVar, color, font, radius } from "../../theme/tokens";
import {
  ACTION_HINT,
  ACTION_LABEL,
  AgentAvatar,
  CapCheckbox,
  FieldLabel,
  GroupCard,
  inputStyle,
  monoInputStyle,
  parsePagesWrite,
  primaryButton,
  secondaryButton,
  SectionLabel,
  slug,
  statusTone,
  StatusPill,
} from "./parts";
import { RunsOnPicker } from "./RunsOnPicker";
import { withLibraryRead } from "./skill-library";
import { cleanSkills } from "./skills";
import { SkillsField } from "./SkillsField";

export function RegisterAgentForm({
  capabilities,
  onRegister,
  onDone,
}: {
  capabilities: string[];
  onRegister: (params: {
    displayName: string;
    agentId: string;
    capability: string;
    allowedActions: string[];
    caps?: ResourceCaps;
    skills?: SkillRef[];
  }) => void;
  /** Called after a successful submit (and by Cancel) so the host can close
   *  the create pane. */
  onDone?: () => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [agentIdInput, setAgentIdInput] = useState("");
  const [capability, setCapability] = useState("");
  const [skills, setSkills] = useState<SkillRef[]>([]);
  const [allowedActions, setAllowedActions] = useState<string[]>(["chat.post"]);
  // The pages_write cap field: page ids (or "*") the agent may write.
  const [pagesWrite, setPagesWrite] = useState("");
  // The library read grant, ON by default: without it the run's assembled
  // context never even mentions the shared library, so a new agent would be the
  // only one in the network that cannot look a skill up.
  const [libraryRead, setLibraryRead] = useState(true);
  // The id is derived from the name by default; this reveals the override.
  const [showAdvanced, setShowAdvanced] = useState(false);

  // `slug` is total: it yields a legal `<id>@agents.duck` label (truncated to
  // the consensus cap, hyphens trimmed) or nothing at all. Nothing is the one
  // case the node would reject, so it is the one case to report.
  const agentId = slug(agentIdInput || displayName);
  const idProblem =
    (agentIdInput || displayName).trim() !== "" && agentId === ""
      ? "id needs a letter or a number"
      : null;
  const ready =
    displayName.trim() !== "" &&
    agentId !== "" &&
    capabilities.includes(capability) &&
    allowedActions.length > 0;

  const toggle = (name: string) =>
    setAllowedActions((prev) =>
      prev.includes(name) ? prev.filter((action) => action !== name) : [...prev, name],
    );

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    const pages = parsePagesWrite(pagesWrite);
    const curated = cleanSkills(skills);
    // One caps record, built from both fields: the library grant is an ordinary
    // duckfs_read prefix, not a flag of its own.
    const caps: ResourceCaps = withLibraryRead(
      pages.length ? { pages_write: pages } : {},
      libraryRead,
    );
    onRegister({
      displayName: displayName.trim(),
      agentId,
      capability: capability.trim(),
      allowedActions,
      ...(Object.keys(caps).length ? { caps } : {}),
      ...(curated.length ? { skills: curated } : {}),
    });
    setDisplayName("");
    setAgentIdInput("");
    setCapability("");
    setSkills([]);
    setAllowedActions(["chat.post"]);
    setPagesWrite("");
    setLibraryRead(true);
    setShowAdvanced(false);
    onDone?.();
  };

  return (
    <section aria-label="Register agent" style={{ minWidth: 0 }}>
      <SectionLabel>REGISTER AGENT</SectionLabel>
      <GroupCard style={{ marginTop: 9 }}>
        <form onSubmit={submit} style={{ padding: 16 }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
            <AgentAvatar name={displayName || "AI"} size={40} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ font: `600 13.5px ${font.sans}`, color: color.ink }}>
                Add an agent
              </div>
              <div
                style={{
                  marginTop: 3,
                  font: `400 11.5px ${font.sans}`,
                  color: color.muted2,
                  lineHeight: 1.45,
                }}
              >
                Give it a name, pick what it runs on, and curate the documents it carries.
              </div>
            </div>
            <StatusPill label="AGENT" tone={statusTone.agent} />
          </div>

          <div
            style={{
              marginTop: 14,
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 150px), 1fr))",
              gap: 9,
            }}
          >
            <div>
              <FieldLabel htmlFor="agent-display-name">Agent display name</FieldLabel>
              <input
                id="agent-display-name"
                name="agent-display-name"
                type="text"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
                placeholder="Triage Agent…"
                style={inputStyle}
              />
            </div>
            <div>
              <FieldLabel htmlFor="agent-capability">Runs on</FieldLabel>
              <RunsOnPicker
                id="agent-capability"
                value={capability}
                capabilities={capabilities}
                onChange={setCapability}
              />
            </div>
          </div>

          <SkillsField
            idPrefix="agent"
            agentId={agentId}
            displayName={displayName}
            skills={skills}
            onChange={setSkills}
          />

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
              PERMISSIONS
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
                      name="agent-capability"
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
                id="agent-library-read"
                label="Can search the global skill library"
                checked={libraryRead}
                onChange={setLibraryRead}
              />
            </div>
            <div>
              <FieldLabel htmlFor="agent-pages-write">Page write access</FieldLabel>
              <input
                id="agent-pages-write"
                name="agent-pages-write"
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

          {showAdvanced && (
            <div style={{ marginTop: 12 }}>
              <FieldLabel htmlFor="agent-id">Agent ID</FieldLabel>
              <input
                id="agent-id"
                name="agent-id"
                type="text"
                spellCheck={false}
                value={agentIdInput}
                onChange={(event) => setAgentIdInput(event.target.value)}
                placeholder={agentId || "triage-agent…"}
                style={monoInputStyle}
              />
              <div
                style={{
                  marginTop: 5,
                  font: `400 10.5px ${font.sans}`,
                  color: color.muted2,
                }}
              >
                Used in @mentions and the API. Defaults to the name.
              </div>
            </div>
          )}

          <div
            style={{
              marginTop: 14,
              display: "flex",
              alignItems: "center",
              gap: 10,
              minWidth: 0,
            }}
          >
            <button
              type="button"
              onClick={() => setShowAdvanced((open) => !open)}
              aria-expanded={showAdvanced}
              style={{
                appearance: "none",
                border: 0,
                background: "transparent",
                cursor: "pointer",
                padding: 0,
                font: `600 10px ${font.mono}`,
                letterSpacing: ".05em",
                color: color.muted2,
                flexShrink: 0,
              }}
            >
              {showAdvanced ? "Hide advanced" : "Advanced"}
            </button>
            <span
              translate="no"
              role={idProblem ? "alert" : undefined}
              style={{
                flex: 1,
                minWidth: 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                textAlign: "right",
                font: `400 11px ${font.mono}`,
                color: idProblem ? color.danger : color.muted2,
              }}
            >
              {idProblem ?? `saved as ${agentId || "—"}`}
            </span>
            {onDone && (
              <button type="button" onClick={onDone} style={secondaryButton}>
                Cancel
              </button>
            )}
            <button type="submit" disabled={!ready} style={primaryButton(ready)}>
              Register agent
            </button>
          </div>
        </form>
      </GroupCard>
    </section>
  );
}
