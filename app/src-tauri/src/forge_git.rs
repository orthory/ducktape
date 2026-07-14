//! read-only local git view over the node's on-disk forge repository.
//!
//! Writes still go through the node's consensus `forge` module. These commands
//! only open the active workspace's local repo and project committed refs
//! (`refs/heads/*`, preferring `dev` and falling back to `main`) into
//! browser-friendly shapes. The one
//! exception is `forge_build_merge`, which builds a CLIENT-COMPUTED merge
//! commit in a throwaway repo — the node repo itself is never written.
//!
//! A DIRECT REMOTE CLIENT (no local workspace) has no node repo on this disk.
//! Every reader therefore takes an optional `remote` origin: when set, it reads
//! a local bare MIRROR of that node's smart-HTTP remote
//! (`<origin>/forge/<repo>`) instead, kept current by [`forge_sync_remote`].
//! The mirror lives under `<app-data>/forge-remote/<origin-key>/<repo>`, keyed
//! by origin so two networks' repos can never shadow each other.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use git2::{
    BranchType, Buf, Commit, Delta, DiffOptions, ErrorCode, FetchOptions, FetchPrune, ObjectType,
    Oid, Patch, Repository, Signature, Sort, Tree,
};
use serde::{Deserialize, Serialize};
use tauri::Manager as _;

const MAIN_REF: &str = "refs/heads/main";
const DEV_REF: &str = "refs/heads/dev";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    id: String,
    summary: String,
    message: String,
    parent_ids: Vec<String>,
    author: String,
    time: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntry {
    name: String,
    kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePage {
    text: String,
    offset: usize,
    next_offset: Option<usize>,
    total_bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    origin: String,
    content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    header: String,
    lines: Vec<DiffLine>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    path: String,
    status: String,
    hunks: Vec<DiffHunk>,
}

/// one materialized repo under a forge base — its ACTUAL on-disk directory name
/// (never a hardcoded label) and its integration head, or `None` if unborn.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoMeta {
    name: String,
    branch: String,
    head: Option<String>,
}

/// one local branch — the `refs/heads/<name>` SHORT name and its 40-hex head.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    name: String,
    head: String,
}

/// one changed file in a compare — a PR "files changed" row. `patch` is the
/// unified patch text for this file (empty for binary deltas).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareFile {
    path: String,
    status: String,
    additions: u32,
    deletions: u32,
    patch: String,
}

/// GitHub-style three-dot compare payload: the diff from `merge_base(base,
/// head)` to `head`, plus the commits on `head` not reachable from `base`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    merge_base: String,
    files: Vec<CompareFile>,
    total_additions: u32,
    total_deletions: u32,
    commits: Vec<CommitInfo>,
}

/// outcome of a client-computed merge: either the merge commit oid + a pack of
/// the NEW objects (hex-encoded — this crate carries no base64 dependency), or
/// the conflicting paths and NO merge.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBuildResult {
    merge_oid: Option<String>,
    pack_hex: Option<String>,
    conflicts: Vec<String>,
}

#[derive(Deserialize)]
struct Registry {
    active: Option<String>,
}

#[derive(Deserialize)]
struct NodeToml {
    storage_dir: Option<String>,
}

/// list the repos this node has materialized under its forge base(s), by their
/// real on-disk names — the desktop Forge view's repo list. Repo-name-agnostic:
/// whatever was pushed (`ducktape`, `default`, ...) shows up under its own name.
#[tauri::command]
pub fn forge_list_repos(app: crate::rt::AppHandle) -> Result<Vec<RepoMeta>, String> {
    list_forge_repos(&app)
}

#[tauri::command]
pub fn forge_head(
    app: crate::rt::AppHandle,
    repo: String,
    remote: Option<String>,
) -> Result<Option<String>, String> {
    let Some(git) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(None);
    };
    Ok(integration_oid(&git)?.map(|oid| oid.to_string()))
}

/// Every local branch (`refs/heads/*`) by SHORT name with its 40-hex head,
/// sorted by name — the PR pickers' branch list.
#[tauri::command]
pub fn forge_list_branches(
    app: crate::rt::AppHandle,
    repo: String,
    remote: Option<String>,
) -> Result<Vec<BranchInfo>, String> {
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(Vec::new());
    };
    let mut branches = Vec::new();
    for branch in repo.branches(Some(BranchType::Local)).map_err(err)? {
        let (branch, _) = branch.map_err(err)?;
        let Some(name) = branch.name().map_err(err)?.map(str::to_owned) else {
            continue;
        };
        // an unborn/symbolic branch has no direct target — nothing to browse.
        let Some(head) = branch.get().target() else {
            continue;
        };
        branches.push(BranchInfo {
            name,
            head: head.to_string(),
        });
    }
    branches.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(branches)
}

