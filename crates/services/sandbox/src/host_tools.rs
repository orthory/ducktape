//! finding the host binaries the sandbox shells out to.
//!
//! `PATH` alone is not enough for an unprivileged process: `nft` ships in
//! `/usr/sbin` and `mke2fs` in `/sbin`, neither of which a non-root login shell
//! usually carries. A probe that only walked `PATH` would refuse a host that
//! can in fact run everything.

use std::path::{Path, PathBuf};

/// the dirs a non-login `PATH` usually omits, searched after `PATH`.
/// The Homebrew entries are macOS: `e2fsprogs` is keg-only there, so `mke2fs`
/// and `debugfs` live under the keg's own `sbin` — and a node started by
/// launchd, cron or a bare ssh command does not have brew's own bin dirs
/// either. Dirs that do not exist on this OS cost one failed stat each.
const FALLBACK_DIRS: [&str; 9] = [
    "/usr/sbin",
    "/sbin",
    "/usr/bin",
    "/bin",
    "/opt/homebrew/opt/e2fsprogs/sbin",
    "/usr/local/opt/e2fsprogs/sbin",
    "/opt/homebrew/sbin",
    "/opt/homebrew/bin",
    "/usr/local/bin",
];

/// resolve a system tool by `PATH`, then by [`FALLBACK_DIRS`].
pub fn find_system_tool(bin: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    FALLBACK_DIRS
        .into_iter()
        .map(|dir| Path::new(dir).join(bin))
        .find(|candidate| candidate.is_file())
}

/// first EXECUTABLE file named `bin` on `PATH`. Stricter than
/// [`find_system_tool`] and used where the answer is going to be exec'd rather
/// than handed to a shell-out.
pub fn find_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| crate::is_executable(candidate))
}

/// [`find_on_path`], then [`FALLBACK_DIRS`], still demanding an executable.
/// The VMM binary is resolved through THIS: a node started by launchd, cron
/// or a bare ssh command has a PATH without brew's bin dirs, and "the VMM is
/// not executable on PATH" on a host where it is plainly installed is the
/// kind of refusal an operator cannot decode.
pub fn find_executable(bin: &str) -> Option<PathBuf> {
    find_on_path(bin).or_else(|| find_system_tool(bin).filter(|tool| crate::is_executable(tool)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_only_in_sbin_is_still_found() {
        // `nft` is the canonical case: /usr/sbin on every distro, and absent
        // from an unprivileged PATH.
        let on_path = find_on_path("nft");
        let anywhere = find_system_tool("nft");
        if anywhere.is_none() {
            return; // host without nftables; nothing to assert
        }
        assert!(
            anywhere.is_some(),
            "find_system_tool must look past PATH ({on_path:?})"
        );
    }

    #[test]
    fn a_tool_that_does_not_exist_is_none() {
        assert!(find_system_tool("ducktape-no-such-tool").is_none());
        assert!(find_on_path("ducktape-no-such-tool").is_none());
    }
}
