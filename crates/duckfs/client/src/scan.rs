//! walk a checkout directory into per-path facts (size, mtime, exec, kind).
//!
//! the walk is the raw OS observation; `status` diffs it against the index. paths
//! are the absolute duckfs paths (the index `prefix` joined with the on-disk
//! relative path), so a scan entry keys directly into the index. the `.duckfs`
//! state dir at the checkout root is skipped — it is client bookkeeping, not
//! replicated content — and so is every `.git` (a checkout holding a git repo is
//! the dogfooding shape; its object store is never the user's content).
//!
//! everything else the user excludes goes in [`IGNORE_FILE`] at the checkout
//! root, gitignore-shaped. there is NO built-in default list: a default that
//! silently drops files is worse than the change ceiling it dodges, so the file
//! is the only place `target/` or `node_modules/` is named. an ignored DIRECTORY
//! is pruned, never walked and filtered — the failure mode is 100k paths under
//! `target/`, and walking them to discard them is still that failure.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// what an on-disk entry is. only these three kinds materialize; anything else
/// (fifo, socket, device) is out of scope for a duckfs checkout and is skipped
/// by the walk — never stat'd into a `File` and read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanKind {
    File,
    Symlink,
    Dir,
}

/// one observed path. `mtime` is split into whole seconds and sub-second nanos so
/// the racy-clean rule can compare at whatever granularity the filesystem offers.
/// `size` is the file byte length, a symlink target's byte length, or 0 for a dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// the absolute duckfs path (index `prefix` + on-disk relative path).
    pub path: String,
    pub kind: ScanKind,
    pub size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub exec: bool,
    /// the symlink target, present only for [`ScanKind::Symlink`].
    pub target: Option<String>,
    /// true only for a directory with no children — the case `status` tracks so a
    /// fresh empty dir can be told from a recorded one.
    pub empty_dir: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("duckfs: scan io: {0}")]
    Io(String),
    #[error("duckfs: non-utf8 path under the checkout: {0}")]
    NonUtf8(String),
    /// a `.duckfsignore` line that is not a pattern. loud, not skipped: a
    /// silently-dropped rule scans the tree the user thought they excluded.
    #[error("duckfs: {IGNORE_FILE} line {line}: {pattern:?} is not a valid pattern ({reason})")]
    BadIgnorePattern {
        line: usize,
        pattern: String,
        reason: String,
    },
}

impl From<std::io::Error> for ScanError {
    fn from(e: std::io::Error) -> Self {
        ScanError::Io(e.to_string())
    }
}

/// the ignore file at the checkout root — the ONE place a checkout says what is
/// not content.
pub const IGNORE_FILE: &str = ".duckfsignore";

/// how a pattern is matched: `*` and `?` stop at a `/` (so `*.log` is a name
/// pattern, not a path one), `**` spans components, and a leading dot is not
/// special (`.cache` is matched by `*`, as gitignore matches it).
const MATCH: glob::MatchOptions = glob::MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

/// one parsed `.duckfsignore` line.
struct Rule {
    pattern: glob::Pattern,
    /// the pattern held a `/`, so it matches the path from the checkout root
    /// down; without one it matches a NAME at any depth (gitignore's rule).
    anchored: bool,
    /// a trailing `/`: only a directory matches.
    dir_only: bool,
    /// a leading `!`: a match RE-INCLUDES instead of excluding.
    negated: bool,
}

/// the parsed `.duckfsignore`, rules in file order — the LAST one that matches
/// decides, which is what makes `!` work. re-including under an already-excluded
/// directory is impossible (gitignore says the same): the directory is pruned,
/// so nothing under it is ever looked at.
#[derive(Default)]
pub(crate) struct Ignore {
    rules: Vec<Rule>,
}

