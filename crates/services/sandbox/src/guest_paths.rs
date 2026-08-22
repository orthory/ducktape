//! hiding host paths from the run.
//!
//! A run's argv and env are written on the host and carry host paths — the
//! operator's home directory, the node's data directory, the real workspace
//! location. Handing those to the guest leaks the operator's identity and
//! layout even though the guest cannot reach any of it.
//!
//! Under the microVM backend the guest layout is FIXED rather than planned:
//! there are no bind mounts to arrange, only block devices that always land at
//! the same mountpoints. So this module is a substring rewrite over a handful
//! of known pairs, not a mount planner.

use std::path::{Path, PathBuf};

/// where a run's workspace always appears inside the guest.
pub const GUEST_WORKSPACE: &str = "/workspace";
/// the guest's `HOME`. The rootfs ships it; no host home is ever visible.
pub const GUEST_HOME: &str = "/root";
/// where the persistent per-agent cache volume always appears.
pub const GUEST_AGENT_VOLUME: &str = "/agent";

/// the host→guest path pairs for one run, longest host path first so a nested
/// path wins over its parent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuestLayout {
    pairs: Vec<(String, String)>,
}

impl GuestLayout {
    /// the layout for a run whose workspace is `workdir` and whose operator
    /// home is `home`.
    pub fn new(workdir: &Path, home: &Path) -> Self {
        let mut layout = Self::default();
        layout.map(workdir, Path::new(GUEST_WORKSPACE));
        layout.map(home, Path::new(GUEST_HOME));
        layout
    }

    /// add one pair. Ordering is maintained on insert, so [`Self::translate`]
    /// never has to think about it.
    pub fn map(&mut self, host: &Path, guest: &Path) {
        let host = host.to_string_lossy().into_owned();
        if host.is_empty() {
            return;
        }
        self.pairs
            .push((host, guest.to_string_lossy().into_owned()));
        // longest host prefix first: an auth directory under HOME must beat
        // HOME itself, or every nested path is rewritten to the wrong place.
        self.pairs
            .sort_by_key(|(host, _)| std::cmp::Reverse(host.len()));
    }

    /// rewrite every host-path substring in `value` to its guest path.
    ///
    /// Substring rather than whole-value on purpose: a host path can be
    /// embedded in a larger string (the codex `projects."<workdir>"` TOML key
    /// is the case that forced this), and a whole-value match would leave those
    /// untouched.
    pub fn translate(&self, value: &str) -> String {
        let mut out = value.to_string();
        for (host, guest) in &self.pairs {
            out = out.replace(host.as_str(), guest.as_str());
        }
        out
    }

    /// the guest path for a host path that is mapped exactly.
    pub fn guest_of(&self, host: &Path) -> Option<PathBuf> {
        let host = host.to_string_lossy();
        self.pairs
            .iter()
            .find(|(candidate, _)| *candidate == host)
            .map(|(_, guest)| PathBuf::from(guest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> GuestLayout {
        GuestLayout::new(
            Path::new("/home/operator/ducktape/runs/run7/ws"),
            Path::new("/home/operator"),
        )
    }

    #[test]
    fn the_workspace_and_home_are_rewritten() {
        let l = layout();
        assert_eq!(
            l.translate("/home/operator/ducktape/runs/run7/ws"),
            GUEST_WORKSPACE
        );
        assert_eq!(l.translate("/home/operator"), GUEST_HOME);
    }

    /// The workspace lives UNDER the home directory here, which is the normal
    /// case. If HOME won, the workspace would translate to
    /// `/root/ducktape/runs/run7/ws` — a path that does not exist in the guest,
    /// and the run would fail with a confusing ENOENT rather than working.
    #[test]
    fn a_nested_path_beats_its_parent_whatever_the_insert_order() {
        let l = layout();
        assert_eq!(
            l.translate("/home/operator/ducktape/runs/run7/ws/src/main.rs"),
            "/workspace/src/main.rs"
        );

        // and the same with the pairs inserted the other way round
        let mut reversed = GuestLayout::default();
        reversed.map(Path::new("/home/operator"), Path::new(GUEST_HOME));
        reversed.map(
            Path::new("/home/operator/ducktape/runs/run7/ws"),
            Path::new(GUEST_WORKSPACE),
        );
        assert_eq!(
            reversed.translate("/home/operator/ducktape/runs/run7/ws/src/main.rs"),
            "/workspace/src/main.rs"
        );
    }

    /// The case that forced substring rewriting: the path is a TOML key, not
    /// the whole value.
    #[test]
    fn a_path_embedded_in_a_larger_string_is_rewritten_too() {
        let l = layout();
        assert_eq!(
            l.translate("projects.\"/home/operator/ducktape/runs/run7/ws\".trust_level=\"trusted\""),
            "projects.\"/workspace\".trust_level=\"trusted\""
        );
    }

    /// The whole point: nothing the run can read names the operator.
    #[test]
    fn no_host_path_survives_translation() {
        let l = layout();
        for value in [
            "--cwd=/home/operator/ducktape/runs/run7/ws",
            "HOME=/home/operator",
            "/home/operator/.config/thing",
        ] {
            let translated = l.translate(value);
            assert!(
                !translated.contains("/home/operator"),
                "{value} -> {translated}"
            );
        }
    }

    #[test]
    fn an_empty_host_path_is_not_a_rewrite_rule() {
        let mut l = GuestLayout::default();
        l.map(Path::new(""), Path::new("/nope"));
        assert_eq!(l.translate("anything at all"), "anything at all");
    }
}
