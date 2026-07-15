//! Native service edge for the transport-free Forge and Agents screens.
//!
//! Forge reads committed Git objects from the active workspace (or a bounded
//! bare mirror for a direct remote connection). Consensus reads/writes use the
//! same module wire shapes as the former TypeScript domain clients. Every
//! user-authored write is submitted as an account-signed frame.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{StreamExt as _, stream};
use git2::{
    BranchType, Buf, Commit as GitCommit, Delta, DiffOptions, ErrorCode, FetchOptions, FetchPrune,
    ObjectType, Oid, Patch, Repository as GitRepository, Signature, Sort, Tree as GitTree,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::backend::{Backend, ContentTarget, Workspace};
use crate::screens::{agents, forge};
use crate::transport::NodeClient;

const DEV_REF: &str = "refs/heads/dev";
const MAIN_REF: &str = "refs/heads/main";
const FILE_PAGE_BYTES: usize = 64 * 1024;
const COMMIT_PAGE: usize = 50;
const MAX_REPOSITORIES: usize = 512;
const MAX_BRANCHES: usize = 2_048;
const MAX_TREE_ENTRIES: usize = 16_384;
const MAX_ITEMS: usize = 1_024;
const MAX_AGENTS: usize = 4_096;
const MAX_CHANNELS: usize = 4_096;
const MAX_WATCHES: usize = 4_096;
const MAX_PENDING_RUNS: usize = 4_096;
const MAX_RECENT_RUNS: usize = 100;
const MAX_USAGE_ROWS: usize = 16_384;
const MAX_CHANGED_FILES: usize = 2_048;
const MAX_REVIEWS: usize = 256;
const MAX_REVIEW_COMMENTS: usize = 64;
const MAX_REVIEW_COMMENT_BYTES: usize = 16 * 1024;
const MAX_PATCH_BYTES: usize = 512 * 1024;
const MAX_TOTAL_PATCH_BYTES: usize = 4 * 1024 * 1024;
const MAX_MERGE_PACK_BYTES: usize = 4 * 1024 * 1024;
const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_TITLE_BYTES: usize = 512;
const MAX_BODY_BYTES: usize = 16 * 1024;
const MAX_NAME_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_CAPABILITY_BYTES: usize = 256;
const MAX_RECORD_BYTES: usize = 4 * 1024;
const MAX_SKILLS: usize = 64;
const MAX_CAP_ENTRIES: usize = 256;
const MAX_CAP_ENTRY_BYTES: usize = 1024;
const MAX_DISCUSSION_MESSAGES: u64 = 256;
const KNOWN_ACTIONS: [&str; 7] = [
    "chat.post",
    "chat.post_message",
    "tasks.create",
    "tasks.update_status",
    "pages.comment",
    "pages.set_checked",
    "duckfs.write_text",
];

#[derive(Debug, Clone)]
struct ForgeLocation {
    state_root: PathBuf,
    workspace_id: Option<String>,
    remote_origin: Option<String>,
}

#[derive(Debug)]
struct RepoMeta {
    name: String,
    branch: String,
    head: Option<String>,
}

#[derive(Debug)]
struct CommitInfo {
    id: String,
    summary: String,
    author: String,
    time: i64,
}

#[derive(Debug)]
struct TreeInfo {
    name: String,
    directory: bool,
}

#[derive(Debug)]
struct FilePageInfo {
    text: String,
    next_offset: Option<usize>,
    total_bytes: usize,
}

#[derive(Debug)]
struct CompareInfo {
    files: Vec<forge::ChangedFile>,
    commits: Vec<CommitInfo>,
}

#[derive(Debug)]
struct MergeInfo {
    merge_oid: Option<String>,
    pack: Option<Vec<u8>>,
    conflicts: Vec<String>,
}

/// Execute one Forge effect. Presentation state never sees a transport or a
/// repository handle.
pub async fn execute_forge(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    node: Option<NodeClient>,
    command: forge::Command,
) -> forge::ServiceEvent {
    use forge::{Command, ServiceEvent};

    match command {
        Command::LoadRepositories => ServiceEvent::RepositoriesLoaded(
            load_repositories(backend.as_ref(), workspace.as_ref(), node.as_ref()).await,
        ),
        Command::LoadRepository {
            repository_id,
            reference,
        } => ServiceEvent::RepositoryLoaded(
            load_repository(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                repository_id,
                reference,
            )
            .await,
        ),
        Command::LoadDirectory {
            repository_id,
            path,
            reference,
        } => {
            let event_path = path.clone();
            ServiceEvent::DirectoryLoaded {
                path: event_path,
                result: load_directory(
                    backend.as_ref(),
                    workspace.as_ref(),
                    node.as_ref(),
                    repository_id,
                    path,
                    reference,
                )
                .await,
            }
        }
        Command::LoadFile {
            repository_id,
            path,
            reference,
            offset,
        } => ServiceEvent::FileLoaded(
            load_file(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                repository_id,
                path,
                reference,
                offset,
            )
            .await,
        ),
        Command::LoadMoreCommits {
            repository_id,
            reference,
            after,
        } => ServiceEvent::MoreCommitsLoaded(
            load_more_commits(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                repository_id,
                reference,
                after,
            )
            .await,
        ),
        Command::OpenIssue {
            repository_id,
            title,
            body,
        } => ServiceEvent::WriteFinished(
            open_issue(backend.as_ref(), node.as_ref(), repository_id, title, body).await,
        ),
        Command::OpenPullRequest {
            repository_id,
            title,
            body,
            source_branch,
            target_branch,
        } => ServiceEvent::WriteFinished(
            open_pull_request(
                backend.as_ref(),
                node.as_ref(),
                repository_id,
                title,
                body,
                source_branch,
                target_branch,
            )
            .await,
        ),
        Command::LoadItem {
            repository_id,
            number,
        } => ServiceEvent::ItemLoaded(
            load_item(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                repository_id,
                number,
            )
            .await,
        ),
        Command::EditItem {
            repository_id,
            number,
            title,
            body,
        } => ServiceEvent::WriteFinished(
            edit_item(
                backend.as_ref(),
                node.as_ref(),
                repository_id,
                number,
                title,
                body,
            )
            .await,
        ),
        Command::SetItemState {
            repository_id,
            number,
            open,
        } => ServiceEvent::WriteFinished(
            set_item_state(backend.as_ref(), node.as_ref(), repository_id, number, open).await,
        ),
        Command::MergePullRequest {
            repository_id,
            number,
        } => ServiceEvent::WriteFinished(
            merge_pull_request(
                backend.as_ref(),
                workspace.as_ref(),
                node.as_ref(),
                repository_id,
                number,
            )
            .await,
        ),
        Command::SubmitReview {
            repository_id,
            number,
            verdict,
            body,
            commit_oid,
            comments,
        } => ServiceEvent::WriteFinished(
            submit_review(
                backend.as_ref(),
                node.as_ref(),
                repository_id,
                number,
                verdict,
                body,
                commit_oid,
                comments,
            )
            .await,
        ),
        Command::AddComment {
            repository_id,
            number,
            body,
        } => ServiceEvent::WriteFinished(
            add_comment(backend.as_ref(), node.as_ref(), repository_id, number, body).await,
        ),
    }
}

/// Execute one Agents effect with the exact `agent`, `runs`, `chat`,
/// `capability`, `dispatch`, and `saga` read contracts.
pub async fn execute_agents(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    command: agents::Command,
) -> agents::ServiceEvent {
    use agents::{Command, ServiceEvent};

    match command {
        Command::Load => ServiceEvent::Loaded(load_agents(backend.as_ref(), node.as_ref()).await),
        Command::RefreshCapabilities => {
            ServiceEvent::CapabilitiesLoaded(load_capabilities(node.as_ref()).await)
        }
        Command::RegisterAgent {
            display_name,
            agent_id,
            capability,
            allowed_actions,
            caps,
            skills,
        } => ServiceEvent::WriteFinished(
            register_agent(
                backend.as_ref(),
                node.as_ref(),
                display_name,
                agent_id,
                capability,
                allowed_actions,
                caps,
                skills,
            )
            .await,
        ),
        Command::UpdateAgent {
            agent_id,
            display_name,
            capability,
            allowed_actions,
            caps,
            skills,
        } => ServiceEvent::WriteFinished(
            update_agent(
                backend.as_ref(),
                node.as_ref(),
                agent_id,
                display_name,
                capability,
                allowed_actions,
                caps,
                skills,
            )
            .await,
        ),
        Command::PauseAgent(agent_id) => ServiceEvent::WriteFinished(
            agent_status(backend.as_ref(), node.as_ref(), agent_id, false).await,
        ),
        Command::ResumeAgent(agent_id) => ServiceEvent::WriteFinished(
            agent_status(backend.as_ref(), node.as_ref(), agent_id, true).await,
        ),
        Command::WatchChannel { channel_id, policy } => ServiceEvent::WriteFinished(
            watch_channel(backend.as_ref(), node.as_ref(), channel_id, policy).await,
        ),
        Command::UnwatchChannel(channel_id) => ServiceEvent::WriteFinished(
            unwatch_channel(backend.as_ref(), node.as_ref(), channel_id).await,
        ),
        Command::SetJobWorker(enabled) => ServiceEvent::WriteFinished(
            set_job_worker(backend.as_ref(), node.as_ref(), enabled).await,
        ),
        Command::CancelRun(run_id) => {
            ServiceEvent::WriteFinished(cancel_run(backend.as_ref(), node.as_ref(), run_id).await)
        }
        Command::ReassignRun { run_id, attempt } => ServiceEvent::WriteFinished(
            reassign_run(backend.as_ref(), node.as_ref(), run_id, attempt).await,
        ),
    }
}

fn forge_location(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
) -> Result<ForgeLocation, String> {
    let state_root = backend
        .ok_or_else(|| "desktop backend is unavailable".to_string())?
        .state_root();
    match workspace {
        Some(workspace) => Ok(ForgeLocation {
            state_root,
            workspace_id: Some(workspace.id.clone()),
            remote_origin: None,
        }),
        None => Ok(ForgeLocation {
            state_root,
            workspace_id: None,
            remote_origin: Some(
                node.ok_or_else(|| "enter a network to use Forge".to_string())?
                    .origin(),
            ),
        }),
    }
}

async fn blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| format!("native repository task failed: {error}"))?
}

/// One reconnecting WebSocket for every expanded run log. The subscription
/// identity includes the topic set, so collapsing the last pane drops the
/// receiver and cancels the transport task.
pub fn run_output_subscription(
    origin: String,
    mut dispatch_ids: Vec<String>,
) -> iced::Subscription<agents::RunLogEvent> {
    dispatch_ids.retain(|dispatch_id| validate_digest(dispatch_id).is_ok());
    dispatch_ids.sort();
    dispatch_ids.dedup();
    if dispatch_ids.is_empty() {
        return iced::Subscription::none();
    }
    iced::Subscription::run_with((origin, dispatch_ids), run_output_stream)
}

