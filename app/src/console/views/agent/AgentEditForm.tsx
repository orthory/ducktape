// The inline edit composer inside the agent detail pane. A blank prompt is
// deliberately omitted from the update payload (keep the current prompt),
// never sent as an empty string.

import { useState } from "react";
import type { FormEvent } from "react";

import type { AgentRecord, ResourceCaps } from "../../../domain/agent-client";
import { KNOWN_ACTIONS } from "../../../domain/agent-client";
import { accentVar, color, font, radius } from "../../theme/tokens";
import {
  ACTION_HINT,
  ACTION_LABEL,
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

export function AgentEditForm({
  agent,
  capabilities,
  onUpdate,
  onClose,
}: {
  agent: AgentRecord;
  capabilities: string[];
  onUpdate: (params: {
    agentId: string;
    displayName?: string;
    capability?: string;
    prompt?: string;
    allowedActions?: string[];
    caps?: ResourceCaps;
  }) => void;
  onClose: () => void;
}) {
  const [displayName, setDisplayName] = useState(agent.display_name);
  const [capability, setCapability] = useState(agent.capability);
  const [prompt, setPrompt] = useState("");
  const [allowedActions, setAllowedActions] = useState<string[]>(agent.allowed_actions);
  const [pagesWrite, setPagesWrite] = useState(
    (agent.caps?.pages_write ?? []).join(" "),
  );

  const toggle = (name: string) =>
    setAllowedActions((prev) =>
      prev.includes(name) ? prev.filter((action) => action !== name) : [...prev, name],
    );

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onUpdate({
      agentId: agent.agent_id,
      displayName: displayName.trim(),
      capability: capability.trim(),
      allowedActions,
      // caps REPLACE wholesale on update: send the record's current caps
      // with only pages_write swapped so the other grants survive.
      caps: { ...agent.caps, pages_write: parsePagesWrite(pagesWrite) },
      ...(prompt.trim() ? { prompt } : {}),
    });
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

      <div style={{ marginTop: 10 }}>
        <FieldLabel htmlFor="agent-edit-prompt">New prompt</FieldLabel>
        <textarea
          id="agent-edit-prompt"
          name="agent-edit-prompt"
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          rows={4}
          placeholder="Leave blank to keep the current prompt"
          style={{
            ...inputStyle,
            resize: "vertical",
            minHeight: 80,
            lineHeight: 1.45,
          }}
        />
      </div>

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
        <button type="submit" style={primaryButton(true)}>
          Save changes
        </button>
      </div>
    </form>
  );
}