impl Ignore {
    /// read `<root>/.duckfsignore`. a missing file is an empty rule set; an
    /// unreadable one is an io error, never a silent empty set.
    pub(crate) fn load(root: &Path) -> Result<Ignore, ScanError> {
        let text = match fs::read_to_string(root.join(IGNORE_FILE)) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ignore::default()),
            Err(e) => return Err(ScanError::Io(e.to_string())),
        };
        Ignore::parse(&text)
    }

    fn parse(text: &str) -> Result<Ignore, ScanError> {
        let mut rules = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            let is_blank = line.is_empty();
            let is_comment = line.starts_with('#');
            if is_blank || is_comment {
                continue;
            }
            let (negated, rest) = match line.strip_prefix('!') {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let dir_only = rest.ends_with('/');
            let body = rest.trim_end_matches('/');
            let anchored = body.contains('/');
            let body = body.trim_start_matches('/');
            // a bare `/`, `!` or `!/` names nothing.
            if body.is_empty() {
                continue;
            }
            let pattern = glob::Pattern::new(body).map_err(|e| ScanError::BadIgnorePattern {
                line: n + 1,
                pattern: raw.to_string(),
                reason: e.to_string(),
            })?;
            rules.push(Rule {
                pattern,
                anchored,
                dir_only,
                negated,
            });
        }
        Ok(Ignore { rules })
    }

    /// does the entry `name` at `rel` (relative to the checkout root) match a
    /// rule? the walk asks this per entry — every ancestor is already known
    /// un-ignored, because an ignored directory was pruned.
    fn matches(&self, rel: &str, name: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if rule.dir_only && !is_dir {
                continue;
            }
            let subject = if rule.anchored { rel } else { name };
            if rule.pattern.matches_with(subject, MATCH) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    /// is `rel` ignored — itself, or under an ignored directory? this is what
    /// the walk's pruning means for a path the walk never reached (an indexed
    /// path `status` has to classify).
    pub(crate) fn ignored_path(&self, rel: &str, is_dir: bool) -> bool {
        let mut walked = String::new();
        let mut segs = rel.split('/').peekable();
        while let Some(seg) = segs.next() {
            if !walked.is_empty() {
                walked.push('/');
            }
            walked.push_str(seg);
            // every ancestor IS a directory; only the leaf carries its own kind.
            let is_leaf = segs.peek().is_none();
            if self.matches(&walked, seg, !is_leaf || is_dir) {
                return true;
            }
        }
        false
    }
}

/// scan `root` (a checkout directory) under duckfs `prefix`, returning every
/// entry sorted by path. directories are emitted (with the `empty_dir` flag) as
/// well as files and symlinks, so `status` can both track empty dirs and treat a
/// non-empty dir as "seen" (never a spurious removal). `.duckfsignore` at the
/// root prunes what it names.
pub fn scan(root: &Path, prefix: &str) -> Result<Vec<ScanEntry>, ScanError> {
    let ignore = Ignore::load(root)?;
    let mut out = Vec::new();
    // latched: the first skipped fifo/socket/device says so, the rest are quiet.
    // a per-entry warn on a checkout holding a socket directory is a log bomb.
    let mut warned_special = false;
    scan_dir(root, root, prefix, &ignore, &mut warned_special, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// the disk path for a duckfs path under this checkout: strip the prefix and
/// re-root. shared with `status` so a rehash reads the right file.
pub fn disk_path(root: &Path, prefix: &str, duckfs_path: &str) -> PathBuf {
    let rel = duckfs_path
        .strip_prefix(prefix)
        .unwrap_or(duckfs_path)
        .trim_start_matches('/');
    root.join(rel)
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    prefix: &str,
    ignore: &Ignore,
    warned_special: &mut bool,
    out: &mut Vec<ScanEntry>,
) -> Result<(), ScanError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))?;

        // client bookkeeping and a git object store are never content: the
        // `.duckfs` dir at the checkout root, and any `.git` (a checkout that
        // holds a git repo is the documented dogfooding shape).
        let is_state_dir = dir == root && name == ".duckfs";
        let is_git = name == ".git";
        if is_state_dir || is_git {
            continue;
        }

        let duckfs_path = duckfs_join(root, &path, prefix)?;
        // lstat, so a symlink is observed as a symlink (never followed).
        let meta = fs::symlink_metadata(&path)?;
        let ft = meta.file_type();

        // an ignored entry is dropped here, before it is emitted and before a
        // directory is descended into — pruned, not walked and filtered.
        let rel = relative(root, &path)?;
        if ignore.matches(&rel, name, ft.is_dir()) {
            continue;
        }

        if ft.is_symlink() {
            let target = fs::read_link(&path)?;
            let target = target
                .to_str()
                .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))?
                .to_string();
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::Symlink,
                size: target.len() as u64,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                target: Some(target),
                empty_dir: false,
            });
        } else if ft.is_dir() {
            let empty = fs::read_dir(&path)?.next().is_none();
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::Dir,
                size: 0,
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec: false,
                target: None,
                empty_dir: empty,
            });
            scan_dir(root, &path, prefix, ignore, warned_special, out)?;
        } else if ft.is_file() {
            // a regular file: exec is the owner/group/other execute bits (the
            // module tracks one exec bit; any set bit means executable).
            let exec = meta.permissions().mode() & 0o111 != 0;
            out.push(ScanEntry {
                path: duckfs_path,
                kind: ScanKind::File,
                size: meta.len(),
                mtime_secs: meta.mtime(),
                mtime_nanos: meta.mtime_nsec() as u32,
                exec,
                target: None,
                empty_dir: false,
            });
        } else {
            // a fifo, socket or device node. the model holds files, symlinks and
            // dirs and nothing else, and this entry is NEVER opened: reading a
            // fifo with no writer blocks forever, which is how a `commit` over a
            // build directory that left a `.sock` behind hung with no output and
            // no log line. skipped, and said once.
            report_special(&path, &ft, warned_special);
        }
    }
    Ok(())
}