fn run_output_stream(
    source: &(String, Vec<String>),
) -> impl iced::futures::Stream<Item = agents::RunLogEvent> + use<> {
    use iced::futures::SinkExt as _;

    let (origin, dispatch_ids) = source.clone();
    iced::stream::channel(64, async move |mut output| {
        let client = match NodeClient::new(&origin) {
            Ok(client) => client,
            Err(error) => {
                let _ = output
                    .send(agents::RunLogEvent::Unavailable {
                        dispatch_id: None,
                        reason: error.to_string(),
                    })
                    .await;
                return;
            }
        };
        let topics = dispatch_ids
            .iter()
            .map(|dispatch_id| format!("run-output:{dispatch_id}"))
            .collect();
        let mut source = match client.subscribe(topics, BTreeMap::new()) {
            Ok(source) => source,
            Err(error) => {
                let _ = output
                    .send(agents::RunLogEvent::Unavailable {
                        dispatch_id: None,
                        reason: error.to_string(),
                    })
                    .await;
                return;
            }
        };
        while let Some(event) = source.recv().await {
            let event = match event {
                crate::transport::StreamEvent::Connected => agents::RunLogEvent::Connected,
                crate::transport::StreamEvent::Disconnected(reason) => {
                    agents::RunLogEvent::Disconnected(reason)
                }
                crate::transport::StreamEvent::Frame(frame) => match frame {
                    crate::transport::ServerFrame::Tail {
                        topic,
                        cursor,
                        item,
                    } => {
                        let Some(dispatch_id) = topic.strip_prefix("run-output:") else {
                            continue;
                        };
                        let Ok(cursor) = cursor.parse::<u64>() else {
                            continue;
                        };
                        let Some(stream) = item.get("stream").and_then(Value::as_str) else {
                            continue;
                        };
                        let stream = match stream {
                            "stdout" => agents::RunStream::Stdout,
                            "stderr" => agents::RunStream::Stderr,
                            _ => continue,
                        };
                        let Some(text) = item.get("line").and_then(Value::as_str) else {
                            continue;
                        };
                        agents::RunLogEvent::Line {
                            dispatch_id: dispatch_id.to_owned(),
                            cursor,
                            stream,
                            text: text.to_owned(),
                        }
                    }
                    crate::transport::ServerFrame::Lagged { topic, cursor } => {
                        let Some(dispatch_id) = topic.strip_prefix("run-output:") else {
                            continue;
                        };
                        let Ok(cursor) = cursor.parse::<u64>() else {
                            continue;
                        };
                        agents::RunLogEvent::Lagged {
                            dispatch_id: dispatch_id.to_owned(),
                            cursor,
                        }
                    }
                    crate::transport::ServerFrame::Error { topic, detail, .. } => {
                        agents::RunLogEvent::Unavailable {
                            dispatch_id: topic.strip_prefix("run-output:").map(str::to_owned),
                            reason: detail,
                        }
                    }
                    _ => continue,
                },
            };
            if output.send(event).await.is_err() {
                return;
            }
        }
    })
}

async fn load_repositories(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
) -> Result<Vec<forge::Repository>, String> {
    if workspace.is_none() {
        let reply = query(node, "forge", Value::String("list_repos".into())).await?;
        let rows = variant_array(&reply, "repos", MAX_REPOSITORIES)?;
        return rows
            .iter()
            .map(|row| {
                let name = bounded_string(row, "name", MAX_NAME_BYTES)?;
                clean_repo_name(&name)?;
                let head = optional_string(row, "head", 40)?;
                validate_optional_oid(head.as_deref(), "repository head")?;
                Ok(forge::Repository {
                    id: name.clone(),
                    name,
                    default_branch: "dev".into(),
                    browsable: head.is_some(),
                    head,
                })
            })
            .collect();
    }

    let location = forge_location(backend, workspace, node)?;
    let repos = blocking(move || list_local_repos(&location)).await?;
    if repos.len() > MAX_REPOSITORIES {
        return Err(format!(
            "Forge returned too many repositories ({} > {MAX_REPOSITORIES})",
            repos.len()
        ));
    }
    Ok(repos
        .into_iter()
        .map(|repo| forge::Repository {
            id: repo.name.clone(),
            name: repo.name,
            default_branch: repo.branch,
            browsable: repo.head.is_some(),
            head: repo.head,
        })
        .collect())
}

async fn prepare_remote(location: &ForgeLocation, repository: &str) -> Result<(), String> {
    if location.remote_origin.is_none() {
        return Ok(());
    }
    let location = location.clone();
    let repository = repository.to_owned();
    blocking(move || sync_remote_mirror(&location, &repository).map(|_| ())).await
}

async fn load_repository(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    reference: Option<String>,
) -> Result<forge::RepositoryData, String> {
    clean_repo_name(&repository)?;
    validate_optional_reference(reference.as_deref())?;
    let location = forge_location(backend, workspace, node)?;
    prepare_remote(&location, &repository).await?;
    let git_location = location.clone();
    let git_repository = repository.clone();
    let git_reference = reference.clone();
    let git = blocking(move || {
        let repo = require_named_repo(&git_location, &git_repository)?;
        let branches = list_branches(&repo)?;
        let tree = read_tree(&repo, "", git_reference.as_deref())?;
        let commits = read_log(&repo, Some(COMMIT_PAGE + 1), git_reference.as_deref(), None)?;
        Ok((branches, tree, commits))
    });
    let items = load_forge_items(node, &repository);
    let ((branches, tree, mut commits), items) = tokio::try_join!(git, items)?;
    let commits_have_more = commits.len() > COMMIT_PAGE;
    commits.truncate(COMMIT_PAGE);
    Ok(forge::RepositoryData {
        branches: branches
            .into_iter()
            .map(|(name, head)| forge::Branch { name, head })
            .collect(),
        tree: tree_entries("", tree, 0),
        commits: commits.into_iter().map(commit_view).collect(),
        commits_have_more,
        items,
    })
}

async fn load_directory(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    path: String,
    reference: Option<String>,
) -> Result<Vec<forge::TreeEntry>, String> {
    clean_repo_name(&repository)?;
    let path = clean_repo_path(&path, true)?;
    validate_optional_reference(reference.as_deref())?;
    let location = forge_location(backend, workspace, node)?;
    let depth = path.split('/').filter(|part| !part.is_empty()).count();
    blocking(move || {
        let repo = require_named_repo(&location, &repository)?;
        read_tree(&repo, &path, reference.as_deref())
            .map(|entries| tree_entries(&path, entries, depth))
    })
    .await
}

async fn load_file(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    path: String,
    reference: Option<String>,
    offset: u64,
) -> Result<forge::FilePage, String> {
    clean_repo_name(&repository)?;
    let path = clean_repo_path(&path, false)?;
    validate_optional_reference(reference.as_deref())?;
    let offset = usize::try_from(offset).map_err(|_| "file offset is too large".to_string())?;
    let location = forge_location(backend, workspace, node)?;
    let view_path = path.clone();
    let page = blocking(move || {
        let repo = require_named_repo(&location, &repository)?;
        read_file_page(&repo, &path, reference.as_deref(), offset, FILE_PAGE_BYTES)?
            .ok_or_else(|| format!("file {path:?} does not exist at this reference"))
    })
    .await?;
    let loaded = page.next_offset.unwrap_or(page.total_bytes);
    Ok(forge::FilePage {
        path: view_path,
        text: page.text,
        loaded_bytes: loaded as u64,
        total_bytes: page.total_bytes as u64,
        has_more: page.next_offset.is_some(),
    })
}

async fn load_more_commits(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    reference: Option<String>,
    after: String,
) -> Result<(Vec<forge::Commit>, bool), String> {
    clean_repo_name(&repository)?;
    validate_optional_reference(reference.as_deref())?;
    validate_oid(&after, "commit cursor")?;
    let location = forge_location(backend, workspace, node)?;
    let mut commits = blocking(move || {
        let repo = require_named_repo(&location, &repository)?;
        read_log(
            &repo,
            Some(COMMIT_PAGE + 1),
            reference.as_deref(),
            Some(&after),
        )
    })
    .await?;
    let more = commits.len() > COMMIT_PAGE;
    commits.truncate(COMMIT_PAGE);
    Ok((commits.into_iter().map(commit_view).collect(), more))
}

async fn load_forge_items(
    node: Option<&NodeClient>,
    repository: &str,
) -> Result<Vec<forge::ForgeItem>, String> {
    let reply = query(
        node,
        "forge",
        json!({ "list_items": { "repo": repository } }),
    )
    .await?;
    variant_array(&reply, "items", MAX_ITEMS)?
        .iter()
        .map(parse_forge_item)
        .collect()
}

async fn load_item(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
) -> Result<Option<forge::ItemDetail>, String> {
    clean_repo_name(&repository)?;
    if number == 0 {
        return Err("Forge item number must be positive".into());
    }
    let reply = query(
        node,
        "forge",
        json!({ "get_item": { "repo": repository, "number": number } }),
    )
    .await?;
    let Some(value) = reply.get("item") else {
        return Err("node returned an invalid item reply".into());
    };
    if value.is_null() {
        return Ok(None);
    }
    let item = parse_forge_item(value)?;
    let body = bounded_string(value, "body", MAX_BODY_BYTES)?;
    let channel_id = bounded_string(value, "channel_id", MAX_NAME_BYTES)?;
    let expected_channel = format!("forge:{repository}:{number}");
    if channel_id != expected_channel {
        return Err("Forge item returned an unexpected discussion channel".into());
    }
    let reviews = parse_reviews(value)?;

    let comments_future = load_discussion(node, &channel_id);
    let compare_future = async {
        if item.kind != forge::ItemKind::PullRequest {
            return Ok(None);
        }
        let source = item
            .source_branch
            .as_deref()
            .ok_or_else(|| "pull request is missing its source branch".to_string())?;
        let target = item
            .target_branch
            .as_deref()
            .ok_or_else(|| "pull request is missing its target branch".to_string())?;
        validate_reference(source)?;
        validate_reference(target)?;
        let location = forge_location(backend, workspace, node)?;
        let repository = repository.clone();
        let source = source.to_owned();
        let target = target.to_owned();
        blocking(move || {
            let repo = require_named_repo(&location, &repository)?;
            compare(&repo, &target, &source).map(Some)
        })
        .await
    };
    let (comments, compared) = tokio::join!(comments_future, compare_future);
    let comments = comments?;
    let (commits, changed_files, compare_error) = match compared {
        Ok(Some(compare)) => (
            compare.commits.into_iter().map(commit_view).collect(),
            compare.files,
            None,
        ),
        Ok(None) => (Vec::new(), Vec::new(), None),
        Err(error) => (Vec::new(), Vec::new(), Some(error)),
    };
    let can_edit = current_identity_key(backend)
        .await?
        .is_some_and(|key| author_external_hex(value.get("author")) == Some(key));
    Ok(Some(forge::ItemDetail {
        item,
        body,
        can_edit,
        comments,
        commits,
        changed_files,
        compare_error,
        reviews,
    }))
}

