//! the agent's SOUL: its curated skills, assembled into ONE markdown document
//! the executor's own CLI auto-loads.
//!
//! a soul is not a stored object — it is a BUILD PRODUCT. consensus commits
//! WHICH skills an agent carries (name + duckfs pin + load mode); the host
//! materializes them as ro mounts and assembles them here, in curation order:
//! every `always` skill inlined in full (this is where the retired
//! `prompt_hash` blob's text now lives), then an INDEX of the on-demand ones
//! (name, description, where to read the body), then the ambient tool-plane
//! instruction. one document, one delivery — capability-host decides the door
//! (the CLI's auto-load path, or the stdin prompt).
//!
//! PURE on purpose: no fs, no duckfs, no `SkillRef`. the provisioner (the only
//! layer allowed to touch the OS-side checkout engine) reads the materialized
//! `SKILL.md` files and calls [`assemble_context_doc`]; everything here is a
//! function of its argument, so the whole shape is unit-testable with no node.
//!
//! degrade rules, deliberately asymmetric: a skill with no/malformed
//! frontmatter still gets a name-only index entry (a cosmetic parse must never
//! fail a run), while a missing body for an `always` skill fails the run
//! loudly upstream — that one IS the persona, and running without it silently
//! produces a different agent.

/// the ambient MCP instruction every souled run carries — MOVED verbatim from
/// the run envelope's runtime section (`runs::envelope`), which this document
/// replaces. it deliberately does not enumerate the tools: the MCP server ships
/// its own instructions with the binary, so restating the surface here would
/// only give the two something to drift apart about.
const TOOL_PLANE_INSTRUCTION: &str = "A Ducktape MCP tool server named \"ducktape\" is available in this session. It is how you read and write Ducktape state — chat, tasks, pages, forge items, and duckfs files. Call its tools instead of guessing; its own instructions describe every tool it offers.";

/// one curated skill, already materialized. `name` is the CURATED name
/// consensus committed (`SkillRef.name`), never a name read out of the document
/// — a doc must not be able to rename itself into another agent's heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDoc {
    pub name: String,
    /// the `description` from the doc's YAML frontmatter; `None` when it has
    /// none (or it is malformed) — a name-only index entry, never an error.
    pub description: Option<String>,
    /// the SKILL.md text with its frontmatter stripped ([`parse_skill_md`]).
    pub body: String,
    pub always: bool,
}

/// assemble the run's context document. CURATION ORDER is the agent's skills
/// order — never sorted: an agent's persona is a composed thing, and reordering
/// it is editing it.
///
/// an EMPTY skill set assembles an EMPTY document: an agent that curated no
/// skills has no soul to load, and the caller maps that to `None` (no doc is
/// written, no prompt is prepended).
// ponytail: that also means a skill-less agent carries no tool-plane sentence,
// where the retired runtime section gave one to every run. drop this guard (and
// always emit the instruction) if a skill-less agent must still be told the MCP
// plane exists.
pub fn assemble_context_doc(skills: &[SkillDoc]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut sections: Vec<String> = skills
        .iter()
        .filter(|s| s.always)
        .map(|s| format!("# {}\n{}", s.name, s.body.trim_end()))
        .collect();

    let index: Vec<String> = skills
        .iter()
        .filter(|s| !s.always)
        .map(|s| {
            let where_ = format!("(`$DUCKTAPE_RUN_SKILLS/{}/SKILL.md`)", s.name);
            match &s.description {
                Some(d) => format!("- **{}** — {d} {where_}", s.name),
                None => format!("- **{}** {where_}", s.name),
            }
        })
        .collect();
    if !index.is_empty() {
        sections.push(format!(
            "## Skills available on demand\nRead the full text when the task \
             calls for it; each lives under the directory named by \
             $DUCKTAPE_RUN_SKILLS.\n{}",
            index.join("\n")
        ));
    }
    sections.push(TOOL_PLANE_INSTRUCTION.to_string());
    sections.join("\n\n")
}

