// "Runs on" — which executor variant backs an agent. Decomposes the network's
// announced executor tags (`state.capabilities`) into cascading
// provider → model → effort selects, then composes the choice back into the
// ONE announced tag string the existing register/update payloads carry.
//
// Tag grammar (shared with the capability-host spec loader):
//   - `codex`                       — base tag: a provider's default argv;
//   - `{provider}_{model}_{effort}` — variant tag: split on `_` into exactly
//     3 non-empty parts (model/effort never contain `_`);
//   - any other shape               — opaque: selectable verbatim, no cascade.
//
// Degrades by registry size:
//   - none announced → a labelled text field (never blocks setup before a
//     host has announced);
//   - announced → selects; an empty value defaults to the first announced
//     tag, so a single-executor node needs no choice at all. Providers with
//     only a base tag collapse Model/Effort to "Default".
// A stored tag absent from the registry (its host went offline) is pinned
// with an "(offline)" mark so an edit never silently rewrites which executor
// the agent runs on.

import { useEffect } from "react";

import { color, font } from "../../theme/tokens";
import { FieldLabel, inputStyle, monoInputStyle, titleCase } from "./parts";

type ParsedTag = {
  /** Cascade group: the provider for base/variant shapes, the whole tag for
   *  opaque ones (opaque keys always contain `_`, so they never collide). */
  key: string;
  model: string | null;
  effort: string | null;
  opaque: boolean;
};

export const parseCapabilityTag = (tag: string): ParsedTag => {
  if (!tag.includes("_")) return { key: tag, model: null, effort: null, opaque: false };
  const parts = tag.split("_");
  if (parts.length === 3 && parts.every((part) => part !== "")) {
    return { key: parts[0], model: parts[1], effort: parts[2], opaque: false };
  }
  return { key: tag, model: null, effort: null, opaque: true };
};

type TagEntry = ParsedTag & { tag: string; announced: boolean };

