//! putblob staging over the module surface: per-owner quota, the deterministic
//! ttl sweep, and consensus-durability of staged bytes across a restart. every
//! test drives the real `Files` module (`execute` with the binary putblob frame,
//! then the async `commit_block` that does the real disk persist) and asserts
//! roots via the `Module` surface — exactly the production op path.
//!
//! each async call is `block_on`'d at the top level (the sync `putblob` helper
//! is the brief's, verbatim): nesting `block_on` inside an outer `block_on`
//! trips futures' LocalPool re-entry guard, so `commit_block` is driven with its
//! own `block_on` rather than an enclosing `async` block.

mod harness;
use harness::*;
use sdk::Module as _;

use files::{CHUNK_SIZE, STAGING_TTL_BLOCKS, encode_putblob};

/// drive one putblob op through the module surface, as a real block op would.
fn putblob(
    f: &mut files::Files,
    origin: sdk::Origin,
    h: u64,
    bytes: &[u8],
) -> Result<(), sdk::Error> {
    futures::executor::block_on(f.execute(
        &mut TestCtx::new(origin, h),
        &sdk::Msg {
            target: "files".into(),
            payload: encode_putblob(bytes),
        },
    ))
}

/// commit the block's pending overlay through the real disk-persist glue.
fn commit(f: &mut files::Files) {
    futures::executor::block_on(f.commit_block()).unwrap();
}

/// a distinct chunk: `len` bytes with an 8-byte counter stamped in, so each
/// `tag` hashes to a different digest (no dedup) yet per-owner byte accounting
/// stays exact and cheap regardless of chunk size.
fn distinct(len: usize, tag: u64) -> Vec<u8> {
    let mut c = vec![0u8; len];
    c[..8].copy_from_slice(&tag.to_le_bytes());
    c
}

#[test]
fn stages_within_caps_and_rejects_breaches() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // pending discipline: the committed root must not move on a stage — only
    // commit_block + adopt advances it.
    let empty_root = f.root();
    putblob(&mut f, sdk::Origin::External(b"a".to_vec()), 1, b"hello").expect("stage");
    assert_eq!(
        f.root(),
        empty_root,
        "staging must not move the root before commit"
    );
    // same-block re-put hits the staging membership no-op (store.has is still
    // false — the bytes are only in pending) — Ok, no double-stage.
    putblob(&mut f, sdk::Origin::External(b"a".to_vec()), 1, b"hello").expect("same-block re-put");
    // empty and oversized frames are not stageable objects.
    assert!(
        putblob(&mut f, sdk::Origin::System, 1, &[]).is_err(),
        "empty chunk"
    );
    let big = vec![0u8; CHUNK_SIZE as usize + 1];
    assert!(
        putblob(&mut f, sdk::Origin::System, 1, &big).is_err(),
        "oversized chunk"
    );
    // the rejected ops left the earlier same-block stage intact — commit
    // persists "hello" and only now adopts the staged refs.
    commit(&mut f);
    assert_ne!(f.root(), empty_root, "commit adopts the staged refs");
    // idempotent re-put after durability: the odb already holds it, so this is a
    // no-op via store.has (no error, no double-stage).
    putblob(&mut f, sdk::Origin::External(b"b".to_vec()), 2, b"hello").expect("no-op re-put");
}

/// the quota-per-owner + expiry LOGIC, on a shrunk quota so it runs in
/// milliseconds. the honest gibibyte version is the `#[ignore]`d twin below.
#[test]
fn quota_is_per_owner_and_expiry_frees_it_small() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // 4 chunks of 1 KiB exactly fill a 4 KiB quota; the 5th breaches it.
    let chunk = 1024usize;
    f.set_staging_quota_for_tests(4 * chunk as u64);

    for i in 0..4 {
        putblob(
            &mut f,
            sdk::Origin::External(b"alice".to_vec()),
            1,
            &distinct(chunk, i),
        )
        .expect("fill");
    }
    assert!(
        putblob(
            &mut f,
            sdk::Origin::External(b"alice".to_vec()),
            1,
            &distinct(chunk, 4)
        )
        .is_err(),
        "quota"
    );
    // bob's quota is independent — alice being full does not stop him.
    putblob(
        &mut f,
        sdk::Origin::External(b"bob".to_vec()),
        1,
        &distinct(chunk, 100),
    )
    .expect("bob unaffected");
    commit(&mut f);
    let r0 = f.root();

    // expiry: the first files op at/after height 1 + TTL sweeps every entry
    // staged at height 1 (alice's four AND bob's — all expire_at = 1 + TTL).
    putblob(
        &mut f,
        sdk::Origin::External(b"carol".to_vec()),
        1 + STAGING_TTL_BLOCKS,
        b"tick",
    )
    .unwrap();
    commit(&mut f);
    assert_ne!(f.root(), r0, "sweep must move the root (staging is state)");

    // alice's quota is freed — her swept entries no longer count.
    putblob(
        &mut f,
        sdk::Origin::External(b"alice".to_vec()),
        2 + STAGING_TTL_BLOCKS,
        &distinct(chunk, 4),
    )
    .expect("quota freed");
}