fn parse_reviews(item: &Value) -> Result<Vec<forge::Review>, String> {
    let rows = item
        .get("reviews")
        .and_then(Value::as_array)
        .ok_or_else(|| "Forge item returned an invalid reviews list".to_string())?;
    if rows.len() > MAX_REVIEWS {
        return Err("Forge item returned too many reviews".into());
    }
    rows.iter()
        .map(|review| {
            let verdict = match review.get("verdict").and_then(Value::as_str) {
                Some("approve") => forge::ReviewVerdict::Approve,
                Some("request_changes") => forge::ReviewVerdict::RequestChanges,
                Some("comment") => forge::ReviewVerdict::Comment,
                _ => return Err("Forge item returned an invalid review verdict".into()),
            };
            let commit_oid = bounded_string(review, "commit_oid", 40)?;
            validate_oid(&commit_oid, "review commit")?;
            let comments = review
                .get("comments")
                .and_then(Value::as_array)
                .ok_or_else(|| "Forge item returned invalid review comments".to_string())?;
            if comments.len() > MAX_REVIEW_COMMENTS {
                return Err("Forge item returned too many review comments".into());
            }
            let comments = comments
                .iter()
                .map(|comment| {
                    let line = required_u64(comment, "line")?;
                    let line = u32::try_from(line)
                        .map_err(|_| "review comment line exceeds u32".to_string())?;
                    if line == 0 {
                        return Err("review comment line must be positive".into());
                    }
                    let side = match comment.get("side").and_then(Value::as_str) {
                        Some("old") => forge::ReviewSide::Old,
                        Some("new") => forge::ReviewSide::New,
                        _ => return Err("Forge item returned an invalid review side".into()),
                    };
                    Ok(forge::ReviewComment {
                        path: bounded_string(comment, "path", MAX_PATH_BYTES)?,
                        line,
                        side,
                        body: bounded_string(comment, "body", MAX_REVIEW_COMMENT_BYTES)?,
                    })
                })
                .collect::<Result<_, String>>()?;
            Ok(forge::Review {
                author: author_name(review.get("author"))?,
                verdict,
                body: bounded_string(review, "body", MAX_BODY_BYTES)?,
                commit_oid,
                comments,
                created_at: format_stamp(required_u64(review, "created_at")?),
            })
        })
        .collect()
}

async fn load_discussion(
    node: Option<&NodeClient>,
    channel_id: &str,
) -> Result<Vec<forge::DiscussionPost>, String> {
    let reply = query(
        node,
        "chat",
        json!({
            "messages_latest": {
                "channel_id": channel_id,
                "limit": MAX_DISCUSSION_MESSAGES
            }
        }),
    )
    .await?;
    variant_array(&reply, "messages", MAX_DISCUSSION_MESSAGES as usize)?
        .iter()
        .filter(|message| {
            !message
                .get("head")
                .and_then(|head| head.get("deleted"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|message| {
            let head = message
                .get("head")
                .and_then(Value::as_object)
                .ok_or_else(|| "chat returned an invalid discussion message".to_string())?;
            let blocks = head
                .get("blocks")
                .and_then(Value::as_array)
                .ok_or_else(|| "chat returned invalid discussion blocks".to_string())?;
            let body = chat_blocks_text(blocks)?;
            if body.len() > MAX_BODY_BYTES {
                return Err("discussion message exceeds the desktop display limit".into());
            }
            Ok(forge::DiscussionPost {
                author: author_name(head.get("author"))?,
                body,
                time: format_stamp(
                    head.get("created_at")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| "discussion message is missing created_at".to_string())?,
                ),
            })
        })
        .collect()
}

async fn open_issue(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    title: String,
    body: String,
) -> Result<(), String> {
    validate_item_input(&repository, &title, &body)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({ "open_issue": { "repo": repository, "title": title.trim(), "body": body } }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn open_pull_request(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    title: String,
    body: String,
    source_branch: String,
    target_branch: String,
) -> Result<(), String> {
    validate_item_input(&repository, &title, &body)?;
    validate_reference(&source_branch)?;
    validate_reference(&target_branch)?;
    if source_branch == target_branch {
        return Err("pull request source and target branches must differ".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({
            "open_pr": {
                "repo": repository,
                "title": title.trim(),
                "body": body,
                "source_branch": source_branch,
                "target_branch": target_branch
            }
        }),
    )
    .await
}

async fn edit_item(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
    title: Option<String>,
    body: Option<String>,
) -> Result<(), String> {
    clean_repo_name(&repository)?;
    if number == 0 || title.is_none() && body.is_none() {
        return Err("Forge item edit must change a positive-numbered item".into());
    }
    let title = title
        .map(|title| -> Result<String, String> {
            let title = title.trim().to_owned();
            if title.is_empty() || title.len() > MAX_TITLE_BYTES {
                return Err("Forge item title must be non-empty and at most 512 bytes".into());
            }
            Ok(title)
        })
        .transpose()?;
    if body
        .as_ref()
        .is_some_and(|body| body.len() > MAX_BODY_BYTES)
    {
        return Err("Forge item body exceeds the 16 KiB limit".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({
            "edit_item": {
                "repo": repository,
                "number": number,
                "title": title,
                "body": body
            }
        }),
    )
    .await
}

async fn set_item_state(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
    open: bool,
) -> Result<(), String> {
    clean_repo_name(&repository)?;
    if number == 0 {
        return Err("Forge item number must be positive".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({ "set_item_state": { "repo": repository, "number": number, "open": open } }),
    )
    .await
}

async fn submit_review(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
    verdict: forge::ReviewVerdict,
    body: String,
    commit_oid: String,
    comments: Vec<forge::ReviewComment>,
) -> Result<(), String> {
    clean_repo_name(&repository)?;
    let body = body.trim();
    if number == 0 || body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err("review summary must be non-empty and at most 16 KiB".into());
    }
    validate_oid(&commit_oid, "review commit")?;
    let comments = review_comments_payload(comments)?;
    let verdict = match verdict {
        forge::ReviewVerdict::Approve => "approve",
        forge::ReviewVerdict::RequestChanges => "request_changes",
        forge::ReviewVerdict::Comment => "comment",
    };
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({
            "submit_review": {
                "repo": repository,
                "number": number,
                "verdict": verdict,
                "body": body,
                "commit_oid": commit_oid,
                "comments": comments
            }
        }),
    )
    .await
}

fn review_comments_payload(comments: Vec<forge::ReviewComment>) -> Result<Vec<Value>, String> {
    if comments.len() > MAX_REVIEW_COMMENTS {
        return Err(format!(
            "a review can contain at most {MAX_REVIEW_COMMENTS} inline comments"
        ));
    }
    comments
        .into_iter()
        .map(|comment| {
            let path = clean_repo_path(&comment.path, false)?;
            let body = comment.body.trim();
            if comment.line == 0 || body.is_empty() || body.len() > MAX_REVIEW_COMMENT_BYTES {
                return Err(
                    "inline comments need a positive line and at most 16 KiB of text".to_string(),
                );
            }
            Ok(json!({
                "path": path,
                "line": comment.line,
                "side": match comment.side {
                    forge::ReviewSide::Old => "old",
                    forge::ReviewSide::New => "new",
                },
                "body": body,
            }))
        })
        .collect()
}

async fn merge_pull_request(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
) -> Result<(), String> {
    clean_repo_name(&repository)?;
    if number == 0 {
        return Err("pull request number must be positive".into());
    }
    let reply = query(
        node,
        "forge",
        json!({ "get_item": { "repo": repository, "number": number } }),
    )
    .await?;
    let detail = reply
        .get("item")
        .filter(|item| !item.is_null())
        .ok_or_else(|| format!("pull request #{number} no longer exists"))?;
    if detail.get("kind").and_then(Value::as_str) != Some("pr") {
        return Err(format!("Forge item #{number} is not a pull request"));
    }
    if detail.get("state").and_then(Value::as_str) != Some("open") {
        return Err(format!("pull request #{number} is not open"));
    }
    let source = bounded_string(detail, "source_branch", MAX_NAME_BYTES)?;
    let target = bounded_string(detail, "target_branch", MAX_NAME_BYTES)?;
    validate_reference(&source)?;
    validate_reference(&target)?;
    let location = forge_location(backend, workspace, node)?;
    prepare_remote(&location, &repository).await?;
    let build_repository = repository.clone();
    let build_source = source.clone();
    let build_target = target.clone();
    let message = format!("Merge pull request #{number} from {source}");
    let build = blocking(move || {
        let repo = require_named_repo(&location, &build_repository)?;
        let ours = require_ref_spec(&repo, &build_target)?;
        let theirs = require_ref_spec(&repo, &build_source)?;
        let result = build_merge(&repo, ours, theirs, &message)?;
        Ok((ours.to_string(), theirs.to_string(), result))
    })
    .await?;
    let (prev_target_oid, expected_source_oid, merge) = build;
    if !merge.conflicts.is_empty() {
        return Err(format!("merge conflicts in {}", merge.conflicts.join(", ")));
    }
    let merge_oid = merge
        .merge_oid
        .ok_or_else(|| "merge did not produce a commit".to_string())?;
    let pack = merge
        .pack
        .ok_or_else(|| "merge did not produce an object pack".to_string())?;
    if pack.is_empty() || pack.len() > MAX_MERGE_PACK_BYTES {
        return Err(format!(
            "merge object pack is outside the 1..={MAX_MERGE_PACK_BYTES} byte limit"
        ));
    }
    let client = node.ok_or_else(|| "enter a network to merge a pull request".to_string())?;
    let pack_digest = client
        .put_blob(pack)
        .await
        .map_err(|error| error.to_string())?
        .to_ascii_lowercase();
    validate_digest(&pack_digest)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Forge,
        json!({
            "merge_pr": {
                "repo": repository,
                "number": number,
                "prev_target_oid": prev_target_oid,
                "expected_source_oid": expected_source_oid,
                "merge_oid": merge_oid,
                "pack_digest": pack_digest
            }
        }),
    )
    .await
}

async fn add_comment(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    repository: String,
    number: u64,
    body: String,
) -> Result<(), String> {
    clean_repo_name(&repository)?;
    let body = body.trim();
    if number == 0 || body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err("comment must be non-empty and at most 16 KiB".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Chat,
        json!({
            "post_message": {
                "channel_id": format!("forge:{repository}:{number}"),
                "message_id": fresh_id("message"),
                "blocks": [{ "paragraph": [{ "text": body, "marks": [] }] }],
                "thread": null,
                "as_agent": null
            }
        }),
    )
    .await
}

fn validate_item_input(repository: &str, title: &str, body: &str) -> Result<(), String> {
    clean_repo_name(repository)?;
    let title = title.trim();
    if title.is_empty() || title.len() > MAX_TITLE_BYTES {
        return Err("Forge item title must be non-empty and at most 512 bytes".into());
    }
    if body.len() > MAX_BODY_BYTES {
        return Err("Forge item body exceeds the 16 KiB limit".into());
    }
    Ok(())
}

async fn load_agents(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
) -> Result<Option<agents::AgentData>, String> {
    let Some(client) = node else {
        return Ok(None);
    };
    let agents_query = client.query("agent", Value::String("agents".into()));
    let capabilities_query = client.query("capability", Value::String("all".into()));
    let channels_query = client.query("chat", Value::String("channels".into()));
    let watches_query = client.query("runs", Value::String("watches".into()));
    let pending_query = client.query("runs", Value::String("pending_runs".into()));
    let recent_query = client.query("runs", Value::String("recent_runs".into()));
    let usage_query = client.view("saga", json!({ "usage": {} }));
    let status_query = client.status();
    let (
        agents_reply,
        capabilities_reply,
        channels_reply,
        watches_reply,
        pending_reply,
        recent_reply,
        usage,
        status,
    ) = tokio::join!(
        agents_query,
        capabilities_query,
        channels_query,
        watches_query,
        pending_query,
        recent_query,
        usage_query,
        status_query,
    );
    let agents_reply = agents_reply.map_err(|error| error.to_string())?;
    let channels_reply = channels_reply.map_err(|error| error.to_string())?;
    let watches_reply = watches_reply.map_err(|error| error.to_string())?;
    let pending_reply = pending_reply.map_err(|error| error.to_string())?;
    let current_height = status.map_err(|error| error.to_string())?.height;
    // The usage index is node-local derived state and may be absent on an old
    // node. Keep that optional read quiet, matching the former UsageCard.
    let usage = usage
        .ok()
        .map(|reply| parse_usage(&reply))
        .transpose()?
        .filter(|usage| usage.requests != 0);
    let identity_key = current_identity_key(backend).await?;
    let (recent_runs, recent_runs_error) = match recent_reply {
        Ok(reply) => match parse_recent_runs(&reply) {
            Ok(runs) => (runs, None),
            Err(error) => (Vec::new(), Some(error)),
        },
        Err(error) => (Vec::new(), Some(error.to_string())),
    };

    let agent_rows = variant_array(&agents_reply, "agents", MAX_AGENTS)?;
    let roster: Vec<_> = agent_rows
        .iter()
        .map(parse_agent_record)
        .collect::<Result<_, _>>()?;
    let (capabilities, capability_status) = match capabilities_reply {
        Ok(reply) => match parse_capabilities(&reply) {
            Ok(capabilities) => (capabilities, agents::CapabilityStatus::Ready),
            Err(_) => (Vec::new(), agents::CapabilityStatus::Error),
        },
        Err(_) => (Vec::new(), agents::CapabilityStatus::Error),
    };
    let channels = parse_channels(&channels_reply)?;
    let watches = parse_watches(&watches_reply)?;
    let pending_rows = variant_array(&pending_reply, "pending_runs", MAX_PENDING_RUNS)?;
    let mut parsed_pending: Vec<_> = pending_rows
        .iter()
        .map(parse_pending_wire)
        .collect::<Result<_, _>>()?;
    parsed_pending.sort_by_key(|pending| std::cmp::Reverse(pending.created_at));
    let dispatch_ids = parsed_pending
        .iter()
        .map(|pending| pending.dispatch_id.clone())
        .collect::<Vec<_>>();
    let leases = stream::iter(dispatch_ids)
        .map(|dispatch_id| {
            let client = client.clone();
            async move {
                let reply = client
                    .query(
                        "dispatch",
                        json!({
                            "dispatch": {
                                "receiver": "runs",
                                "dispatch_id": dispatch_id
                            }
                        }),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                parse_dispatch_lease(&reply)
            }
        })
        .buffered(16)
        .collect::<Vec<_>>()
        .await;
    let mut pending_runs = Vec::with_capacity(parsed_pending.len());
    for (wire, lease) in parsed_pending.into_iter().zip(leases) {
        let lease = lease.unwrap_or_default();
        pending_runs.push(agents::PendingRun {
            run_id: wire.run_id,
            dispatch_id: wire.dispatch_id,
            agent_id: wire.agent_id,
            channel_id: wire.channel_id,
            anchor_sequence: wire.anchor_sequence,
            job_id: wire.job_id,
            created_at: format_stamp(wire.created_at),
            requested_by_me: identity_key
                .as_ref()
                .is_some_and(|key| wire.requester_external.as_ref() == Some(key)),
            attempt: lease.attempt,
            lease_remaining: lease
                .expires_at
                .map(|expires| expires.saturating_sub(current_height)),
            pending: false,
        });
    }
    Ok(Some(agents::AgentData {
        agents: roster,
        capabilities,
        capability_status,
        channels,
        watches,
        pending_runs,
        recent_runs,
        recent_runs_error,
        usage,
        job_worker_pending: false,
    }))
}

async fn load_capabilities(node: Option<&NodeClient>) -> Result<Vec<String>, String> {
    let reply = query(node, "capability", Value::String("all".into())).await?;
    parse_capabilities(&reply)
}

#[allow(clippy::too_many_arguments)]
async fn register_agent(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    display_name: String,
    agent_id: String,
    capability: String,
    allowed_actions: Vec<String>,
    caps: agents::ResourceCaps,
    skills: Vec<agents::SkillRef>,
) -> Result<(), String> {
    validate_agent_record(
        &agent_id,
        &display_name,
        &capability,
        &allowed_actions,
        &caps,
        &skills,
    )?;
    submit_signed(
        backend,
        node,
        ContentTarget::Agent,
        json!({
            "register_agent": {
                "agent_id": agent_id,
                "display_name": display_name.trim(),
                "capability": capability.trim(),
                "allowed_actions": canonical_actions(allowed_actions)?,
                "caps": caps_value(&caps)?,
                "skills": skills_value(&skills)?
            }
        }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_agent(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    agent_id: String,
    display_name: String,
    capability: String,
    allowed_actions: Vec<String>,
    caps: agents::ResourceCaps,
    skills: Vec<agents::SkillRef>,
) -> Result<(), String> {
    validate_agent_record(
        &agent_id,
        &display_name,
        &capability,
        &allowed_actions,
        &caps,
        &skills,
    )?;
    submit_signed(
        backend,
        node,
        ContentTarget::Agent,
        json!({
            "update_agent": {
                "agent_id": agent_id,
                "display_name": display_name.trim(),
                "capability": capability.trim(),
                "allowed_actions": canonical_actions(allowed_actions)?,
                "caps": caps_value(&caps)?,
                "skills": skills_value(&skills)?
            }
        }),
    )
    .await
}

async fn agent_status(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    agent_id: String,
    active: bool,
) -> Result<(), String> {
    validate_agent_id(&agent_id)?;
    let payload = if active {
        json!({ "resume_agent": { "agent_id": agent_id } })
    } else {
        json!({ "pause_agent": { "agent_id": agent_id } })
    };
    submit_signed(backend, node, ContentTarget::Agent, payload).await
}

async fn watch_channel(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    channel_id: String,
    policy: agents::TurnPolicy,
) -> Result<(), String> {
    validate_channel_id(&channel_id)?;
    let policy = match policy {
        agents::TurnPolicy::Mention => Value::String("mention".into()),
        agents::TurnPolicy::All => Value::String("all".into()),
        agents::TurnPolicy::RoundRobin => Value::String("round_robin".into()),
        agents::TurnPolicy::Assigned(agent_id) => {
            validate_agent_id(&agent_id)?;
            json!({ "assigned": agent_id })
        }
    };
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "watch_channel": { "channel_id": channel_id, "policy": policy } }),
    )
    .await
}

async fn unwatch_channel(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    channel_id: String,
) -> Result<(), String> {
    validate_channel_id(&channel_id)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "unwatch_channel": { "channel_id": channel_id } }),
    )
    .await
}

async fn set_job_worker(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    enabled: bool,
) -> Result<(), String> {
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "enable_job_worker": { "enabled": enabled } }),
    )
    .await
}