/// Read a commit log, newest first. `reference` picks the starting point — a
/// branch short name or 40-hex oid, `None` for the integration branch. `after`
/// is an exclusive commit cursor from the same walk. `limit` is optional: `None` walks the
/// whole reachable history, `Some(n)` caps to the newest `n`.
#[tauri::command]
pub fn forge_log(
    app: crate::rt::AppHandle,
    repo: String,
    limit: Option<usize>,
    reference: Option<String>,
    after: Option<String>,
    remote: Option<String>,
) -> Result<Vec<CommitInfo>, String> {
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(Vec::new());
    };
    let Some(head) = commit_at(&repo, reference.as_deref())? else {
        return Ok(Vec::new());
    };

    let mut walk = repo.revwalk().map_err(err)?;
    walk.push(head.id()).map_err(err)?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(err)?;

    let after_oid = after
        .as_deref()
        .map(str::trim)
        .filter(|cursor| !cursor.is_empty())
        .map(|cursor| {
            Oid::from_str(cursor).map_err(|e| format!("invalid commit cursor {cursor:?}: {e}"))
        })
        .transpose()?;
    let mut cursor_seen = after_oid.is_none();
    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid.map_err(err)?;
        if !cursor_seen {
            if after_oid.as_ref().is_some_and(|cursor| *cursor == oid) {
                cursor_seen = true;
            }
            continue;
        }
        let commit = repo.find_commit(oid).map_err(err)?;
        commits.push(commit_info(&commit));
        if let Some(limit) = limit
            && commits.len() >= limit
        {
            break;
        }
    }
    Ok(commits)
}

#[tauri::command]
pub fn forge_tree(
    app: crate::rt::AppHandle,
    repo: String,
    path: String,
    reference: Option<String>,
    remote: Option<String>,
) -> Result<Vec<TreeEntry>, String> {
    let path = clean_repo_path(&path, true)?;
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(Vec::new());
    };
    let Some(commit) = commit_at(&repo, reference.as_deref())? else {
        return Ok(Vec::new());
    };

    let root = commit.tree().map_err(err)?;
    let tree = match subtree(&repo, &root, &path)? {
        Some(tree) => tree,
        None => return Ok(Vec::new()),
    };

    let mut entries = Vec::new();
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let kind = match entry.kind() {
            Some(ObjectType::Tree) => "dir",
            _ => "file",
        };
        entries.push(TreeEntry {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    entries.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    Ok(entries)
}

#[tauri::command]
pub fn forge_read_file(
    app: crate::rt::AppHandle,
    repo: String,
    path: String,
    reference: Option<String>,
    remote: Option<String>,
) -> Result<Option<String>, String> {
    let path = clean_repo_path(&path, false)?;
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(None);
    };
    let Some(commit) = commit_at(&repo, reference.as_deref())? else {
        return Ok(None);
    };

    let tree = commit.tree().map_err(err)?;
    let entry = match tree.get_path(Path::new(&path)) {
        Ok(entry) => entry,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    if entry.kind() != Some(ObjectType::Blob) {
        return Ok(None);
    }
    let blob = repo.find_blob(entry.id()).map_err(err)?;
    let text = std::str::from_utf8(blob.content())
        .map_err(|_| format!("{path} is not utf-8 text"))?
        .to_string();
    Ok(Some(text))
}

#[tauri::command]
pub fn forge_read_file_page(
    app: crate::rt::AppHandle,
    repo: String,
    path: String,
    reference: Option<String>,
    offset: usize,
    limit: usize,
    remote: Option<String>,
) -> Result<Option<FilePage>, String> {
    let path = clean_repo_path(&path, false)?;
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(None);
    };
    let Some(commit) = commit_at(&repo, reference.as_deref())? else {
        return Ok(None);
    };

    let tree = commit.tree().map_err(err)?;
    let entry = match tree.get_path(Path::new(&path)) {
        Ok(entry) => entry,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    if entry.kind() != Some(ObjectType::Blob) {
        return Ok(None);
    }
    let blob = repo.find_blob(entry.id()).map_err(err)?;
    utf8_text_page(blob.content(), offset, limit)
        .map(Some)
        .map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
pub fn forge_diff(
    app: crate::rt::AppHandle,
    repo: String,
    from: Option<String>,
    to: Option<String>,
    remote: Option<String>,
) -> Result<Vec<FileDiff>, String> {
    let Some(repo) = open_named_repo(&app, &repo, remote.as_deref())? else {
        return Ok(Vec::new());
    };
    let from_tree = tree_for_spec(&repo, from.as_deref(), false)?;
    let to_tree = tree_for_spec(&repo, to.as_deref(), true)?;
    if to_tree.is_none() {
        return Ok(Vec::new());
    }

    let mut opts = DiffOptions::new();
    opts.context_lines(3).interhunk_lines(0);
    let diff = repo
        .diff_tree_to_tree(from_tree.as_ref(), to_tree.as_ref(), Some(&mut opts))
        .map_err(err)?;

    let files = RefCell::new(Vec::<FileDiff>::new());
    let current_file = Cell::new(None::<usize>);
    let current_hunk = Cell::new(None::<usize>);

    let mut file_cb = |delta: git2::DiffDelta<'_>, _progress: f32| {
        let path = delta_path(&delta);
        let status = delta_status(delta.status()).to_string();
        let mut files = files.borrow_mut();
        files.push(FileDiff {
            path,
            status,
            hunks: Vec::new(),
        });
        current_file.set(Some(files.len() - 1));
        current_hunk.set(None);
        true
    };

    let mut hunk_cb = |_delta: git2::DiffDelta<'_>, hunk: git2::DiffHunk<'_>| {
        let Some(file_index) = current_file.get() else {
            return true;
        };
        let header = text_lossy(hunk.header()).trim_end().to_string();
        let mut files = files.borrow_mut();
        files[file_index].hunks.push(DiffHunk {
            header,
            lines: Vec::new(),
        });
        current_hunk.set(Some(files[file_index].hunks.len() - 1));
        true
    };

    let mut line_cb = |_delta: git2::DiffDelta<'_>,
                       hunk: Option<git2::DiffHunk<'_>>,
                       line: git2::DiffLine<'_>| {
        let Some(file_index) = current_file.get() else {
            return true;
        };
        if current_hunk.get().is_none() {
            let header = hunk
                .map(|h| text_lossy(h.header()).trim_end().to_string())
                .unwrap_or_default();
            let mut files = files.borrow_mut();
            files[file_index].hunks.push(DiffHunk {
                header,
                lines: Vec::new(),
            });
            current_hunk.set(Some(files[file_index].hunks.len() - 1));
        }
        let Some(hunk_index) = current_hunk.get() else {
            return true;
        };
        let mut files = files.borrow_mut();
        files[file_index].hunks[hunk_index].lines.push(DiffLine {
            origin: line.origin().to_string(),
            content: text_lossy(line.content())
                .trim_end_matches('\n')
                .to_string(),
        });
        true
    };

    diff.foreach(&mut file_cb, None, Some(&mut hunk_cb), Some(&mut line_cb))
        .map_err(err)?;

    Ok(files.into_inner())
}

