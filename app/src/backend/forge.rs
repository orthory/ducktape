use super::*;
use ::forge;

/// One forge repo row: the module's committed head, plus the card facts
/// derived from the local mirror at that head.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ForgeRepo {
    pub name: String,
    pub head: String,
    /// The README's opening prose. Empty when the repo has none — the card
    /// keeps its min-height rather than inventing a description.
    pub about: String,
    /// The extension that owns the most files at the head revision.
    pub language: String,
    /// The head commit's committer time in UNIX SECONDS — a real wall clock,
    /// because a forge commit is stamped by a git client, not by consensus.
    /// Render it with `relative_time`, NOT with `height_label_short`. 0 when
    /// the repo has no born head.
    pub updated_at: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeData {
    pub generation: i64,
    pub repos: Vec<ForgeRepo>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeRepoData {
    pub generation: i64,
    pub repo: String,
    pub branches: Vec<String>,
    pub items: Vec<ForgeItem>,
}

/// One item in full — the module-owned view model plus the loader's scope.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ForgeItemData {
    pub generation: i64,
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub kind: String,
    pub body: String,
    pub author_name: String,
    pub branches: String,
    pub channel_id: String,
    pub source_branch: String,
    pub source_oid: String,
    pub target_oid: String,
    pub merge_oid: String,
    pub diff: String,
    pub diff_truncated: bool,
    pub files_changed: i64,
    pub additions: i64,
    pub deletions: i64,
    pub reviews: Vec<ForgeReview>,
    pub approvals: i64,
    pub change_requests: i64,
}

async fn list_forge_repos(rpc: &str) -> Result<Vec<serde_json::Value>, String> {
    let client = rpc_client(rpc)?;
    let reply: serde_json::Value = client
        .query("forge", &serde_json::json!("list_repos"))
        .await?;
    Ok(reply["repos"].as_array().cloned().unwrap_or_default())
}

fn listed_forge_repo(repo: &serde_json::Value) -> (String, String) {
    (
        repo["name"].as_str().unwrap_or_default().to_string(),
        repo["head"].as_str().unwrap_or("(unborn)").to_string(),
    )
}

/// The repo namespace with committed heads. This is the screen's fast answer:
/// card facts come from [`load_forge_details`] after these rows are visible.
pub async fn load_forge(rpc: String, generation: i64) -> Result<ForgeData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let repos = list_forge_repos(&rpc)
            .await?
            .into_iter()
            .map(|repo| {
                let (name, head) = listed_forge_repo(&repo);
                ForgeRepo {
                    name,
                    head: short_digest(&head),
                    ..ForgeRepo::default()
                }
            })
            .collect();
        Ok(ForgeData { generation, repos })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Fill the optional repo-card facts from local mirrors after the committed
/// repo rows have landed. Re-listing is a small consensus query and keeps this
/// work self-contained without carrying full object IDs through the UI.
pub async fn load_forge_details(rpc: String, generation: i64) -> Result<ForgeData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let listed = list_forge_repos(&rpc).await?;
        let mut deriving = Vec::with_capacity(listed.len());
        for repo in listed {
            let (name, head) = listed_forge_repo(&repo);
            let endpoint = rpc.clone();
            deriving.push(tokio::task::spawn_blocking(move || {
                let (about, language, updated_at) = repo_card_facts(&endpoint, &name, &head);
                ForgeRepo {
                    head: short_digest(&head),
                    name,
                    about,
                    language,
                    updated_at,
                }
            }));
        }
        let mut repos = Vec::with_capacity(deriving.len());
        for task in deriving {
            let row = task
                .await
                .map_err(|error| format!("forge card-details task failed: {error}"))?;
            repos.push(row);
        }
        Ok(ForgeData { generation, repos })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// One repo's branches and tracker items.