async fn cancel_run(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    run_id: String,
) -> Result<(), String> {
    validate_run_id(&run_id)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "cancel_run": { "run_id": run_id } }),
    )
    .await
}

async fn reassign_run(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    run_id: String,
    attempt: u32,
) -> Result<(), String> {
    validate_run_id(&run_id)?;
    if attempt > 32 {
        return Err("run attempt is outside the supported bound".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "reassign_run": { "run_id": run_id, "attempt": attempt } }),
    )
    .await
}

async fn query(node: Option<&NodeClient>, target: &str, value: Value) -> Result<Value, String> {
    node.ok_or_else(|| format!("enter a network to use {target}"))?
        .query(target, value)
        .await
        .map_err(|error| error.to_string())
}

async fn submit_signed(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    target: ContentTarget,
    payload: Value,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop identity backend is unavailable".to_string())?;
    let client = node.ok_or_else(|| format!("enter a network to use {}", target.as_str()))?;
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("could not encode {} write: {error}", target.as_str()))?;
    if bytes.is_empty() || bytes.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{} write exceeds the signed-frame payload limit",
            target.as_str()
        ));
    }
    let frame = backend
        .sign_content_frame(target, hex_encode(&bytes))
        .await?;
    client
        .submit_frame(hex_decode(&frame)?)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn current_identity_key(backend: Option<&Backend>) -> Result<Option<String>, String> {
    match backend {
        Some(backend) => backend.identity_state().await.map(|state| state.pubkey),
        None => Ok(None),
    }
}

#[derive(Debug)]
struct PendingWire {
    run_id: String,
    dispatch_id: String,
    agent_id: String,
    channel_id: String,
    anchor_sequence: u64,
    job_id: Option<String>,
    created_at: u64,
    requester_external: Option<String>,
}

#[derive(Debug, Default)]
struct DispatchLease {
    attempt: u32,
    expires_at: Option<u64>,
}

fn parse_agent_record(value: &Value) -> Result<agents::AgentRecord, String> {
    let id = bounded_string(value, "agent_id", 63)?;
    validate_agent_id(&id)?;
    let display_name = bounded_string(value, "display_name", MAX_NAME_BYTES)?;
    let capability = bounded_string(value, "capability", MAX_CAPABILITY_BYTES)?;
    let allowed_actions = string_array(value, "allowed_actions", KNOWN_ACTIONS.len(), 64)?;
    let allowed_actions = canonical_actions(allowed_actions)?;
    let status = match value.get("status").and_then(Value::as_str) {
        Some("active") => agents::AgentStatus::Active,
        Some("paused") => agents::AgentStatus::Paused,
        _ => return Err("agent returned an invalid status".into()),
    };
    let created_at = required_u64(value, "created_at")?;
    let updated_at = required_u64(value, "updated_at")?;
    let caps = value
        .get("caps")
        .filter(|caps| !caps.is_null())
        .map(parse_caps)
        .transpose()?
        .unwrap_or_default();
    let skills = value
        .get("skills")
        .filter(|skills| !skills.is_null())
        .map(parse_skills)
        .transpose()?
        .unwrap_or_default();
    Ok(agents::AgentRecord {
        id,
        owner: parse_owner(
            value
                .get("owner")
                .ok_or_else(|| "agent is missing its owner".to_string())?,
        )?,
        display_name,
        capability,
        allowed_actions,
        status,
        created_at: format_stamp(created_at),
        updated_at: format_stamp(updated_at),
        caps,
        skills,
        pending: false,
    })
}

fn parse_owner(value: &Value) -> Result<agents::Owner, String> {
    if value.as_str() == Some("system") {
        return Ok(agents::Owner::System);
    }
    if let Some(bytes) = value.get("external") {
        return Ok(agents::Owner::External(bytes_hex(bytes)?));
    }
    if let Some(module) = value.get("module").and_then(Value::as_str) {
        validate_bounded_text(module, "owner module", MAX_NAME_BYTES, false)?;
        return Ok(agents::Owner::Module(module.to_owned()));
    }
    Err("agent returned an invalid owner".into())
}

fn parse_caps(value: &Value) -> Result<agents::ResourceCaps, String> {
    Ok(agents::ResourceCaps {
        forge_read: optional_string_array(value, "forge_read")?,
        forge_push: optional_string_array(value, "forge_push")?,
        duckfs_read: optional_string_array(value, "duckfs_read")?,
        duckfs_write: optional_string_array(value, "duckfs_write")?,
        tools: optional_string_array(value, "tools")?,
        secrets: optional_string_array(value, "secrets")?,
        pages_write: optional_string_array(value, "pages_write")?,
        subagent_budget: value
            .get("subagent_budget")
            .and_then(Value::as_u64)
            .map(|value| {
                u32::try_from(value).map_err(|_| "subagent budget exceeds u32".to_string())
            })
            .transpose()?,
    })
}

