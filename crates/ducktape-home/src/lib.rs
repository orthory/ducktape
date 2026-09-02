//! THE ducktape home resolution: `$DUCKTAPE_HOME` when set to a non-empty
//! value, else `$HOME/.ducktape`.
//!
//! One function in a zero-dependency leaf because the alternative was four
//! copies of it, and four copies of a one-line rule is how it drifts — two of
//! them honoured a set-but-empty override while the other two did not, so a
//! run with `DUCKTAPE_HOME=` exported put the node's workspaces under
//! `$HOME/.ducktape` and the signing key under `./keys`.

use std::ffi::OsString;
use std::path::PathBuf;

/// everything this operator keeps on disk: `$DUCKTAPE_HOME` when the override
/// is set to a non-empty value (tests, portable setups, huddle lanes), else
/// `~/.ducktape`.
///
/// set-but-empty is unset. That is how the shell readers beside this one spell
/// it (`${DUCKTAPE_HOME:-$HOME/.ducktape}`), and honouring an empty value here
/// would resolve every root under it to a RELATIVE path in whatever directory
/// the process happened to start in.
pub fn root() -> Result<PathBuf, String> {
    root_from(std::env::var_os("DUCKTAPE_HOME"), std::env::var_os("HOME"))
}

/// the resolution above with both variables passed in, so a test covers set /
/// set-but-empty / unset without mutating this process's environment.
fn root_from(root: Option<OsString>, home: Option<OsString>) -> Result<PathBuf, String> {
    let overridden = root.filter(|root| !root.is_empty());
    match overridden {
        Some(root) => Ok(PathBuf::from(root)),
        None => {
            let home =
                home.ok_or("cannot resolve the ducktape home — set $DUCKTAPE_HOME or $HOME")?;
            Ok(PathBuf::from(home).join(".ducktape"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty-means-unset is the half that is easy to get wrong and the half a
    /// shell reader gets for free: with `DUCKTAPE_HOME=` exported, a resolver
    /// that only asks "is it set" hands back a relative path in whatever
    /// directory the process happened to start in.
    #[test]
    fn takes_an_override_and_reads_an_empty_one_as_unset() {
        let home = Some(OsString::from("/home/duck"));
        let default_root = PathBuf::from("/home/duck/.ducktape");
        assert_eq!(
            root_from(Some(OsString::from("/srv/duck")), home.clone()),
            Ok(PathBuf::from("/srv/duck")),
            "a non-empty override is the root"
        );
        assert_eq!(
            root_from(Some(OsString::new()), home.clone()),
            Ok(default_root.clone()),
            "set-but-empty is unset, the way the shell readers take it"
        );
        assert_eq!(root_from(None, home), Ok(default_root), "the default");
        let neither = root_from(None, None).expect_err("no root without either variable");
        assert!(
            neither.contains("DUCKTAPE_HOME") && neither.contains("HOME"),
            "the error names both variables an operator can set: {neither}"
        );
    }
}