pub async fn load_forge_repo(
    rpc: String,
    repo: String,
    generation: i64,
) -> Result<ForgeRepoData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let refs: serde_json::Value = rpc
            .query(
                "forge",
                &serde_json::json!({ "list_refs": { "repo": repo } }),
            )
            .await?;
        let items: serde_json::Value = rpc
            .query(
                "forge",
                &serde_json::json!({ "list_items": { "repo": repo } }),
            )
            .await?;
        let branches = refs["refs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|branch| branch["name"].as_str().map(str::to_string))
            .collect();
        let summaries: Vec<forge::ItemSummary> =
            serde_json::from_value(items["items"].clone()).map_err(|error| error.to_string())?;
        Ok(ForgeRepoData {
            generation,
            repo,
            branches,
            items: forge::client::item_rows(&summaries),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// One item in full, with the PR patch when there is one.
pub async fn load_forge_item(
    rpc: String,
    repo: String,
    number: i64,
    generation: i64,
) -> Result<ForgeItemData, HydrationError> {
    async {
        let number = u64::try_from(number).map_err(|_| "invalid item number".to_string())?;
        let rpc = rpc_client(&rpc)?;
        let reply: serde_json::Value = rpc
            .query(
                "forge",
                &serde_json::json!({ "get_item": { "repo": repo, "number": number } }),
            )
            .await?;
        let item = &reply["item"];
        if item.is_null() {
            return Err("item was not found".to_string());
        }
        let detail: forge::ItemDetail =
            serde_json::from_value(item.clone()).map_err(|error| error.to_string())?;
        // the wire's snake_case kind — the shipped `== "pull"` check never
        // matched it, so PR patches silently failed to load.
        let is_pr = detail.summary.kind == forge::ItemKind::Pr;
        let diff: Option<forge::PrDiff> = match is_pr {
            false => None,
            true => rpc
                .query::<_, serde_json::Value>(
                    "forge",
                    &serde_json::json!({ "pr_diff": { "repo": repo, "number": number } }),
                )
                .await
                .ok()
                .and_then(|reply| serde_json::from_value(reply["pr_diff"].clone()).ok()),
        };
        let view = forge::client::item_view(&detail, diff.as_ref());
        let branches = match view.source_branch.is_empty() {
            true => String::new(),
            false => format!("{} → {}", view.source_branch, view.target_branch),
        };
        Ok(ForgeItemData {
            generation,
            repo,
            number: view.number,
            title: view.title,
            state: view.state,
            kind: view.kind,
            body: view.body,
            author_name: view.author_name,
            branches,
            channel_id: view.channel_id,
            source_branch: view.source_branch,
            source_oid: view.source_oid,
            target_oid: view.target_oid,
            merge_oid: view.merge_oid,
            diff: view.diff,
            diff_truncated: view.diff_truncated,
            files_changed: view.files_changed,
            additions: view.additions,
            deletions: view.deletions,
            reviews: view.reviews,
            approvals: view.approvals,
            change_requests: view.change_requests,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// One forge item's discussion — the hidden `forge:<repo>:<n>` chat channel
/// rendered through the exact same rows the chat pane uses.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeDiscussionData {
    pub generation: i64,
    pub channel_id: String,
    pub messages: Vec<ChatMessage>,
    /// the channel's members — the composer's mention vocabulary.
    pub members: Vec<ChatMember>,
}

/// Hydrate one item's discussion channel: the message window off the channel
/// record's head plus the mention vocabulary.
pub async fn load_forge_discussion(
    rpc: String,
    channel_id: String,
    generation: i64,
) -> Result<ForgeDiscussionData, HydrationError> {
    async {
        let channel = load_channel_row(&rpc, &channel_id).await?;
        let rpc = rpc_client(&rpc)?;
        let head = u64::try_from(channel.head_seq).unwrap_or(0);
        let messages = load_messages(&rpc, &channel_id, head).await?;
        let members = load_channel_members(&rpc, &channel_id).await?;
        Ok(ForgeDiscussionData {
            generation,
            channel_id,
            messages,
            members,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Submit a batched review on a PR, pinned to the source head the reviewer
/// saw. Approvals stay advisory — the wire never gates the merge.
// The eighth argument is the staged comments, and they cannot be split off into
// their own call: a review and its line comments are ONE transaction on the
// wire. `Tracker::submit_review` carries the same allow for the same reason.
#[allow(clippy::too_many_arguments)]
pub async fn submit_forge_review(
    rpc: String,
    password: String,
    repo: String,
    number: i64,
    verdict: String,
    body: String,
    commit_oid: String,
    comments: Vec<ForgeDraftComment>,
) -> Result<bool, AppError> {
    async {
        let number = u64::try_from(number).map_err(|_| "invalid item number".to_string())?;
        let verdict = match verdict.as_str() {
            "approve" => forge::ReviewVerdict::Approve,
            "request_changes" => forge::ReviewVerdict::RequestChanges,
            "comment" => forge::ReviewVerdict::Comment,
            other => return Err(format!("unknown review verdict {other:?}")),
        };
        let body = bounded_exact_text(body, "review body", forge::MAX_BODY_BYTES)?;
        let comments = review_comments(comments)?;
        if commit_oid.is_empty() {
            return Err("the pull request diff has not loaded yet".to_string());
        }
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "forge",
            forge::encode_msg(&forge::ForgeMsg::SubmitReview {
                repo,
                number,
                verdict,
                body,
                commit_oid,
                comments,
            }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The staged drafts, re-checked at the wire and turned into the module's own
/// `ReviewComment`. `stage_forge_comment` already refuses an unusable draft, so
/// nothing here should ever fire — but this is the boundary where a bad anchor
/// would become a committed record, and a rejection with a reason beats a
/// comment silently landing on line 0 of nothing.
pub(crate) fn review_comments(
    comments: Vec<ForgeDraftComment>,
) -> Result<Vec<forge::ReviewComment>, String> {
    comments
        .into_iter()
        .map(|draft| {
            let line = draft
                .line
                .parse::<u32>()
                .map_err(|_| format!("comment anchor {:?} has no line number", draft.anchor))?;
            let side = match draft.side.as_str() {
                "new" => forge::DiffSide::New,
                "old" => forge::DiffSide::Old,
                other => return Err(format!("unknown diff side {other:?}")),
            };
            Ok(forge::ReviewComment {
                path: bounded_exact_text(draft.path, "comment path", forge::MAX_PATH_BYTES)?,
                line,
                side,
                body: bounded_exact_text(
                    draft.body,
                    "comment body",
                    forge::MAX_REVIEW_COMMENT_BYTES,
                )?,
            })
        })
        .collect()
}

/// The merge box's outcome: either the CAS'd merge landed, or the merge
/// conflicted locally and NOTHING was submitted.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeMergeOutcome {
    pub merged: bool,
    pub merge_oid: String,
    pub conflicts: Vec<String>,
}

/// Merge an open PR the way the wire demands it: the merge commit is
/// CLIENT-COMPUTED. Build it against a local bare mirror of the node's
/// `/forge/{repo}` smart-HTTP remote, land the minimal pack in the node-local
/// blob store, then submit the double-CAS'd `MergePr`.
pub async fn merge_forge_pr(
    rpc: String,
    password: String,
    repo: String,
    number: i64,
    source_branch: String,
    expected_source_oid: String,
    prev_target_oid: String,
) -> Result<ForgeMergeOutcome, AppError> {
    let outcome = async {
        let item = u64::try_from(number).map_err(|_| "invalid item number".to_string())?;
        if expected_source_oid.is_empty() || prev_target_oid.is_empty() {
            return Err("the pull request diff has not loaded yet".to_string());
        }
        let message = format!("Merge pull request #{item} from {source_branch}");
        let build = {
            let endpoint = rpc.clone();
            let repo = repo.clone();
            let ours = prev_target_oid.clone();
            let theirs = expected_source_oid.clone();
            tokio::task::spawn_blocking(move || {
                build_forge_merge(&endpoint, &repo, &ours, &theirs, &message)
            })
            .await
            .map_err(|error| format!("merge build task failed: {error}"))??
        };
        let (merge_oid, pack) = match build {
            MergeBuild::Conflicts(paths) => {
                return Ok(ForgeMergeOutcome {
                    merged: false,
                    merge_oid: String::new(),
                    conflicts: paths,
                });
            }
            MergeBuild::Clean { merge_oid, pack } => (merge_oid, pack),
        };
        let client = rpc_client(&rpc)?;
        let pack_digest = client.put_blob(pack).await?.to_lowercase();
        signed_write(
            &client,
            "forge",
            forge::encode_msg(&forge::ForgeMsg::MergePr {
                repo: repo.clone(),
                number: item,
                prev_target_oid,
                expected_source_oid,
                merge_oid: merge_oid.clone(),
                pack_digest,
            }),
            password,
        )
        .await?;
        Ok(ForgeMergeOutcome {
            merged: true,
            merge_oid,
            conflicts: Vec::new(),
        })
    }
    .await;
    outcome.map_err(app_error)
}

/// The local half of the client-computed merge.
pub(crate) enum MergeBuild {
    Clean { merge_oid: String, pack: Vec<u8> },
    Conflicts(Vec<String>),
}

/// Build the merge commit for `theirs` (source head) into `ours` (target
/// head) without touching the mirror: a throwaway bare repo whose odb reads
/// the mirror's objects through a disk alternate, exactly the shape the
/// decommissioned desktop shipped. Returns the new oid plus the MINIMAL pack —
/// only objects reachable from the merge but from NEITHER parent.
fn build_forge_merge(
    endpoint: &str,
    repo: &str,
    ours: &str,
    theirs: &str,
    message: &str,
) -> Result<MergeBuild, String> {
    let mirror = sync_forge_mirror(endpoint, repo)?;
    let ours_oid = git2::Oid::from_str(ours).map_err(git_err)?;
    let theirs_oid = git2::Oid::from_str(theirs).map_err(git_err)?;
    merge_against_mirror(&mirror, ours_oid, theirs_oid, message)
}

/// The mirror-independent half: merge two commits readable from `mirror`'s
/// odb and pack what neither parent already carries.
pub(crate) fn merge_against_mirror(
    mirror: &git2::Repository,
    ours_oid: git2::Oid,
    theirs_oid: git2::Oid,
    message: &str,
) -> Result<MergeBuild, String> {
    let scratch = ScratchDir::create()?;
    let temp = git2::Repository::init_bare(scratch.path()).map_err(git_err)?;
    let objects = mirror.path().join("objects");
    let objects = objects
        .to_str()
        .ok_or_else(|| format!("non-utf8 objects path {}", objects.display()))?;
    temp.odb()
        .map_err(git_err)?
        .add_disk_alternate(objects)
        .map_err(git_err)?;

    let ours_commit = temp.find_commit(ours_oid).map_err(|_| {
        "the target head is not in the local mirror; the branch may have moved — reload the item"
            .to_string()
    })?;
    let theirs_commit = temp.find_commit(theirs_oid).map_err(|_| {
        "the source head is not in the local mirror; the branch may have moved — reload the item"
            .to_string()
    })?;
    let mut index = temp
        .merge_commits(&ours_commit, &theirs_commit, None)
        .map_err(git_err)?;
    if index.has_conflicts() {
        let mut conflicts = Vec::new();
        for conflict in index.conflicts().map_err(git_err)? {
            let conflict = conflict.map_err(git_err)?;
            let Some(entry) = conflict.our.or(conflict.their).or(conflict.ancestor) else {
                continue;
            };
            conflicts.push(String::from_utf8_lossy(&entry.path).into_owned());
        }
        conflicts.sort();
        conflicts.dedup();
        return Ok(MergeBuild::Conflicts(conflicts));
    }

    let tree_oid = index.write_tree_to(&temp).map_err(git_err)?;
    let tree = temp.find_tree(tree_oid).map_err(git_err)?;
    let signature = git2::Signature::now("ducktape", "ducktape@localhost").map_err(git_err)?;
    let merge_oid = temp
        .commit(
            None,
            &signature,
            &signature,
            message,
            &tree,
            &[&ours_commit, &theirs_commit],
        )
        .map_err(git_err)?;

    let mut builder = temp.packbuilder().map_err(git_err)?;
    let mut walk = temp.revwalk().map_err(git_err)?;
    walk.push(merge_oid).map_err(git_err)?;
    walk.hide(ours_oid).map_err(git_err)?;
    walk.hide(theirs_oid).map_err(git_err)?;
    builder.insert_walk(&mut walk).map_err(git_err)?;
    let mut buf = git2::Buf::new();
    builder.write_buf(&mut buf).map_err(git_err)?;

    Ok(MergeBuild::Clean {
        merge_oid: merge_oid.to_string(),
        pack: buf.to_vec(),
    })
}

/// Open (creating on first use) and refresh the bare mirror of one repo's
/// smart-HTTP remote. The mirror is a persistent per-endpoint cache under the
/// same root the user key lives in, so two networks' repos never shadow each
/// other.
fn sync_forge_mirror(endpoint: &str, repo: &str) -> Result<git2::Repository, String> {
    let dir = forge_mirror_dir(endpoint, repo)?;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("create forge mirror dir {}: {error}", dir.display()))?;
    let mirror = match git2::Repository::open_bare(&dir) {
        Ok(existing) => existing,
        Err(_) => git2::Repository::init_bare(&dir).map_err(git_err)?,
    };
    {
        let mut remote = mirror
            .remote_anonymous(&format!("{}/forge/{repo}", endpoint.trim_end_matches('/')))
            .map_err(git_err)?;
        remote
            .fetch(&["+refs/heads/*:refs/heads/*"], None, None)
            .map_err(|error| format!("fetch forge remote for {repo:?}: {error}"))?;
    }
    Ok(mirror)
}

/// `<key-root>/forge-remote/<endpoint-slug>/<repo>` — the key root is the same
/// resolution order the user key uses (`DUCKTAPE_HOME`, then `~/.ducktape`).
fn forge_mirror_dir(endpoint: &str, repo: &str) -> Result<PathBuf, String> {
    if repo.is_empty() || repo.contains('/') || repo.contains('\\') || repo.starts_with('.') {
        return Err(format!("invalid forge repo name {repo:?}"));
    }
    let root = match std::env::var_os("DUCKTAPE_HOME") {
        Some(home) => PathBuf::from(home),
        None => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".ducktape"))
            .ok_or_else(|| "cannot locate a home for the forge mirror".to_string())?,
    };
    let slug: String = endpoint
        .chars()
        .map(|character| match character.is_ascii_alphanumeric() {
            true => character,
            false => '-',
        })
        .collect();
    Ok(root.join("forge-remote").join(slug).join(repo))
}

fn git_err(error: git2::Error) -> String {
    error.message().to_string()
}

/// Process-unique throwaway directory under the OS temp dir, removed
/// (best-effort) on drop — the merge scratch is one bare repo per click.
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
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("create merge scratch dir {}: {error}", dir.display()))?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One entry of a repo's tree at one revision. `kind` is `dir` | `file`; a
/// directory has no size on the wire and reads 0.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct TreeEntry {
    pub name: String,
    /// The full path from the repo root, so a row navigates without the view
    /// having to re-join it against the current directory.
    pub path: String,
    pub kind: String,
    pub size: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeTreeData {
    pub generation: i64,
    pub repo: String,
    pub rev: String,
    pub path: String,
    pub entries: Vec<TreeEntry>,
}

/// One file's contents at one revision, in the shape the preview pane reads.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlobView {
    pub generation: i64,
    pub repo: String,
    pub rev: String,
    pub path: String,
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
    pub lines: i64,
}

/// The default revision a repo browse opens at: the integration branch the
/// module itself prefers (`dev`, else `main`), else whatever is born.
fn default_rev(mirror: &git2::Repository) -> Result<String, String> {
    for preferred in ["dev", "main"] {
        if mirror
            .find_branch(preferred, git2::BranchType::Local)
            .is_ok()
        {
            return Ok(preferred.to_string());
        }
    }
    let branches = mirror
        .branches(Some(git2::BranchType::Local))
        .map_err(git_err)?;
    for branch in branches {
        let (branch, _) = branch.map_err(git_err)?;
        if let Some(name) = branch.name().map_err(git_err)? {
            return Ok(name.to_string());
        }
    }
    Err("this repo has no born branch yet".into())
}

/// Resolve `rev` (a branch name, or empty for the default) to its commit.
pub(crate) fn mirror_commit_at<'repo>(
    mirror: &'repo git2::Repository,
    rev: &str,
) -> Result<git2::Commit<'repo>, String> {
    let rev = match rev.is_empty() {
        true => default_rev(mirror)?,
        false => rev.to_string(),
    };
    let object = mirror
        .revparse_single(&rev)
        .map_err(|_| format!("no such revision {rev:?} in this repo"))?;
    object.peel_to_commit().map_err(git_err)
}

/// The tree at `path` under `rev`, directories first then files, name order.
pub(crate) fn read_tree(
    mirror: &git2::Repository,
    rev: &str,
    path: &str,
) -> Result<Vec<TreeEntry>, String> {
    let commit = mirror_commit_at(mirror, rev)?;
    let root = commit.tree().map_err(git_err)?;
    let path = path.trim_matches('/');
    let tree = match path.is_empty() {
        true => root,
        false => {
            let entry = root
                .get_path(Path::new(path))
                .map_err(|_| format!("no such path {path:?} at this revision"))?;
            entry
                .to_object(mirror)
                .map_err(git_err)?
                .peel_to_tree()
                .map_err(|_| format!("{path:?} is a file, not a directory"))?
        }
    };
    let mut entries = Vec::with_capacity(tree.len());
    for entry in tree.iter() {
        let name = entry.name().unwrap_or_default().to_string();
        if name.is_empty() {
            continue;
        }
        let is_dir = entry.kind() == Some(git2::ObjectType::Tree);
        let size = match is_dir {
            true => 0,
            false => entry
                .to_object(mirror)
                .ok()
                .and_then(|object| object.into_blob().ok())
                .map_or(0, |blob| count_i64(blob.size())),
        };
        entries.push(TreeEntry {
            path: match path.is_empty() {
                true => name.clone(),
                false => format!("{path}/{name}"),
            },
            kind: match is_dir {
                true => "dir".into(),
                false => "file".into(),
            },
            name,
            size,
        });
    }
    entries.sort_by(|left, right| {
        let dirs_lead = (right.kind == "dir").cmp(&(left.kind == "dir"));
        dirs_lead.then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

/// List one repo directory at one revision. No new module wire: the app
/// already keeps a bare mirror of every branch for the client-computed merge,
/// so the whole tree is readable locally.
pub async fn forge_tree(
    rpc: String,
    repo: String,
    rev: String,
    path: String,
    generation: i64,
) -> Result<ForgeTreeData, HydrationError> {
    async {
        tokio::task::spawn_blocking(move || {
            let mirror = sync_forge_mirror(&rpc, &repo)?;
            let entries = read_tree(&mirror, &rev, &path)?;
            Ok::<_, String>(ForgeTreeData {
                generation,
                repo,
                rev,
                path,
                entries,
            })
        })
        .await
        .map_err(|error| format!("forge tree task failed: {error}"))?
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The preview window one blob read returns, matching duckfs's 64 KiB cap.
const MAX_BLOB_PREVIEW: usize = 64 * 1024;

/// One blob's decoded head, its truncation flag and its line count.
pub(crate) fn read_blob(
    mirror: &git2::Repository,
    repo: String,
    rev: String,
    path: String,
    generation: i64,
) -> Result<BlobView, String> {
    let commit = mirror_commit_at(mirror, &rev)?;
    let tree = commit.tree().map_err(git_err)?;
    let entry = tree
        .get_path(Path::new(path.trim_matches('/')))
        .map_err(|_| format!("no such path {path:?} at this revision"))?;
    let blob = entry
        .to_object(mirror)
        .map_err(git_err)?
        .into_blob()
        .map_err(|_| format!("{path:?} is a directory, not a file"))?;
    let content = blob.content();
    let truncated = content.len() > MAX_BLOB_PREVIEW;
    let window = &content[..content.len().min(MAX_BLOB_PREVIEW)];
    let readable = std::str::from_utf8(window)
        .ok()
        .filter(|text| !text.contains('\0'));
    let Some(text) = readable else {
        return Ok(BlobView {
            generation,
            repo,
            rev,
            path,
            text: format!("{} binary bytes", content.len()),
            truncated: false,
            binary: true,
            lines: 0,
        });
    };
    Ok(BlobView {
        generation,
        repo,
        rev,
        path,
        lines: count_i64(text.lines().count()),
        text: text.to_string(),
        truncated,
        binary: false,
    })
}

/// Read one file at one revision out of the local mirror.
pub async fn forge_blob(
    rpc: String,
    repo: String,
    rev: String,
    path: String,
    generation: i64,
) -> Result<BlobView, HydrationError> {
    async {
        tokio::task::spawn_blocking(move || {
            let mirror = sync_forge_mirror(&rpc, &repo)?;
            read_blob(&mirror, repo, rev, path, generation)
        })
        .await
        .map_err(|error| format!("forge blob task failed: {error}"))?
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The README names a repo browse recognizes, in preference order.
const README_NAMES: &[&str] = &["README.md", "README", "readme.md", "README.txt"];

/// The repo "about" line: the README's first prose paragraph, headings and
/// badges skipped. Empty when there is no README — the card keeps its
/// min-height rather than inventing a description.
pub(crate) fn readme_about(mirror: &git2::Repository, commit: &git2::Commit) -> String {
    let Ok(tree) = commit.tree() else {
        return String::new();
    };
    let found = README_NAMES.iter().find_map(|name| {
        let entry = tree.get_name(name)?;
        let blob = entry.to_object(mirror).ok()?.into_blob().ok()?;
        String::from_utf8(blob.content().to_vec()).ok()
    });
    let Some(text) = found else {
        return String::new();
    };
    let lines = text.lines().map(str::trim).collect::<Vec<_>>();
    let opens_prose = |line: &&&str| {
        let empty = line.is_empty();
        let heading = line.starts_with('#');
        let badge = line.starts_with('[') || line.starts_with('!');
        !empty && !heading && !badge
    };
    let Some(start) = lines.iter().position(|line| opens_prose(&line)) else {
        return String::new();
    };
    // The whole paragraph, not its first physical line. A README is hard
    // wrapped, so taking one line ended this repo's own card mid-clause —
    // "…one BFT-replicated state machine that" — which reads as a UI
    // truncation and carries no ellipsis to say so. A blank line ends the
    // paragraph, which is what a paragraph IS; a wrapped continuation may
    // legitimately begin with a bracket, so only the OPENING line is screened
    // for headings and badges.
    let prose = lines[start..]
        .iter()
        .take_while(|line| !line.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    match prose.char_indices().nth(200) {
        Some((cut, _)) => format!("{}…", &prose[..cut]),
        None => prose,
    }
}

/// The repo's language, by which source extension owns the most files at the
/// head revision.
//
// ponytail: a file-count heuristic over a bounded walk, not linguist's
// byte-weighted classifier — upgrade to bytes-per-extension if a repo of
// generated files starts reading wrong.
pub(crate) fn dominant_language(commit: &git2::Commit) -> String {
    const MAX_WALKED_ENTRIES: usize = 4096;
    const LANGUAGES: &[(&str, &str)] = &[
        ("rs", "Rust"),
        ("ts", "TypeScript"),
        ("tsx", "TypeScript"),
        ("js", "JavaScript"),
        ("py", "Python"),
        ("go", "Go"),
        ("swift", "Swift"),
        ("kt", "Kotlin"),
        ("java", "Java"),
        ("c", "C"),
        ("h", "C"),
        ("cpp", "C++"),
        ("rb", "Ruby"),
        ("sh", "Shell"),
        ("ice", "Ice"),
        ("md", "Markdown"),
    ];
    let Ok(tree) = commit.tree() else {
        return String::new();
    };
    let mut walked = 0usize;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let _ = tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
        walked += 1;
        if walked > MAX_WALKED_ENTRIES {
            return git2::TreeWalkResult::Abort;
        }
        let name = entry.name().unwrap_or_default();
        let extension = name.rsplit_once('.').map(|(_, tail)| tail).unwrap_or("");
        if let Some((_, language)) = LANGUAGES.iter().find(|(suffix, _)| *suffix == extension) {
            *counts.entry(*language).or_default() += 1;
        }
        git2::TreeWalkResult::Ok
    });
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(language, _)| language.to_string())
        .unwrap_or_default()
}

/// One repo's card facts — about line, language, head committer time — read
/// off the local mirror at the module's committed head. A repo whose head the
/// mirror cannot produce renders blank rather than a guess.
///
/// BLOCKING: git2 walks a tree and may fetch. Callers run it on the blocking
/// pool.
pub(crate) fn repo_card_facts(endpoint: &str, repo: &str, head_oid: &str) -> (String, String, i64) {
    const BLANK: (String, String, i64) = (String::new(), String::new(), 0);
    let Ok(head) = git2::Oid::from_str(head_oid) else {
        return BLANK;
    };
    let Ok(mirror) = mirror_holding(endpoint, repo, head) else {
        return BLANK;
    };
    let Ok(commit) = mirror.find_commit(head) else {
        return BLANK;
    };
    (
        readme_about(&mirror, &commit),
        dominant_language(&commit),
        commit.time().seconds(),
    )
}

/// The mirror holding `head`. The mirror IS the cache: a head the resident
/// clone already carries costs no network, so re-listing the repos after every
/// forge event never refetches a repo whose head has not moved.
fn mirror_holding(endpoint: &str, repo: &str, head: git2::Oid) -> Result<git2::Repository, String> {
    let dir = forge_mirror_dir(endpoint, repo)?;
    let resident = git2::Repository::open_bare(&dir).ok();
    let already_holds_head = resident.filter(|mirror| mirror.find_commit(head).is_ok());
    match already_holds_head {
        Some(mirror) => Ok(mirror),
        None => sync_forge_mirror(endpoint, repo),
    }
}

/// The listed row for `name`, so the open repo's body reads its about line,
/// language and updated stamp out of the resident list instead of re-deriving
/// them. An unknown name yields a blank row.
pub fn forge_repo_row(repos: Vec<ForgeRepo>, name: String) -> ForgeRepo {
    repos
        .into_iter()
        .find(|repo| repo.name == name)
        .unwrap_or_default()
}

/// True when one live update invalidates forge state: a folded forge op, a
/// forge replay the stream could not fold (`resync`), or the stream (re)
/// subscribing (`ready` — anything may have landed while it was down).
pub fn forge_live_hit(kind: String, module: String) -> bool {
    let folded_forge_op = kind == "forge";
    let unfoldable_forge_replay = kind == "resync" && module == "forge";
    let stream_caught_up = kind == "ready";
    folded_forge_op || unfoldable_forge_replay || stream_caught_up
}

/// One scoped forge catch-up, flag-selected per slice like [`LiveRefresh`]:
/// the repo list reloads only while the forge surface is open; the open repo's
/// slice and the open item reload when the op's scope reaches them.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeLiveData {
    pub generation: i64,
    pub repos_loaded: bool,
    pub repos: Vec<ForgeRepo>,
    pub repo_loaded: bool,
    pub branches: Vec<String>,
    pub items: Vec<ForgeItem>,
    pub item_loaded: bool,
    pub item: ForgeItemData,
}

/// Reload the forge slices one committed op (or an unfoldable replay)
/// invalidated. A non-hit update no-ops with every flag false (the handler's
/// keeps leave state untouched); an empty op scope means the scope is
/// unknown — reload every open slice.
///
/// `forge_open` is the repo LIST's surface gate. That one load was the only
/// unscoped slice here — a git-mirror walk per repo, on every forge op, for a
/// list no other tab draws. The repo and item slices keep running off-tab on
/// purpose: they are already scoped to what the forge pane has open, and
/// dropping them would hand a stale PR back on the return trip (the tab-switch
/// handler's `load_forge` refills the list, and nothing else).
// Eight arguments, and none of them can be folded away: this is one Ice extern
// and every argument is a separate piece of handler state the `run` reads at
// the call site. Grouping them into a struct would mean a new Ice type declared
// for one call.
#[allow(clippy::too_many_arguments)]
pub async fn forge_live_refresh(
    rpc: String,
    open_repo: String,
    open_item: i64,
    kind: String,
    module: String,
    refresh: ForgeRefresh,
    forge_open: bool,
    generation: i64,
) -> Result<ForgeLiveData, HydrationError> {
    let noop = ForgeLiveData {
        generation,
        repos_loaded: false,
        repos: Vec::new(),
        repo_loaded: false,
        branches: Vec::new(),
        items: Vec::new(),
        item_loaded: false,
        item: ForgeItemData {
            generation,
            ..ForgeItemData::default()
        },
    };
    if !forge_live_hit(kind, module) {
        return Ok(noop);
    }
    let scope_unknown = refresh.repo.is_empty();
    let repo_hit = !open_repo.is_empty() && (scope_unknown || refresh.repo == open_repo);
    let item_hit = repo_hit
        && open_item > 0
        && (scope_unknown || refresh.number == open_item || refresh.refs_moved);
    let repos = match forge_open {
        false => None,
        true => Some(load_forge(rpc.clone(), generation).await?),
    };
    let repo_slice = match repo_hit {
        false => None,
        true => Some(load_forge_repo(rpc.clone(), open_repo.clone(), generation).await?),
    };
    let item_slice = match item_hit {
        false => None,
        true => Some(load_forge_item(rpc, open_repo, open_item, generation).await?),
    };
    Ok(ForgeLiveData {
        repos_loaded: repos.is_some(),
        repos: repos.map(|data| data.repos).unwrap_or_default(),
        repo_loaded: repo_slice.is_some(),
        branches: repo_slice
            .as_ref()
            .map(|slice| slice.branches.clone())
            .unwrap_or_default(),
        items: repo_slice.map(|slice| slice.items).unwrap_or_default(),
        item_loaded: item_slice.is_some(),
        ..noop
    })
}

/// The PR stats line: `3 files · +12 −4`.
/// One rendered line of a unified patch. `kind` is `file` | `hunk` | `add` |
/// `del` | `ctx` — the gutters, the sign column and the row tint all key on it.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct DiffLine {
    /// Session-stable identity for keyed rendering.
    pub key: i64,
    pub kind: String,
    pub old_no: String,
    pub new_no: String,
    pub sign: String,
    pub text: String,
    /// The file this row belongs to, carried from the patch's own `+++ b/…`
    /// header. `forge_item_diff` is the WHOLE multi-file patch as one string,
    /// so a row cannot say which file it is from without this — and
    /// `ReviewComment` anchors on `(path, line, side)`, so a comment cannot be
    /// authored from a row that does not know its own path.
    ///
    /// Empty on `file` and `hunk` rows, and on a deletion's `/dev/null` side:
    /// those are not commentable positions.
    pub path: String,
    /// Which side of the diff this row addresses — `new` for an addition or a
    /// context line, `old` for a deletion, empty for a non-code row. Mirrors
    /// `forge::DiffSide`, as a string because that is what crosses into `.ice`.
    pub side: String,
}

/// One numbered source line of a blob. `number` is a string for the same reason
/// `DiffLine.old_no` is: the gutter is a rendered column, not an integer, and
/// the splitter owns the numbering.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct SourceLine {
    pub number: String,
    pub text: String,
}

/// Split a blob into numbered rows. `BlobView.text` arrives as ONE string and
/// Ice has no string ops, so the viewer cannot walk it — this is the exact
/// counterpart `diff_lines` already is for a patch.
///
/// An empty blob has no lines, not one blank line: `"".lines()` is empty and
/// that is the reading the empty plate is drawn for.
pub fn source_lines(text: String) -> Vec<SourceLine> {
    text.lines()
        .enumerate()
        .map(|(index, line)| SourceLine {
            number: (index + 1).to_string(),
            text: line.to_string(),
        })
        .collect()
}

/// Split a unified patch into painted rows, tracking both line counters
/// across hunk headers.
/// The command that makes a repo. Forge IS a git remote — there is no "new
/// repository" button anywhere, because a repo comes into existence when a push
/// lands on it. An empty Forge screen that does not say so is a dead end: it
/// tells the reader a repo "appears here once it is created" and names no way
/// to create one.
pub fn forge_push_command(rpc: String) -> String {
    let endpoint = rpc.trim_end_matches('/');
    // `my-repo`, not `NAME`: `forge::norm_repo` accepts `[a-z0-9._-]` only, so
    // an uppercase placeholder pasted verbatim 404s the ref advertisement and
    // git reports "repository not found" — a hint that teaches the wrong thing.
    format!("git remote add ducktape {endpoint}/forge/my-repo && git push ducktape main")
}

pub fn diff_lines(diff: String) -> Vec<DiffLine> {
    // A patch line has no durable id. Reusing content/occurrence across two
    // patch revisions can move focus to an identical line's comment button,
    // while line-number keys can move it to unrelated content. Namespace the
    // whole row set by the exact patch: unchanged rebuilds retain identity;
    // any patch edit deliberately drops row state instead of transferring it.
    use std::hash::{Hash as _, Hasher as _};

    let mut patch_hasher = std::hash::DefaultHasher::new();
    diff.hash(&mut patch_hasher);
    let patch_key = patch_hasher.finish() as i64;
    let mut rows = Vec::new();
    let mut old_no = 0i64;
    let mut new_no = 0i64;
    // The path every following code row is anchored to, taken from the patch's
    // own `+++ b/…` header. A comment cannot be authored from a row that does
    // not know its file, and `forge_item_diff` is the whole multi-file patch as
    // one string, so this is the only place the association exists.
    let mut path = String::new();
    // What the open hunk still owes on each side.
    //
    // A hunk header DECLARES how many lines its body covers, and while either
    // side is still owed one, every line is body content — never a header.
    // That budget is the ONLY thing separating a real `+++ b/<path>` header
    // from a source line reading `++ x`, which a patch writes as `+++ x`.
    // Without it, adding such a line silently re-anchored every row after it
    // to a path that does not exist, and a comment written below it would be
    // submitted against that path.
    let mut old_left = 0i64;
    let mut new_left = 0i64;
    for line in diff.lines() {
        let inside_hunk_body = old_left > 0 || new_left > 0;
        if !inside_hunk_body {
            if let Some(target) = added_side_path(line) {
                path = target;
                rows.push(marker_row(line));
                continue;
            }
            if is_file_header(line) {
                rows.push(marker_row(line));
                continue;
            }
            if let Some(span) = hunk_span(line) {
                old_no = span.old_start;
                new_no = span.new_start;
                old_left = span.old_len;
                new_left = span.new_len;
                rows.push(diff_row(
                    "hunk",
                    String::new(),
                    String::new(),
                    "",
                    line,
                    "",
                    "",
                ));
                continue;
            }
        }
        // `\ No newline at end of file` is a note ABOUT the previous line. It
        // holds no position on either side, so it consumes neither a line
        // number nor the hunk's budget — counting it would end the hunk one
        // line early and re-open header detection inside the body.
        if line.starts_with('\\') {
            rows.push(marker_row(line));
            continue;
        }
        match line.chars().next() {
            Some('+') => {
                rows.push(diff_row(
                    "add",
                    String::new(),
                    new_no.to_string(),
                    "+",
                    &line[1..],
                    &path,
                    "new",
                ));
                new_no += 1;
                new_left -= 1;
            }
            Some('-') => {
                rows.push(diff_row(
                    "del",
                    old_no.to_string(),
                    String::new(),
                    "-",
                    &line[1..],
                    &path,
                    "old",
                ));
                old_no += 1;
                old_left -= 1;
            }
            _ => {
                let text = line.strip_prefix(' ').unwrap_or(line);
                rows.push(diff_row(
                    "ctx",
                    old_no.to_string(),
                    new_no.to_string(),
                    "",
                    text,
                    &path,
                    "new",
                ));
                old_no += 1;
                new_no += 1;
                old_left -= 1;
                new_left -= 1;
            }
        }
    }
    for (index, row) in rows.iter_mut().enumerate() {
        row.key = patch_key.wrapping_add(count_i64(index));
    }
    rows
}

/// The non-code rows: a file header, and the `\ No newline` note. Neither is a
/// commentable position, so both carry an empty path and side.
fn marker_row(line: &str) -> DiffLine {
    diff_row("file", String::new(), String::new(), "", line, "", "")
}

fn is_file_header(line: &str) -> bool {
    line.starts_with("diff ")
        || line.starts_with("--- ")
        || line.starts_with("index ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
}

/// The head-side path a `+++ b/<path>` header names, or `None` for any other
/// line. A pure deletion writes `+++ /dev/null`, which names no file on the
/// head side and yields an empty path — its rows are then uncommentable, which
/// is correct: there is no head line to anchor to.
fn added_side_path(line: &str) -> Option<String> {
    let target = line.strip_prefix("+++ ")?;
    if target == "/dev/null" {
        return Some(String::new());
    }
    // git writes `b/<path>`; a patch produced without prefixes writes the path
    // bare, so strip the marker only when it is there.
    Some(target.strip_prefix("b/").unwrap_or(target).to_string())
}

fn diff_row(
    kind: &str,
    old_no: String,
    new_no: String,
    sign: &str,
    text: &str,
    path: &str,
    side: &str,
) -> DiffLine {
    DiffLine {
        key: 0,
        kind: kind.into(),
        old_no,
        new_no,
        path: path.into(),
        side: side.into(),
        sign: sign.into(),
        text: text.to_string(),
    }
}

/// One line comment staged for a review that has not been submitted yet.
///
/// `anchor` is display-ready (`src/main.rs:14 (new)`) exactly as
/// `ReviewCommentRow.anchor` is on the read side, so the view never re-derives
/// diff vocabulary — and it doubles as the row's IDENTITY. Restaging a line
/// replaces the comment there instead of stacking a second one on one position,
/// which is the only sane reading of clicking the same gutter twice.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ForgeDraftComment {
    pub anchor: String,
    pub path: String,
    /// The anchored line number, as the string the gutter already renders.
    /// Parsed back to `u32` at the wire.
    pub line: String,
    /// `new` | `old` — mirrors `forge::DiffSide`.
    pub side: String,
    pub body: String,
}

/// Stage one line comment, or replace the one already on that line.
///
/// Returns `staged` UNCHANGED when the anchor or body is not usable — an empty
/// path (a deleted file's rows), a blank body, a line number that is not a
/// positive `u32`, or a full list. The composer disables its own submit at the
/// cap via `forge_comment_cap_reached`, so a user never reaches the silent
/// arm; this is the invariant behind that, not the message that carries it.
pub fn stage_forge_comment(
    staged: Vec<ForgeDraftComment>,
    path: String,
    line: String,
    side: String,
    body: String,
) -> Vec<ForgeDraftComment> {
    let anchored = !path.is_empty() && line.parse::<u32>().is_ok_and(|no| no > 0);
    let sided = side == "new" || side == "old";
    let usable_body = !body.trim().is_empty() && body.len() <= forge::MAX_REVIEW_COMMENT_BYTES;
    if !anchored || !sided || !usable_body || path.len() > forge::MAX_PATH_BYTES {
        return staged;
    }
    let comment = ForgeDraftComment {
        anchor: comment_anchor(&path, &line, &side),
        path,
        line,
        side,
        body,
    };
    let mut staged = staged;
    match staged.iter().position(|row| row.anchor == comment.anchor) {
        Some(at) => staged[at] = comment,
        None if staged.len() < forge::MAX_REVIEW_COMMENTS => staged.push(comment),
        None => {}
    }
    staged
}

/// Drop the comment staged at one anchor. A miss leaves the list alone.
pub fn drop_forge_comment(
    staged: Vec<ForgeDraftComment>,
    anchor: String,
) -> Vec<ForgeDraftComment> {
    let mut staged = staged;
    staged.retain(|row| row.anchor != anchor);
    staged
}

/// The staged list is at the module's per-review cap, so the composer must
/// refuse to take another. The literal lives HERE and nowhere in `.ice`, so the
/// gate and the module's own limit cannot drift apart.
pub fn forge_comment_cap_reached(staged: Vec<ForgeDraftComment>) -> bool {
    staged.len() >= forge::MAX_REVIEW_COMMENTS
}

/// The label a picked-but-unstaged line wears above the composer, empty when no
/// line is picked — the composer keys its whole visibility on this.
pub fn forge_comment_target(path: String, line: String, side: String) -> String {
    if path.is_empty() {
        return String::new();
    }
    comment_anchor(&path, &line, &side)
}

/// `src/main.rs:14 (new)` — the one place the anchor string is spelled, shared
/// by the staged rows and the composer header so they can never disagree.
fn comment_anchor(path: &str, line: &str, side: &str) -> String {
    format!("{path}:{line} ({side})")
}

/// A staged comment outlives the diff it was written against when a live
/// refresh moves the PR's source head.
///
/// It CANNOT be carried across: the anchor is `(path, line, side)` into a
/// specific patch, and the review would be submitted pinning the NEW head — so
/// a comment about a line the author read would land on whatever now occupies
/// that number, and `outdated` would read false because the pin matches. The
/// module has no position tracking across a moved branch by design; dropping is
/// the only reading that cannot publish a false claim.
fn staged_comments_outlived_their_diff(loaded: bool, next_oid: &str, current_oid: &str) -> bool {
    let moved = !next_oid.is_empty() && !current_oid.is_empty() && next_oid != current_oid;
    loaded && moved
}

/// The staged comments, or none once the branch has moved under them.
pub fn keep_staged_comments(
    loaded: bool,
    next_oid: String,
    current_oid: String,
    staged: Vec<ForgeDraftComment>,
) -> Vec<ForgeDraftComment> {
    if staged_comments_outlived_their_diff(loaded, &next_oid, &current_oid) {
        return Vec::new();
    }
    staged
}

/// One in-composer string (the picked path, the body being typed) held only
/// while the diff it belongs to is still on screen.
pub fn keep_comment_text(
    loaded: bool,
    next_oid: String,
    current_oid: String,
    value: String,
) -> String {
    if staged_comments_outlived_their_diff(loaded, &next_oid, &current_oid) {
        return String::new();
    }
    value
}

/// Discarded work is never silent. This says WHY the staged comments vanished,
/// and only when there were some to lose — a refresh that moved the branch
/// while nothing was staged is not an error and reports none.
pub fn staged_comment_drop_note(
    loaded: bool,
    next_oid: String,
    current_oid: String,
    staged: Vec<ForgeDraftComment>,
    error: String,
) -> String {
    let lost =
        !staged.is_empty() && staged_comments_outlived_their_diff(loaded, &next_oid, &current_oid);
    if !lost {
        return error;
    }
    "The branch moved while comments were staged. They anchored to lines in the old diff, so they were discarded rather than posted against the new one.".into()
}

/// A hunk header's two starting line numbers and the two line counts its body
/// covers. The counts are what bound the body — see `diff_lines`.
struct HunkSpan {
    old_start: i64,
    new_start: i64,
    old_len: i64,
    new_len: i64,
}

/// `@@ -138,9 +138,12 @@ …` → the starts and the lengths. A range written
/// without a comma covers exactly one line (`@@ -1 +1 @@`).
fn hunk_span(line: &str) -> Option<HunkSpan> {
    let body = line.strip_prefix("@@ ")?;
    let (ranges, _) = body.split_once(" @@")?;
    let (old, new) = ranges.split_once(' ')?;
    let range = |range: &str| -> Option<(i64, i64)> {
        let digits = range.trim_start_matches(['-', '+']);
        let (start, len) = match digits.split_once(',') {
            Some((start, len)) => (start, len.parse().ok()?),
            None => (digits, 1),
        };
        Some((start.parse().ok()?, len))
    };
    let (old_start, old_len) = range(old)?;
    let (new_start, new_len) = range(new)?;
    Some(HunkSpan {
        old_start,
        new_start,
        old_len,
        new_len,
    })
}

/// The tracker's Pull requests / Issues split.
pub fn filter_forge_items(items: Vec<ForgeItem>, kind: String) -> Vec<ForgeItem> {
    items.into_iter().filter(|item| item.kind == kind).collect()
}

/// The tab count chips — open work only: a PR counts until it merges, an
/// issue until it closes.
pub fn forge_open_count(items: Vec<ForgeItem>, kind: String) -> i64 {
    count_i64(
        items
            .iter()
            .filter(|item| item.kind == kind)
            .filter(|item| match kind.as_str() {
                "pr" => item.state != "merged",
                _ => item.state == "open",
            })
            .count(),
    )
}

// There is NO forge write gate, and this file used to invent one. `MergePr`,
// `SubmitReview` and the tracker verbs each check only `author_from_origin`
// (crates/modules/apps/forge/src/lib.rs) — any user key may merge, and this
// node's valset seat is not even the axis the write is signed on. A refusal
// plate over an action the chain accepts is worse than no plate.

pub fn forge_stats(files: i64, additions: i64, deletions: i64) -> String {
    format!("{files} files · +{additions} −{deletions}")
}

/// The merged-state banner: the short merge oid plus the branch line.
pub fn forge_merge_note(merge_oid: String, branches: String) -> String {
    let short: String = merge_oid.chars().take(8).collect();
    match branches.is_empty() {
        true => format!("Merged as {short}"),
        false => format!("Merged as {short} · {branches}"),
    }
}

/// A review verdict key as its timeline verb.
pub fn verdict_label(verdict: String) -> String {
    match verdict.as_str() {
        "approve" => "approved".into(),
        "request_changes" => "requested changes".into(),
        _ => "commented".into(),
    }
}

/// A verdict picker label, dotted when it is the current pick.
pub fn verdict_pick_label(current: String, key: String, label: String) -> String {
    match current == key {
        true => format!("● {label}"),
        false => label,
    }
}

pub fn keep_forge_repos(
    loaded: bool,
    next: Vec<ForgeRepo>,
    current: Vec<ForgeRepo>,
) -> Vec<ForgeRepo> {
    if loaded { next } else { current }
}

pub fn keep_branches(loaded: bool, next: Vec<String>, current: Vec<String>) -> Vec<String> {
    if loaded { next } else { current }
}

pub fn keep_forge_items(
    loaded: bool,
    next: Vec<ForgeItem>,
    current: Vec<ForgeItem>,
) -> Vec<ForgeItem> {
    if loaded { next } else { current }
}

pub fn keep_forge_reviews(
    loaded: bool,
    next: Vec<ForgeReview>,
    current: Vec<ForgeReview>,
) -> Vec<ForgeReview> {
    if loaded { next } else { current }
}
