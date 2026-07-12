// The global skill library, as a picker. It browses /shared/skills over the
// ordinary files-client (`ls` the root, `read` the head of each SKILL.md) and
// lists what it finds by the frontmatter's name + description. Choosing a row
// curates that skill on the agent with its prefix filled in; typing a name that
// isn't there publishes a new library skill instead.
//
// Nothing here is authoritative: a library skill is just a duckfs directory, so
// an empty (or absent) /shared/skills is an empty library, not an error.

import { useEffect, useState } from "react";

import { base64ToBytes, ls, read } from "../../../domain/files-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { inputStyle, secondaryButton } from "./parts";
import { filterLibrary, LIBRARY_ROOT, librarySkill, type LibrarySkill } from "./skill-library";
import { skillDocPath } from "./skills";

/** Enough of a SKILL.md to hold its frontmatter — the body is read by the agent,
 *  not by this list. */
const HEAD_BYTES = 2048;

export function SkillLibraryPicker({
  curated,
  onPick,
  onCreate,
  onClose,
}: {
  /** Prefixes already on the agent — listed, but not addable twice. */
  curated: string[];
  onPick: (skill: LibrarySkill) => void;
  /** Publish a new library skill under the typed name. */
  onCreate: (name: string) => void;
  onClose: () => void;
}) {
  const { transport } = useDucktape();
  const [skills, setSkills] = useState<LibrarySkill[] | null>(null);
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (!transport) return;
    let alive = true;
    void (async () => {
      let found: LibrarySkill[] = [];
      try {
        const page = await ls(transport, { path: LIBRARY_ROOT });
        const dirs = page.entries.filter((entry) => entry.kind === "dir");
        found = await Promise.all(
          dirs.map(async (dir) => {
            try {
              const range = await read(transport, {
                path: skillDocPath(dir.path),
                len: HEAD_BYTES,
              });
              return librarySkill(dir.path, new TextDecoder().decode(base64ToBytes(range.b64)));
            } catch {
              // No SKILL.md (or it wouldn't read): still a skill, name-only.
              return librarySkill(dir.path, null);
            }
          }),
        );
      } catch {
        // A network with no /shared/skills yet has an empty library, not a fault.
        found = [];
      }
      if (alive) setSkills(found);
    })();
    return () => {
      alive = false;
    };
  }, [transport]);

  const matches = filterLibrary(skills ?? [], query);
  const name = query.trim();
  const exists = matches.some((skill) => skill.name.toLowerCase() === name.toLowerCase());

  return (
    <div
      role="group"
      aria-label="Skill library"
      style={{
        marginTop: 8,
        border: `1px solid ${color.borderStrong}`,
        borderRadius: radius.sm,
        background: color.paper,
        padding: 9,
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <input
          type="search"
          autoFocus
          value={query}
          aria-label="Search the skill library"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search /shared/skills…"
          style={{ ...inputStyle, flex: 1, minHeight: 30, padding: "6px 9px" }}
        />
        <button
          type="button"
          onClick={onClose}
          style={{ ...secondaryButton, minHeight: 30, padding: "0 9px", font: `600 10.5px ${font.sans}` }}
        >
          Close
        </button>
      </div>

      <div
        style={{
          maxHeight: 220,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: 5,
        }}
      >
        {skills === null && (
          <div style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
            Reading the library…
          </div>
        )}
        {skills !== null && matches.length === 0 && (
          <div style={{ font: `400 10.5px ${font.sans}`, color: color.muted2, lineHeight: 1.5 }}>
            {skills.length === 0
              ? "The library is empty. Publish the first skill and every agent can compose from it."
              : "Nothing in the library matches."}
          </div>
        )}
        {matches.map((skill) => {
          const added = curated.includes(skill.prefix);
          return (
            <button
              key={skill.prefix}
              type="button"
              disabled={added}
              onClick={() => onPick(skill)}
              aria-label={`Add ${skill.name} from the library`}
              style={{
                appearance: "none",
                textAlign: "left",
                border: `1px solid ${color.border}`,
                borderRadius: radius.sm,
                background: added ? color.sunken : color.paper,
                cursor: added ? "default" : "pointer",
                padding: "7px 9px",
                display: "flex",
                flexDirection: "column",
                gap: 2,
                minWidth: 0,
              }}
            >
              <span
                style={{
                  display: "flex",
                  alignItems: "baseline",
                  gap: 7,
                  font: `600 11.5px ${font.sans}`,
                  color: added ? color.muted2 : color.ink,
                }}
              >
                {skill.name}
                {added && (
                  <span style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>
                    already curated
                  </span>
                )}
              </span>
              <span
                style={{
                  font: `400 10.5px ${font.sans}`,
                  color: color.muted2,
                  lineHeight: 1.45,
                }}
              >
                {skill.description ?? "No description — add one to its SKILL.md frontmatter."}
              </span>
              <span translate="no" style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>
                {skill.prefix}
              </span>
            </button>
          );
        })}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <button
          type="button"
          disabled={name === "" || exists}
          onClick={() => onCreate(name)}
          style={{
            ...secondaryButton,
            minHeight: 28,
            padding: "0 9px",
            font: `600 10.5px ${font.sans}`,
            cursor: name === "" || exists ? "default" : "pointer",
          }}
        >
          {name === "" ? "Publish a new skill…" : `Publish “${name}” to the library`}
        </button>
        <span style={{ font: `400 10px ${font.sans}`, color: color.muted2 }}>
          Type a name, publish it, then write its document in Files.
        </span>
      </div>
    </div>
  );
}