fn parse_skills(value: &Value) -> Result<Vec<agents::SkillRef>, String> {
    let rows = value
        .as_array()
        .ok_or_else(|| "agent returned an invalid skills list".to_string())?;
    if rows.len() > MAX_SKILLS {
        return Err("agent returned too many skills".into());
    }
    rows.iter()
        .map(|skill| {
            let name = bounded_string(skill, "name", MAX_NAME_BYTES)?;
            let source_prefix = bounded_string(skill, "source_prefix", MAX_CAP_ENTRY_BYTES)?;
            if name.is_empty() || source_prefix.is_empty() {
                return Err("skill name and source prefix must be non-empty".into());
            }
            let snapshot = optional_string(skill, "source_snapshot", MAX_NAME_BYTES)?;
            if snapshot.as_deref() == Some("") {
                return Err("skill snapshot must be non-empty when present".into());
            }
            let load = match skill.get("load").and_then(Value::as_str) {
                None | Some("on_demand") => agents::LoadMode::OnDemand,
                Some("always") => agents::LoadMode::Always,
                _ => return Err("agent returned an invalid skill load mode".into()),
            };
            Ok(agents::SkillRef {
                name,
                source_prefix,
                snapshot,
                load,
            })
        })
        .collect()
}

fn parse_capabilities(reply: &Value) -> Result<Vec<String>, String> {
    let rows = variant_array(reply, "all", MAX_AGENTS)?;
    let mut tags = BTreeSet::new();
    for row in rows {
        let pair = row
            .as_array()
            .filter(|row| row.len() == 2)
            .ok_or_else(|| "capability registry returned an invalid row".to_string())?;
        let _ = bytes_hex(&pair[0])?;
        let announced = pair[1]
            .as_array()
            .ok_or_else(|| "capability registry returned invalid tags".to_string())?;
        if announced.len() > MAX_CAP_ENTRIES {
            return Err("capability registry row exceeds the desktop limit".into());
        }
        for tag in announced {
            let tag = tag
                .as_str()
                .ok_or_else(|| "capability registry tag is not text".to_string())?;
            validate_bounded_text(tag, "capability", MAX_CAPABILITY_BYTES, false)?;
            tags.insert(tag.to_owned());
        }
    }
    Ok(tags.into_iter().collect())
}

fn parse_channels(reply: &Value) -> Result<Vec<agents::Channel>, String> {
    variant_array(reply, "channels", MAX_CHANNELS)?
        .iter()
        .filter(|channel| {
            !channel
                .get("archived")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && !channel
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.contains(':'))
        })
        .map(|channel| {
            let id = bounded_string(channel, "id", MAX_NAME_BYTES)?;
            validate_channel_id(&id)?;
            Ok(agents::Channel {
                id,
                name: bounded_string(channel, "name", MAX_NAME_BYTES)?,
            })
        })
        .collect()
}

fn parse_watches(reply: &Value) -> Result<Vec<agents::Watch>, String> {
    variant_array(reply, "watches", MAX_WATCHES)?
        .iter()
        .map(|watch| {
            let channel_id = bounded_string(watch, "channel_id", MAX_NAME_BYTES)?;
            validate_channel_id(&channel_id)?;
            let policy = watch
                .get("policy")
                .ok_or_else(|| "runs returned a watch without a policy".to_string())?;
            let policy = match policy.as_str() {
                Some("mention") => agents::TurnPolicy::Mention,
                Some("all") => agents::TurnPolicy::All,
                Some("round_robin") => agents::TurnPolicy::RoundRobin,
                _ => {
                    let assigned = policy
                        .get("assigned")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "runs returned an invalid watch policy".to_string())?;
                    validate_agent_id(assigned)?;
                    agents::TurnPolicy::Assigned(assigned.to_owned())
                }
            };
            Ok(agents::Watch {
                channel_id,
                policy,
                pending: false,
            })
        })
        .collect()
}

fn parse_pending_wire(value: &Value) -> Result<PendingWire, String> {
    let run_id = bounded_string(value, "run_id", MAX_NAME_BYTES)?;
    validate_run_id(&run_id)?;
    let dispatch_id = bounded_string(value, "dispatch_id", 64)?;
    validate_digest(&dispatch_id)?;
    let agent_id = bounded_string(value, "agent_id", 63)?;
    validate_agent_id(&agent_id)?;
    let channel_id = bounded_string(value, "channel_id", MAX_NAME_BYTES)?;
    if !channel_id.is_empty() {
        validate_channel_id(&channel_id)?;
    }
    let requester_external = value
        .get("requester")
        .and_then(|requester| requester.get("external"))
        .map(bytes_hex)
        .transpose()?;
    Ok(PendingWire {
        run_id,
        dispatch_id,
        agent_id,
        channel_id,
        anchor_sequence: required_u64(value, "anchor_seq")?,
        job_id: optional_string(value, "job_id", MAX_NAME_BYTES)?,
        created_at: required_u64(value, "created_at")?,
        requester_external,
    })
}

fn parse_recent_runs(reply: &Value) -> Result<Vec<agents::RunRecord>, String> {
    variant_array(reply, "recent_runs", MAX_RECENT_RUNS)?
        .iter()
        .map(|record| {
            let run_id = bounded_string(record, "run_id", MAX_NAME_BYTES)?;
            validate_run_id(&run_id)?;
            let agent_id = bounded_string(record, "agent_id", 63)?;
            validate_agent_id(&agent_id)?;
            let channel_id = bounded_string(record, "channel_id", MAX_NAME_BYTES)?;
            if !channel_id.is_empty() {
                validate_channel_id(&channel_id)?;
            }
            let outcome = match record.get("outcome").and_then(Value::as_str) {
                Some("delivered") => agents::RunOutcome::Delivered,
                Some("failed") => agents::RunOutcome::Failed,
                _ => return Err("runs returned an invalid delivered-run outcome".into()),
            };
            let executing_node = bounded_string(record, "executing_node", 64)?;
            if executing_node != "unknown" {
                validate_digest(&executing_node)?;
            }
            let pr_number = record.get("pr_number").and_then(Value::as_u64);
            if pr_number == Some(0) {
                return Err("runs returned an invalid pull request number".into());
            }
            Ok(agents::RunRecord {
                dispatch_id: hex_encode(&Sha256::digest(run_id.as_bytes())),
                run_id,
                agent_id,
                channel_id,
                anchor_sequence: required_u64(record, "anchor_seq")?,
                outcome,
                degraded: record
                    .get("degraded")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| "runs returned a record without degraded state".to_string())?,
                created_at: required_u64(record, "created_at")?,
                delivered_at: required_u64(record, "delivered_at")?,
                executing_node,
                output_ref: optional_string(record, "output_ref", MAX_NAME_BYTES)?,
                pr_number,
            })
        })
        .collect()
}

fn parse_dispatch_lease(reply: &Value) -> Result<DispatchLease, String> {
    let Some(dispatch) = reply.get("dispatch") else {
        return Err("dispatch returned an invalid reply".into());
    };
    if dispatch.is_null() {
        return Ok(DispatchLease::default());
    }
    Ok(DispatchLease {
        attempt: dispatch
            .get("attempt")
            .and_then(Value::as_u64)
            .map(|attempt| {
                u32::try_from(attempt).map_err(|_| "dispatch attempt exceeds u32".to_string())
            })
            .transpose()?
            .unwrap_or(0),
        expires_at: dispatch.get("lease_expires_at").and_then(Value::as_u64),
    })
}

fn parse_usage(reply: &Value) -> Result<agents::Usage, String> {
    let rows = variant_array(reply, "usage", MAX_USAGE_ROWS)?;
    let mut usage = agents::Usage {
        requests: 0,
        failed: 0,
        duration_blocks: 0,
        input_tokens: 0,
        output_tokens: 0,
    };
    for row in rows {
        let runs = required_u64(row, "runs")?;
        usage.requests = usage.requests.saturating_add(runs);
        if !row
            .get("outcomeOk")
            .and_then(Value::as_bool)
            .ok_or_else(|| "usage row is missing outcomeOk".to_string())?
        {
            usage.failed = usage.failed.saturating_add(runs);
        }
        usage.duration_blocks = usage
            .duration_blocks
            .saturating_add(required_u64(row, "totalDurationBlocks")?);
        usage.input_tokens = usage
            .input_tokens
            .saturating_add(required_u64(row, "inputTokens")?);
        usage.output_tokens = usage
            .output_tokens
            .saturating_add(required_u64(row, "outputTokens")?);
    }
    Ok(usage)
}

fn validate_agent_record(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    actions: &[String],
    caps: &agents::ResourceCaps,
    skills: &[agents::SkillRef],
) -> Result<(), String> {
    validate_agent_id(agent_id)?;
    validate_bounded_text(
        display_name.trim(),
        "agent display name",
        MAX_NAME_BYTES,
        false,
    )?;
    validate_bounded_text(
        capability.trim(),
        "agent capability",
        MAX_CAPABILITY_BYTES,
        false,
    )?;
    let actions = canonical_actions(actions.to_vec())?;
    if actions.is_empty() {
        return Err("an agent needs at least one allowed action".into());
    }
    let caps = caps_value(caps)?;
    let skills = skills_value(skills)?;
    let record = json!({
        "agent_id": agent_id,
        "display_name": display_name.trim(),
        "capability": capability.trim(),
        "allowed_actions": actions,
        "caps": caps,
        "skills": skills
    });
    if serde_json::to_vec(&record)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_RECORD_BYTES
    {
        return Err("agent record exceeds the 4 KiB consensus limit".into());
    }
    Ok(())
}

fn canonical_actions(actions: Vec<String>) -> Result<Vec<String>, String> {
    let mut actions = actions;
    if actions.len() > KNOWN_ACTIONS.len() {
        return Err("agent has too many allowed actions".into());
    }
    for action in &actions {
        if !KNOWN_ACTIONS.contains(&action.as_str()) {
            return Err(format!("unknown agent action: {action}"));
        }
    }
    actions.sort();
    actions.dedup();
    Ok(actions)
}

fn caps_value(caps: &agents::ResourceCaps) -> Result<Value, String> {
    let mut object = serde_json::Map::new();
    for (key, values) in [
        ("forge_read", &caps.forge_read),
        ("forge_push", &caps.forge_push),
        ("duckfs_read", &caps.duckfs_read),
        ("duckfs_write", &caps.duckfs_write),
        ("tools", &caps.tools),
        ("secrets", &caps.secrets),
        ("pages_write", &caps.pages_write),
    ] {
        if values.len() > MAX_CAP_ENTRIES {
            return Err(format!("{key} contains too many grants"));
        }
        let mut canonical = BTreeSet::new();
        for value in values {
            validate_bounded_text(value, key, MAX_CAP_ENTRY_BYTES, false)?;
            canonical.insert(value.clone());
        }
        if !canonical.is_empty() {
            object.insert(key.into(), json!(canonical.into_iter().collect::<Vec<_>>()));
        }
    }
    if let Some(budget) = caps.subagent_budget
        && budget != 0
    {
        object.insert("subagent_budget".into(), json!(budget));
    }
    Ok(Value::Object(object))
}