/// GitHub-style compare between `base` and `head` (branch short names or 40-hex
/// oids): the diff runs from `merge_base(base, head)` to `head` (three-dot),
/// and `commits` are the commits on `head` NOT reachable from `base` — the PR
/// "files changed" payload.
#[tauri::command]
pub fn forge_compare(
    app: crate::rt::AppHandle,
    repo: String,
    base: String,
    head: String,
    remote: Option<String>,
) -> Result<CompareResult, String> {
    let repo = require_named_repo(&app, &repo, remote.as_deref())?;
    let base_oid = require_ref_spec(&repo, &base)?;
    let head_oid = require_ref_spec(&repo, &head)?;
    let merge_base = repo
        .merge_base(base_oid, head_oid)
        .map_err(|e| format!("no merge base between {base:?} and {head:?}: {e}"))?;

    let base_tree = repo
        .find_commit(merge_base)
        .and_then(|commit| commit.tree())
        .map_err(err)?;
    let head_tree = repo
        .find_commit(head_oid)
        .and_then(|commit| commit.tree())
        .map_err(err)?;

    let mut opts = DiffOptions::new();
    opts.context_lines(3).interhunk_lines(0);
    let mut diff = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut opts))
        .map_err(err)?;
    // surface renames the way GitHub's compare does, not as add+delete pairs.
    diff.find_similar(None).map_err(err)?;

    let mut files = Vec::new();
    let mut total_additions = 0u32;
    let mut total_deletions = 0u32;
    for (index, delta) in diff.deltas().enumerate() {
        let path = delta_path(&delta);
        let status = delta_status(delta.status()).to_string();
        let (additions, deletions, patch) = match Patch::from_diff(&diff, index).map_err(err)? {
            Some(mut patch) => {
                let (_context, additions, deletions) = patch.line_stats().map_err(err)?;
                let text = patch.to_buf().map_err(err)?;
                (additions as u32, deletions as u32, text_lossy(&text))
            }
            // binary/unrepresentable deltas still list, with no text patch.
            None => (0, 0, String::new()),
        };
        total_additions += additions;
        total_deletions += deletions;
        files.push(CompareFile {
            path,
            status,
            additions,
            deletions,
            patch,
        });
    }

    let mut walk = repo.revwalk().map_err(err)?;
    walk.push(head_oid).map_err(err)?;
    walk.hide(base_oid).map_err(err)?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(err)?;
    let mut commits = Vec::new();
    for oid in walk {
        let commit = repo.find_commit(oid.map_err(err)?).map_err(err)?;
        commits.push(commit_info(&commit));
    }

    Ok(CompareResult {
        merge_base: merge_base.to_string(),
        files,
        total_additions,
        total_deletions,
        commits,
    })
}

