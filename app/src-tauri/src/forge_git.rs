//! read-only local git view over the node's on-disk forge repository.
//!
//! Writes still go through the node's consensus `forge` module. These commands
//! only open the active workspace's local repo and project committed
//! `refs/heads/main` into browser-friendly shapes.

use std::cell::Cell;
use std::cell::RefCell;
use std::fs;
use std::path::{Component, Path, PathBuf};

use git2::{Commit, Delta, DiffOptions, ErrorCode, ObjectType, Oid, Repository, Sort, Tree};
use serde::{Deserialize, Serialize};
use tauri::Manager as _;

const MAIN_REF: &str = "refs/heads/main";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    id: String,
    summary: String,
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

#[derive(Deserialize)]
struct Registry {
    active: Option<String>,
}

#[derive(Deserialize)]
struct NodeToml {
    storage_dir: Option<String>,
}

#[tauri::command]
pub fn forge_head(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let Some(repo) = open_forge_repo(&app)? else {
        return Ok(None);
    };
    Ok(main_oid(&repo)?.map(|oid| oid.to_string()))
}

#[tauri::command]
pub fn forge_log(app: tauri::AppHandle, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let Some(repo) = open_forge_repo(&app)? else {
        return Ok(Vec::new());
    };
    let Some(head) = main_oid(&repo)? else {
        return Ok(Vec::new());
    };

    let mut walk = repo.revwalk().map_err(err)?;
    walk.push(head).map_err(err)?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(err)?;

    let limit = limit.clamp(1, 200);
    let mut commits = Vec::new();
    for oid in walk.take(limit) {
        let commit = repo.find_commit(oid.map_err(err)?).map_err(err)?;
        commits.push(commit_info(&commit));
    }
    Ok(commits)
}

#[tauri::command]
pub fn forge_tree(app: tauri::AppHandle, path: String) -> Result<Vec<TreeEntry>, String> {
    let path = clean_repo_path(&path, true)?;
    let Some(repo) = open_forge_repo(&app)? else {
        return Ok(Vec::new());
    };
    let Some(commit) = main_commit(&repo)? else {
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
pub fn forge_read_file(app: tauri::AppHandle, path: String) -> Result<Option<String>, String> {
    let path = clean_repo_path(&path, false)?;
    let Some(repo) = open_forge_repo(&app)? else {
        return Ok(None);
    };
    let Some(commit) = main_commit(&repo)? else {
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
pub fn forge_diff(
    app: tauri::AppHandle,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<FileDiff>, String> {
    let Some(repo) = open_forge_repo(&app)? else {
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

fn open_forge_repo(app: &tauri::AppHandle) -> Result<Option<Repository>, String> {
    for base in repo_candidates(app)? {
        if let Some(repo) = open_repo_in_base(&base)? {
            return Ok(Some(repo));
        }
    }
    Ok(None)
}

/// Open the git repo the forge module materialized under `base`.
///
/// forge namespaces repos at `<base>/<name>` (a `Push`/`Commit` creates the dir
/// lazily), so the on-disk repo lives ONE LEVEL DOWN — scanning the base is what
/// makes the desktop Forge view see a pushed repo at all. A repo AT `base` is
/// still accepted for the legacy single-repo layout. Repo-name-agnostic: opens
/// the first repo dir in sorted order (dev dogfooding materializes exactly one),
/// so nothing here hardcodes the `ducktape`/`default` name.
fn open_repo_in_base(base: &Path) -> Result<Option<Repository>, String> {
    // legacy: the base dir is itself a repo.
    if base.join(".git").exists() {
        return Repository::open(base)
            .map(Some)
            .map_err(|e| format!("open forge repo {}: {e}", base.display()));
    }
    // multi-repo: `<base>/<name>` per repo. pick the first git repo, sorted so
    // the choice is deterministic across reads.
    let read = match fs::read_dir(base) {
        Ok(read) => read,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("scan forge base {}: {err}", base.display())),
    };
    let mut repo_dirs: Vec<PathBuf> = Vec::new();
    for entry in read {
        let entry = entry.map_err(|e| format!("scan forge base {}: {e}", base.display()))?;
        let path = entry.path();
        if path.join(".git").exists() {
            repo_dirs.push(path);
        }
    }
    repo_dirs.sort();
    match repo_dirs.into_iter().next() {
        Some(dir) => Repository::open(&dir)
            .map(Some)
            .map_err(|e| format!("open forge repo {}: {e}", dir.display())),
        None => Ok(None),
    }
}

fn repo_candidates(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
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

fn active_workspace_storage(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
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

fn main_commit(repo: &Repository) -> Result<Option<Commit<'_>>, String> {
    main_oid(repo)?
        .map(|oid| repo.find_commit(oid).map_err(err))
        .transpose()
}

fn commit_info(commit: &Commit<'_>) -> CommitInfo {
    CommitInfo {
        id: commit.id().to_string(),
        summary: commit.summary().unwrap_or("(no summary)").to_string(),
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
    empty_means_main: bool,
) -> Result<Option<Tree<'repo>>, String> {
    let spec = spec.unwrap_or("").trim();
    let oid = if spec.is_empty() {
        if empty_means_main {
            main_oid(repo)?
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

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
