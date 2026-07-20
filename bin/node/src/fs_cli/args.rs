//! hand-rolled arg parsing + node-address resolution (the `bin/node` shape — no
//! clap anywhere in the workspace).

use std::collections::BTreeMap;

/// a CLI failure carrying the process exit code. code 2 is a usage/verb error;
/// code 1 is a general operational failure (and a dirty `status`). an EMPTY
/// message prints nothing — `status` writes its own A/M/D lines and then exits
/// non-zero without a redundant error line.
#[derive(Debug)]
pub struct CliError {
    pub code: u8,
    pub message: String,
}

impl CliError {
    /// a usage error (exit 2): a bad verb, a missing arg, an unresolved node.
    pub fn usage(m: impl Into<String>) -> Self {
        CliError {
            code: 2,
            message: m.into(),
        }
    }

    /// a general operational failure (exit 1): a node rejection, an io error.
    pub fn failed(m: impl Into<String>) -> Self {
        CliError {
            code: 1,
            message: m.into(),
        }
    }

    /// exit with `code` and print NOTHING — the verb already wrote its output
    /// (a dirty `status` prints its A/M/D lines, then exits 1 silently; a commit
    /// conflict prints its report, then exits 2).
    pub fn silent(code: u8) -> Self {
        CliError {
            code,
            message: String::new(),
        }
    }
}

/// split `args` into positionals and `--key value` flags. a `--flag` consumes
/// the next token as its value UNLESS that token is itself a `--flag`, in which
/// case it is a value-less boolean (`--no-rebase`) recorded as an empty string —
/// so `commit --no-rebase --message m` parses correctly (a duckfs path or value
/// never starts with `--`).
pub fn parse_flags(args: &[String]) -> Result<(Vec<String>, BTreeMap<String, String>), CliError> {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            let value = match it.peek() {
                Some(next) if !next.starts_with("--") => it.next().cloned().unwrap_or_default(),
                _ => String::new(),
            };
            flags.insert(name.to_string(), value);
        } else {
            positional.push(a.clone());
        }
    }
    Ok((positional, flags))
}

/// the node http base from the `--node` flag or the `DUCKTAPE_NODE` env, or
/// `None` if neither is set (worktree verbs fall back to the index from here).
pub fn node_flag_or_env(flags: &BTreeMap<String, String>) -> Option<String> {
    if let Some(url) = flags.get("node")
        && !url.is_empty()
    {
        return Some(url.clone());
    }
    std::env::var("DUCKTAPE_NODE")
        .ok()
        .filter(|url| !url.is_empty())
}

/// resolve the node http base for a read verb: `--node <url>` flag, else the
/// `DUCKTAPE_NODE` env var. read verbs have no working-copy index to fall back
/// to (worktree verbs add that fallback in `work_cmds`).
pub fn resolve_node(flags: &BTreeMap<String, String>) -> Result<String, CliError> {
    node_flag_or_env(flags).ok_or_else(|| {
        CliError::usage("no node address: pass --node <http-url> or set DUCKTAPE_NODE")
    })
}

/// parse an optional numeric flag, naming it on a bad value.
pub fn flag_u64(flags: &BTreeMap<String, String>, key: &str) -> Result<Option<u64>, CliError> {
    match flags.get(key) {
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| CliError::usage(format!("--{key} must be a number"))),
        None => Ok(None),
    }
}
