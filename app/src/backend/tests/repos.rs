use super::*;

#[test]
fn the_forge_hint_is_a_command_that_actually_pushes() {
    // Verified end to end against a live node: a push to a NEW lowercase name
    // creates the repo, and an uppercase one 404s the ref advertisement because
    // `forge::norm_repo` accepts `[a-z0-9._-]` only — so the placeholder has to
    // be a name the reader can paste unchanged.
    let hint = forge_push_command("http://127.0.0.1:38259".into());
    assert_eq!(
        hint,
        "git remote add ducktape http://127.0.0.1:38259/forge/my-repo && git push ducktape main"
    );
    let placeholder = hint
        .split("/forge/")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .expect("the hint names a repo");
    assert!(forge::norm_repo(placeholder).is_ok());
    // A trailing slash on the endpoint must not double up in the URL.
    assert_eq!(forge_push_command("http://127.0.0.1:38259/".into()), hint);
}

#[test]
fn forge_code_tokens_follow_the_path_and_rust_really_colors() {
    assert_eq!(code_token("src/main.rs"), "rs");
    assert_eq!(code_token("a/b/query.SQL"), "sql");
    assert_eq!(code_token("Makefile"), "makefile");
    assert_eq!(code_token(".gitignore"), "gitignore");
    // the stream really tokenizes: one rust line yields more than one ink,
    // and an unknown token degrades to plain text (uniform ink), never an
    // error — exactly the old single-ink reading.
    let colors = |token: &str| -> std::collections::BTreeSet<String> {
        let mut stream = iced::highlighter::Stream::new(&iced::highlighter::Settings {
            theme: code_theme(true),
            token: token.into(),
        });
        stream
            .highlight_line("fn main() { let answer = 42; }")
            .map(|(_, highlight)| format!("{:?}", highlight.color()))
            .collect()
    };
    assert!(
        colors("rs").len() > 1,
        "rust source highlights with more than one color"
    );
    assert_eq!(
        colors("no-such-language").len(),
        1,
        "an unknown token is plain text in one ink"
    );
}

#[test]
fn the_tracker_splits_into_open_prs_and_open_issues() {
    let item = |number: i64, kind: &str, state: &str| ForgeItem {
        number,
        kind: kind.into(),
        state: state.into(),
        title: format!("item {number}"),
        author: "user:aa".into(),
        author_name: "aa".into(),
    };
    let items = vec![
        item(1, "pr", "open"),
        item(2, "pr", "merged"),
        item(3, "issue", "open"),
        item(4, "issue", "closed"),
    ];
    assert_eq!(filter_forge_items(items.clone(), ForgeTab::Pulls).len(), 2);
    assert_eq!(filter_forge_items(items.clone(), ForgeTab::Issues).len(), 2);
    assert!(filter_forge_items(items.clone(), ForgeTab::Code).is_empty());
    assert_eq!(forge_open_count(items.clone(), "pr".into()), 1);
    assert_eq!(forge_open_count(items, "issue".into()), 1);
}

#[test]
fn merge_builder_produces_the_cas_commit_and_its_minimal_pack() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n"), ("b.txt", "keep\n")]);
    let ours = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "ours\n"), ("b.txt", "keep\n")],
    );
    let theirs = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "base\n"), ("b.txt", "theirs\n")],
    );

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #1").unwrap();
    let MergeBuild::Clean { merge_oid, pack } = build else {
        panic!("disjoint edits must merge cleanly");
    };

    // land the pack in the mirror and read the merge commit back out —
    // exactly what a validator does after the blob fan-out.
    let odb = mirror.odb().unwrap();
    let mut writepack = odb.packwriter().unwrap();
    std::io::Write::write_all(&mut writepack, &pack).unwrap();
    writepack.commit().unwrap();
    let merged = mirror
        .find_commit(git2::Oid::from_str(&merge_oid).unwrap())
        .unwrap();
    let parents: Vec<git2::Oid> = merged.parent_ids().collect();
    assert_eq!(parents, vec![ours, theirs], "target first, source second");
    let tree = merged.tree().unwrap();
    let read = |path: &str| {
        let entry = tree.get_path(Path::new(path)).unwrap();
        String::from_utf8(mirror.find_blob(entry.id()).unwrap().content().to_vec()).unwrap()
    };
    assert_eq!(read("a.txt"), "ours\n");
    assert_eq!(read("b.txt"), "theirs\n");
}

#[test]
fn merge_builder_reports_conflicts_and_builds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n")]);
    let ours = mirror_commit(&mirror, Some(base), &[("a.txt", "ours\n")]);
    let theirs = mirror_commit(&mirror, Some(base), &[("a.txt", "theirs\n")]);

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #2").unwrap();
    let MergeBuild::Conflicts(paths) = build else {
        panic!("competing edits must conflict");
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
}

#[test]
fn forge_code_replies_keep_the_server_revision_and_preview_flags() {
    let rev = "1".repeat(40);
    let tree = tree_data(
        serde_json::json!({ "tree": {
            "rev": rev,
            "born": true,
            "entries": [{
                "path": "src/lib.rs",
                "name": "lib.rs",
                "kind": "file"
            }],
            "truncated": true
        }}),
        "core".into(),
        "src".into(),
    )
    .unwrap();
    assert_eq!(tree.rev, "1".repeat(40));
    assert!(tree.born);
    assert!(tree.truncated);
    assert_eq!(tree.entries[0].path, "src/lib.rs");

    let text = blob_view(
        serde_json::json!({ "blob": {
            "rev": "1".repeat(40),
            "path": "src/lib.rs",
            "text": "one\ntwo\n",
            "size": 8,
            "truncated": true,
            "binary": false
        }}),
        "core".into(),
    )
    .unwrap();
    assert_eq!(text.lines, 2);
    assert!(text.truncated && !text.binary);

    let binary = blob_view(
        serde_json::json!({ "blob": {
            "rev": "1".repeat(40),
            "path": "asset.bin",
            "text": "",
            "size": 400,
            "truncated": false,
            "binary": true
        }}),
        "core".into(),
    )
    .unwrap();
    assert!(binary.binary);
    assert_eq!(binary.lines, 0);

    assert_eq!(
        blob_view(serde_json::json!({ "blob": null }), "core".into()).unwrap_err(),
        "the requested file was not found"
    );
}

/// THE FORGE REPO LIST IS THE ONE UNSCOPED SLICE. Every other slice here is
/// keyed on what the forge pane has open. Off the forge tab this reloads
/// nothing, and reaching the (unreachable) node is what a lost gate looks like.
#[tokio::test(flavor = "current_thread")]
async fn a_forge_op_does_not_load_the_repo_list_for_a_closed_pane() {
    let data = forge_live_refresh(
        "http://127.0.0.1:9".into(),
        String::new(),
        0,
        crate::LiveKind::Forge,
        "forge".into(),
        ForgeRefresh::default(),
        false,
        4,
    )
    .await
    .expect("a closed forge pane loads nothing, so nothing can fail");

    assert_eq!(data.generation, 4);
    assert!(
        !data.repos_loaded,
        "an unloaded list must leave the handler's keep alone"
    );
    assert!(data.repos.is_empty());
}
