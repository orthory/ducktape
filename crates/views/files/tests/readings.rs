//! The folds the screen is drawn from, read straight: the module's write rule
//! and the listing fold the view now does for itself.

use files_view::host::{fold_entries, fold_history, fs_child, fs_parent, write_refusal};

/// duckfs's `check_authority` restated in the view's own words: a home tree
/// needs its label AND an entry under it, `/shared/**` is every member's, and
/// both namespace roots are nobody's.
#[test]
fn the_write_rule_names_the_directories_nothing_can_be_written_in() {
    for writable in ["/shared", "/shared/docs", "/home/alice", "/home/alice/notes"] {
        assert_eq!(write_refusal(writable), "", "{writable}");
    }
    assert_eq!(write_refusal("/home"), "home root is not writable");
    assert_eq!(write_refusal("/shared"), "");
    assert_eq!(write_refusal("/"), "path is outside /home and /shared");
    assert_eq!(write_refusal("/etc"), "path is outside /home and /shared");
}

/// The root is `/`, never "" — duckfs refuses an empty path.
#[test]
fn the_parent_of_a_top_level_directory_is_the_root() {
    assert_eq!(fs_parent("/shared/docs/a.md"), "/shared/docs");
    assert_eq!(fs_parent("/shared"), "/");
    assert_eq!(fs_parent("/"), "/");
    assert_eq!(fs_child("/shared", "  notes  "), "/shared/notes");
    assert_eq!(fs_child("/", "notes"), "/notes");
}

/// The rows the browser draws come out of the `ls` reply, and a path is the
/// same row across every re-read, so the virtual list keeps its place.
#[test]
fn a_listing_folds_into_rows_keyed_by_path() {
    let reply = serde_json::json!({ "entries": [
        { "path": "/shared/docs", "kind": "dir", "size": 2, "object": "aa" },
        { "path": "/shared/a.md", "kind": "file", "size": 7, "object": "bb" },
    ]});
    let rows = fold_entries(&reply);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "docs");
    assert_eq!(rows[0].kind, "dir");
    assert_eq!(rows[1].name, "a.md");
    assert_eq!(rows[1].size, 7);
    assert_eq!(
        fold_entries(&reply)[1].key,
        rows[1].key,
        "a path keeps its row identity across re-reads"
    );
    assert_ne!(rows[0].key, rows[1].key);
}

/// A snapshot rail prints short digests, never the whole 64 hex.
#[test]
fn history_folds_into_short_digests() {
    let reply = serde_json::json!({ "snapshots": [
        { "id": "a".repeat(64), "author": "b".repeat(64), "height": 12, "message": "first" },
    ]});
    let rows = fold_history(&reply);
    assert_eq!(rows[0].short_id, format!("{}…", "a".repeat(12)));
    assert_eq!(rows[0].id, "a".repeat(64));
    assert_eq!(rows[0].height, 12);
}