fn skills_value(skills: &[agents::SkillRef]) -> Result<Value, String> {
    if skills.len() > MAX_SKILLS {
        return Err(format!("an agent may curate at most {MAX_SKILLS} skills"));
    }
    let rows = skills
        .iter()
        .map(|skill| {
            validate_bounded_text(&skill.name, "skill name", MAX_NAME_BYTES, false)?;
            validate_bounded_text(
                &skill.source_prefix,
                "skill source prefix",
                MAX_CAP_ENTRY_BYTES,
                false,
            )?;
            if !skill.source_prefix.starts_with('/') {
                return Err("skill source prefix must be an absolute DuckFS path".into());
            }
            if let Some(snapshot) = skill.snapshot.as_deref() {
                validate_bounded_text(snapshot, "skill snapshot", MAX_NAME_BYTES, false)?;
            }
            Ok(json!({
                "name": skill.name,
                "source_prefix": skill.source_prefix,
                "source_snapshot": skill.snapshot,
                "load": match skill.load {
                    agents::LoadMode::Always => "always",
                    agents::LoadMode::OnDemand => "on_demand",
                }
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Value::Array(rows))
}

fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.is_empty()
        || agent_id.len() > 63
        || agent_id.starts_with('-')
        || agent_id.ends_with('-')
        || !agent_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("agent id must be a 1..=63 byte lowercase DNS label".into());
    }
    Ok(())
}

fn validate_channel_id(channel_id: &str) -> Result<(), String> {
    validate_bounded_text(channel_id, "channel id", MAX_NAME_BYTES, false)?;
    if channel_id.contains('\0') {
        return Err("channel id contains an unsupported character".into());
    }
    Ok(())
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    validate_bounded_text(run_id, "run id", MAX_NAME_BYTES, false)
}

fn parse_forge_item(value: &Value) -> Result<forge::ForgeItem, String> {
    let number = required_u64(value, "number")?;
    if number == 0 {
        return Err("Forge returned item number zero".into());
    }
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some("issue") => forge::ItemKind::Issue,
        Some("pr") => forge::ItemKind::PullRequest,
        _ => return Err("Forge returned an invalid item kind".into()),
    };
    let state = match value.get("state").and_then(Value::as_str) {
        Some("open") => forge::ItemState::Open,
        Some("closed") => forge::ItemState::Closed,
        Some("merged") => forge::ItemState::Merged,
        _ => return Err("Forge returned an invalid item state".into()),
    };
    Ok(forge::ForgeItem {
        number,
        kind,
        state,
        title: bounded_string(value, "title", MAX_TITLE_BYTES)?,
        author: author_name(value.get("author"))?,
        updated: format_stamp(required_u64(value, "updated_at")?),
        source_branch: optional_string(value, "source_branch", MAX_NAME_BYTES)?,
        target_branch: optional_string(value, "target_branch", MAX_NAME_BYTES)?,
    })
}

fn author_name(value: Option<&Value>) -> Result<String, String> {
    let value = value.ok_or_else(|| "record is missing its author".to_string())?;
    if value.as_str() == Some("system") {
        return Ok("system".into());
    }
    if let Some(bytes) = value.get("user") {
        let bytes = bytes_vec(bytes)?;
        if let Ok(text) = std::str::from_utf8(&bytes)
            && !text.is_empty()
            && text.chars().all(|character| !character.is_control())
        {
            return Ok(text.to_owned());
        }
        let hex = hex_encode(&bytes);
        return Ok(short_hex(&hex));
    }
    if let Some(agent) = value.get("agent") {
        let module = bounded_string(agent, "module", MAX_NAME_BYTES)?;
        let id = bounded_string(agent, "agent_id", 63)?;
        return Ok(format!("{module}/{id}"));
    }
    if let Some(module) = value.get("module").and_then(Value::as_str) {
        validate_bounded_text(module, "author module", MAX_NAME_BYTES, false)?;
        return Ok(module.to_owned());
    }
    Err("record returned an invalid author".into())
}

fn author_external_hex(value: Option<&Value>) -> Option<String> {
    value?.get("user").and_then(|value| bytes_hex(value).ok())
}

fn chat_blocks_text(blocks: &[Value]) -> Result<String, String> {
    let mut lines = Vec::with_capacity(blocks.len());
    for block in blocks {
        if block.as_str() == Some("divider") {
            lines.push("---".to_string());
        } else if let Some(spans) = block.get("paragraph").or_else(|| block.get("quote")) {
            let spans = spans
                .as_array()
                .ok_or_else(|| "chat block spans are invalid".to_string())?;
            let mut line = String::new();
            for span in spans {
                let text = span
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "chat span is missing text".to_string())?;
                line.push_str(text);
            }
            lines.push(line);
        } else if let Some(code) = block.get("code") {
            lines.push(
                code.get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "chat code block is missing text".to_string())?
                    .to_owned(),
            );
        } else {
            return Err("chat returned an unsupported discussion block".into());
        }
    }
    Ok(lines.join("\n"))
}

fn variant_array<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a Vec<Value>, String> {
    let rows = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {key} reply"))?;
    if rows.len() > max {
        return Err(format!(
            "node returned too many {key} rows ({} > {max})",
            rows.len()
        ));
    }
    Ok(rows)
}

fn bounded_string(value: &Value, field: &str, max: usize) -> Result<String, String> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("record is missing {field}"))?;
    validate_bounded_text(text, field, max, true)?;
    Ok(text.to_owned())
}

fn optional_string(value: &Value, field: &str, max: usize) -> Result<Option<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            validate_bounded_text(text, field, max, true)?;
            Ok(Some(text.clone()))
        }
        _ => Err(format!("record contains an invalid {field}")),
    }
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("record is missing {field}"))
}

fn validate_bounded_text(
    text: &str,
    field: &str,
    max: usize,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && text.is_empty()) || text.len() > max || text.contains('\0') {
        return Err(format!("{field} is missing, too long, or contains NUL"));
    }
    Ok(())
}

fn string_array(
    value: &Value,
    field: &str,
    max_items: usize,
    max_bytes: usize,
) -> Result<Vec<String>, String> {
    let rows = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("record is missing {field}"))?;
    if rows.len() > max_items {
        return Err(format!("{field} contains too many entries"));
    }
    rows.iter()
        .map(|row| {
            let text = row
                .as_str()
                .ok_or_else(|| format!("{field} contains a non-text entry"))?;
            validate_bounded_text(text, field, max_bytes, false)?;
            Ok(text.to_owned())
        })
        .collect()
}

fn optional_string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(_) => string_array(value, field, MAX_CAP_ENTRIES, MAX_CAP_ENTRY_BYTES),
    }
}

fn bytes_vec(value: &Value) -> Result<Vec<u8>, String> {
    let bytes = value
        .as_array()
        .ok_or_else(|| "wire key is not a byte array".to_string())?;
    if bytes.is_empty() || bytes.len() > 64 {
        return Err("wire key is outside the 1..=64 byte bound".into());
    }
    bytes
        .iter()
        .map(|byte| {
            byte.as_u64()
                .and_then(|byte| u8::try_from(byte).ok())
                .ok_or_else(|| "wire key contains an invalid byte".to_string())
        })
        .collect()
}

fn bytes_hex(value: &Value) -> Result<String, String> {
    bytes_vec(value).map(|bytes| hex_encode(&bytes))
}

fn short_hex(value: &str) -> String {
    if value.len() > 18 {
        format!("{}…{}", &value[..10], &value[value.len() - 6..])
    } else {
        value.to_owned()
    }
}

fn format_stamp(stamp: u64) -> String {
    let seconds = if stamp >= 1_000_000_000_000 {
        stamp / 1_000
    } else if stamp >= 1_000_000_000 {
        stamp
    } else {
        return String::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(seconds);
    let delta = now.saturating_sub(seconds);
    match delta {
        0..=59 => "now".into(),
        60..=3_599 => format!("{}m ago", delta / 60),
        3_600..=86_399 => format!("{}h ago", delta / 3_600),
        _ => format!("{}d ago", delta / 86_400),
    }
}

fn fresh_id(prefix: &str) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{millis:x}-{:x}-{sequence:x}", std::process::id())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("signed frame is not even-length hexadecimal".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            u8::from_str_radix(text, 16).map_err(|error| error.to_string())
        })
        .collect()
}

fn validate_oid(value: &str, field: &str) -> Result<(), String> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a 40-character Git oid"));
    }
    Ok(())
}

fn validate_optional_oid(value: Option<&str>, field: &str) -> Result<(), String> {
    value.map_or(Ok(()), |value| validate_oid(value, field))
}

fn validate_digest(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("blob digest must be 64-character hexadecimal".into());
    }
    Ok(())
}

fn validate_optional_reference(reference: Option<&str>) -> Result<(), String> {
    reference.map_or(Ok(()), validate_reference)
}

fn validate_reference(reference: &str) -> Result<(), String> {
    let reference = reference.trim();
    if reference.is_empty()
        || reference.len() > MAX_NAME_BYTES
        || reference.starts_with('-')
        || reference.contains(['\0', '\\', '~', '^', ':', '?', '*', '['])
        || reference.contains("..")
        || reference.contains("@{")
        || reference.ends_with('.')
        || reference.ends_with('/')
        || reference
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("branch/reference name is invalid".into());
    }
    Ok(())
}

fn clean_repo_name(name: &str) -> Result<&str, String> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err("Forge repository name is missing or too long".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("Forge repository name must be one path segment".into());
    }
    match Path::new(name).components().next() {
        Some(Component::Normal(_)) if name != "." && name != ".." => Ok(name),
        _ => Err(format!("invalid Forge repository name {name:?}")),
    }
}

fn clean_repo_path(raw: &str, allow_empty: bool) -> Result<String, String> {
    if raw.len() > 4 * 1024 || raw.contains('\0') {
        return Err("Forge path exceeds the desktop safety limit".into());
    }
    let path = raw.trim().trim_matches('/');
    if path.is_empty() {
        return if allow_empty {
            Ok(String::new())
        } else {
            Err("Forge path is required".into())
        };
    }
    if path.contains('\\') {
        return Err("backslashes are not valid Forge paths".into());
    }
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Forge paths must stay inside the repository".into());
            }
        }
    }
    Ok(path.to_owned())
}

fn forge_base_dirs(location: &ForgeLocation) -> Result<Vec<PathBuf>, String> {
    let workspace = location
        .workspace_id
        .as_deref()
        .ok_or_else(|| "local Forge browsing requires an active workspace".to_string())?;
    clean_repo_name(workspace)?;
    let workspace_dir = location.state_root.join("workspaces").join(workspace);
    let storage = storage_from_node_toml(&workspace_dir.join("node.toml"))?
        .unwrap_or_else(|| workspace_dir.join("storage"));
    Ok(vec![storage.join("forge-repo"), storage.join("forge-git")])
}

fn storage_from_node_toml(path: &Path) -> Result<Option<PathBuf>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };
    let value: toml::Value =
        toml::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))?;
    let raw = value
        .get("storage_dir")
        .and_then(toml::Value::as_str)
        .unwrap_or("storage");
    let raw = Path::new(raw);
    if raw.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::CurDir | Component::Prefix(_)
        )
    }) {
        return Err("workspace storage_dir contains unsafe path components".into());
    }
    Ok(Some(if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(raw)
    }))
}

fn remote_repo_dir(location: &ForgeLocation, repository: &str) -> Result<PathBuf, String> {
    clean_repo_name(repository)?;
    let origin = location
        .remote_origin
        .as_deref()
        .ok_or_else(|| "remote Forge origin is missing".to_string())?
        .trim_end_matches('/');
    if origin.is_empty() {
        return Err("remote Forge origin is missing".into());
    }
    Ok(location
        .state_root
        .join("forge-remote")
        .join(hex_encode(origin.as_bytes()))
        .join(repository))
}

fn sync_remote_mirror(location: &ForgeLocation, repository: &str) -> Result<RepoMeta, String> {
    let dir = remote_repo_dir(location, repository)?;
    let git = match GitRepository::open(&dir) {
        Ok(git) => git,
        Err(error) if error.code() == ErrorCode::NotFound => {
            fs::create_dir_all(&dir)
                .map_err(|error| format!("create Forge mirror {}: {error}", dir.display()))?;
            GitRepository::init_bare(&dir).map_err(git_error)?
        }
        Err(error) => return Err(format!("open Forge mirror {}: {error}", dir.display())),
    };
    let origin = location.remote_origin.as_deref().unwrap_or_default();
    let url = format!("{}/forge/{repository}", origin.trim_end_matches('/'));
    let mut remote = git.remote_anonymous(&url).map_err(git_error)?;
    let mut options = FetchOptions::new();
    options.prune(FetchPrune::On);
    remote
        .fetch(&["+refs/heads/*:refs/heads/*"], Some(&mut options), None)
        .map_err(|error| format!("fetch Forge repository {repository:?}: {error}"))?;
    drop(remote);
    let (branch, head) = integration_ref(&git)?
        .map(|(branch, oid)| (branch.to_owned(), Some(oid.to_string())))
        .unwrap_or_else(|| ("dev".into(), None));
    Ok(RepoMeta {
        name: repository.to_owned(),
        branch,
        head,
    })
}

