use duckfs_core::{Actor, Authority, Change, Content, Fs, MemStore, ObjectStore, Refs, STAGING_TTL_BLOCKS};

fn signed(key: u8, account: Option<u64>) -> Authority {
    Authority::External { key: vec![key], account }
}

fn write(fs: &mut Fs<MemStore>, authority: &Authority, height: u64, path: &str) -> Result<(), String> {
    fs.commit(authority, height, height, None, "write".into(), vec![Change::Put {
        path: path.into(), exec: false, meta: Default::default(), content: Content::Inline { b64: String::new() },
    }]).map(|_| ())
}

fn flush(fs: &mut Fs<MemStore>) {
    let (refs, _, objects) = fs.commit_block().unwrap();
    for (kind, body) in objects { fs.store_mut().put(kind, &body).unwrap(); }
    fs.adopt_refs(refs);
}

#[test]
fn account_homes_and_old_key_rights_follow_actual_authority() {
    let mut fs = Fs::new(MemStore::new(), Refs::default());
    write(&mut fs, &signed(0xaa, None), 1, "/home/ext:aa/old").unwrap();
    let snapshot = duckfs_core::to_hex(&fs.pending_refs().head.unwrap());
    fs.pin(&signed(0xaa, None), 1, snapshot, "key pin".into()).unwrap();
    assert_eq!(fs.pending_refs().pins["key pin"].owner, Actor::Key(vec![0xaa]));
    flush(&mut fs);

    write(&mut fs, &signed(0xaa, Some(7)), 2, "/home/ext:aa/new").unwrap();
    write(&mut fs, &signed(0xaa, Some(7)), 2, "/home/acct:7/new").unwrap();
    let before = fs.pending_refs().clone();
    assert!(write(&mut fs, &signed(0xbb, Some(7)), 2, "/home/ext:aa/sibling").is_err());
    assert!(fs.unpin(&signed(0xbb, Some(7)), 2, "key pin".into()).is_err());
    assert!(write(&mut fs, &Authority::Program(7), 2, "/home/ext:aa/program").is_err());
    assert_eq!(*fs.pending_refs(), before);
    write(&mut fs, &Authority::Program(7), 2, "/home/acct:7/program").unwrap();
    // Removing/reassigning the key preserves only its historical key rights.
    assert!(write(&mut fs, &signed(0xaa, Some(8)), 2, "/home/acct:7/reassigned").is_err());
    fs.unpin(&signed(0xaa, Some(8)), 2, "key pin".into()).unwrap();
    write(&mut fs, &signed(0xaa, Some(8)), 2, "/home/ext:aa/reassigned").unwrap();
}

#[test]
fn module_names_cannot_impersonate_system_accounts_or_keys() {
    for module in ["system", "acct:7", "ext:aa"] {
        let authority = Authority::Module(module.into());
        let mut fs = Fs::new(MemStore::new(), Refs::default());
        assert!(write(&mut fs, &authority, 1, "/anywhere").is_err());
        assert!(write(&mut fs, &authority, 1, "/home/acct:7/private").is_err());
        assert!(write(&mut fs, &authority, 1, "/home/ext:aa/private").is_err());
        write(&mut fs, &authority, 1, &format!("/home/module:{module}/own")).unwrap();
        assert!(fs.watch(&authority, 1, "/shared".into(), "another".into()).is_err());
        fs.watch(&authority, 1, "/shared".into(), module.into()).unwrap();
    }
}

#[test]
fn rejected_verb_restores_expiry_and_prior_operations_and_revision_survives_reload() {
    let mut fs = Fs::new(MemStore::new(), Refs::default());
    fs.putblob(&signed(0xaa, None), 1, b"staged").unwrap();
    let staged = fs.pending_refs().clone();
    assert_eq!(staged.source_revision, 1);
    flush(&mut fs);
    let expiry = 1 + STAGING_TTL_BLOCKS;
    assert!(fs.unpin(&Authority::System, expiry, "missing".into()).is_err());
    assert_eq!(*fs.pending_refs(), staged, "refusal preserves even due chunks");
    fs.watch(&Authority::Module("watcher".into()), expiry, "/shared".into(), "watcher".into()).unwrap();
    assert!(fs.pending_refs().staging.is_empty());
    assert_eq!(fs.pending_refs().source_revision, 2);
    let prior = fs.pending_refs().clone();
    assert!(fs.putblob(&signed(0xaa, None), expiry, &[]).is_err());
    assert_eq!(*fs.pending_refs(), prior);
    fs.putblob(&signed(0xaa, None), expiry, b"next").unwrap();
    assert_eq!(fs.pending_refs().source_revision, 3);
    flush(&mut fs);
    let restored = duckfs_core::decode_refs(&fs.snapshot_refs()).unwrap();
    let mut reloaded = Fs::new(MemStore::new(), restored);
    reloaded.putblob(&signed(0xaa, None), expiry + 1, b"after reload").unwrap();
    assert_eq!(reloaded.pending_refs().source_revision, 4);
}

#[test]
fn canonical_account_and_its_actual_old_key_share_staging_quota() {
    let mut fs = Fs::new(MemStore::new(), Refs::default());
    fs.set_staging_quota_for_tests(5);
    fs.putblob(&signed(0xaa, None), 1, b"old").unwrap();
    let before = fs.pending_refs().clone();
    assert!(fs.putblob(&signed(0xaa, Some(7)), 1, b"new").is_err());
    assert_eq!(*fs.pending_refs(), before);
    fs.putblob(&signed(0xaa, Some(7)), 1, b"ok").unwrap();
    let owners: Vec<_> = fs.pending_refs().staging.values().map(|entry| entry.owner.clone()).collect();
    assert!(owners.contains(&Actor::Key(vec![0xaa])));
    assert!(owners.contains(&Actor::Account(7)));
    assert!(fs.putblob(&Authority::Program(7), 1, b"tool").is_err());
}

#[test]
fn exhausted_revision_refuses_without_changing_refs_or_objects() {
    let mut fs = Fs::new(MemStore::new(), Refs { source_revision: u64::MAX, ..Refs::default() });
    let before = fs.pending_refs().clone();
    assert!(fs.putblob(&Authority::System, 1, b"nope").unwrap_err().contains("revision exhausted"));
    assert_eq!(*fs.pending_refs(), before);
    assert!(fs.block_objects().is_empty());
}