/// Build the CLIENT-COMPUTED merge commit for `MergePr`: merge `theirs` (the
/// source head) into `ours` (the target head) in a TEMPORARY bare repo wired to
/// the node repo's objects through a disk alternate, and return the new commit
/// oid plus a minimal pack of the objects the node does not have yet. The node
/// repo is opened read-only and never written — consensus only CASes the oid,
/// because client-computed merges are ordinary git commits. Conflicts return
/// the conflicting paths and NO merge.
#[tauri::command]
pub fn forge_build_merge(
    app: crate::rt::AppHandle,
    repo: String,
    ours: String,
    theirs: String,
    message: String,
    remote: Option<String>,
) -> Result<MergeBuildResult, String> {
    let node_repo = require_named_repo(&app, &repo, remote.as_deref())?;
    let ours_oid = require_ref_spec(&node_repo, &ours)?;
    let theirs_oid = require_ref_spec(&node_repo, &theirs)?;

    // throwaway bare repo; its odb reads the node repo's objects through a
    // disk alternate, so parents resolve without copying anything.
    let scratch = ScratchDir::create()?;
    let temp = Repository::init_bare(scratch.path()).map_err(err)?;
    let objects = node_repo.path().join("objects");
    let objects = objects
        .to_str()
        .ok_or_else(|| format!("non-utf8 objects path {}", objects.display()))?;
    temp.odb()
        .map_err(err)?
        .add_disk_alternate(objects)
        .map_err(err)?;

    let ours_commit = temp.find_commit(ours_oid).map_err(err)?;
    let theirs_commit = temp.find_commit(theirs_oid).map_err(err)?;
    let mut index = temp
        .merge_commits(&ours_commit, &theirs_commit, None)
        .map_err(err)?;

    if index.has_conflicts() {
        let mut conflicts = Vec::new();
        for conflict in index.conflicts().map_err(err)? {
            let conflict = conflict.map_err(err)?;
            let Some(entry) = conflict.our.or(conflict.their).or(conflict.ancestor) else {
                continue;
            };
            conflicts.push(text_lossy(&entry.path));
        }
        conflicts.sort();
        conflicts.dedup();
        return Ok(MergeBuildResult {
            merge_oid: None,
            pack_hex: None,
            conflicts,
        });
    }

    let tree_oid = index.write_tree_to(&temp).map_err(err)?;
    let tree = temp.find_tree(tree_oid).map_err(err)?;
    let signature = Signature::now("ducktape", "ducktape@localhost").map_err(err)?;
    let merge_oid = temp
        .commit(
            None,
            &signature,
            &signature,
            &message,
            &tree,
            &[&ours_commit, &theirs_commit],
        )
        .map_err(err)?;

    // MINIMAL pack: only objects reachable from the merge but from NEITHER
    // parent — the hidden commits mark their trees uninteresting.
    let mut builder = temp.packbuilder().map_err(err)?;
    let mut walk = temp.revwalk().map_err(err)?;
    walk.push(merge_oid).map_err(err)?;
    walk.hide(ours_oid).map_err(err)?;
    walk.hide(theirs_oid).map_err(err)?;
    builder.insert_walk(&mut walk).map_err(err)?;
    let mut buf = Buf::new();
    builder.write_buf(&mut buf).map_err(err)?;

    Ok(MergeBuildResult {
        merge_oid: Some(merge_oid.to_string()),
        pack_hex: Some(hex_encode(&buf)),
        conflicts: Vec::new(),
    })
}

/// Process-unique throwaway directory under the OS temp dir, removed
/// (best-effort) on drop. `tempfile` is not a dependency of this crate and a
/// one-shot merge scratch does not justify adding one.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn create() -> Result<Self, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ducktape-forge-merge-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)
            .map_err(|e| format!("create merge scratch dir {}: {e}", dir.display()))?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Validate a caller-supplied forge repo NAME: a single normal path segment (no
/// `/` or `\`, not `.`/`..`, no absolute root), so `base.join(name)` can never
/// escape the forge base. This is the `repo` arg's boundary — the analogue of
/// [`clean_repo_path`] for the `path` arg — because `repo` arrives straight from
/// a renderer `invoke` and `PathBuf::join` would otherwise let an absolute or
/// `..`-laden value open an arbitrary on-disk git repo. forge only ever creates
/// `[a-z0-9._-]` slugs (its `norm_repo`), so every real repo name passes.
fn clean_repo_name(name: &str) -> Result<&str, String> {
    if name.is_empty() {
        return Err("forge repo name is required".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("forge repo name must be a single path segment".into());
    }
    match Path::new(name).components().next() {
        Some(Component::Normal(_)) if name != "." && name != ".." => Ok(name),
        _ => Err(format!("invalid forge repo name {name:?}")),
    }
}

/// Open a forge repo BY NAME from the first base that has materialized it.
///
/// forge namespaces repos at `<base>/<name>` (a `Push`/`Commit` creates the dir
/// lazily), so the on-disk repo lives ONE LEVEL DOWN. The caller passes the repo
/// name it wants to read (from [`list_forge_repos`]/the UI's selection), so
/// nothing here hardcodes or guesses a repo name; the name is validated as a
/// single path segment first so it cannot escape the base.
///
/// `remote` switches the source entirely: a remote client reads its local
/// MIRROR of that origin's repo (see [`forge_sync_remote`]) and never touches
/// the workspace bases — the two planes must not shadow each other. A mirror
/// not synced yet reads as absent, the same empty shape an unborn local repo
/// projects.
fn open_named_repo(
    app: &crate::rt::AppHandle,
    repo: &str,
    remote: Option<&str>,
) -> Result<Option<Repository>, String> {
    let repo = clean_repo_name(repo)?;
    if let Some(origin) = remote {
        let dir = remote_repo_dir(app, origin, repo)?;
        return match Repository::open(&dir) {
            Ok(git) => Ok(Some(git)),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(format!("open forge mirror {}: {e}", dir.display())),
        };
    }
    for base in forge_base_dirs(app)? {
        let dir = base.join(repo);
        if dir.join(".git").exists() {
            return Repository::open(&dir)
                .map(Some)
                .map_err(|e| format!("open forge repo {}: {e}", dir.display()));
        }
    }
    Ok(None)
}

/// Like [`open_named_repo`] but the repo MUST be materialized — compare/merge
/// have no meaningful empty shape.
fn require_named_repo(
    app: &crate::rt::AppHandle,
    repo: &str,
    remote: Option<&str>,
) -> Result<Repository, String> {
    open_named_repo(app, repo, remote)?
        .ok_or_else(|| format!("forge repo {repo:?} is not materialized on this node"))
}

/// Where the local mirror of a REMOTE origin's repo lives:
/// `<app-data>/forge-remote/<origin-key>/<repo>`. The key hex-encodes the
/// normalized origin into one lossless path segment so distinct nodes can
/// never share a mirror.
fn remote_origin_key(origin: &str) -> Result<String, String> {
    let origin = origin.trim_end_matches('/');
    if origin.is_empty() {
        return Err("remote origin url is required".into());
    }
    Ok(hex_encode(origin.as_bytes()))
}

fn remote_repo_dir(
    app: &crate::rt::AppHandle,
    origin: &str,
    repo: &str,
) -> Result<PathBuf, String> {
    let key = remote_origin_key(origin)?;
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| format!("no app-data dir: {err}"))?
        .join("forge-remote")
        .join(key)
        .join(repo))
}