fn list_local_repos(location: &ForgeLocation) -> Result<Vec<RepoMeta>, String> {
    let mut repos = BTreeMap::new();
    for base in forge_base_dirs(location)? {
        let entries = match fs::read_dir(&base) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("scan Forge base {}: {error}", base.display())),
        };
        for entry in entries.flatten() {
            if repos.len() >= MAX_REPOSITORIES {
                return Err("Forge repository count exceeds the desktop limit".into());
            }
            let dir = entry.path();
            if !dir.join(".git").exists() {
                continue;
            }
            let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if clean_repo_name(name).is_err() || repos.contains_key(name) {
                continue;
            }
            let (branch, head) = GitRepository::open(&dir)
                .ok()
                .and_then(|repo| integration_ref(&repo).ok().flatten())
                .map(|(branch, oid)| (branch.to_owned(), Some(oid.to_string())))
                .unwrap_or_else(|| ("dev".into(), None));
            repos.insert(
                name.to_owned(),
                RepoMeta {
                    name: name.to_owned(),
                    branch,
                    head,
                },
            );
        }
    }
    Ok(repos.into_values().collect())
}

fn open_named_repo(
    location: &ForgeLocation,
    repository: &str,
) -> Result<Option<GitRepository>, String> {
    let repository = clean_repo_name(repository)?;
    if location.remote_origin.is_some() {
        let dir = remote_repo_dir(location, repository)?;
        return match GitRepository::open(&dir) {
            Ok(repo) => Ok(Some(repo)),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(format!("open Forge mirror {}: {error}", dir.display())),
        };
    }
    for base in forge_base_dirs(location)? {
        let dir = base.join(repository);
        if dir.join(".git").exists() {
            return GitRepository::open(&dir)
                .map(Some)
                .map_err(|error| format!("open Forge repository {}: {error}", dir.display()));
        }
    }
    Ok(None)
}

fn require_named_repo(location: &ForgeLocation, repository: &str) -> Result<GitRepository, String> {
    open_named_repo(location, repository)?
        .ok_or_else(|| format!("Forge repository {repository:?} is not materialized"))
}

fn integration_ref(repo: &GitRepository) -> Result<Option<(&'static str, Oid)>, String> {
    match repo.refname_to_id(DEV_REF) {
        Ok(oid) => Ok(Some(("dev", oid))),
        Err(error) if matches!(error.code(), ErrorCode::NotFound | ErrorCode::UnbornBranch) => {
            match repo.refname_to_id(MAIN_REF) {
                Ok(oid) => Ok(Some(("main", oid))),
                Err(error)
                    if matches!(error.code(), ErrorCode::NotFound | ErrorCode::UnbornBranch) =>
                {
                    Ok(None)
                }
                Err(error) => Err(git_error(error)),
            }
        }
        Err(error) => Err(git_error(error)),
    }
}

fn resolve_ref_spec(repo: &GitRepository, reference: Option<&str>) -> Result<Option<Oid>, String> {
    let reference = reference.unwrap_or("").trim();
    if reference.is_empty() {
        return Ok(integration_ref(repo)?.map(|(_, oid)| oid));
    }
    if reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Oid::from_str(reference).map(Some).map_err(git_error);
    }
    validate_reference(reference)?;
    let full = if reference.starts_with("refs/heads/") {
        reference.to_owned()
    } else {
        format!("refs/heads/{reference}")
    };
    match repo.refname_to_id(&full) {
        Ok(oid) => Ok(Some(oid)),
        Err(error)
            if matches!(
                error.code(),
                ErrorCode::NotFound | ErrorCode::UnbornBranch | ErrorCode::InvalidSpec
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(git_error(error)),
    }
}

fn require_ref_spec(repo: &GitRepository, reference: &str) -> Result<Oid, String> {
    resolve_ref_spec(repo, Some(reference))?
        .ok_or_else(|| format!("cannot resolve {reference:?} to a commit"))
}

fn commit_at<'repo>(
    repo: &'repo GitRepository,
    reference: Option<&str>,
) -> Result<Option<GitCommit<'repo>>, String> {
    let Some(oid) = resolve_ref_spec(repo, reference)? else {
        return Ok(None);
    };
    match repo.find_commit(oid) {
        Ok(commit) => Ok(Some(commit)),
        Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
        Err(error) => Err(git_error(error)),
    }
}

fn list_branches(repo: &GitRepository) -> Result<Vec<(String, String)>, String> {
    let mut branches = Vec::new();
    for branch in repo.branches(Some(BranchType::Local)).map_err(git_error)? {
        if branches.len() >= MAX_BRANCHES {
            return Err("Forge branch count exceeds the desktop limit".into());
        }
        let (branch, _) = branch.map_err(git_error)?;
        let Some(name) = branch.name().map_err(git_error)?.map(str::to_owned) else {
            continue;
        };
        let Some(head) = branch.get().target() else {
            continue;
        };
        branches.push((name, head.to_string()));
    }
    branches.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(branches)
}

fn read_log(
    repo: &GitRepository,
    limit: Option<usize>,
    reference: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<CommitInfo>, String> {
    let Some(head) = commit_at(repo, reference)? else {
        return Ok(Vec::new());
    };
    let limit = limit.unwrap_or(10_000).min(10_000);
    let after = after.map(Oid::from_str).transpose().map_err(git_error)?;
    let mut walk = repo.revwalk().map_err(git_error)?;
    walk.push(head.id()).map_err(git_error)?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(git_error)?;
    let mut cursor_seen = after.is_none();
    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid.map_err(git_error)?;
        if !cursor_seen {
            if after.is_some_and(|cursor| cursor == oid) {
                cursor_seen = true;
            }
            continue;
        }
        commits.push(commit_info(&repo.find_commit(oid).map_err(git_error)?));
        if commits.len() >= limit {
            break;
        }
    }
    Ok(commits)
}

fn commit_info(commit: &GitCommit<'_>) -> CommitInfo {
    CommitInfo {
        id: commit.id().to_string(),
        summary: commit.summary().unwrap_or("(no summary)").to_owned(),
        author: commit.author().name().unwrap_or("ducktape").to_owned(),
        time: commit.time().seconds(),
    }
}

fn commit_view(commit: CommitInfo) -> forge::Commit {
    forge::Commit {
        id: commit.id,
        summary: commit.summary,
        author: commit.author,
        time: format_stamp(commit.time.max(0) as u64),
    }
}

fn subtree<'repo>(
    repo: &'repo GitRepository,
    root: &GitTree<'repo>,
    path: &str,
) -> Result<Option<GitTree<'repo>>, String> {
    if path.is_empty() {
        return Ok(Some(root.clone()));
    }
    let entry = match root.get_path(Path::new(path)) {
        Ok(entry) => entry,
        Err(error) if error.code() == ErrorCode::NotFound => return Ok(None),
        Err(error) => return Err(git_error(error)),
    };
    if entry.kind() != Some(ObjectType::Tree) {
        return Ok(None);
    }
    repo.find_tree(entry.id()).map(Some).map_err(git_error)
}

