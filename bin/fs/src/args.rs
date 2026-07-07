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
}

/// split `args` into positionals and `--key value` flags. `--flag` always
/// consumes the next token as its value (duckfs has no boolean-only flags that
/// need `--no-rebase` handling here — that verb reads the flag's presence via a
/// sentinel value written by the caller, see `work_cmds`).
pub fn parse_flags(args: &[String]) -> Result<(Vec<String>, BTreeMap<String, String>), CliError> {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            // a value-less trailing flag (like `--no-rebase`) records an empty
            // string, so a verb can test presence without a value.
            match it.next() {
                Some(v) => {
                    flags.insert(name.to_string(), v.clone());
                }
                None => {
                    flags.insert(name.to_string(), String::new());
                }
            }
        } else {
            positional.push(a.clone());
        }
    }
    Ok((positional, flags))
}

/// resolve the node http base for a read verb: `--node <url>` flag, else the
/// `DUCKTAPE_NODE` env var. read verbs have no working-copy index to fall back
/// to (worktree verbs add that fallback in `work_cmds`).
pub fn resolve_node(flags: &BTreeMap<String, String>) -> Result<String, CliError> {
    if let Some(url) = flags.get("node")
        && !url.is_empty()
    {
        return Ok(url.clone());
    }
    if let Ok(url) = std::env::var("DUCKTAPE_NODE")
        && !url.is_empty()
    {
        return Ok(url);
    }
    Err(CliError::usage(
        "no node address: pass --node <http-url> or set DUCKTAPE_NODE",
    ))
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