/// Fetch `<origin>/forge/<repo>` (the node's git smart-HTTP surface) into the
/// local bare mirror at `dir`, creating it on first sync. All branch heads
/// come over (`+refs/heads/*:refs/heads/*`, pruned), so the browse readers and
/// the client-computed merge see exactly what the node's repo holds. Returns
/// the mirror's integration head — the same shape [`list_forge_repos`] lists.
fn sync_remote_mirror(dir: &Path, origin: &str, repo: &str) -> Result<RepoMeta, String> {
    let git = match Repository::open(dir) {
        Ok(git) => git,
        Err(e) if e.code() == ErrorCode::NotFound => {
            fs::create_dir_all(dir)
                .map_err(|e| format!("create forge mirror dir {}: {e}", dir.display()))?;
            Repository::init_bare(dir).map_err(err)?
        }
        Err(e) => return Err(format!("open forge mirror {}: {e}", dir.display())),
    };
    let url = format!("{}/forge/{}", origin.trim_end_matches('/'), repo);
    let mut origin_remote = git.remote_anonymous(&url).map_err(err)?;
    let mut opts = FetchOptions::new();
    opts.prune(FetchPrune::On);
    origin_remote
        .fetch(&["+refs/heads/*:refs/heads/*"], Some(&mut opts), None)
        .map_err(|e| format!("fetch {url}: {e}"))?;
    drop(origin_remote);
    let (branch, head) = integration_ref(&git)?
        .map(|(branch, oid)| (branch.to_owned(), Some(oid.to_string())))
        .unwrap_or_else(|| ("dev".into(), None));
    Ok(RepoMeta {
        name: repo.to_owned(),
        branch,
        head,
    })
}

/// Bring a remote client's local mirror of `<origin>/forge/<repo>` up to date
/// and report its integration head. Network-bound, so it runs off the IPC
/// thread; the browse readers then serve from the mirror without touching the
/// network again.
#[tauri::command]
pub async fn forge_sync_remote(
    app: crate::rt::AppHandle,
    origin: String,
    repo: String,
) -> Result<RepoMeta, String> {
    let name = clean_repo_name(&repo)?.to_owned();
    let dir = remote_repo_dir(&app, &origin, &name)?;
    tauri::async_runtime::spawn_blocking(move || sync_remote_mirror(&dir, &origin, &name))
        .await
        .map_err(|e| format!("forge remote sync task: {e}"))?
}

/// Enumerate every repo materialized under the forge base(s), by its REAL on-disk
/// directory name, with its integration head (or `None` if unborn).
/// Sorted by name and de-duplicated (the first base wins) so the list is
/// deterministic. A missing base is simply empty; a single flaky dir entry is
/// skipped, not fatal.
fn list_forge_repos(app: &crate::rt::AppHandle) -> Result<Vec<RepoMeta>, String> {
    let mut repos: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for base in forge_base_dirs(app)? {
        let read = match fs::read_dir(&base) {
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("scan forge base {}: {err}", base.display())),
        };
        for entry in read.flatten() {
            let dir = entry.path();
            if !dir.join(".git").exists() {
                continue;
            }
            let Some(name) = dir.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if repos.contains_key(&name) {
                continue; // first base wins
            }
            // a corrupt/unreadable repo lists with an unborn head rather than
            // failing the whole enumeration.
            let (branch, head) = Repository::open(&dir)
                .ok()
                .and_then(|repo| integration_ref(&repo).ok().flatten())
                .map(|(branch, oid)| (branch.to_owned(), Some(oid.to_string())))
                .unwrap_or_else(|| ("dev".into(), None));
            repos.insert(name, (branch, head));
        }
    }
    Ok(repos
        .into_iter()
        .map(|(name, (branch, head))| RepoMeta { name, branch, head })
        .collect())
}

