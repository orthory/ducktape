use super::*;

fn save_event(path: &str, text: &str, context: &str) -> module_view::ModuleViewEvent {
    module_view::ModuleViewEvent {
        kind: "save".into(),
        detail: serde_json::json!({"path": path, "text": text, "context": context, "namespace": "guest-a", "base": "original-snapshot", "request": 1})
            .to_string(),
    }
}

#[test]
fn a_files_save_from_before_reconnect_cannot_overwrite_the_current_preview() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.network_chain_id = "network-a".into();
    let original_connection = app.connect_generation;
    let original = save_event(
        "/shared/note.txt",
        "unsaved A",
        &backend::files_context(
            app.connected_rpc.clone(),
            app.network_chain_id.clone(),
            app.connect_generation,
        ),
    );
    let _ = app.__update(__DucktapeMessage::Reconnect);
    assert!(app.connect_generation > original_connection);
    // The endpoint and chain are unchanged: only the connection occurrence differs.
    app.connected = true;
    app.loading = false;
    app.fs_loading = false;
    app.network_chain_id = "network-a".into();
    app.fs_preview_path = "/shared/note.txt".into();
    app.fs_preview_text = "B source".into();
    app.fs_preview_base = "B snapshot".into();
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(original));
    assert_eq!(
        app.fs_preview_text, "B source",
        "a delayed Save cannot overwrite another connection's file"
    );
    assert!(!app.fs_loading, "no stale write was admitted");
    assert_eq!(
        app.fs_save_reply,
        backend::no_fs_save_reply(),
        "a stale intent cannot replace another save acknowledgement"
    );
}

#[test]
fn a_files_write_failure_from_a_retired_connection_cannot_clear_a_new_write() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.network_chain_id = "network-a".into();
    app.fs_preview_path = "/shared/note.txt".into();
    app.fs_preview_base = "A snapshot".into();
    let a_context = backend::files_context(
        app.connected_rpc.clone(),
        app.network_chain_id.clone(),
        app.connect_generation,
    );
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(save_event(
        "/shared/note.txt",
        "A draft",
        &a_context,
    )));
    let a_op = app.fs_write_pending.clone();
    assert!(app.fs_loading, "A's write was admitted");
    let _ = app.__update(__DucktapeMessage::Reconnect);
    app.connected = true;
    app.loading = false;
    app.fs_loading = false;
    app.network_chain_id = "network-b".into();
    app.fs_preview_path = "/shared/note.txt".into();
    app.fs_preview_base = "B snapshot".into();
    let b_context = backend::files_context(
        app.connected_rpc.clone(),
        app.network_chain_id.clone(),
        app.connect_generation,
    );
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(save_event(
        "/shared/note.txt",
        "B draft",
        &b_context,
    )));
    let b_op = app.fs_write_pending.clone();
    assert!(app.fs_loading, "B's write was admitted");
    let _ = app.__update(__DucktapeMessage::FsWriteFailed(
        a_context.clone(),
        "guest-a".into(),
        a_op.clone(),
        1,
        backend::AppError {
            message: "A failed late".into(),
            committed: false,
        },
    ));
    assert!(
        app.fs_loading,
        "a retired write cannot clear B's pending state"
    );
    assert_ne!(app.error, "A failed late");
    assert_eq!(app.fs_write_pending, b_op);
    let _ = app.__update(__DucktapeMessage::FsWrote(
        a_context,
        "guest-a".into(),
        a_op,
        1,
        true,
    ));
    assert_eq!(
        app.fs_write_pending, b_op,
        "stale success must not clear B either"
    );
    assert_eq!(app.fs_writes, 0);
}

#[test]
fn a_retired_files_operation_cannot_consume_a_retry_on_the_same_connection() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.network_chain_id = "network-a".into();
    app.fs_preview_path = "/shared/note.txt".into();
    app.fs_preview_base = "original-snapshot".into();
    let context = backend::files_context(
        app.connected_rpc.clone(),
        app.network_chain_id.clone(),
        app.connect_generation,
    );
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(save_event(
        "/shared/note.txt",
        "A draft",
        &context,
    )));
    let first = app.fs_write_pending.clone();
    let _ = app.__update(__DucktapeMessage::FsWriteFailed(
        context.clone(),
        "guest-a".into(),
        first.clone(),
        1,
        backend::AppError {
            message: "retry me".into(),
            committed: false,
        },
    ));
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(save_event(
        "/shared/note.txt",
        "A retry",
        &context,
    )));
    let retry = app.fs_write_pending.clone();
    assert!(!retry.is_empty());
    assert_ne!(first, retry);
    let _ = app.__update(__DucktapeMessage::FsWrote(
        context,
        "guest-a".into(),
        first,
        1,
        true,
    ));
    assert_eq!(app.fs_write_pending, retry);
    assert!(app.fs_loading);
    assert_eq!(app.fs_writes, 0);
    assert!(!app.fs_save_reply.replies.last().unwrap().success);
}

#[test]
fn a_late_files_refusal_preserves_another_guests_completed_save() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.network_chain_id = "network-a".into();
    app.fs_preview_path = "/shared/note.txt".into();
    app.fs_preview_base = "original-snapshot".into();
    let context = backend::files_context(
        app.connected_rpc.clone(),
        app.network_chain_id.clone(),
        app.connect_generation,
    );
    let mut current = save_event("/shared/note.txt", "B draft", &context);
    let mut payload: serde_json::Value = serde_json::from_str(&current.detail).unwrap();
    payload["namespace"] = "guest-b".into();
    current.detail = payload.to_string();
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(current));
    let op = app.fs_write_pending.clone();
    let _ = app.__update(__DucktapeMessage::FsWrote(
        context.clone(),
        "guest-b".into(),
        op,
        1,
        true,
    ));
    assert!(app.fs_loading, "post-write list reload is still running");
    let _ = app.__update(__DucktapeMessage::FilesViewEvent(save_event(
        "/shared/note.txt",
        "old A draft",
        &context,
    )));
    assert!(
        app.fs_save_reply
            .replies
            .iter()
            .any(|reply| reply.namespace == "guest-b" && reply.success),
        "a queued old refusal cannot hide B's completed save"
    );
    assert!(
        app.fs_save_reply
            .replies
            .iter()
            .any(|reply| reply.namespace == "guest-a" && !reply.success)
    );
}