/// the brief's honest fill: 1,024 x 1 MiB chunks (~1 GiB disk in the tempdir).
/// kept for fidelity but `#[ignore]`d — the small twin above covers the same
/// logic in milliseconds. run explicitly with `cargo test -p files -- --ignored`.
#[test]
#[ignore = "stages a full gibibyte; the _small twin covers the logic"]
fn quota_is_per_owner_and_expiry_frees_it() {
    let d = tempfile::tempdir().unwrap();
    let mut f = open_files(&d);
    // fill alice's quota with max-size chunks of distinct bytes.
    let n = (files::STAGING_QUOTA_BYTES / CHUNK_SIZE) as usize;
    for i in 0..n {
        let mut c = vec![0u8; CHUNK_SIZE as usize];
        c[..8].copy_from_slice(&(i as u64).to_le_bytes());
        putblob(&mut f, sdk::Origin::External(b"alice".to_vec()), 1, &c).expect("fill");
    }
    let mut extra = vec![1u8; CHUNK_SIZE as usize];
    extra[..2].copy_from_slice(b"xx");
    assert!(
        putblob(&mut f, sdk::Origin::External(b"alice".to_vec()), 1, &extra).is_err(),
        "quota"
    );
    putblob(&mut f, sdk::Origin::External(b"bob".to_vec()), 1, &extra).expect("bob unaffected");
    commit(&mut f);
    let r0 = f.root();
    // expiry: first files op at/after height 1 + TTL sweeps alice's entries.
    putblob(
        &mut f,
        sdk::Origin::External(b"carol".to_vec()),
        1 + STAGING_TTL_BLOCKS,
        b"tick",
    )
    .unwrap();
    commit(&mut f);
    assert_ne!(f.root(), r0, "sweep must move the root (staging is state)");
    putblob(
        &mut f,
        sdk::Origin::External(b"alice".to_vec()),
        2 + STAGING_TTL_BLOCKS,
        &extra,
    )
    .expect("quota freed");
}

/// staged bytes are consensus-durable — the whole point of the design. stage a
/// chunk, commit the block, drop the module, reopen over the same dir, and prove
/// (1) the staged bytes are readable from the odb and (2) the staging table
/// survived in refs (the root is byte-identical after the restart).
#[test]
fn restart_recovers_staged_bytes_and_table() {
    use files::ObjectStore as _;

    let d = tempfile::tempdir().unwrap();
    let root_before = {
        let mut f = open_files(&d);
        putblob(
            &mut f,
            sdk::Origin::External(b"alice".to_vec()),
            1,
            b"durable chunk",
        )
        .expect("stage");
        commit(&mut f);
        f.root()
    };

    // reopen over the same dir — a durable restart. the staging table lives in
    // the refs file, so the recovered root must be byte-identical.
    let f2 = open_files(&d);
    assert_eq!(f2.root(), root_before, "staging table survived in refs");

    // and the staged bytes are in the odb, recoverable by content id — proof the
    // chunk was flushed to disk at the block commit, not just held in memory.
    let store = files::DiskStore::open(d.path().join("objects")).expect("reopen odb");
    let id = files::objects::object_id(files::Kind::Chunk, b"durable chunk");
    assert!(store.has(&id), "staged chunk durable in the odb");
    assert_eq!(
        store.get(&id).unwrap(),
        Some((files::Kind::Chunk, b"durable chunk".to_vec())),
        "the exact staged bytes round-trip from the odb"
    );
}