fn read_tree(
    repo: &GitRepository,
    path: &str,
    reference: Option<&str>,
) -> Result<Vec<TreeInfo>, String> {
    let Some(commit) = commit_at(repo, reference)? else {
        return Ok(Vec::new());
    };
    let root = commit.tree().map_err(git_error)?;
    let Some(tree) = subtree(repo, &root, path)? else {
        return Ok(Vec::new());
    };
    if tree.len() > MAX_TREE_ENTRIES {
        return Err("Forge directory exceeds the desktop entry limit".into());
    }
    let mut entries = tree
        .iter()
        .filter_map(|entry| {
            Some(TreeInfo {
                name: entry.name()?.to_owned(),
                directory: entry.kind() == Some(ObjectType::Tree),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .directory
            .cmp(&left.directory)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

fn tree_entries(path: &str, entries: Vec<TreeInfo>, depth: usize) -> Vec<forge::TreeEntry> {
    entries
        .into_iter()
        .map(|entry| forge::TreeEntry {
            path: if path.is_empty() {
                entry.name.clone()
            } else {
                format!("{path}/{}", entry.name)
            },
            name: entry.name,
            kind: if entry.directory {
                forge::TreeKind::Directory
            } else {
                forge::TreeKind::File
            },
            depth,
            open: false,
        })
        .collect()
}

fn read_file_page(
    repo: &GitRepository,
    path: &str,
    reference: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<Option<FilePageInfo>, String> {
    let Some(commit) = commit_at(repo, reference)? else {
        return Ok(None);
    };
    let tree = commit.tree().map_err(git_error)?;
    let entry = match tree.get_path(Path::new(path)) {
        Ok(entry) => entry,
        Err(error) if error.code() == ErrorCode::NotFound => return Ok(None),
        Err(error) => return Err(git_error(error)),
    };
    if entry.kind() != Some(ObjectType::Blob) {
        return Ok(None);
    }
    let blob = repo.find_blob(entry.id()).map_err(git_error)?;
    utf8_text_page(blob.content(), offset, limit).map(Some)
}

fn utf8_text_page(content: &[u8], offset: usize, limit: usize) -> Result<FilePageInfo, String> {
    if limit == 0 || limit > FILE_PAGE_BYTES {
        return Err("file page limit is outside the supported bound".into());
    }
    let text = std::str::from_utf8(content).map_err(|_| "file is not UTF-8 text".to_string())?;
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err("file page offset is outside a UTF-8 boundary".into());
    }
    let mut end = offset.saturating_add(limit).min(text.len());
    while end > offset && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < text.len() {
        end = text[offset..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| offset + index)
            .unwrap_or(text.len());
    }
    Ok(FilePageInfo {
        text: text[offset..end].to_owned(),
        next_offset: (end < text.len()).then_some(end),
        total_bytes: text.len(),
    })
}

fn compare(repo: &GitRepository, base: &str, head: &str) -> Result<CompareInfo, String> {
    let base_oid = require_ref_spec(repo, base)?;
    let head_oid = require_ref_spec(repo, head)?;
    let merge_base = repo
        .merge_base(base_oid, head_oid)
        .map_err(|error| format!("no merge base between {base:?} and {head:?}: {error}"))?;
    let base_tree = repo
        .find_commit(merge_base)
        .and_then(|commit| commit.tree())
        .map_err(git_error)?;
    let head_tree = repo
        .find_commit(head_oid)
        .and_then(|commit| commit.tree())
        .map_err(git_error)?;
    let mut options = DiffOptions::new();
    options.context_lines(3).interhunk_lines(0);
    let mut diff = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut options))
        .map_err(git_error)?;
    diff.find_similar(None).map_err(git_error)?;
    if diff.deltas().len() > MAX_CHANGED_FILES {
        return Err("pull request changed-file count exceeds the desktop limit".into());
    }
    let mut files = Vec::new();
    let mut total_patch_bytes = 0usize;
    for (index, delta) in diff.deltas().enumerate() {
        let path = delta_path(&delta);
        let (additions, deletions, patch) = match Patch::from_diff(&diff, index)
            .map_err(git_error)?
        {
            Some(mut patch) => {
                let (_context, additions, deletions) = patch.line_stats().map_err(git_error)?;
                let patch = patch.to_buf().map_err(git_error)?;
                if patch.len() > MAX_PATCH_BYTES {
                    return Err(format!(
                        "diff for {path:?} exceeds the desktop display limit"
                    ));
                }
                total_patch_bytes = total_patch_bytes.saturating_add(patch.len());
                if total_patch_bytes > MAX_TOTAL_PATCH_BYTES {
                    return Err("pull request patch text exceeds the desktop display limit".into());
                }
                (
                    additions as u64,
                    deletions as u64,
                    String::from_utf8_lossy(&patch).into_owned(),
                )
            }
            None => (0, 0, String::new()),
        };
        files.push(forge::ChangedFile {
            path,
            additions,
            deletions,
            patch,
        });
    }
    let mut walk = repo.revwalk().map_err(git_error)?;
    walk.push(head_oid).map_err(git_error)?;
    walk.hide(base_oid).map_err(git_error)?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(git_error)?;
    let mut commits = Vec::new();
    for oid in walk {
        if commits.len() >= 10_000 {
            return Err("pull request commit count exceeds the desktop limit".into());
        }
        commits.push(commit_info(
            &repo
                .find_commit(oid.map_err(git_error)?)
                .map_err(git_error)?,
        ));
    }
    Ok(CompareInfo { files, commits })
}

fn build_merge(
    node_repo: &GitRepository,
    ours_oid: Oid,
    theirs_oid: Oid,
    message: &str,
) -> Result<MergeInfo, String> {
    validate_bounded_text(message, "merge message", MAX_TITLE_BYTES, false)?;
    let scratch = ScratchDir::create()?;
    let temp = GitRepository::init_bare(scratch.path()).map_err(git_error)?;
    let objects = node_repo.path().join("objects");
    let objects = objects
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 Git objects path {}", objects.display()))?;
    temp.odb()
        .map_err(git_error)?
        .add_disk_alternate(objects)
        .map_err(git_error)?;
    let ours = temp.find_commit(ours_oid).map_err(git_error)?;
    let theirs = temp.find_commit(theirs_oid).map_err(git_error)?;
    let mut index = temp
        .merge_commits(&ours, &theirs, None)
        .map_err(git_error)?;
    if index.has_conflicts() {
        let mut conflicts = Vec::new();
        for conflict in index.conflicts().map_err(git_error)? {
            if conflicts.len() >= MAX_CHANGED_FILES {
                return Err("merge conflict count exceeds the desktop limit".into());
            }
            let conflict = conflict.map_err(git_error)?;
            let Some(entry) = conflict.our.or(conflict.their).or(conflict.ancestor) else {
                continue;
            };
            conflicts.push(String::from_utf8_lossy(&entry.path).into_owned());
        }
        conflicts.sort();
        conflicts.dedup();
        return Ok(MergeInfo {
            merge_oid: None,
            pack: None,
            conflicts,
        });
    }
    let tree_oid = index.write_tree_to(&temp).map_err(git_error)?;
    let tree = temp.find_tree(tree_oid).map_err(git_error)?;
    let signature = Signature::now("ducktape", "ducktape@localhost").map_err(git_error)?;
    let merge_oid = temp
        .commit(
            None,
            &signature,
            &signature,
            message,
            &tree,
            &[&ours, &theirs],
        )
        .map_err(git_error)?;
    let mut builder = temp.packbuilder().map_err(git_error)?;
    let mut walk = temp.revwalk().map_err(git_error)?;
    walk.push(merge_oid).map_err(git_error)?;
    walk.hide(ours_oid).map_err(git_error)?;
    walk.hide(theirs_oid).map_err(git_error)?;
    builder.insert_walk(&mut walk).map_err(git_error)?;
    let mut buffer = Buf::new();
    builder.write_buf(&mut buffer).map_err(git_error)?;
    if buffer.len() > MAX_MERGE_PACK_BYTES {
        return Err("merge object pack exceeds the node's 4 MiB upload limit".into());
    }
    Ok(MergeInfo {
        merge_oid: Some(merge_oid.to_string()),
        pack: Some(buffer.to_vec()),
        conflicts: Vec::new(),
    })
}

struct ScratchDir(tempfile::TempDir);

impl ScratchDir {
    fn create() -> Result<Self, String> {
        tempfile::Builder::new()
            .prefix("ducktape-iced-forge-merge-")
            .tempdir()
            .map(Self)
            .map_err(|error| format!("create merge scratch: {error}"))
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

fn delta_path(delta: &git2::DiffDelta<'_>) -> String {
    let file = if delta.status() == Delta::Deleted {
        delta.old_file()
    } else {
        delta.new_file()
    };
    file.path()
        .or_else(|| delta.old_file().path())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".into())
}

fn git_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Time;

    #[test]
    fn repository_and_tree_paths_reject_escape_attempts() {
        for name in ["", ".", "..", "../secret", "a/b", "a\\b", "/etc"] {
            assert!(clean_repo_name(name).is_err(), "accepted {name:?}");
        }
        for name in ["ducktape", "default", "a.b_c-1"] {
            assert_eq!(clean_repo_name(name).unwrap(), name);
        }
        for path in ["../secret", "a/../../b", "a\\b", "/../etc"] {
            assert!(clean_repo_path(path, false).is_err(), "accepted {path:?}");
        }
        assert_eq!(
            clean_repo_path("/src/lib.rs/", false).unwrap(),
            "src/lib.rs"
        );
    }

    #[test]
    fn file_pages_preserve_utf8_boundaries_and_bounds() {
        let content = "a🦆z".as_bytes();
        let first = utf8_text_page(content, 0, 2).unwrap();
        assert_eq!(first.text, "a");
        assert_eq!(first.next_offset, Some(1));
        let duck = utf8_text_page(content, 1, 2).unwrap();
        assert_eq!(duck.text, "🦆");
        assert_eq!(duck.next_offset, Some(5));
        assert!(utf8_text_page(content, 2, 2).is_err());
        assert!(utf8_text_page(content, content.len() + 1, 2).is_err());
    }

    #[test]
    fn capability_and_usage_replies_are_strictly_bounded() {
        let capabilities = parse_capabilities(&json!({
            "all": [[[1, 2, 3], ["codex", "claude"]], [[4], ["codex"]]]
        }))
        .unwrap();
        assert_eq!(capabilities, vec!["claude", "codex"]);

        let usage = parse_usage(&json!({
            "usage": [{
                "outcomeOk": false,
                "runs": 2,
                "totalDurationBlocks": 7,
                "inputTokens": 11,
                "outputTokens": 5
            }]
        }))
        .unwrap();
        assert_eq!(usage.requests, 2);
        assert_eq!(usage.failed, 2);
        assert_eq!(usage.duration_blocks, 7);
        assert_eq!(usage.input_tokens + usage.output_tokens, 16);
    }

    #[test]
    fn forge_reviews_and_recent_runs_keep_the_wire_evidence() {
        let reviews = parse_reviews(&json!({
            "reviews": [{
                "author": "system",
                "verdict": "request_changes",
                "body": "Please fix this",
                "commit_oid": "a".repeat(40),
                "comments": [{
                    "path": "src/lib.rs",
                    "line": 9,
                    "side": "new",
                    "body": "This can panic"
                }],
                "created_at": 7
            }]
        }))
        .unwrap();
        assert_eq!(reviews[0].verdict, forge::ReviewVerdict::RequestChanges);
        assert_eq!(reviews[0].comments[0].line, 9);

        let recent = parse_recent_runs(&json!({
            "recent_runs": [{
                "run_id": "run-1",
                "agent_id": "triage",
                "channel_id": "general",
                "anchor_seq": 4,
                "outcome": "delivered",
                "degraded": true,
                "created_at": 10,
                "delivered_at": 13,
                "executing_node": "unknown",
                "output_ref": "feature@deadbeef",
                "pr_number": 12
            }]
        }))
        .unwrap();
        assert_eq!(recent[0].outcome, agents::RunOutcome::Delivered);
        assert_eq!(recent[0].dispatch_id.len(), 64);
        assert_eq!(recent[0].pr_number, Some(12));
    }

    #[test]
    fn inline_review_comments_are_bounded_and_keep_new_side_coordinates() {
        let payload = review_comments_payload(vec![forge::ReviewComment {
            path: "src/lib.rs".into(),
            line: 9,
            side: forge::ReviewSide::New,
            body: "  This can panic  ".into(),
        }])
        .unwrap();
        assert_eq!(
            payload,
            vec![json!({
                "path": "src/lib.rs",
                "line": 9,
                "side": "new",
                "body": "This can panic"
            })]
        );
        assert!(
            review_comments_payload(vec![forge::ReviewComment {
                path: "../outside".into(),
                line: 1,
                side: forge::ReviewSide::New,
                body: "no".into(),
            }])
            .is_err()
        );
    }

    #[test]
    fn agent_records_reject_unknown_actions_and_oversized_caps() {
        let caps = agents::ResourceCaps::default();
        assert!(
            validate_agent_record(
                "helper",
                "Helper",
                "codex",
                &["chat.post".into()],
                &caps,
                &[],
            )
            .is_ok()
        );
        assert!(
            validate_agent_record(
                "helper",
                "Helper",
                "codex",
                &["shell.root".into()],
                &caps,
                &[],
            )
            .is_err()
        );
        assert!(validate_agent_id("Bad_Agent").is_err());

        let caps = agents::ResourceCaps {
            tools: (0..=MAX_CAP_ENTRIES)
                .map(|index| format!("tool-{index}"))
                .collect(),
            ..agents::ResourceCaps::default()
        };
        assert!(caps_value(&caps).is_err());
    }

    #[test]
    fn local_git_projection_and_merge_pack_match_committed_refs() {
        let scratch = ScratchDir::create().unwrap();
        let repo = GitRepository::init(scratch.path()).unwrap();
        let base = commit(&repo, "refs/heads/dev", "hello\n", "base", &[]);
        let feature = commit(
            &repo,
            "refs/heads/feature",
            "hello\nfeature\n",
            "feature",
            &[base],
        );

        let branches = list_branches(&repo).unwrap();
        assert_eq!(branches.len(), 2);
        let compare = compare(&repo, "dev", "feature").unwrap();
        assert_eq!(compare.commits.len(), 1);
        assert_eq!(compare.files.len(), 1);
        assert_eq!(compare.files[0].path, "README.md");
        assert_eq!(compare.files[0].additions, 1);

        let merge = build_merge(&repo, base, feature, "Merge feature").unwrap();
        assert!(merge.conflicts.is_empty());
        assert!(merge.merge_oid.is_some());
        assert!(merge.pack.is_some_and(|pack| !pack.is_empty()));
    }

    fn commit(
        repo: &GitRepository,
        reference: &str,
        content: &str,
        message: &str,
        parents: &[Oid],
    ) -> Oid {
        let blob = repo.blob(content.as_bytes()).unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder.insert("README.md", blob, 0o100644).unwrap();
        let tree_oid = builder.write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let parents = parents
            .iter()
            .map(|oid| repo.find_commit(*oid).unwrap())
            .collect::<Vec<_>>();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        let signature = Signature::new("Ducktape", "duck@localhost", &Time::new(1, 0)).unwrap();
        repo.commit(
            Some(reference),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .unwrap()
    }
}