/// the forge base container dir(s) this node materializes repos under, in
/// priority order (active workspace storage, then the app-data node dir). each
/// holds repos at `<base>/<name>`.
fn forge_base_dirs(app: &crate::rt::AppHandle) -> Result<Vec<PathBuf>, String> {
    let mut storages = Vec::new();
    if let Some(active) = active_workspace_storage(app)? {
        storages.push(active);
    }
    let node_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("no app-data dir: {err}"))?
        .join("node");
    if let Some(storage) = storage_from_node_toml(&node_dir.join("node.toml"))? {
        storages.push(storage);
    } else {
        storages.push(node_dir.join("storage"));
    }

    let mut candidates = Vec::new();
    for storage in storages {
        candidates.push(storage.join("forge-repo"));
        candidates.push(storage.join("forge-git"));
    }
    Ok(candidates)
}

fn active_workspace_storage(app: &crate::rt::AppHandle) -> Result<Option<PathBuf>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|err| format!("no home dir: {err}"))?;
    let root = home.join(".ducktape");
    let registry_path = root.join("registry.json");
    let text = match fs::read_to_string(&registry_path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read {registry_path:?}: {err}")),
    };
    let registry: Registry =
        serde_json::from_str(&text).map_err(|err| format!("parse {registry_path:?}: {err}"))?;
    let Some(active) = registry.active else {
        return Ok(None);
    };
    storage_from_node_toml(&root.join("workspaces").join(active).join("node.toml"))
}

fn storage_from_node_toml(path: &Path) -> Result<Option<PathBuf>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read {path:?}: {err}")),
    };
    let raw: NodeToml = toml::from_str(&text).map_err(|err| format!("parse {path:?}: {err}"))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(Some(
        base.join(raw.storage_dir.as_deref().unwrap_or("storage")),
    ))
}

fn main_oid(repo: &Repository) -> Result<Option<Oid>, String> {
    match repo.refname_to_id(MAIN_REF) {
        Ok(oid) => Ok(Some(oid)),
        Err(e) if matches!(e.code(), ErrorCode::NotFound | ErrorCode::UnbornBranch) => Ok(None),
        Err(e) => Err(err(e)),
    }
}

fn integration_ref(repo: &Repository) -> Result<Option<(&'static str, Oid)>, String> {
    match repo.refname_to_id(DEV_REF) {
        Ok(oid) => Ok(Some(("dev", oid))),
        Err(e) if matches!(e.code(), ErrorCode::NotFound | ErrorCode::UnbornBranch) => {
            main_oid(repo).map(|oid| oid.map(|oid| ("main", oid)))
        }
        Err(e) => Err(err(e)),
    }
}

fn integration_oid(repo: &Repository) -> Result<Option<Oid>, String> {
    Ok(integration_ref(repo)?.map(|(_, oid)| oid))
}

/// Resolve a caller-supplied `reference` to an oid: `None`/empty -> committed
/// dev with a legacy-main fallback; `"main"` remains the release ref; a
/// 40-hex string -> that commit oid verbatim, anything else
/// -> `refs/heads/<reference>`. An unknown branch resolves to `None` rather
/// than erroring, so the browse commands degrade to their existing empty
/// shapes (the same way an unborn main does).
fn resolve_ref_spec(repo: &Repository, reference: Option<&str>) -> Result<Option<Oid>, String> {
    let spec = reference.unwrap_or("").trim();
    if spec.is_empty() {
        return integration_oid(repo);
    }
    if spec == "main" || spec == MAIN_REF {
        return main_oid(repo);
    }
    if spec.len() == 40 && spec.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Oid::from_str(spec).map(Some).map_err(err);
    }
    match repo.refname_to_id(&format!("refs/heads/{spec}")) {
        Ok(oid) => Ok(Some(oid)),
        Err(e)
            if matches!(
                e.code(),
                ErrorCode::NotFound | ErrorCode::UnbornBranch | ErrorCode::InvalidSpec
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(err(e)),
    }
}

/// Like [`resolve_ref_spec`] but the reference MUST resolve — compare/merge
/// have no meaningful empty shape.
fn require_ref_spec(repo: &Repository, spec: &str) -> Result<Oid, String> {
    resolve_ref_spec(repo, Some(spec))?
        .ok_or_else(|| format!("cannot resolve {spec:?} to a commit"))
}

/// The commit `reference` points at (see [`resolve_ref_spec`]); a resolvable
/// oid that is not a commit in this repo reads as absent, not as an error.
fn commit_at<'repo>(
    repo: &'repo Repository,
    reference: Option<&str>,
) -> Result<Option<Commit<'repo>>, String> {
    let Some(oid) = resolve_ref_spec(repo, reference)? else {
        return Ok(None);
    };
    match repo.find_commit(oid) {
        Ok(commit) => Ok(Some(commit)),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}

