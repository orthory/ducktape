//! the CLI error type, the shared node-addressing flags, and node-address
//! resolution. clap owns the parsing now; this file only turns the typed
//! addressing flags into an http base.

use crate::config;

/// a CLI failure carrying the process exit code. code 2 is a usage error (an
/// unresolved node) and a commit conflict; code 1 is a general operational
/// failure (and a dirty `status`). an EMPTY message prints nothing — `status`
/// writes its own A/M/D lines and then exits non-zero without a redundant error
/// line.
#[derive(Debug)]
pub struct CliError {
    pub code: u8,
    pub message: String,
}

impl CliError {
    /// a usage error (exit 2): an unresolved node address.
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

/// the node-addressing flags every verb but `status` shares. `-n` is the short
/// alias for `--network`, the workspace selector shared with the node family.
#[derive(Debug, clap::Args)]
pub struct NodeAddr {
    /// the node's http base url (wins over -n/--network and DUCKTAPE_NODE)
    #[arg(long, value_name = "HTTP-URL")]
    pub node: Option<String>,
    /// a registered workspace's chain id — resolves to its node.toml http_listen
    #[arg(short = 'n', long, value_name = "CHAIN-ID")]
    pub network: Option<String>,
}

/// resolve the node http base honoring the fs addressing precedence: an explicit
/// `--node <url>` wins, then `-n/--network <id>` (registry → the workspace
/// node.toml's `http_listen`), then the `DUCKTAPE_NODE` env. `None` only when
/// NONE of the three is set — worktree verbs then fall back to the checkout
/// index. a set-but-broken `--network` (unknown/ambiguous workspace, or one with
/// no http listen) is a hard usage error, never a silent fall-through to env.
pub fn resolve_node_addr(addr: &NodeAddr) -> Result<Option<String>, CliError> {
    if let Some(url) = addr.node.as_deref().filter(|url| !url.is_empty()) {
        return Ok(Some(url.to_string()));
    }
    if let Some(needle) = addr.network.as_deref().filter(|needle| !needle.is_empty()) {
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
pub fn resolve_node(addr: &NodeAddr) -> Result<String, CliError> {
    resolve_node_addr(addr)?.ok_or_else(|| {
        CliError::usage(
            "no node address: pass --node <http-url>, -n/--network <id>, or set DUCKTAPE_NODE",
        )
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;
    use crate::fs_cli::FsCmd;

    /// a top-level `Parser` wrapper so the tests can drive clap over `FsCmd`
    /// (a `Subcommand` can't be parsed on its own).
    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: FsCmd,
    }

    fn parse(argv: &[&str]) -> Result<FsCmd, clap::Error> {
        // argv[0] is the binary name clap discards.
        TestCli::try_parse_from(std::iter::once("ducktape").chain(argv.iter().copied()))
            .map(|c| c.cmd)
    }

    /// `ls --json <path>` — the valueless `--json` must NOT swallow the path,
    /// which stays positional.
    #[test]
    fn ls_json_keeps_path_positional() {
        let FsCmd::Ls(a) = parse(&["ls", "--json", "/dir"]).unwrap() else {
            panic!("expected ls");
        };
        assert!(a.json);
        assert_eq!(a.path, "/dir");
    }

    /// `commit --no-rebase <dir>` — the valueless `--no-rebase` leaves `<dir>`
    /// positional; `--message` still binds its value.
    #[test]
    fn commit_no_rebase_keeps_dir_positional() {
        let FsCmd::Commit(a) = parse(&["commit", "--no-rebase", "wt/dir", "--message", "m"]).unwrap()
        else {
            panic!("expected commit");
        };
        assert!(a.no_rebase);
        assert_eq!(a.dir.as_deref(), Some("wt/dir"));
        assert_eq!(a.message, "m");
    }

    /// `-n` is the short alias for `--network` and does not eat the positional.
    #[test]
    fn dash_n_maps_to_network_and_leaves_path() {
        let FsCmd::Ls(a) = parse(&["ls", "-n", "ducktape", "some/path"]).unwrap() else {
            panic!("expected ls");
        };
        assert_eq!(a.addr.network.as_deref(), Some("ducktape"));
        assert_eq!(a.path, "some/path");
    }

    /// a value flag binds the following token as its value.
    #[test]
    fn snapshot_flag_binds_its_value() {
        let FsCmd::Ls(a) = parse(&["ls", "/p", "--snapshot", "s1"]).unwrap() else {
            panic!("expected ls");
        };
        assert_eq!(a.path, "/p");
        assert_eq!(a.snapshot.as_deref(), Some("s1"));
    }

    /// clap validates the numeric flag: a non-number is a parse error (exit 2).
    #[test]
    fn limit_rejects_a_non_number() {
        assert!(parse(&["ls", "/p", "--limit", "abc"]).is_err());
    }

    /// --node short-circuits before any registry walk, so a bogus -n never
    /// errors: the explicit flag is the top of the precedence chain.
    #[test]
    fn explicit_node_wins_over_network_without_touching_the_registry() {
        let addr = NodeAddr {
            node: Some("http://explicit:8844".into()),
            network: Some("no-such-workspace".into()),
        };
        assert_eq!(
            resolve_node_addr(&addr).unwrap(),
            Some("http://explicit:8844".to_string())
        );
    }
}
