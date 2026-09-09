use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn a_file_preview_reads_the_snapshot_it_will_use_for_save() {
    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc = rpc_client(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                value = listener.accept() => value.unwrap(),
                _ = &mut stopped => break,
            };
            let (mut socket, _) = accepted;
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
                assert!(request.len() <= 8192);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap();
            let body = if request.starts_with("GET /v1/files/refs") {
                serde_json::json!({"head": BASE})
            } else {
                assert!(request.starts_with("GET /v1/files/read"), "{request}");
                let bytes = if request.contains(&format!("snapshot={BASE}")) {
                    b"A source"
                } else {
                    b"B source"
                };
                serde_json::json!({"b64": base64_encode(bytes), "eof": true})
            }
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let preview = files_text(&rpc, "/shared/note.txt".into(), 7)
        .await
        .unwrap();
    stop.send(()).unwrap();
    server.await.unwrap();
    assert_eq!(preview.base_snapshot, BASE);
    assert_eq!(
        preview.text, "A source",
        "the read must not race the head after choosing its save base"
    );
}

#[test]
fn a_retained_file_base_refuses_an_external_edit_of_the_same_path() {
    use duckfs_core::{
        Authority, Change, Content, FilesQuery, FilesReply, Fs, MemStore, ObjectStore, Refs,
    };
    let mut fs = Fs::new(MemStore::new(), Refs::default());
    let authority = Authority::External {
        key: vec![0xaa],
        account: None,
    };
    let change = |text: &str| Change::Put {
        path: "/shared/note.txt".into(),
        exec: false,
        meta: Default::default(),
        content: Content::Inline {
            b64: base64_encode(text.as_bytes()),
        },
    };
    let flush = |fs: &mut Fs<MemStore>| {
        let (refs, _, objects) = fs.commit_block().unwrap();
        for (kind, body) in objects {
            fs.store_mut().put(kind, &body).unwrap();
        }
        fs.adopt_refs(refs);
    };
    fs.commit(&authority, 1, 1, None, "A".into(), vec![change("A source")])
        .unwrap();
    flush(&mut fs);
    let base = duckfs_core::to_hex(&fs.pending_refs().head.unwrap());
    fs.commit(
        &authority,
        2,
        2,
        Some(base.clone()),
        "external B".into(),
        vec![change("B source")],
    )
    .unwrap();
    flush(&mut fs);
    let before = fs.pending_refs().clone();
    let bytes = files_commit_payload(
        Some(base.clone()),
        "save draft".into(),
        serde_json::to_value(change("unsaved A")).unwrap(),
    )
    .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        payload["commit"]["base_snapshot"], base,
        "saving must retain the read snapshot"
    );
    let changes: Vec<Change> =
        serde_json::from_value(payload["commit"]["changes"].clone()).unwrap();
    let Err(error) = fs.commit(
        &authority,
        3,
        3,
        serde_json::from_value(payload["commit"]["base_snapshot"].clone()).unwrap(),
        "draft".into(),
        changes,
    ) else {
        panic!("the original snapshot must conflict with the external edit");
    };
    assert!(error.contains("changed since base"), "{error}");
    assert_eq!(
        *fs.pending_refs(),
        before,
        "conflict cannot change the current file"
    );
    let reply = fs
        .query(FilesQuery::Read {
            path: "/shared/note.txt".into(),
            snapshot: None,
            offset: 0,
            len: 100,
        })
        .unwrap();
    let FilesReply::Read { b64, .. } = reply else {
        panic!("not a read reply")
    };
    assert_eq!(base64_decode(&b64).unwrap(), b"B source");
}

#[test]
fn old_refusals_cannot_erase_newer_files_save_confirmations() {
    let success = fs_save_reply(
        "connection".into(),
        "new-guest".into(),
        2,
        true,
        String::new(),
        no_fs_save_reply(),
    );
    let with_old = fs_save_reply(
        "connection".into(),
        "old-guest".into(),
        1,
        false,
        "old refusal".into(),
        success,
    );
    assert!(
        with_old
            .replies
            .iter()
            .any(|reply| reply.namespace == "new-guest" && reply.request == 2 && reply.success),
        "an old refused request must not erase B's success"
    );
    let previous = with_old.clone();
    let older = fs_save_reply(
        "connection".into(),
        "new-guest".into(),
        1,
        false,
        "obsolete".into(),
        with_old,
    );
    assert_eq!(
        older, previous,
        "request ordering is within a guest namespace"
    );
    let duplicate = fs_save_reply(
        "connection".into(),
        "new-guest".into(),
        2,
        false,
        "duplicate refusal".into(),
        older,
    );
    assert_eq!(
        duplicate, previous,
        "a confirmed request cannot become refused"
    );
}

#[test]
fn files_save_history_has_fixed_identity_and_message_budgets_with_explicit_loss() {
    let mut history = no_fs_save_reply();
    for n in 0..8 {
        history = fs_save_reply(
            "connection".into(),
            format!("guest-{n}"),
            1,
            false,
            "한".repeat(1000),
            history,
        );
    }
    assert_eq!(history.replies.len(), 8);
    assert!(history.overflow.is_empty());
    assert!(
        history
            .replies
            .iter()
            .all(|reply| reply.message.len() <= 512
                && reply.message.is_char_boundary(reply.message.len()))
    );
    let history = fs_save_reply(
        "connection".into(),
        "guest-8".into(),
        1,
        false,
        "last".into(),
        history,
    );
    assert_eq!(history.replies.len(), 8);
    assert!(
        !history.overflow.is_empty(),
        "reply eviction must be visible to pending guests"
    );
    assert!(
        history
            .replies
            .iter()
            .all(|reply| reply.namespace != "guest-0")
    );
    assert_eq!(history.replies.last().unwrap().namespace, "guest-8");
}
