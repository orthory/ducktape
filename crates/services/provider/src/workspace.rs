//! `[workspace]` — the child process's working-directory policy.
//!
//! the v1 fence ran every provider child in an empty scratch directory. an
//! AGENTIC run wants the opposite: a stable per-agent directory the executor
//! can read and write across runs, so work accumulates instead of vanishing
//! with each invocation. `[workspace] mode = "persistent"` opts a spec into
//! that: when the run carries an agent identity (see [`crate::RunContext`])
//! and the host wired a workspaces root, the child's cwd becomes
//! `<workspaces_root>/<agent_id>`, created on demand. everything else — no
//! `[workspace]` section, an agent-less run, an embedder that wired
//! no root — keeps the scratch-dir fence unchanged.

use serde::Deserialize;

/// where a provider child runs. absent `[workspace]` = [`Self::Scratch`],
/// the v1 empty-scratch-dir fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceMode {
    /// an empty scratch dir, never the node's data directory.
    #[default]
    Scratch,
    /// a stable `<workspaces_root>/<agent_id>` dir shared by every run of
    /// one agent on this host — host-local state, invisible to consensus.
    Persistent,
}

/// the on-disk `[workspace]` shape — a dumb serde mirror; unknown fields
/// fail loud like everywhere else in the spec format.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWorkspace {
    mode: String,
}

/// validate one `[workspace]` section. only `"persistent"` exists — the
/// scratch default is expressed by OMITTING the section, so there is no
/// redundant spelling of the default to drift from it.
pub(crate) fn parse_workspace(raw: &RawWorkspace, origin: &str) -> Result<WorkspaceMode, String> {
    match raw.mode.as_str() {
        "persistent" => Ok(WorkspaceMode::Persistent),
        other => Err(format!(
            "{origin}: workspace.mode {other:?} is not supported \
             (want \"persistent\"; omit [workspace] for the scratch default)"
        )),
    }
}

/// defense for ids used as a path component under the host's agent roots.
/// registry id caps bound the charset upstream, but the host is the last
/// line before the filesystem: a separator or a traversal token here would
/// escape the root, so both are rejected by name regardless of what
/// consensus admitted.
pub(crate) fn safe_path_component(id: &str) -> Result<(), String> {
    if id.is_empty() || id == "." || id == ".." {
        return Err(format!(
            "agent id {id:?} is not usable as a path component"
        ));
    }
    if id.bytes().any(|b| b == b'/' || b == b'\\' || b == 0) {
        return Err(format!(
            "agent id {id:?} carries a path separator; refusing to use it \
             under the agent roots"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_component_defense_rejects_separators_and_traversal() {
        for bad in ["", ".", "..", "a/b", "a\\b", "../up", "a\0b"] {
            assert!(
                safe_path_component(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
        for ok in ["bot", "my-agent.v2", "b_ot", ".hidden-is-just-a-name"] {
            assert!(safe_path_component(ok).is_ok(), "{ok:?} must pass");
        }
    }
}
