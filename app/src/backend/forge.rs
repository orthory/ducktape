use super::*;
use ::forge;

/// One forge repo row: the module's committed name and head.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ForgeRepo {
    pub name: String,
    pub head: String,
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

/// The repo namespace with committed heads.
pub async fn load_forge(rpc: String, generation: i64) -> Result<ForgeData, HydrationError> {
    async {
        let repos = list_forge_repos(&rpc)
            .await?
            .into_iter()
            .map(|repo| {
                let (name, head) = listed_forge_repo(&repo);
                ForgeRepo {
                    name,
                    head: short_digest(&head),
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

/// One repo's branches and tracker items.
pub async fn load_forge_repo(
    rpc: String,
    repo: String,
    generation: i64,
) -> Result<ForgeRepoData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        // Branches and tracker summaries are independent committed reads. The
        // repo seat needs both, but neither is a reason to queue behind the
        // other on the node's query lane.
        let refs_query = serde_json::json!({ "list_refs": { "repo": &repo } });
        let items_query = serde_json::json!({ "list_items": { "repo": &repo } });
        let (refs, items): (serde_json::Value, serde_json::Value) = tokio::try_join!(
            rpc.query("forge", &refs_query),
            rpc.query("forge", &items_query)
        )?;
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
) -> Result<ForgeDiscussionData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let (message_page, members) = tokio::try_join!(
            load_messages(&rpc, &channel_id),
            load_channel_members(&rpc, &channel_id)
        )?;
        Ok(ForgeDiscussionData {
            channel_id,
            messages: message_page.messages,
            members,
        })
    }
    .await
    .map_err(app_error)
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
    verdict: crate::ForgeReviewVerdict,
    body: String,
    commit_oid: String,
    comments: Vec<ForgeDraftComment>,
) -> Result<bool, AppError> {
    async {
        let number = u64::try_from(number).map_err(|_| "invalid item number".to_string())?;
        let verdict = match verdict {
            crate::ForgeReviewVerdict::Comment => forge::ReviewVerdict::Comment,
            crate::ForgeReviewVerdict::Approve => forge::ReviewVerdict::Approve,
            crate::ForgeReviewVerdict::RequestChanges => forge::ReviewVerdict::RequestChanges,
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
static FORGE_MIRROR_LOCKS: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

fn forge_mirror_lock(dir: &Path) -> Result<Arc<Mutex<()>>, String> {
    let locks = FORGE_MIRROR_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut locks = locks
        .lock()
        .map_err(|_| "forge mirror lock registry is poisoned".to_string())?;
    Ok(locks
        .entry(dir.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

fn sync_forge_mirror(endpoint: &str, repo: &str) -> Result<git2::Repository, String> {
    let dir = forge_mirror_dir(endpoint, repo)?;
    let lock = forge_mirror_lock(&dir)?;
    let _guard = lock
        .lock()
        .map_err(|_| format!("forge mirror lock is poisoned for {repo:?}"))?;
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

/// One entry of a repo's tree at one revision. `kind` is `dir` | `file`.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct TreeEntry {
    pub name: String,
    /// The full path from the repo root, so a row navigates without the view
    /// having to re-join it against the current directory.
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ForgeTreeData {
    pub repo: String,
    pub rev: String,
    pub path: String,
    /// Whether the repo has at least one branch. An empty `entries` list can
    /// also be a real empty commit, so the view must not infer "unborn" from
    /// the list alone.
    pub born: bool,
    pub entries: Vec<TreeEntry>,
    pub truncated: bool,
}

/// One file's contents at one revision, in the shape the preview pane reads.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlobView {
    pub repo: String,
    pub rev: String,
    pub path: String,
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
    pub lines: i64,
}

pub(crate) fn tree_data(
    reply: serde_json::Value,
    repo: String,
    path: String,
) -> Result<ForgeTreeData, String> {
    let tree = reply
        .get("tree")
        .filter(|tree| !tree.is_null())
        .ok_or_else(|| "the repository tree was not found".to_string())?;
    let tree: forge::TreeReply =
        serde_json::from_value(tree.clone()).map_err(|error| error.to_string())?;
    let entries = tree
        .entries
        .into_iter()
        .map(|entry| {
            let kind = match entry.kind {
                forge::TreeEntryKind::Dir => "dir",
                forge::TreeEntryKind::File => "file",
            };
            TreeEntry {
                name: entry.name,
                path: entry.path,
                kind: kind.into(),
            }
        })
        .collect();
    Ok(ForgeTreeData {
        repo,
        rev: tree.rev,
        path,
        born: tree.born,
        entries,
        truncated: tree.truncated,
    })
}

pub(crate) fn blob_view(reply: serde_json::Value, repo: String) -> Result<BlobView, String> {
    let blob = reply
        .get("blob")
        .filter(|blob| !blob.is_null())
        .ok_or_else(|| "the requested file was not found".to_string())?;
    let blob: forge::BlobReply =
        serde_json::from_value(blob.clone()).map_err(|error| error.to_string())?;
    let lines = match blob.binary {
        true => 0,
        false => count_i64(blob.text.lines().count()),
    };
    Ok(BlobView {
        repo,
        rev: blob.rev,
        path: blob.path,
        text: blob.text,
        truncated: blob.truncated,
        binary: blob.binary,
        lines,
    })
}

/// List one repo directory at one pinned revision on the node. Opening Code
/// transfers only this bounded listing; the merge-only mirror stays cold.
pub async fn forge_tree(
    rpc: String,
    repo: String,
    rev: String,
    path: String,
) -> Result<ForgeTreeData, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let query = serde_json::json!({ "tree": {
            "repo": &repo,
            "rev": &rev,
            "path": &path,
        }});
        let reply = client.query("forge", &query).await?;
        tree_data(reply, repo, path)
    }
    .await
    .map_err(app_error)
}

/// Read one bounded file preview at the tree's exact revision on the node.
pub async fn forge_blob(
    rpc: String,
    repo: String,
    rev: String,
    path: String,
) -> Result<BlobView, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let query = serde_json::json!({ "blob": {
            "repo": &repo,
            "rev": &rev,
            "path": &path,
        }});
        let reply = client.query("forge", &query).await?;
        blob_view(reply, repo)
    }
    .await
    .map_err(app_error)
}

/// True when one live update invalidates forge state: a folded forge op, a
/// forge replay the stream could not fold (`resync`), or the stream (re)
/// subscribing (`ready` — anything may have landed while it was down).
pub fn forge_live_hit(kind: crate::LiveKind, module: String) -> bool {
    match kind {
        crate::LiveKind::Forge | crate::LiveKind::Ready => true,
        crate::LiveKind::Resync => module == "forge",
        crate::LiveKind::Retry
        | crate::LiveKind::Tip
        | crate::LiveKind::Chat
        | crate::LiveKind::Bell
        | crate::LiveKind::Pages
        | crate::LiveKind::Plane => false,
    }
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
/// `forge_open` is the repo LIST's surface gate. That one load is the only
/// unscoped slice here, and no other tab draws it. The repo and item slices
/// keep running off-tab on purpose: they are already scoped to what the forge
/// pane has open, and
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
    kind: crate::LiveKind,
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
        item: item_slice.unwrap_or(noop.item),
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

/// The forge code reader's row metrics. One place on purpose: the shape lint
/// in `app/src/tests.rs` pins these against `DiffRow`'s Ice metrics so the
/// source and patch surfaces cannot drift apart.
pub const CODE_SIZE: f32 = 11.5;
pub const CODE_ROW_HEIGHT: f32 = 20.0;
pub const CODE_GUTTER_WIDTH: f32 = 44.0;

/// The highlighter's language token: the path's final extension, else the
/// file name itself lowercased (Makefile, Dockerfile). syntect matches both
/// and falls back to plain text on an unknown token — an unknown file renders
/// exactly as the single-ink viewer used to.
pub fn code_token(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => name.to_ascii_lowercase(),
    }
}

/// The syntect theme per appearance. Only token FOREGROUNDS are taken from
/// it — the plate and gutter stay the app's own rail tokens, so the reader
/// keeps one surface even where the theme's background would disagree.
pub fn code_theme(dark: bool) -> iced::highlighter::Theme {
    match dark {
        true => iced::highlighter::Theme::Base16Eighties,
        false => iced::highlighter::Theme::InspiredGitHub,
    }
}

/// The forge blob reader: numbered gutter + syntect-highlighted code, one
/// paragraph each at the same row pitch, and the code one drag-selectable
/// across lines ([`CodeSelect`]). Replaces the Ice `ForgeCodeLine` loop —
/// token colour needs per-span inks, which Ice's named-token text nodes
/// cannot carry, so the whole surface renders here (the `agent_markdown`
/// idiom). Colours are the app palette's stable code roles (`rail`,
/// `forge_gutter_ink`, `strong_ink` in `theme.ice`), matched per appearance
/// like `AgentMarkdown`.
///
/// EAGER ON PURPOSE. This extern once wrapped its surface in a raw
/// `iced::widget::lazy` — the app's ONLY use of iced's own Lazy, a boundary
/// nothing else in the codebase exercises — and the shipped pane drew nothing
/// for every code blob while the same tree passed the headless probes. The
/// memo boundary lives at the Ice mount now (`lazy … by` in
/// screens/forge.ice), the projection idiom every other cached surface here
/// already uses, so the tokenize + paragraph build still runs only when the blob,
/// path, or appearance moves. `BlobView.text` is read-capped at 64 KiB
/// upstream, which bounds the one-time build; the screen's scroll pane owns
/// scrolling.
pub fn forge_code(source: String, path: String, dark: bool) -> iced::Element<'static, ()> {
    code_surface(&source, &path, dark)
}

fn code_surface(source: &str, path: &str, dark: bool) -> iced::Element<'static, ()> {
    use iced::Length;
    use iced::advanced::text::{LineHeight, Span};
    use iced::alignment::Horizontal;
    use iced::highlighter::{Settings, Stream};
    use iced::widget::{container, row, text};

    let (rail, gutter_ink, plain_ink) = match dark {
        true => (
            iced::Color::from_rgb8(0x20, 0x1f, 0x1b),
            iced::Color::from_rgb8(0x9d, 0x9b, 0x92),
            iced::Color::from_rgb8(0xdc, 0xda, 0xd2),
        ),
        false => (
            iced::Color::from_rgb8(0xfa, 0xfa, 0xf8),
            iced::Color::from_rgb8(0x66, 0x64, 0x5e),
            iced::Color::from_rgb8(0x3a, 0x39, 0x34),
        ),
    };
    // An empty blob must say so: zero rows is a zero-height, invisible
    // surface, and a pane that renders nothing is unreportable.
    if source.is_empty() {
        return container(
            text("This file is empty.")
                .size(CODE_SIZE)
                .font(CODE_FONT)
                .color(gutter_ink),
        )
        .padding(iced::Padding::ZERO.left(13.0))
        .into();
    }
    let mut stream = Stream::new(&Settings {
        theme: code_theme(dark),
        token: code_token(path),
    });
    // The gutter is one numbers paragraph at the plate's row pitch; the plate
    // is one drag-selectable paragraph of the highlighted lines.
    let lines: Vec<Vec<Span<'static, (), iced::Font>>> = source
        .lines()
        .map(|line| {
            let spans = stream
                .highlight_line(line)
                .map(|(range, highlight)| {
                    Span::new(line[range].to_string()).color(highlight.color().unwrap_or(plain_ink))
                })
                .collect();
            stream.commit();
            spans
        })
        .collect();
    let numbers = (1..=lines.len())
        .map(|number| number.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let code = CodeSelect::new(lines, READER_METRICS, Some(plain_ink), Length::Fill);
    let gutter = container(
        text(numbers)
            .size(CODE_SIZE)
            .font(CODE_FONT)
            .color(gutter_ink)
            .line_height(LineHeight::Absolute(CODE_ROW_HEIGHT.into()))
            .width(Length::Fill)
            .align_x(Horizontal::Right),
    )
    .width(CODE_GUTTER_WIDTH)
    .padding(iced::Padding::ZERO.right(12.0))
    .style(move |_| iced::widget::container::Style {
        background: Some(rail.into()),
        ..Default::default()
    });
    let code = container(code)
        .width(Length::Fill)
        .padding(iced::Padding::ZERO.left(13.0))
        .clip(true);
    row![gutter, code].width(Length::Fill).into()
}

const CODE_FONT: iced::Font = iced::Font::with_name("Geist Mono");

/// The renderer's paragraph by its concrete name: the plate needs the
/// cosmic-text buffer under it, because `Paragraph::hit_test` answers with
/// the byte index INSIDE the hit line and drops the line — right for one
/// line of text, wrong for a file of them.
type CodeParagraph = iced::advanced::graphics::text::Paragraph;

/// One selectable run of text, the state every selectable surface here
/// shares: a paragraph laid out like the drawn one (for the code plate it IS
/// the drawn one), the window's selection token, the drag, and the quads.
#[derive(Default)]
struct SelectState {
    key: u64,
    paragraph: CodeParagraph,
    token: u64,
    anchor: usize,
    cursor: usize,
    dragging: bool,
    /// The highlight quads for `anchor..cursor`, paragraph-relative. Refreshed
    /// where the selection or the bounds move (`update`, `layout`) and never
    /// in `draw`, so a scroll with a selection open costs nothing extra.
    highlight: Vec<iced::Rectangle>,
}

impl SelectState {
    fn is_active(&self) -> bool {
        ui_lang_runtime::selection::holds(self.token)
    }

    fn range(&self, text: &SelectText) -> Option<std::ops::Range<usize>> {
        if !self.is_active() {
            return None;
        }
        let start = self.anchor.min(self.cursor);
        let end = self.anchor.max(self.cursor);
        (start != end && text.content.get(start..end).is_some()).then_some(start..end)
    }

    fn selected<'a>(&self, text: &'a SelectText) -> Option<&'a str> {
        text.content.get(self.range(text)?)
    }

    /// The byte offset under a paragraph-relative point. cosmic-text already
    /// lands a point above the first line on it, one below the last on its
    /// end, and one past a line's right edge on that line's end — the clamps
    /// a drag needs.
    fn hit(&self, point: iced::Point, text: &SelectText) -> Option<usize> {
        let cursor = self.paragraph.buffer().hit(point.x, point.y)?;
        let line_start = text.line_starts.get(cursor.line).copied()?;
        Some(line_start + cursor.index)
    }

    /// One quad per laid-out line the selection touches, read off its
    /// glyphs: the run of the first glyph that ends inside the selection
    /// through the last that starts inside it. A line the selection only
    /// passes the newline of (an empty line, or a start exactly at a line's
    /// end) has no such glyphs and draws nothing.
    fn reselect(&mut self, text: &SelectText) {
        self.highlight.clear();
        let Some(range) = self.range(text) else {
            return;
        };
        for run in self.paragraph.buffer().layout_runs() {
            let Some(&line_start) = text.line_starts.get(run.line_i) else {
                continue;
            };
            let low = range.start.saturating_sub(line_start);
            let high = range.end.saturating_sub(line_start);
            let first = run.glyphs.iter().find(|glyph| glyph.end > low);
            let last = run.glyphs.iter().rev().find(|glyph| glyph.start < high);
            let (Some(first), Some(last)) = (first, last) else {
                continue;
            };
            let right = last.x + last.w;
            if right <= first.x {
                continue;
            }
            self.highlight.push(iced::Rectangle::new(
                iced::Point::new(first.x, run.line_top),
                iced::Size::new(right - first.x, run.line_height),
            ));
        }
    }

    /// A (re)laid-out paragraph: a new key drops the old text's selection,
    /// whose offsets meant other text; the same key keeps it and refreshes
    /// the quads where the bounds moved.
    fn relayout<Link>(
        &mut self,
        key: u64,
        text: iced::advanced::text::Text<
            &[iced::advanced::text::Span<'_, Link, iced::Font>],
            iced::Font,
        >,
        select: &SelectText,
    ) {
        use iced::advanced::text::{Difference, Paragraph as _, Text};
        let other_text = self.key != key;
        if other_text {
            self.paragraph = CodeParagraph::with_spans(text);
            self.key = key;
            self.token = 0;
            self.dragging = false;
            self.highlight.clear();
            return;
        }
        let probe = Text {
            content: (),
            bounds: text.bounds,
            size: text.size,
            line_height: text.line_height,
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
        };
        match self.paragraph.compare(probe) {
            Difference::None => {}
            Difference::Bounds => {
                self.paragraph.resize(text.bounds);
                self.reselect(select);
            }
            Difference::Shape => {
                self.paragraph = CodeParagraph::with_spans(text);
                self.reselect(select);
            }
        }
    }

    /// The selection's own input: press starts a drag and takes the window's
    /// selection, the drag moves the cursor, Ctrl+A takes everything, Ctrl+C
    /// copies, Escape lets go.
    fn update<Message>(
        &mut self,
        text: &SelectText,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
    ) {
        use iced::advanced::clipboard;
        use iced::keyboard;
        use iced::mouse;
        use ui_lang_runtime::selection;
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_in(layout.bounds()) else {
                    return;
                };
                let Some(offset) = self.hit(position, text) else {
                    return;
                };
                self.token = selection::claim();
                self.anchor = offset;
                self.cursor = offset;
                self.dragging = true;
                self.reselect(text);
                shell.request_redraw();
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if self.dragging => {
                let Some(position) = cursor.position_from(layout.position()) else {
                    return;
                };
                let Some(offset) = self.hit(position, text) else {
                    return;
                };
                if self.cursor == offset {
                    return;
                }
                self.cursor = offset;
                self.reselect(text);
                shell.request_redraw();
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.dragging =>
            {
                self.dragging = false;
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) if self.is_active() && modifiers.command() => match key.to_latin(*physical_key) {
                Some('a') => {
                    self.anchor = 0;
                    self.cursor = text.content.len();
                    self.reselect(text);
                    shell.capture_event();
                    shell.request_redraw();
                }
                Some('c') => {
                    if let Some(selected) = self.selected(text) {
                        clipboard.write(clipboard::Kind::Standard, selected.to_owned());
                        shell.capture_event();
                    }
                }
                _ => {}
            },
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if self.is_active() => {
                selection::clear();
                self.dragging = false;
                self.highlight.clear();
                shell.request_redraw();
            }
            _ => {}
        }
    }

    /// The highlight wash under the text, clipped to what is on screen.
    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        bounds: iced::Rectangle,
        clip: iced::Rectangle,
        ink: iced::Color,
    ) {
        use iced::advanced::Renderer as _;
        if !self.is_active() {
            return;
        }
        let translation = bounds.position() - iced::Point::ORIGIN;
        let wash = ink.scale_alpha(0.28);
        for quad in &self.highlight {
            let Some(quad) = (*quad + translation).intersection(&clip) else {
                continue;
            };
            renderer.fill_quad(
                iced::advanced::renderer::Quad {
                    bounds: quad,
                    ..Default::default()
                },
                wash,
            );
        }
    }
}

/// The text a selection indexes: the exact string the spans spell, byte
/// for byte, and the byte offset of each line's first character — what
/// turns the buffer's (line, index) cursor into one offset and back.
struct SelectText {
    content: String,
    line_starts: Vec<usize>,
}

impl SelectText {
    fn new(content: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(content.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        Self {
            content,
            line_starts,
        }
    }
}

/// The text layout a code plate draws with: a pinned size and row pitch in
/// the code font, no wrapping.
#[derive(Clone, Copy)]
pub struct CodeMetrics {
    pub size: iced::Pixels,
    pub line_height: iced::advanced::text::LineHeight,
    pub font: iced::Font,
}

/// The reader's metrics, pinned to `DiffRow`'s by the shape lint.
const READER_METRICS: CodeMetrics = CodeMetrics {
    size: iced::Pixels(CODE_SIZE),
    line_height: iced::advanced::text::LineHeight::Absolute(iced::Pixels(CODE_ROW_HEIGHT)),
    font: CODE_FONT,
};

fn code_text<C>(
    content: C,
    bounds: iced::Size,
    metrics: CodeMetrics,
) -> iced::advanced::text::Text<C, iced::Font> {
    use iced::advanced::text::{Alignment, Shaping, Text, Wrapping};
    Text {
        content,
        bounds,
        size: metrics.size,
        line_height: metrics.line_height,
        font: metrics.font,
        align_x: Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::None,
    }
}

/// A code plate: highlighted lines as one paragraph that can be dragged
/// across, like every plain Ice `text` in the app (ducktape-ui wraps those in
/// `selectable_text`; this is the same contract for per-span inks, which
/// that wrapper's plain `Text` cannot carry). It takes the window's
/// selection through `ui_lang_runtime::selection`, so a drag here quiets any
/// other highlight and vice versa; Ctrl+A takes the file, Ctrl+C copies,
/// Escape lets go. The forge reader and Markdown code blocks both draw it.
pub struct CodeSelect {
    /// Identity of the lines and their inks: a new key rebuilds the
    /// paragraph and drops the old text's selection.
    key: u64,
    text: SelectText,
    spans: Vec<iced::advanced::text::Span<'static, (), iced::Font>>,
    /// The ink for spans without one; `None` takes the theme's text colour.
    ink: Option<iced::Color>,
    metrics: CodeMetrics,
    width: iced::Length,
}

impl CodeSelect {
    /// One plate from per-line spans, newline spans between the lines, so a
    /// drag runs across rows and a copy carries the line breaks. `content` is
    /// the exact text the spans spell, byte for byte: the selection's
    /// offsets index it.
    pub fn new<'a, Link>(
        lines: impl IntoIterator<Item = impl AsRef<[iced::advanced::text::Span<'a, Link, iced::Font>]>>,
        metrics: CodeMetrics,
        ink: Option<iced::Color>,
        width: iced::Length,
    ) -> Self {
        use iced::advanced::text::Span;
        use std::hash::{Hash as _, Hasher as _};
        let mut content = String::new();
        let mut spans = Vec::new();
        let mut hasher = std::hash::DefaultHasher::new();
        for (index, line) in lines.into_iter().enumerate() {
            if index > 0 {
                content.push('\n');
                spans.push(Span::new("\n"));
            }
            for span in line.as_ref() {
                content.push_str(&span.text);
                span.color
                    .map(|color| [color.r, color.g, color.b, color.a].map(f32::to_bits))
                    .hash(&mut hasher);
                spans.push(
                    Span::new(span.text.to_string())
                        .color_maybe(span.color)
                        .font_maybe(span.font),
                );
            }
        }
        content.hash(&mut hasher);
        Self {
            key: hasher.finish(),
            text: SelectText::new(content),
            spans,
            ink,
            metrics,
            width,
        }
    }
}

impl<Message> iced::advanced::Widget<Message, iced::Theme, iced::Renderer> for CodeSelect {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<SelectState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(SelectState::default())
    }

    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size::new(self.width, iced::Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        use iced::advanced::text::Paragraph as _;
        let state = tree.state.downcast_mut::<SelectState>();
        iced::advanced::layout::sized(limits, self.width, iced::Length::Shrink, |limits| {
            let text = code_text(self.spans.as_slice(), limits.max(), self.metrics);
            state.relayout(self.key, text, &self.text);
            state.paragraph.min_bounds()
        })
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        _renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SelectState>();
        state.update(&self.text, event, layout, cursor, clipboard, shell);
    }

    fn mouse_interaction(
        &self,
        _tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        _viewport: &iced::Rectangle,
        _renderer: &iced::Renderer,
    ) -> iced::mouse::Interaction {
        match cursor.is_over(layout.bounds()) {
            true => iced::mouse::Interaction::Text,
            false => iced::mouse::Interaction::default(),
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        use iced::advanced::text::Renderer as _;
        let bounds = layout.bounds();
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        let ink = self.ink.unwrap_or(style.text_color);
        let state = tree.state.downcast_ref::<SelectState>();
        state.draw(renderer, bounds, clip, ink);
        renderer.fill_paragraph(&state.paragraph, bounds.position(), ink, clip);
    }
}

impl<'a, Message: 'a> From<CodeSelect> for iced::Element<'a, Message> {
    fn from(code: CodeSelect) -> Self {
        Self::new(code)
    }
}

/// A rich text run that can be dragged across — iced's own `Rich` draws it
/// (inline-code plates, link inks, link clicks all stay its), and a shadow
/// paragraph laid out with the same spans, size, font and bounds answers
/// where the glyphs are. One run is one selection: a drag stops at the
/// block's edge, exactly as it does on the app's plain Ice `text`.
pub struct SelectRich<'a, Message> {
    key: u64,
    child: iced::Element<'a, Message>,
    text: SelectText,
    spans: std::sync::Arc<[iced::advanced::text::Span<'static, String, iced::Font>]>,
    size: iced::Pixels,
}

impl<'a, Message: 'a> SelectRich<'a, Message> {
    /// `rich` must be the `Rich` built from `spans` at `size` in the
    /// renderer's default font — the shadow paragraph mirrors those.
    pub fn new(
        rich: iced::widget::text::Rich<'a, String, Message>,
        spans: std::sync::Arc<[iced::advanced::text::Span<'static, String, iced::Font>]>,
        size: iced::Pixels,
    ) -> Self {
        use std::hash::{Hash as _, Hasher as _};
        let content: String = spans.iter().map(|span| span.text.as_ref()).collect();
        let mut hasher = std::hash::DefaultHasher::new();
        (&content, size.0.to_bits()).hash(&mut hasher);
        Self {
            key: hasher.finish(),
            child: rich.into(),
            text: SelectText::new(content),
            spans,
            size,
        }
    }
}

impl<Message> iced::advanced::Widget<Message, iced::Theme, iced::Renderer>
    for SelectRich<'_, Message>
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<SelectState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(SelectState::default())
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.children[0].diff(&self.child);
    }

    fn size(&self) -> iced::Size<iced::Length> {
        self.child.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        self.child.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        use iced::advanced::text::{Alignment, LineHeight, Renderer as _, Shaping, Text, Wrapping};
        let node = self
            .child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        // The same `Text` `Rich::layout` lays its spans out with, bounds
        // included — it reads `limits.max()` before sizing, as this does.
        let text = Text {
            content: self.spans.as_ref(),
            bounds: limits.max(),
            size: self.size,
            line_height: LineHeight::default(),
            font: renderer.default_font(),
            align_x: Alignment::Default,
            align_y: iced::alignment::Vertical::Top,
            shaping: Shaping::Advanced,
            wrapping: Wrapping::default(),
        };
        let state = tree.state.downcast_mut::<SelectState>();
        state.relayout(self.key, text, &self.text);
        node
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.child
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let state = tree.state.downcast_mut::<SelectState>();
        state.update(&self.text, event, layout, cursor, clipboard, shell);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::mouse::Interaction {
        let own = self.child.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );
        let over_a_link = own != iced::mouse::Interaction::default();
        match (over_a_link, cursor.is_over(layout.bounds())) {
            (true, _) => own,
            (false, true) => iced::mouse::Interaction::Text,
            (false, false) => own,
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        if let Some(clip) = bounds.intersection(viewport) {
            let state = tree.state.downcast_ref::<SelectState>();
            state.draw(renderer, bounds, clip, style.text_color);
        }
        self.child.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.child.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<SelectRich<'a, Message>> for iced::Element<'a, Message> {
    fn from(rich: SelectRich<'a, Message>) -> Self {
        Self::new(rich)
    }
}

/// Whether a tree path names a Markdown document the reader renders as a
/// document rather than line-numbers. Extension-based on purpose: the wire's
/// `binary` flag only separates text from bytes, and forge carries no
/// language field — the path is the one discriminator the app holds.
pub fn markdown_path(path: String) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
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

/// The reader header's path, gated on the directory AND revision the file
/// was opened under: a preview opened in another directory or an older
/// commit was retired by that move, so the header must not keep naming it.
/// (Another repository is another component instance — the call site keys
/// on the repo, so cross-repo staleness cannot arise.)
pub fn forge_file_header(
    opened_dir: String,
    opened_rev: String,
    dir: String,
    rev: String,
    path: String,
) -> String {
    let same_place = opened_dir == dir;
    let same_commit = opened_rev == rev;
    if same_place && same_commit { path } else { String::new() }
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
pub fn filter_forge_items(items: Vec<ForgeItem>, tab: crate::ForgeTab) -> Vec<ForgeItem> {
    let kind = match tab {
        crate::ForgeTab::Code => return Vec::new(),
        crate::ForgeTab::Pulls => "pr",
        crate::ForgeTab::Issues => "issue",
    };
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
pub fn verdict_pick_label(
    current: crate::ForgeReviewVerdict,
    key: crate::ForgeReviewVerdict,
    label: String,
) -> String {
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
