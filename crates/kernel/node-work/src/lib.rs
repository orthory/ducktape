//! The native bridge executes state-derived work supplied by a consensus module.
//!
//! Policy belongs to the module: membership, grants, ballots, deadlines and
//! success conditions never enter this protocol. A node asks the configured
//! source for its next directive at each committed block, executes it, and
//! asks again. Directives can repeat after a crash or concurrent submission:
//! messages MUST be idempotent, and the source MUST derive its next directive
//! from committed results. Staging is content-addressed and repeatable.
//!
//! A blob is an already assembled file. Artifact formats, framing and validation
//! rules belong to its consumer; the bridge only verifies its bytes and hash.
use serde::{Deserialize, Serialize};

/// The existing module staging bound, shared by both ingress routes.
pub const MAX_STAGED_BLOB_BYTES: usize = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
/// Clock values are local committed-status hints, not authority. Targets must
/// validate every resulting submission against their actual execution context.
pub enum Query {
    NodeWork {
        node_key: Vec<u8>,
        height: u64,
        consensus_time: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    NodeWork(Option<Directive>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub target: String,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Directive {
    /// Submit verbatim as the node. The target owns admission and deduplication.
    Submit(Submission),
    /// Stage an immutable forge file, then submit the source's chosen message.
    /// Missing local objects retry on later block wakes; invalid paths or bytes
    /// submit `on_invalid`. Neither continuation is interpreted by the bridge.
    StageBlob {
        blob: ForgeBlob,
        on_ready: Submission,
        on_invalid: Submission,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeBlob {
    pub repo: String,
    pub commit: String,
    pub path: String,
    pub hash: [u8; 32],
}

pub fn relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directive_carries_foreign_messages_without_interpreting_their_wire() {
        let directive = Directive::Submit(Submission {
            target: "a-module-the-executor-has-never-heard-of".into(),
            payload: br#"{"set":{"key":"x","value":7}}"#.to_vec(),
        });
        let bytes = sdk::wire::encode(&Reply::NodeWork(Some(directive.clone())));
        let decoded: Reply = sdk::wire::decode(&bytes).unwrap();
        assert_eq!(decoded, Reply::NodeWork(Some(directive)));
    }
}
