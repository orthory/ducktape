//! the portable workspace SOURCE — the tagged `workspace` block of a v3 run
//! envelope (wire contract §1: `kind` = `duckfs` | `forge`) and the plain-data
//! vocabulary ([`WorkspaceSource`]) the plan/spec carry it in across the
//! reachability wall.
//!
//! FLAG DAY: the pre-forge flat duckfs shape (no `kind`) no longer decodes —
//! a mixed-binary envelope fails loudly at the serde step, never silently
//! misreads. within a tagged variant, unknown ADDITIVE fields stay tolerated
//! (internally-tagged serde ignores them), so the old advisory `mount_path`
//! or a future additive key never kills an in-flight run.

use serde::Deserialize;

/// where a portable run's workspace comes from — the plain-data twin of the
/// envelope's tagged `workspace` block, carried on
/// [`crate::provision::PortablePlan`] and mirrored onto
/// [`crate::provision::WorkspaceSpec`] so the provisioner sees exactly the
/// committed pin. NO `mount_path` in either variant (D7/W1: the provisioner
/// mints its own writable cwd; the spec-level advisory field is duckfs-era
/// debt that must not spread).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSource {
    /// a duckfs SOURCE subtree: exactly the flat era's two coordinates.
    Duckfs {
        /// the rw source duckfs subtree (envelope `workspace.source_prefix`).
        source_prefix: String,
        /// the pinned source snapshot id (W2); `None` = committed head.
        source_snapshot: Option<String>,
    },
    /// a forge repo pinned at a commit, worked on `branch`.
    Forge {
        repo: String,
        /// authoritative Forge tracker title when the composing node supplied
        /// it; absent on older in-flight v3 envelopes.
        item_title: Option<String>,
        /// the pinned base commit (40-hex sha1) — COMMITTED refs at compose
        /// height, the checkout/fork point.
        commit: String,
        /// the work branch (`agent/item-<n>`, or a PR's own source branch).
        branch: String,
        /// advisory compose-time metadata: whether `branch` existed in
        /// COMMITTED forge refs at compose height. the provisioner derives
        /// its push CAS base from the FETCHED remote advertisement (a fetch
        /// miss ⇒ zero-oid create), not this flag — kept as a pinned wire
        /// surface and an audit/M2 signal.
        branch_born: bool,
    },
}

impl WorkspaceSource {
    /// the receipt's source coordinates (contract §5): duckfs echoes its
    /// prefix/pin verbatim; a forge receipt carries `forge:<repo>` and the
    /// pinned commit — so every receipt names WHAT was checked out without a
    /// per-kind receipt shape.
    pub(crate) fn receipt_coords(&self) -> (String, Option<String>) {
        match self {
            WorkspaceSource::Duckfs {
                source_prefix,
                source_snapshot,
            } => (source_prefix.clone(), source_snapshot.clone()),
            WorkspaceSource::Forge { repo, commit, .. } => {
                (format!("forge:{repo}"), Some(commit.clone()))
            }
        }
    }

    /// the forge work branch, `None` for duckfs — the `pushed` receipt
    /// constructor stamps it as the pushed branch (§5).
    pub(crate) fn forge_branch(&self) -> Option<String> {
        match self {
            WorkspaceSource::Forge { branch, .. } => Some(branch.clone()),
            WorkspaceSource::Duckfs { .. } => None,
        }
    }
}

/// the wire decode of a v3 envelope's `workspace` block (contract §1). source
/// coordinates are required; additive metadata defaults for old in-flight
/// envelopes. [`WireWorkspace::validate`] adds the per-field non-empty checks
/// and surfaces the plain [`WorkspaceSource`].
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WireWorkspace {
    Duckfs {
        source_prefix: String,
        source_snapshot: Option<String>,
    },
    Forge {
        repo: String,
        /// additive in v3: old in-flight envelopes carry no fallback title.
        #[serde(default)]
        item_title: Option<String>,
        commit: String,
        branch: String,
        branch_born: bool,
    },
}

