use super::*;
use ::forge;

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

/// DOES THE ANSWER A SEARCH WAS SENT FOR STILL SPEAK FOR WHAT IS IN THE BOX?
///
/// All three search surfaces are ENTER-TO-SUBMIT and two-way bound with no
/// `change=` route, so a keystroke writes the draft and runs no handler at
/// all: nothing but this comparison can retire an answer as the reader types
/// on. `query` is the string a search was actually SENT for and is empty when
/// no answer is standing; `searching` covers the round trip the submit opened,
/// during which no answer exists yet.
///
/// It replaces three hand-synced conjunct arms — pages, chat and the explorer
/// each carried their own copy, and a fourth surface would have grown a
/// fourth.
pub fn search_answer_stands(query: &str, draft: &str, searching: bool) -> bool {
    !searching && !query.is_empty() && draft.trim() == query
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PaletteSearchData {
    pub chat_hits: Vec<ChatSearchHit>,
    pub page_hits: Vec<PageSearchHit>,
}

/// The command palette's per-keystroke search: one debounced call covering
/// chat and pages together. Typing a word used to issue two RPC round trips
/// per keystroke. The Ice `replace` lane owns cancellation and stale delivery;
/// dropping a superseded task during this sleep prevents its RPCs from firing.
pub async fn palette_search(rpc: String, text: String) -> Result<PaletteSearchData, AppError> {
    tokio::time::sleep(Duration::from_millis(250)).await;
    let (chat, pages) = tokio::join!(
        search_chat(rpc.clone(), String::new(), text.clone()),
        search_pages(rpc, String::new(), text)
    );
    let both_failed = chat.is_err() && pages.is_err();
    if both_failed {
        return Err(AppError {
            message: "Search did not reach the node. Retry in a moment.".into(),
            committed: false,
        });
    }
    Ok(PaletteSearchData {
        chat_hits: chat.map(|data| data.hits).unwrap_or_default(),
        page_hits: pages.map(|data| data.hits).unwrap_or_default(),
    })
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerResults {
    pub hits: Vec<ExplorerHit>,
    /// One chip per source that ANSWERED. A source that refused or timed out
    /// keeps no chip — see `partial`.
    pub kinds: Vec<KindCount>,
    /// The sentence naming the sources that did not answer, empty when they all
    /// did. Prose here for the same reason `HydrationError.message` is prose:
    /// the screen prints it verbatim.
    pub partial: String,
}

/// Search the whole workspace: chat, pages, the forge trackers, duckfs paths,
/// tasks and agent runs.
pub async fn search_workspace(rpc: String, text: String) -> ExplorerResults {
    let needle = text.trim().to_lowercase();
    if needle.is_empty() {
        return ExplorerResults {
            hits: Vec::new(),
            kinds: Vec::new(),
            partial: String::new(),
        };
    }
    // SIX INDEPENDENT SOURCES, ONE WAIT. They were awaited one after another, so
    // a search cost their SUM. Nothing here reads anything another leg produces,
    // which is exactly the case `load_workspace` (backend/load.rs) already fans
    // out with its own note: "Concurrent, the console opens on the slowest leg,
    // not their sum." `palette_search` twenty lines above does the same for its
    // two.
    //
    // What this buys, measured against the demo node: warm, every leg answers in
    // 1-5 ms and the fan-out is worth nothing. COLD it is the whole cost — a
    // module's first touch runs 10-54 s (forge `list_repos` 33.8 s cold /
    // 0.0015 s warm on this box), and `RpcClient`'s ceiling is 30 s, so serial
    // is several ceilings end to end and this is one.
    //
    // This is a READ fan-out. The `join_all` ban in backend/document.rs is about
    // the WRITE chain, where an op built on the block before it must land after
    // it; no ordering exists between these.
    //
    // The extend order below is the ORDER ON SCREEN.
    let (chat, pages, forge, files, tasks, runs) = tokio::join!(
        search_chat(rpc.clone(), String::new(), text.clone()),
        search_pages(rpc.clone(), String::new(), text.clone()),
        search_forge_items(&rpc, &needle),
        search_files(&rpc, text.trim()),
        search_tasks(&rpc, &needle),
        load_agent_runs(rpc.clone()),
    );
    // A SOURCE THAT DID NOT ANSWER IS NOT A SOURCE WITH NOTHING TO SAY. Every
    // leg fails silently — `if let Ok(..)` on two, an empty vector on the rest —
    // so a search that reached the node and timed out on three of its six
    // sources still rendered "1 result", a full chip strip counting 0 for what
    // it never read, and "Nothing matched that query in this workspace" when the
    // one survivor was empty. Three confident lies off one timeout. What went
    // unanswered is collected here, kept OFF the chip strip, and named on
    // screen.
    let mut hits = Vec::new();
    let mut silent: Vec<&str> = Vec::new();
    match chat {
        Err(_) => silent.push("Messages"),
        Ok(chat) => hits.extend(chat.hits.into_iter().map(|hit| ExplorerHit {
            kind: "message".into(),
            code: "ms".into(),
            // `hit.author` IS ALREADY A DISPLAY NAME — `search_chat` runs it
            // through `author_display`, which yields "you", "user 48cedb0d…" or
            // "@quackbot". Running `author_name` over that a second time found
            // no `user:`/`agent:` prefix to split and fell through its `_` arm,
            // so EVERY message hit in the Explorer was attributed to "system".
            // Driven: the same message reads `user 48cedb0d…` in the timeline
            // and `system` in search.
            title: hit.author,
            snippet: hit.text,
            // `hit.meta` already reads `general · #12` — see backend/chat.rs.
            // Composing the channel again here printed it twice.
            meta: hit.meta,
            target: hit.channel_id,
        })),
    }
    match pages {
        Err(_) => silent.push("Pages"),
        Ok(pages) => hits.extend(pages.hits.into_iter().map(|hit| ExplorerHit {
            kind: "page".into(),
            code: "pg".into(),
            // THE ROW'S HEADING IS THE PAGE, the block text is the snippet
            // beneath it — the shape every other hit here already has (a
            // message row heads with its author, a forge row with its item).
            // Both were `hit.text`, so a page hit printed the same sentence
            // twice and the only metadata it carried was the block KIND, which
            // never said which page the match came from.
            title: hit.page_title,
            snippet: hit.text,
            meta: format!("pages · {}", hit.kind),
            target: hit.page_id,
        })),
    }
    match forge {
        None => silent.push("Code"),
        Some(forge) => hits.extend(forge),
    }
    match files {
        None => silent.push("Files"),
        Some(files) => hits.extend(files),
    }
    match tasks {
        None => silent.push("Tasks"),
        Some(tasks) => hits.extend(tasks),
    }
    match runs {
        Err(_) => silent.push("Runs"),
        Ok(runs) => hits.extend(
            runs.into_iter()
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
        ),
    }
    // ONE CHIP PER SOURCE THAT ANSWERED. The strip's own contract (screens/
    // storage.ice) is "every chip here names a kind `search_workspace` genuinely
    // ran, so a count of 0 means nothing matched, never no loader" — a source
    // that timed out breaks exactly that, so it gets no chip and is named in
    // `partial` instead. The labels ARE the source names; one table, no second
    // list to drift.
    let kinds = [
        ("message", "Messages"),
        ("page", "Pages"),
        ("code", "Code"),
        ("file", "Files"),
        ("task", "Tasks"),
        ("run", "Runs"),
    ]
    .into_iter()
    .filter(|(_, label)| !silent.contains(label))
    .map(|(kind, label)| KindCount {
        count: count_i64(hits.iter().filter(|hit| hit.kind == kind).count()),
        kind: kind.into(),
        label: label.into(),
    })
    .collect();
    let partial = match silent.is_empty() {
        true => String::new(),
        false => format!(
            "{} did not answer — these results are incomplete.",
            silent.join(", ")
        ),
    };
    ExplorerResults {
        hits,
        kinds,
        partial,
    }
}

/// The duckfs half of the workspace search: `GET /v1/files/grep`, the node's
/// only CONTENT search. `find`'s prefix is a raw path prefix in full-path
/// order, so it answers "what is under this directory", never "who mentions
/// this word" — a content query would come back empty through it.
///
/// `None` = the node did not answer, which is NOT the same fact as "no file
/// mentions this word" and must never render as it.
async fn search_files(rpc: &str, pattern: &str) -> Option<Vec<ExplorerHit>> {
    let client = rpc_client(rpc).ok()?;
    let reply = client
        .files_get("grep", &[("pattern", pattern)])
        .await
        .ok()?;
    Some(
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
            .collect(),
    )
}

/// The tasks half of the workspace search: the three bounded status pages of
/// the tasks index, filtered on title and id client-side (that index has no
/// text query either). A workspace with no tasks yet contributes no hits and
/// its chip reads 0 — empty is not the same as absent.
///
/// `None` = at least one status page did not answer. The old code returned the
/// pages it had gotten so far, which reads on screen as a complete answer that
/// is silently missing every open task.
async fn search_tasks(rpc: &str, needle: &str) -> Option<Vec<ExplorerHit>> {
    const STATUS_PAGES: &[(&str, &str)] = &[
        ("open", "open"),
        ("in_progress", "in progress"),
        ("done", "done"),
    ];
    const PAGE_LIMIT: usize = 256;
    let client = rpc_client(rpc).ok()?;
    // The three status pages are three independent views of one index, and this
    // leg awaited them one at a time inside the leg that was already fanned out
    // around it.
    let client = &client;
    let pages =
        iced::futures::future::join_all(STATUS_PAGES.iter().map(|(status, label)| async move {
            let query = serde_json::json!({
                "by_status": { "status": status, "limit": PAGE_LIMIT }
            });
            let reply = client.view::<_, serde_json::Value>("tasks", &query).await;
            reply.map(|reply| (reply, *label))
        }))
        .await;
    let mut hits = Vec::new();
    for page in pages {
        let (reply, label) = page.ok()?;
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
    Some(hits)
}

/// The forge half of the workspace search: every repo's tracker, filtered on
/// the title client-side (the module has no text query).
///
/// IT ASKS THE MODULE FOR THE REPO NAMES, NOT `load_forge`. That loader derives
/// an about line, a dominant language and an updated stamp per repo out of a
/// local git mirror, and `mirror_holding` refreshes the mirror over the node's
/// smart-HTTP bridge whenever it does not already hold the repo's head — so a
/// first search, or any search after a push, paid a full `git fetch` per repo.
/// Measured against the demo node: 62.6 s / 50 MB for its `ducktape` repo,
/// 65.7 s across its three. This search reads exactly ONE field off a repo, its
/// name, and `list_repos` already serves that (`{name, head}`, 1.5 ms warm).
///
/// `list_items` only, for the same reason: `load_forge_repo` also reads the
/// branch list, and no hit here carries a branch.
async fn search_forge_items(rpc: &str, needle: &str) -> Option<Vec<ExplorerHit>> {
    let client = rpc_client(rpc).ok()?;
    let listed: serde_json::Value = client
        .query("forge", &serde_json::json!("list_repos"))
        .await
        .ok()?;
    let repos: Vec<String> = listed["repos"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|repo| repo["name"].as_str().map(str::to_string))
        .collect();
    // One tracker read per repo, and they were serial too — a workspace with ten
    // repos paid ten round trips inside the one leg that was already the
    // slowest. Each is independent; the replies are zipped back onto their repo
    // so the rows keep the repo list's order.
    //
    // ponytail: unbounded fan-out, one in-flight request per repo. Fine for the
    // workspaces this console is built for (the demo has three); if a workspace
    // ever carries enough repos for the burst to matter, bound it with a
    // semaphore or chunk the iterator — do not go back to serial.
    let client = &client;
    let names = account_names(client).await;
    let loaded = iced::futures::future::join_all(repos.iter().map(|repo| async move {
        client
            .query::<_, serde_json::Value>(
                "forge",
                &serde_json::json!({ "list_items": { "repo": repo } }),
            )
            .await
    }))
    .await;
    let mut hits = Vec::new();
    for (repo, reply) in repos.iter().zip(loaded) {
        // ONE REPO'S REFUSAL SILENCES THE WHOLE SOURCE. Skipping it would render
        // a tracker list that is quietly missing a repo, with nothing on screen
        // saying which.
        let reply = reply.ok()?;
        let summaries: Vec<forge::ItemSummary> =
            serde_json::from_value(reply["items"].clone()).ok()?;
        hits.extend(
            forge::client::item_rows(&summaries, &names)
                .into_iter()
                .filter(|item| item.title.to_lowercase().contains(needle))
                .map(|item| ExplorerHit {
                    kind: "code".into(),
                    code: "fg".into(),
                    title: format!("#{} {}", item.number, item.title),
                    snippet: format!("{} · {}", item.kind, item.state),
                    meta: format!("{} · {repo}", item.author_name),
                    target: format!("{repo}#{}", item.number),
                }),
        );
    }
    Some(hits)
}