/// split a `SKILL.md` into its frontmatter `description` and its body.
///
/// the frontmatter is the convention this repo's own `skills/` already follow:
/// a leading `---` line, `key: value` lines, a closing `---`. deliberately NOT
/// a YAML parser (no dep, no surface): anything that is not exactly that shape
/// is simply a document with no frontmatter — body verbatim, description
/// `None`. a cosmetic parse must never fail a run.
pub fn parse_skill_md(text: &str) -> (Option<String>, String) {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (None, text.to_string());
    };
    // the closing fence: the first line that is exactly `---`.
    let Some((front, body)) = rest.split_once("\n---\n").or_else(|| {
        rest.split_once("\r\n---\r\n")
            .or_else(|| rest.strip_suffix("\n---").map(|f| (f, "")))
    }) else {
        // an UNCLOSED fence is not frontmatter — the `---` was the document's.
        return (None, text.to_string());
    };
    let description = front.lines().find_map(|line| {
        let value = line.trim().strip_prefix("description:")?.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        (!value.is_empty()).then(|| value.to_string())
    });
    (description, body.trim_start_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, always: bool, description: Option<&str>, body: &str) -> SkillDoc {
        SkillDoc {
            name: name.into(),
            description: description.map(str::to_string),
            body: body.into(),
            always,
        }
    }

    #[test]
    fn an_empty_skill_set_assembles_an_empty_document() {
        assert!(assemble_context_doc(&[]).is_empty());
    }

    #[test]
    fn always_skills_inline_in_curation_order_and_on_demand_ones_only_index() {
        // curation order, NOT sorted: `zeta` was curated first, so it leads.
        let doc = assemble_context_doc(&[
            skill("zeta", true, Some("z desc"), "You are Zeta.\n"),
            skill("release", false, Some("cut a release"), "the whole release body"),
            skill("alpha", true, None, "Always quack twice."),
        ]);
        assert_eq!(
            doc,
            "# zeta\nYou are Zeta.\n\n\
             # alpha\nAlways quack twice.\n\n\
             ## Skills available on demand\n\
             Read the full text when the task calls for it; each lives under the \
             directory named by $DUCKTAPE_RUN_SKILLS.\n\
             - **release** — cut a release (`$DUCKTAPE_RUN_SKILLS/release/SKILL.md`)\n\n\
             A Ducktape MCP tool server named \"ducktape\" is available in this session. \
             It is how you read and write Ducktape state — chat, tasks, pages, forge \
             items, and duckfs files. Call its tools instead of guessing; its own \
             instructions describe every tool it offers."
        );
        // the on-demand skill is INDEXED, never inlined — the whole point of the
        // load mode is that its body does not inflate every prompt.
        assert!(
            !doc.contains("the whole release body"),
            "an on_demand body must not be inlined: {doc}"
        );
    }

    #[test]
    fn a_frontmatterless_skill_degrades_to_a_name_only_index_entry() {
        let doc = assemble_context_doc(&[skill("qa", false, None, "body")]);
        assert!(
            doc.contains("- **qa** (`$DUCKTAPE_RUN_SKILLS/qa/SKILL.md`)"),
            "got {doc}"
        );
    }

    #[test]
    fn the_tool_plane_instruction_appears_exactly_once() {
        let doc = assemble_context_doc(&[
            skill("a", true, None, "A"),
            skill("b", true, None, "B"),
            skill("c", false, Some("c"), "C"),
        ]);
        assert_eq!(
            doc.matches("A Ducktape MCP tool server").count(),
            1,
            "the ambient instruction is stated once, at the end: {doc}"
        );
        assert!(doc.trim_end().ends_with("every tool it offers."));
    }

    #[test]
    fn frontmatter_is_stripped_from_the_body_and_yields_the_description() {
        let (description, body) = parse_skill_md(
            "---\nname: qa\ndescription: Drive the QA fleet.\n---\n\n# QA\nthe body\n",
        );
        assert_eq!(description.as_deref(), Some("Drive the QA fleet."));
        assert_eq!(body, "# QA\nthe body\n");
    }

    #[test]
    fn a_malformed_or_absent_frontmatter_never_swallows_the_body() {
        // no frontmatter at all: body verbatim.
        assert_eq!(
            parse_skill_md("# QA\nthe body"),
            (None, "# QA\nthe body".to_string())
        );
        // an UNCLOSED fence is the document's own `---`, not frontmatter — the
        // body must survive intact (silently eating an always-skill's persona
        // is exactly the quiet corruption the loud degrade rules exist to kill).
        assert_eq!(
            parse_skill_md("---\nnot closed\nthe body"),
            (None, "---\nnot closed\nthe body".to_string())
        );
        // frontmatter with no description key: name-only, body still stripped.
        assert_eq!(
            parse_skill_md("---\nname: qa\n---\nbody"),
            (None, "body".to_string())
        );
        // quoted values unwrap.
        assert_eq!(
            parse_skill_md("---\ndescription: \"quoted\"\n---\nbody").0.as_deref(),
            Some("quoted")
        );
    }
}