fn commit_info(commit: &Commit<'_>) -> CommitInfo {
    CommitInfo {
        id: commit.id().to_string(),
        summary: commit.summary().unwrap_or("(no summary)").to_string(),
        message: commit.message().unwrap_or("").to_string(),
        parent_ids: commit.parent_ids().map(|oid| oid.to_string()).collect(),
        author: commit.author().name().unwrap_or("ducktape").to_string(),
        time: commit.time().seconds(),
    }
}

fn subtree<'repo>(
    repo: &'repo Repository,
    root: &Tree<'repo>,
    path: &str,
) -> Result<Option<Tree<'repo>>, String> {
    if path.is_empty() {
        return Ok(Some(root.clone()));
    }
    let entry = match root.get_path(Path::new(path)) {
        Ok(entry) => entry,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    if entry.kind() != Some(ObjectType::Tree) {
        return Ok(None);
    }
    repo.find_tree(entry.id()).map(Some).map_err(err)
}

fn tree_for_spec<'repo>(
    repo: &'repo Repository,
    spec: Option<&str>,
    empty_means_default: bool,
) -> Result<Option<Tree<'repo>>, String> {
    let spec = spec.unwrap_or("").trim();
    let oid = if spec.is_empty() {
        if empty_means_default {
            integration_oid(repo)?
        } else {
            None
        }
    } else {
        resolve_commitish(repo, spec)?
    };
    oid.map(|oid| {
        repo.find_commit(oid)
            .and_then(|commit| commit.tree())
            .map_err(err)
    })
    .transpose()
}

fn resolve_commitish(repo: &Repository, spec: &str) -> Result<Option<Oid>, String> {
    if spec == "main" || spec == MAIN_REF {
        return main_oid(repo);
    }
    if let Ok(oid) = Oid::from_str(spec) {
        return Ok(Some(oid));
    }
    match repo.revparse_single(spec) {
        Ok(object) => object
            .peel_to_commit()
            .map(|commit| Some(commit.id()))
            .map_err(err),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}

fn clean_repo_path(raw: &str, allow_empty: bool) -> Result<String, String> {
    let path = raw.trim().trim_matches('/');
    if path.is_empty() {
        return if allow_empty {
            Ok(String::new())
        } else {
            Err("path is required".into())
        };
    }
    if path.contains('\\') {
        return Err("backslashes are not valid forge paths".into());
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("forge paths must stay inside the repository".into());
            }
        }
    }
    Ok(path.to_string())
}

fn utf8_text_page(content: &[u8], offset: usize, limit: usize) -> Result<FilePage, String> {
    if limit == 0 {
        return Err("file page limit must be greater than 0".into());
    }
    let text = std::str::from_utf8(content).map_err(|_| "file is not utf-8 text".to_string())?;
    let total_bytes = text.len();
    if offset > total_bytes {
        return Err(format!(
            "file page offset {offset} exceeds file size {total_bytes}"
        ));
    }
    if !text.is_char_boundary(offset) {
        return Err(format!("file page offset {offset} is not a utf-8 boundary"));
    }

    let mut end = offset.saturating_add(limit).min(total_bytes);
    while end > offset && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < total_bytes {
        end = text[offset..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| offset + index)
            .unwrap_or(total_bytes);
    }

    Ok(FilePage {
        text: text[offset..end].to_string(),
        offset,
        next_offset: (end < total_bytes).then_some(end),
        total_bytes,
    })
}

fn delta_path(delta: &git2::DiffDelta<'_>) -> String {
    let file = match delta.status() {
        Delta::Deleted => delta.old_file(),
        _ => delta.new_file(),
    };
    file.path()
        .or_else(|| delta.old_file().path())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".into())
}

fn delta_status(delta: Delta) -> &'static str {
    match delta {
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Modified => "modified",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Typechange => "typechange",
        Delta::Unmodified => "unmodified",
        Delta::Ignored => "ignored",
        Delta::Untracked => "untracked",
        Delta::Unreadable => "unreadable",
        Delta::Conflicted => "conflicted",
    }
}

