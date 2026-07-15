import type { ResourceCaps } from "../../../domain/agent-client";
import { color, font } from "../../theme/tokens";
import { CapCheckbox, FieldLabel, monoInputStyle, parseCapList } from "./parts";
import { LIBRARY_ROOT, withLibraryRead } from "./skill-library";

export interface ResourceCapsDraft {
  forgeRead: string;
  forgePush: string;
  duckfsRead: string;
  duckfsWrite: string;
  tools: string;
  secrets: string;
  pagesWrite: string;
  subagentBudget: string;
}

export const capsDraftFrom = (caps?: ResourceCaps): ResourceCapsDraft => ({
  forgeRead: (caps?.forge_read ?? []).join(" "),
  forgePush: (caps?.forge_push ?? []).join(" "),
  duckfsRead: (caps?.duckfs_read ?? [])
    .filter((prefix) => prefix !== LIBRARY_ROOT)
    .join(" "),
  duckfsWrite: (caps?.duckfs_write ?? []).join(" "),
  tools: (caps?.tools ?? []).join(" "),
  secrets: (caps?.secrets ?? []).join(" "),
  pagesWrite: (caps?.pages_write ?? []).join(" "),
  subagentBudget: caps?.subagent_budget ? String(caps.subagent_budget) : "0",
});

export const capsFromDraft = (
  draft: ResourceCapsDraft,
  libraryRead: boolean,
): ResourceCaps => {
  const caps: ResourceCaps = {};
  const lists: Array<[keyof ResourceCaps, string]> = [
    ["forge_read", draft.forgeRead],
    ["forge_push", draft.forgePush],
    ["duckfs_read", draft.duckfsRead],
    ["duckfs_write", draft.duckfsWrite],
    ["tools", draft.tools],
    ["secrets", draft.secrets],
    ["pages_write", draft.pagesWrite],
  ];
  for (const [key, text] of lists) {
    const values = parseCapList(text);
    if (values.length > 0) {
      (caps[key] as string[] | undefined) = values;
    }
  }
  const budget = Math.max(0, Number.parseInt(draft.subagentBudget, 10) || 0);
  if (budget > 0) caps.subagent_budget = budget;
  return withLibraryRead(caps, libraryRead);
};

const listFields: Array<{
  key: Exclude<keyof ResourceCapsDraft, "subagentBudget">;
  label: string;
  placeholder: string;
}> = [
  { key: "forgeRead", label: "Forge read repositories", placeholder: "repo names" },
  { key: "forgePush", label: "Forge push repositories", placeholder: "repo names" },
  {
    key: "duckfsRead",
    label: "Additional DuckFS read prefixes",
    placeholder: "/shared/data /projects/demo",
  },
  {
    key: "duckfsWrite",
    label: "DuckFS write prefixes",
    placeholder: "/shared/agents/my-agent",
  },
  { key: "tools", label: "Allowed tool IDs", placeholder: "tool or MCP ids" },
  { key: "secrets", label: "Secret references", placeholder: "opaque vault references" },
  { key: "pagesWrite", label: "Page write access", placeholder: "page ids, or * for all" },
];

export function ResourceCapsFields({
  idPrefix,
  draft,
  onChange,
  libraryRead,
  onLibraryReadChange,
}: {
  idPrefix: string;
  draft: ResourceCapsDraft;
  onChange: (draft: ResourceCapsDraft) => void;
  libraryRead: boolean;
  onLibraryReadChange: (granted: boolean) => void;
}) {
  return (
    <>
      <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
        <CapCheckbox
          id={`${idPrefix}-library-read`}
          label="Can search the global skill library"
          checked={libraryRead}
          onChange={onLibraryReadChange}
        />
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 210px), 1fr))",
          gap: 9,
        }}
      >
        {listFields.map((field) => {
          const id = `${idPrefix}-${field.key}`;
          return (
            <div key={field.key}>
              <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
              <input
                id={id}
                type="text"
                spellCheck={false}
                value={draft[field.key]}
                onChange={(event) => onChange({ ...draft, [field.key]: event.target.value })}
                placeholder={field.placeholder}
                style={monoInputStyle}
              />
            </div>
          );
        })}
        <div>
          <FieldLabel htmlFor={`${idPrefix}-subagentBudget`}>Peer-call budget</FieldLabel>
          <input
            id={`${idPrefix}-subagentBudget`}
            type="number"
            min={0}
            max={4_294_967_295}
            step={1}
            value={draft.subagentBudget}
            onChange={(event) => onChange({ ...draft, subagentBudget: event.target.value })}
            style={monoInputStyle}
          />
          <div style={{ marginTop: 5, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
            Total peer calls across the whole recursive call tree; 0 disables calls and the
            runtime hard cap is 8.
          </div>
        </div>
      </div>
    </>
  );
}