/// say — ONCE per scan — that a non-regular entry was skipped. the latch is the
/// point: a hang with no line is the worst outcome, a line per socket is the
/// second worst.
fn report_special(path: &Path, ft: &fs::FileType, warned: &mut bool) {
    if *warned {
        return;
    }
    *warned = true;
    tracing::warn!(
        target: "ducktape::files",
        reason = "unsupported_file_kind",
        kind = special_kind(ft),
        path = %path.display(),
        "skipped a non-regular entry in the checkout; later ones this scan are silent",
    );
}

/// name the kind of a non-regular entry for the log line.
fn special_kind(ft: &fs::FileType) -> &'static str {
    use std::os::unix::fs::FileTypeExt as _;
    if ft.is_fifo() {
        return "fifo";
    }
    if ft.is_socket() {
        return "socket";
    }
    if ft.is_block_device() {
        return "block_device";
    }
    if ft.is_char_device() {
        return "char_device";
    }
    "unknown"
}

/// `path` relative to the checkout root, as a `/`-joined string — what an
/// ignore rule matches against.
fn relative(root: &Path, path: &Path) -> Result<String, ScanError> {
    path.strip_prefix(root)
        .map_err(|_| ScanError::Io("path escaped the checkout root".into()))?
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))
}

/// join the duckfs `prefix` with the on-disk path relative to the checkout root.
fn duckfs_join(root: &Path, path: &Path, prefix: &str) -> Result<String, ScanError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| ScanError::Io("path escaped the checkout root".into()))?;
    let mut joined = prefix.trim_end_matches('/').to_string();
    for comp in rel.components() {
        let seg = comp
            .as_os_str()
            .to_str()
            .ok_or_else(|| ScanError::NonUtf8(path.display().to_string()))?;
        joined.push('/');
        joined.push_str(seg);
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(text: &str) -> Ignore {
        Ignore::parse(text).expect("parse")
    }

    /// the shapes the file promises: a bare name at any depth, a `/`-anchored
    /// path, a directory-only rule, a glob that stops at `/`, `**` that does
    /// not, comments and blanks.
    #[test]
    fn the_pattern_shapes_match_where_they_say() {
        let ig = rules(
            "# a comment\n\n\
             target/\n\
             *.log\n\
             /build\n\
             src/**/generated\n",
        );
        assert!(
            ig.matches("target", "target", true),
            "a bare name, as a dir"
        );
        assert!(
            ig.matches("a/b/target", "target", true),
            "a bare name at any depth"
        );
        assert!(
            !ig.matches("target", "target", false),
            "a trailing slash means directories only"
        );
        assert!(ig.matches("a/b/x.log", "x.log", false), "a name glob");
        assert!(ig.matches("build", "build", true), "anchored at the root");
        assert!(
            !ig.matches("a/build", "build", true),
            "an anchored rule does not match deeper"
        );
        assert!(
            ig.matches("src/a/b/generated", "generated", true),
            "** spans components"
        );
    }

    /// a later `!` re-includes — the last matching rule decides.
    #[test]
    fn negation_re_includes_what_an_earlier_rule_excluded() {
        let ig = rules("*.log\n!keep.log\n");
        assert!(!ig.matches("keep.log", "keep.log", false), "re-included");
        assert!(ig.matches("drop.log", "drop.log", false));
    }

    /// what the walk's pruning means for a path the walk never reached: a file
    /// under an ignored directory is ignored too.
    #[test]
    fn a_path_under_an_ignored_directory_is_ignored() {
        let ig = rules("target/\n");
        assert!(ig.ignored_path("target/debug/app", false));
        assert!(!ig.ignored_path("src/target.rs", false));
    }

    /// a malformed pattern fails loudly, naming its line — never a silently
    /// dropped rule.
    #[test]
    fn a_malformed_pattern_names_its_line() {
        let Err(err) = Ignore::parse("ok\nsrc/**a/x\n") else {
            panic!("a `**` that is not a whole component is not a pattern");
        };
        let msg = err.to_string();
        assert!(msg.contains("line 2"), "names the line: {msg}");
    }
}
