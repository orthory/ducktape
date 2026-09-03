//! an in-process duckfs node for the CLI e2e, over noded's shared in-proc
//! daemon testkit: a REAL `host::Host::genesis` with ONLY the files module,
//! fronted by noded's router on a loopback listener. the CLI subprocess
//! (`env!(CARGO_BIN_EXE_ducktape) fs`) drives it over http exactly as it would
//! a real daemon.
//!
//! the `NodeCommand` actor this used to hand-mirror now lives ONCE in
//! `noded::testkit`; this harness just builds the host and hands it over.
#![allow(dead_code)]

use std::process::Command;

use host::Host;
use noded::testkit::InProcDaemon;

/// a running in-process node plus the CLI-under-test's path.
pub struct Harness {
    // dropped BEFORE `dir` (fields drop in declaration order): the daemon's Drop
    // joins the actor, closing qmdb, so the tempdir is removed only afterward.
    daemon: InProcDaemon,
    dir: tempfile::TempDir,
}

impl Harness {
    /// stand up the node: genesis the files module on the testkit's actor thread
    /// and block until `/v1/status` answers.
    pub fn start() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("ducktape fs-e2e")
            .tempdir()
            .expect("harness tempdir");
        let duckfs_dir = dir.path().join("duckfs");
        let daemon = InProcDaemon::start(
            move || {
                let files = files::Files::open("files", duckfs_dir).expect("open files");
                Host::genesis(vec![Box::new(files)]).expect("genesis")
            },
            vec!["files".into()],
        );
        Harness { daemon, dir }
    }

    /// the http base the CLI's `--node` flag takes.
    pub fn node_url(&self) -> String {
        self.daemon.node_url()
    }

    /// a duckfs transport whose writes this node admits: the harness owns the
    /// node, so it presents the operator credential the daemon minted.
    pub fn files(&self) -> duckfs_client::http::HttpNode {
        self.daemon.files()
    }

    /// a `ducktape fs` invocation pre-pointed at this node via `--node`.
    pub fn cli(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape"));
        cmd.arg("fs").args(args).arg("--node").arg(self.node_url());
        cmd
    }

    /// a bare `ducktape fs` invocation (no `--node`) — for the resolution-error
    /// and stub-verb cases.
    ///
    /// `DUCKTAPE_HOME` points at this harness's own temp dir so the bottom rung
    /// of the addressing ladder (the lone registered workspace) sees an EMPTY
    /// registry. Without it the run reads the developer's real
    /// `~/.ducktape/workspaces` and the outcome depends on whose box it is.
    pub fn cli_bare(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape"));
        cmd.arg("fs")
            .args(args)
            .env("DUCKTAPE_HOME", self.dir.path());
        cmd
    }
}
