//! hand-rolled arg parsing + node-address resolution (the `bin/node` shape — no
//! clap anywhere in the workspace).

use std::collections::BTreeMap;

use crate::config;

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

/// the flag this token names, if any: the one short alias `-n` (→ `network`,
/// the workspace selector shared with the node family) or any `--long` flag. a
/// positional (a duckfs path, a flag's value) names none. the flag test must
/// recognize `-n` too, else a `-n` sitting right after a value-less flag is
/// silently eaten as its ignored value and the network is dropped (a value never
/// begins with `-`).
fn flag_name(tok: &str) -> Option<&str> {
    match tok {
        "-n" => Some("network"),
        other => other.strip_prefix("--"),
    }
}

/// flags that take NO value — a bare presence is the whole signal, so they never
/// consume the following token even when it is a positional (`ls --json <path>`
/// keeps `<path>` positional, `commit --no-rebase <dir>` keeps `<dir>`
/// positional). every other flag consumes the next token unless that token is
/// itself a flag.
const BOOL_FLAGS: &[&str] = &["json", "no-rebase"];

/// split `args` into positionals and `--key value` flags. a flag consumes the
/// next token as its value UNLESS the flag is a known valueless boolean
/// ([`BOOL_FLAGS`]) or the next token is itself a flag (recognized via
/// [`flag_name`], so `-n` counts), in which case the value is an empty string —
/// so `commit --no-rebase --message m`, `commit --no-rebase -n net`, `commit
/// --no-rebase <dir>`, and `ls --json <path>` all parse correctly.
pub fn parse_flags(args: &[String]) -> Result<(Vec<String>, BTreeMap<String, String>), CliError> {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        let Some(name) = flag_name(a) else {
            positional.push(a.clone());
            continue;
        };
        let is_bool_flag = BOOL_FLAGS.contains(&name);
        let next_is_value = it.peek().is_some_and(|next| flag_name(next).is_none());
        let value = if !is_bool_flag && next_is_value {
            it.next().cloned().unwrap_or_default()
        } else {
            String::new()
        };
        flags.insert(name.to_string(), value);
    }
    Ok((positional, flags))
}

/// resolve the node http base honoring the fs addressing precedence: an explicit
/// `--node <url>` wins, then `-n/--network <id>` (registry → the workspace
/// node.toml's `http_listen`), then the `DUCKTAPE_NODE` env. `None` only when
/// NONE of the three is set — worktree verbs then fall back to the checkout
/// index. a set-but-broken `--network` (unknown/ambiguous workspace, or one with
/// no http listen) is a hard usage error, never a silent fall-through to env.
pub fn resolve_node_addr(flags: &BTreeMap<String, String>) -> Result<Option<String>, CliError> {
    if let Some(url) = flags.get("node").filter(|url| !url.is_empty()) {
        return Ok(Some(url.clone()));
    }
    if let Some(needle) = flags.get("network").filter(|needle| !needle.is_empty()) {
        let (_dir, http) = config::resolve_network(needle).map_err(CliError::usage)?;
        let base = http.ok_or_else(|| {
            CliError::usage(format!(
                "network {needle:?} resolves to a workspace with no http listen \
                 (its node.toml sets no http_listen) — pass --node <http-url>"
            ))
        })?;
        return Ok(Some(base));
    }
    Ok(std::env::var("DUCKTAPE_NODE")
        .ok()
        .filter(|url| !url.is_empty()))
}

/// resolve the node http base for a read verb: the addressing chain above, which
/// read verbs require (they have no working-copy index to fall back to — worktree
/// verbs add that fallback in `work_cmds`).
pub fn resolve_node(flags: &BTreeMap<String, String>) -> Result<String, CliError> {
    resolve_node_addr(flags)?.ok_or_else(|| {
        CliError::usage(
            "no node address: pass --node <http-url>, -n/--network <id>, or set DUCKTAPE_NODE",
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dash_n_maps_to_network_and_leaves_positionals() {
        let (pos, f) = parse_flags(&["-n".into(), "ducktape".into(), "some/path".into()]).unwrap();
        assert_eq!(f.get("network").map(String::as_str), Some("ducktape"));
        assert_eq!(pos, vec!["some/path".to_string()]);
    }

    #[test]
    fn dash_n_after_value_less_boolean_is_not_swallowed() {
        // `--no-rebase` is a value-less boolean; a `-n` directly after it must be
        // recognized as its own flag, not eaten as --no-rebase's ignored value.
        let (pos, f) = parse_flags(&[
            "wt/dir".into(),
            "--no-rebase".into(),
            "-n".into(),
            "mynet".into(),
            "--message".into(),
            "m".into(),
        ])
        .unwrap();
        assert_eq!(f.get("no-rebase").map(String::as_str), Some(""));
        assert_eq!(f.get("network").map(String::as_str), Some("mynet"));
        assert_eq!(f.get("message").map(String::as_str), Some("m"));
        assert_eq!(pos, vec!["wt/dir".to_string()]);
    }

    #[test]
    fn explicit_node_wins_over_network_without_touching_the_registry() {
        // --node short-circuits before any registry walk, so the bogus -n never
        // errors: the explicit flag is the top of the precedence chain.
        let f = flags(&[
            ("node", "http://explicit:8844"),
            ("network", "no-such-workspace"),
        ]);
        assert_eq!(
            resolve_node_addr(&f).unwrap(),
            Some("http://explicit:8844".to_string())
        );
    }

    #[test]
    fn value_flag_consumes_next_token() {
        let (pos, flags) = parse_flags(&args(&["/p", "--snapshot", "s1"])).unwrap();
        assert_eq!(pos, vec!["/p".to_string()]);
        assert_eq!(flags.get("snapshot").map(String::as_str), Some("s1"));
    }

    #[test]
    fn json_before_positional_does_not_eat_it() {
        // `ls --json <path>` — --json must NOT swallow the path.
        let (pos, flags) = parse_flags(&args(&["--json", "/dir"])).unwrap();
        assert!(flags.contains_key("json"));
        assert_eq!(pos, vec!["/dir".to_string()]);
    }

    #[test]
    fn json_as_last_arg() {
        let (pos, flags) = parse_flags(&args(&["/dir", "--json"])).unwrap();
        assert!(flags.contains_key("json"));
        assert_eq!(pos, vec!["/dir".to_string()]);
    }

    #[test]
    fn no_rebase_before_positional_does_not_eat_it() {
        // `commit --no-rebase <dir>` — --no-rebase is valueless, <dir> stays positional.
        let (pos, flags) = parse_flags(&args(&["--no-rebase", "mydir"])).unwrap();
        assert!(flags.contains_key("no-rebase"));
        assert_eq!(pos, vec!["mydir".to_string()]);
    }

    #[test]
    fn no_rebase_between_value_flags() {
        let (pos, flags) = parse_flags(&args(&["--no-rebase", "--message", "hi"])).unwrap();
        assert!(pos.is_empty());
        assert!(flags.contains_key("no-rebase"));
        assert_eq!(flags.get("message").map(String::as_str), Some("hi"));
    }
}