export function RunsOnPicker({
  id,
  value,
  capabilities,
  onChange,
}: {
  id: string;
  /** The stored capability tag — always one whole composed string. */
  value: string;
  /** The network's announced executor registry (`state.capabilities`). */
  capabilities: string[];
  onChange: (next: string) => void;
}) {
  // Adopt a sane default once the registry loads: an empty value with executors
  // available picks the first announced tag, so the common single-executor case
  // is one fewer decision. Never overrides a value the user (or record) set.
  useEffect(() => {
    if (value === "" && capabilities.length > 0) onChange(capabilities[0]);
  }, [value, capabilities, onChange]);

  // No executors announced: free text so a first-time operator can register
  // before any host announces. Keyed on the registry alone, never on `value` —
  // a value-gated guard would flip to the select branch on the first keystroke
  // (unmounting the input mid-word).
  if (capabilities.length === 0) {
    return (
      <>
        <input
          id={id}
          name={id}
          type="text"
          spellCheck={false}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="e.g. codex"
          style={monoInputStyle}
        />
        <div
          style={{
            marginTop: 5,
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            lineHeight: 1.4,
          }}
        >
          Name of an executor your node can run (for example codex or claude).
        </div>
      </>
    );
  }

  const entries: TagEntry[] = capabilities.map((tag) => ({
    ...parseCapabilityTag(tag),
    tag,
    announced: true,
  }));
  // Pin an off-registry stored value into the universe so it stays selectable;
  // every option that only exists because of the pin carries "(offline)".
  if (value !== "" && !capabilities.includes(value)) {
    entries.push({ ...parseCapabilityTag(value), tag: value, announced: false });
  }

  const groupKeys = [...new Set(entries.map((entry) => entry.key))];
  const current = value === "" ? null : parseCapabilityTag(value);
  const groupKey = current?.key ?? "";
  const group = entries.filter((entry) => entry.key === groupKey);
  const modelKey = current?.model ?? "";

  const offlineMark = (offline: boolean, label: string) =>
    offline ? `${label} (offline)` : label;
  const groupOffline = (key: string) =>
    !entries.some((entry) => entry.key === key && entry.announced);

  // Only announced (or pinned) combinations are offered: "Default" stands for
  // the base/opaque tag itself, so bare-tag providers collapse to it.
  const models = [
    ...new Set(group.filter((entry) => entry.model !== null).map((entry) => entry.model as string)),
  ];
  const modelOptions: { value: string; label: string }[] = [];
  if (group.some((entry) => entry.model === null) || models.length === 0) {
    const defaultOffline =
      group.length > 0 && !group.some((entry) => entry.model === null && entry.announced);
    modelOptions.push({ value: "", label: offlineMark(defaultOffline, "Default") });
  }
  for (const model of models) {
    const offline = !group.some((entry) => entry.model === model && entry.announced);
    modelOptions.push({ value: model, label: offlineMark(offline, model) });
  }

  const effortOptions: { value: string; label: string }[] =
    modelKey === ""
      ? [{ value: "", label: "Default" }]
      : [
          ...new Set(
            group
              .filter((entry) => entry.model === modelKey)
              .map((entry) => entry.effort as string),
          ),
        ].map((effort) => ({
          value: effort,
          label: offlineMark(
            !group.some(
              (entry) => entry.model === modelKey && entry.effort === effort && entry.announced,
            ),
            effort,
          ),
        }));

  // Every pick resolves to an entry and emits its verbatim tag — the output is
  // always one announced (or pinned) capability string, never a synthesis.
  const pickProvider = (key: string) => {
    const list = entries.filter((entry) => entry.key === key);
    const pick = list.find((entry) => entry.model === null) ?? list[0];
    if (pick) onChange(pick.tag);
  };
  const pickModel = (model: string) => {
    if (model === "") {
      const base = group.find((entry) => entry.model === null);
      onChange(base ? base.tag : groupKey);
      return;
    }
    const list = group.filter((entry) => entry.model === model);
    const pick = list.find((entry) => entry.effort === current?.effort) ?? list[0];
    if (pick) onChange(pick.tag);
  };
  const pickEffort = (effort: string) => {
    const pick = group.find((entry) => entry.model === modelKey && entry.effort === effort);
    if (pick) onChange(pick.tag);
  };

  const modelLocked = modelOptions.length <= 1;
  const effortLocked = effortOptions.length <= 1;

  return (
    <div style={{ display: "grid", gap: 8 }}>
      <select
        id={id}
        name={id}
        value={groupKey}
        onChange={(event) => pickProvider(event.target.value)}
        style={{ ...inputStyle, cursor: "pointer" }}
      >
        {value === "" && (
          <option value="" disabled>
            Choose an executor…
          </option>
        )}
        {groupKeys.map((key) => (
          <option key={key} value={key}>
            {offlineMark(groupOffline(key), titleCase(key))}
          </option>
        ))}
      </select>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 110px), 1fr))",
          gap: 8,
        }}
      >
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor={`${id}-model`}>Model</FieldLabel>
          <select
            id={`${id}-model`}
            name={`${id}-model`}
            value={modelKey}
            disabled={modelLocked}
            onChange={(event) => pickModel(event.target.value)}
            style={{ ...inputStyle, cursor: modelLocked ? "default" : "pointer" }}
          >
            {modelOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
        <div style={{ minWidth: 0 }}>
          <FieldLabel htmlFor={`${id}-effort`}>Effort</FieldLabel>
          <select
            id={`${id}-effort`}
            name={`${id}-effort`}
            value={current?.effort ?? ""}
            disabled={effortLocked}
            onChange={(event) => pickEffort(event.target.value)}
            style={{ ...inputStyle, cursor: effortLocked ? "default" : "pointer" }}
          >
            {effortOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </div>
      {value !== "" && (
        <div
          title={value}
          translate="no"
          style={{
            font: `400 10.5px ${font.mono}`,
            color: color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {value}
        </div>
      )}
    </div>
  );
}