impl WireWorkspace {
    /// per-variant, per-field loud validation: a v3 marker with an empty
    /// coordinate is a mixed-network signal, never silently downgraded.
    pub(crate) fn validate(self) -> Result<WorkspaceSource, String> {
        match self {
            WireWorkspace::Duckfs {
                source_prefix,
                source_snapshot,
            } => {
                if source_prefix.is_empty() {
                    return Err(
                        "v3 run envelope workspace.source_prefix must not be empty".into()
                    );
                }
                Ok(WorkspaceSource::Duckfs {
                    source_prefix,
                    source_snapshot,
                })
            }
            WireWorkspace::Forge {
                repo,
                item_title,
                commit,
                branch,
                branch_born,
            } => {
                for (field, value) in [("repo", &repo), ("commit", &commit), ("branch", &branch)] {
                    if value.is_empty() {
                        return Err(format!(
                            "v3 run envelope forge workspace.{field} must not be empty"
                        ));
                    }
                }
                Ok(WorkspaceSource::Forge {
                    repo,
                    item_title,
                    commit,
                    branch,
                    branch_born,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tagged_duckfs_shape_decodes_with_a_stated_null_pin() {
        // the EXACT composer bytes (task-1 report / contract §1): the duckfs
        // variant keeps the flat era's two fields; a null pin is a STATED key.
        let ws: WireWorkspace = serde_json::from_str(
            r#"{"kind":"duckfs","source_prefix":"/shared/agent-workspaces/bot","source_snapshot":null}"#,
        )
        .unwrap();
        assert_eq!(
            ws.validate().unwrap(),
            WorkspaceSource::Duckfs {
                source_prefix: "/shared/agent-workspaces/bot".into(),
                source_snapshot: None,
            }
        );
    }

    #[test]
    fn the_tagged_forge_shape_decodes_verbatim() {
        // the EXACT composer bytes (task-1 report §"Exact final serde shapes").
        let ws: WireWorkspace = serde_json::from_str(&format!(
            r#"{{"kind":"forge","repo":"app","item_title":"Fix the gate","commit":"{}","branch":"agent/item-7","branch_born":false}}"#,
            "d0".repeat(20)
        ))
        .unwrap();
        assert_eq!(
            ws.validate().unwrap(),
            WorkspaceSource::Forge {
                repo: "app".into(),
                item_title: Some("Fix the gate".into()),
                commit: "d0".repeat(20),
                branch: "agent/item-7".into(),
                branch_born: false,
            }
        );
    }

    #[test]
    fn a_pre_title_forge_envelope_keeps_running_without_a_fallback() {
        let ws: WireWorkspace = serde_json::from_str(
            r#"{"kind":"forge","repo":"app","commit":"d0","branch":"agent/item-7","branch_born":false}"#,
        )
        .unwrap();
        assert!(matches!(
            ws.validate().unwrap(),
            WorkspaceSource::Forge {
                item_title: None,
                ..
            }
        ));
    }

    #[test]
    fn an_untagged_or_unknown_kind_workspace_fails_to_decode() {
        // flag day: the flat pre-forge shape (no kind) is rejected …
        let flat = r#"{"source_prefix":"/shared/agent-workspaces/bot","source_snapshot":null}"#;
        assert!(serde_json::from_str::<WireWorkspace>(flat).is_err());
        // … and so is a kind this worker does not understand.
        let alien = r#"{"kind":"svn","url":"http://x"}"#;
        assert!(serde_json::from_str::<WireWorkspace>(alien).is_err());
    }

    #[test]
    fn a_forge_workspace_missing_a_field_fails_to_decode() {
        for broken in [
            r#"{"kind":"forge","commit":"c","branch":"b","branch_born":true}"#, // no repo
            r#"{"kind":"forge","repo":"app","branch":"b","branch_born":true}"#, // no commit
            r#"{"kind":"forge","repo":"app","commit":"c","branch_born":true}"#, // no branch
            r#"{"kind":"forge","repo":"app","commit":"c","branch":"b"}"#,       // no branch_born
        ] {
            assert!(
                serde_json::from_str::<WireWorkspace>(broken).is_err(),
                "decoded: {broken}"
            );
        }
    }

    #[test]
    fn validation_rejects_empty_forge_coordinates_per_field() {
        let cases = [
            (r#"{"kind":"forge","repo":"","commit":"c","branch":"b","branch_born":true}"#, "repo"),
            (r#"{"kind":"forge","repo":"app","commit":"","branch":"b","branch_born":true}"#, "commit"),
            (r#"{"kind":"forge","repo":"app","commit":"c","branch":"","branch_born":true}"#, "branch"),
        ];
        for (json, field) in cases {
            let err = serde_json::from_str::<WireWorkspace>(json)
                .unwrap()
                .validate()
                .unwrap_err();
            assert!(
                err.contains(&format!("workspace.{field} must not be empty")),
                "{field}: got {err:?}"
            );
        }
    }

    #[test]
    fn validation_rejects_an_empty_duckfs_prefix() {
        let err = serde_json::from_str::<WireWorkspace>(
            r#"{"kind":"duckfs","source_prefix":"","source_snapshot":null}"#,
        )
        .unwrap()
        .validate()
        .unwrap_err();
        assert!(err.contains("source_prefix must not be empty"), "got {err:?}");
    }

    #[test]
    fn receipt_coords_echo_duckfs_verbatim_and_pin_forge_to_repo_and_commit() {
        let duckfs = WorkspaceSource::Duckfs {
            source_prefix: "/shared/agent-workspaces/bot".into(),
            source_snapshot: Some("aa".repeat(32)),
        };
        assert_eq!(
            duckfs.receipt_coords(),
            ("/shared/agent-workspaces/bot".to_string(), Some("aa".repeat(32)))
        );
        // contract §5: source_prefix = "forge:<repo>", source_snapshot = the
        // pinned commit — every receipt names WHAT was checked out without a
        // per-kind receipt shape.
        let forge = WorkspaceSource::Forge {
            repo: "app".into(),
            item_title: Some("Fix the gate".into()),
            commit: "d0".repeat(20),
            branch: "agent/item-7".into(),
            branch_born: true,
        };
        assert_eq!(
            forge.receipt_coords(),
            ("forge:app".to_string(), Some("d0".repeat(20)))
        );
    }

    #[test]
    fn extra_fields_inside_a_tagged_workspace_are_tolerated() {
        // additive-field tolerance survives the tagged shape: an old advisory
        // mount_path (or any future additive key) inside the object decodes
        // fine — internally-tagged serde ignores unknown fields.
        let ws: WireWorkspace = serde_json::from_str(
            r#"{"kind":"duckfs","source_prefix":"/p","source_snapshot":null,"mount_path":"/tmp/x"}"#,
        )
        .unwrap();
        assert!(ws.validate().is_ok());
    }
}
