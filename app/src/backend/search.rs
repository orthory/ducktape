use super::*;

/// One workspace-search result row, whatever plane it came from.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerHit {
    /// `message` | `page` | `code` | `file` | `run`.
    pub kind: String,
    /// the 2-letter mono plate: `ms` / `pg` / `fg` / `fl` / `ag`.
    pub code: String,
    pub title: String,
    pub snippet: String,
    pub meta: String,
    /// where the row navigates: the channel id, page id, `repo#number`, path
    /// or run id of the hit.
    pub target: String,
}

/// One filter chip. Only kinds with a real loader are emitted — a chip that
/// always reads zero is a fake surface.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct KindCount {
    pub kind: String,
    pub label: String,
    pub count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerResults {
    pub generation: i64,
    pub hits: Vec<ExplorerHit>,
    pub kinds: Vec<KindCount>,
}

/// Search the whole workspace: chat, pages, the forge trackers, duckfs paths
/// and agent runs. Tasks are not searched — that module has no app loader.
pub async fn search_workspace(
    rpc: String,
    text: String,
    generation: i64,
) -> Result<ExplorerResults, HydrationError> {
    let needle = text.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(ExplorerResults {
            generation,
            hits: Vec::new(),
            kinds: Vec::new(),
        });
    }
    let mut hits = Vec::new();
    if let Ok(chat) = search_chat(rpc.clone(), String::new(), text.clone(), generation).await {
        hits.extend(chat.hits.into_iter().map(|hit| ExplorerHit {
            kind: "message".into(),
            code: "ms".into(),
            title: author_name(&hit.author),
            snippet: hit.text,
            meta: format!("{} · {}", hit.channel_id, hit.meta),
            target: hit.channel_id,
        }));
    }
    if let Ok(pages) = search_pages(rpc.clone(), String::new(), text.clone(), generation).await {
        hits.extend(pages.hits.into_iter().map(|hit| ExplorerHit {
            kind: "page".into(),
            code: "pg".into(),
            title: hit.text.clone(),
            snippet: hit.text,
            meta: format!("pages · {}", hit.kind),
            target: hit.page_id,
        }));
    }
    hits.extend(search_forge_items(&rpc, &needle, generation).await);
    hits.extend(search_files(&rpc, text.trim()).await);
    hits.extend(search_tasks(&rpc, &needle).await);
    if let Ok(runs) = load_agent_runs(rpc, String::new(), generation).await {
        hits.extend(
            runs.runs
                .into_iter()
                .filter(|run| {
                    run.run_id.to_lowercase().contains(&needle)
                        || run.agent_id.to_lowercase().contains(&needle)
                })
                .map(|run| ExplorerHit {
                    kind: "run".into(),
                    code: "ag".into(),
                    title: format!("{} · {}", run.run_id, run.agent_id),
                    snippet: run.outcome,
                    // `created_at` is the creation BLOCK, so it prints as a
                    // height — this search has no tip to count back from.
                    meta: format!("agent · {}", height_label_short(run.created_at)),
                    target: run.run_id,
                }),
        );
    }
    let kinds = [
        ("message", "Messages"),
        ("page", "Pages"),
        ("code", "Code"),
        ("file", "Files"),
        ("task", "Tasks"),
        ("run", "Runs"),
    ]
    .into_iter()
    .map(|(kind, label)| KindCount {
        count: count_i64(hits.iter().filter(|hit| hit.kind == kind).count()),
        kind: kind.into(),
        label: label.into(),
    })
    .collect();
    Ok(ExplorerResults {
        generation,
        hits,
        kinds,
    })
}

/// The duckfs half of the workspace search: `GET /v1/files/grep`, the node's
/// only CONTENT search. `find`'s prefix is a raw path prefix in full-path
/// order, so it answers "what is under this directory", never "who mentions
/// this word" — a content query would come back empty through it.
async fn search_files(rpc: &str, pattern: &str) -> Vec<ExplorerHit> {
    let Ok(client) = rpc_client(rpc) else {
        return Vec::new();
    };
    let Ok(reply) = client.files_get("grep", &[("pattern", pattern)]).await else {
        return Vec::new();
    };
    reply["hits"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|hit| {
            let path = hit["path"].as_str().unwrap_or_default().to_string();
            let line = hit["line"].as_i64().unwrap_or(0);
            ExplorerHit {
                kind: "file".into(),
                code: "fl".into(),
                title: path.rsplit('/').next().unwrap_or(&path).to_string(),
                snippet: hit["text"].as_str().unwrap_or_default().trim().to_string(),
                meta: format!("{path}:{line}"),
                target: path,
            }
        })
        .collect()
}

/// The tasks half of the workspace search: the three bounded status pages of
/// the tasks index, filtered on title and id client-side (that index has no
/// text query either). A workspace with no tasks yet contributes no hits and
/// its chip reads 0 — empty is not the same as absent.
async fn search_tasks(rpc: &str, needle: &str) -> Vec<ExplorerHit> {
    const STATUS_PAGES: &[(&str, &str)] = &[
        ("open", "open"),
        ("in_progress", "in progress"),
        ("done", "done"),
    ];
    const PAGE_LIMIT: usize = 256;
    let Ok(client) = rpc_client(rpc) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for (status, label) in STATUS_PAGES {
        let query = serde_json::json!({
            "by_status": { "status": status, "limit": PAGE_LIMIT }
        });
        let Ok(reply) = client.view::<_, serde_json::Value>("tasks", &query).await else {
            return hits;
        };
        for row in reply["tasks"]["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let title = row["title"].as_str().unwrap_or_default().to_string();
            let id = row["task_id"].as_str().unwrap_or_default().to_string();
            let matched =
                title.to_lowercase().contains(needle) || id.to_lowercase().contains(needle);
            if !matched {
                continue;
            }
            let author = short_label(row["created_by"].as_str().unwrap_or_default());
            // `updated_height` is a BLOCK, so it prints as a height — this
            // search has no tip to count back from.
            let updated = height_label_short(row["updated_height"].as_i64().unwrap_or(0));
            hits.push(ExplorerHit {
                kind: "task".into(),
                code: "tk".into(),
                title,
                snippet: (*label).to_string(),
                meta: format!("{author} · tasks · {updated}"),
                target: id,
            });
        }
    }
    hits
}

/// The forge half of the workspace search: every repo's tracker, filtered on
/// the title client-side (the module has no text query).
async fn search_forge_items(rpc: &str, needle: &str, generation: i64) -> Vec<ExplorerHit> {
    let Ok(forge) = load_forge(rpc.to_string(), generation).await else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for repo in forge.repos {
        let Ok(data) = load_forge_repo(rpc.to_string(), repo.name.clone(), generation).await else {
            continue;
        };
        hits.extend(
            data.items
                .into_iter()
                .filter(|item| item.title.to_lowercase().contains(needle))
                .map(|item| ExplorerHit {
                    kind: "code".into(),
                    code: "fg".into(),
                    title: format!("#{} {}", item.number, item.title),
                    snippet: format!("{} · {}", item.kind, item.state),
                    meta: format!("{} · {}", item.author_name, repo.name),
                    target: format!("{}#{}", repo.name, item.number),
                }),
        );
    }
    hits
}
