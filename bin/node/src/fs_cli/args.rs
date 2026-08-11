//! the `fs` CLI error type, and the exit code it maps an unresolved node
//! address to. The addressing flags and the resolution ladder itself are
//! [`crate::cli_args::NodeAddr`] — ONE ladder for every family.

pub use crate::cli_args::NodeAddr;

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

/// resolve the node http base for a verb with no ambient address of its own:
/// [`NodeAddr::resolve`], the ONE ladder every family shares. Worktree verbs
/// that DO have one (a checkout's `.duckfs` index) call
/// [`NodeAddr::resolve_with`] in `work_cmds` instead.
pub fn resolve_node(addr: &NodeAddr) -> Result<String, CliError> {
    addr.resolve().map_err(CliError::usage)
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

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

    /// `--node` binds through the shared addressing group. The PRECEDENCE it
    /// sits at the top of is pinned once, in
    /// `cli_args::tests::the_node_address_ladder_ranks_flag_network_env_context_registry`
    /// — not re-asserted per family, which is how four of them drifted apart.
    #[test]
    fn dash_dash_node_binds_its_url() {
        let FsCmd::Ls(a) = parse(&["ls", "--node", "http://explicit:8844", "/p"]).unwrap() else {
            panic!("expected ls");
        };
        assert_eq!(a.addr.node.as_deref(), Some("http://explicit:8844"));
    }
}