fn text_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Lowercase hex — the pack transport encoding (this crate has no base64 dep,
/// and the TS side just forwards the bytes to the node).
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0xf)] as char);
    }
    out
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        DEV_REF, MAIN_REF, clean_repo_name, commit_at, integration_oid, integration_ref,
        remote_origin_key, resolve_ref_spec, sync_remote_mirror, utf8_text_page,
    };
    use git2::{Repository, Signature};

    #[test]
    fn accepts_real_forge_repo_slugs() {
        for name in ["ducktape", "default", "my-repo", "a.b_c-1", "x"] {
            assert_eq!(
                clean_repo_name(name).unwrap(),
                name,
                "{name} should be valid"
            );
        }
    }

    #[test]
    fn rejects_traversal_and_absolute_and_separators() {
        // these must NOT be joined onto the forge base — they would escape it.
        for name in [
            "",
            ".",
            "..",
            "../secret",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "/Users/eddy/dev/private/ducktape/ducktape",
            "../../../etc",
        ] {
            assert!(
                clean_repo_name(name).is_err(),
                "{name:?} must be rejected as a repo name"
            );
        }
    }

    #[test]
    fn remote_origin_keys_are_lossless_and_ignore_trailing_slashes() {
        assert_ne!(
            remote_origin_key("http://host:26800").unwrap(),
            remote_origin_key("http://host/26800").unwrap(),
        );
        assert_eq!(
            remote_origin_key("http://host:26800").unwrap(),
            remote_origin_key("http://host:26800/").unwrap(),
        );
    }

    #[test]
    fn default_reads_prefer_dev_and_keep_a_legacy_main_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let signature = Signature::now("test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let main = repo
            .commit(Some(MAIN_REF), &signature, &signature, "main", &tree, &[])
            .unwrap();
        let main_commit = repo.find_commit(main).unwrap();
        let dev = repo
            .commit(
                Some(DEV_REF),
                &signature,
                &signature,
                "dev",
                &tree,
                &[&main_commit],
            )
            .unwrap();

        assert_eq!(integration_oid(&repo).unwrap(), Some(dev));
        assert_eq!(integration_ref(&repo).unwrap(), Some(("dev", dev)));
        assert_eq!(resolve_ref_spec(&repo, None).unwrap(), Some(dev));
        assert_eq!(resolve_ref_spec(&repo, Some("main")).unwrap(), Some(main));

        repo.find_reference(DEV_REF).unwrap().delete().unwrap();
        assert_eq!(integration_oid(&repo).unwrap(), Some(main));
        assert_eq!(integration_ref(&repo).unwrap(), Some(("main", main)));
    }

    #[test]
    fn remote_mirror_sync_tracks_the_origin() {
        // origin laid out the way a node serves it: <origin>/forge/<repo>.
        // file:// exercises the same fetch path the smart-HTTP origin takes.
        let dir = tempfile::tempdir().unwrap();
        let origin_dir = dir.path().join("forge").join("demo");
        std::fs::create_dir_all(&origin_dir).unwrap();
        let origin = Repository::init(&origin_dir).unwrap();
        let signature = Signature::now("test", "test@example.com").unwrap();
        let tree_id = origin.index().unwrap().write_tree().unwrap();
        let tree = origin.find_tree(tree_id).unwrap();
        let first = origin
            .commit(Some(DEV_REF), &signature, &signature, "one", &tree, &[])
            .unwrap();

        let origin_url = format!("file://{}", dir.path().display());
        let mirror_dir = dir.path().join("mirror");

        // first sync creates the bare mirror and lands the dev head.
        let meta = sync_remote_mirror(&mirror_dir, &origin_url, "demo").unwrap();
        assert_eq!(meta.name, "demo");
        assert_eq!(meta.branch, "dev");
        assert_eq!(meta.head, Some(first.to_string()));

        // origin advances; a re-sync follows it and the mirror serves reads.
        let first_commit = origin.find_commit(first).unwrap();
        let second = origin
            .commit(
                Some(DEV_REF),
                &signature,
                &signature,
                "two",
                &tree,
                &[&first_commit],
            )
            .unwrap();
        let meta = sync_remote_mirror(&mirror_dir, &origin_url, "demo").unwrap();
        assert_eq!(meta.head, Some(second.to_string()));
        let mirror = Repository::open(&mirror_dir).unwrap();
        assert_eq!(commit_at(&mirror, None).unwrap().unwrap().id(), second);

        // a branch deleted at the origin is pruned from the mirror.
        origin
            .commit(
                Some("refs/heads/feature"),
                &signature,
                &signature,
                "wip",
                &tree,
                &[&first_commit],
            )
            .unwrap();
        sync_remote_mirror(&mirror_dir, &origin_url, "demo").unwrap();
        assert!(mirror.refname_to_id("refs/heads/feature").is_ok());
        origin
            .find_reference("refs/heads/feature")
            .unwrap()
            .delete()
            .unwrap();
        sync_remote_mirror(&mirror_dir, &origin_url, "demo").unwrap();
        assert!(mirror.refname_to_id("refs/heads/feature").is_err());
    }

    #[test]
    fn text_page_reports_next_offset() {
        let page = utf8_text_page("hello world".as_bytes(), 0, 5).unwrap();
        assert_eq!(page.text, "hello");
        assert_eq!(page.offset, 0);
        assert_eq!(page.next_offset, Some(5));
        assert_eq!(page.total_bytes, 11);
    }

    #[test]
    fn text_page_ends_on_utf8_boundary() {
        let text = "a🙂b";
        let first = utf8_text_page(text.as_bytes(), 0, 2).unwrap();
        assert_eq!(first.text, "a");
        assert_eq!(first.next_offset, Some(1));

        let second = utf8_text_page(text.as_bytes(), first.next_offset.unwrap(), 4).unwrap();
        assert_eq!(second.text, "🙂");
        assert_eq!(second.next_offset, Some(5));
    }
}
