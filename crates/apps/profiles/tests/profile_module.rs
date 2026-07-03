//! the profiles registry through both the raw module seam and a REAL host:
//! writes are origin-gated (a submitter can only name itself), the staged
//! overlay is read-your-writes while `root()` stays committed-only, queries
//! paginate, and a snapshot verify-then-adopts to the same root.

use futures::executor::block_on;
use host::{BlockContext, Host, SubmitError};
use profiles::Profiles;
use profiles_interface::{
    MAX_NAME_LEN, Profile, ProfileMsg, ProfileQuery, ProfileReply, decode_reply, encode_msg,
    encode_query,
};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot, StateSyncHandle};

const PROFILES: &str = "profiles";

fn set_name(name: &str) -> Msg {
    Msg {
        target: PROFILES.into(),
        payload: encode_msg(&ProfileMsg::SetName {
            display_name: name.into(),
        }),
    }
}

// ---- a test ctx that carries an arbitrary origin --------------------------

struct TestCtx {
    env: Env,
}

impl TestCtx {
    fn external(who: &[u8], consensus_time: u64) -> Self {
        Self {
            env: Env {
                height: 0,
                consensus_time,
                origin: Origin::External(who.to_vec()),
                me: PROFILES.into(),
            },
        }
    }

    fn with_origin(origin: Origin) -> Self {
        Self {
            env: Env {
                height: 0,
                consensus_time: 1,
                origin,
                me: PROFILES.into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

async fn all(profiles: &Profiles) -> Vec<Profile> {
    match decode_reply(
        &profiles
            .query(&encode_query(&ProfileQuery::All {
                from: 0,
                limit: 256,
            }))
            .await
            .expect("query all"),
    )
    .expect("decode reply")
    {
        ProfileReply::Profiles(profiles) => profiles,
        other => panic!("expected Profiles, got {other:?}"),
    }
}

async fn get(profiles: &Profiles, key: &[u8]) -> Option<Profile> {
    match decode_reply(
        &profiles
            .query(&encode_query(&ProfileQuery::Get { key: key.to_vec() }))
            .await
            .expect("query get"),
    )
    .expect("decode reply")
    {
        ProfileReply::Profile(profile) => profile,
        other => panic!("expected Profile, got {other:?}"),
    }
}

#[test]
fn each_write_keys_on_its_own_origin() {
    block_on(async {
        let mut profiles = Profiles::new(PROFILES);
        let (alice, bob) = (b"alice".to_vec(), b"bob".to_vec());

        // alice names herself; bob names himself. two independent records.
        profiles
            .execute(&mut TestCtx::external(&alice, 10), &set_name("Alice"))
            .await
            .expect("alice names herself");
        profiles
            .execute(&mut TestCtx::external(&bob, 11), &set_name("Bob"))
            .await
            .expect("bob names himself");
        profiles.commit_block().await.expect("commit");

        assert_eq!(get(&profiles, &alice).await.unwrap().display_name, "Alice");
        assert_eq!(get(&profiles, &bob).await.unwrap().display_name, "Bob");

        // a write from bob CANNOT touch alice's record: it keys on bob's origin.
        // even a "Bob-as-Alice" attempt only overwrites bob's own name.
        profiles
            .execute(
                &mut TestCtx::external(&bob, 12),
                &set_name("Bob-the-usurper"),
            )
            .await
            .expect("bob renames himself");
        profiles.commit_block().await.expect("commit");

        assert_eq!(
            get(&profiles, &alice).await.unwrap().display_name,
            "Alice",
            "alice's name is untouched by bob's write"
        );
        assert_eq!(
            get(&profiles, &bob).await.unwrap().display_name,
            "Bob-the-usurper"
        );
        assert_eq!(get(&profiles, &bob).await.unwrap().updated_at, 12);
    });
}

#[test]
fn empty_name_clears_and_overlong_rejects() {
    block_on(async {
        let mut profiles = Profiles::new(PROFILES);
        let alice = b"alice".to_vec();

        profiles
            .execute(&mut TestCtx::external(&alice, 1), &set_name("Alice"))
            .await
            .expect("set");
        profiles.commit_block().await.expect("commit");
        assert!(get(&profiles, &alice).await.is_some());

        // a name that trims to empty clears the record.
        profiles
            .execute(&mut TestCtx::external(&alice, 2), &set_name("   "))
            .await
            .expect("clear via whitespace");
        profiles.commit_block().await.expect("commit");
        assert!(
            get(&profiles, &alice).await.is_none(),
            "trimming to empty removes the record"
        );
        assert!(all(&profiles).await.is_empty());

        // an over-long name is rejected; nothing is staged.
        let overlong = "x".repeat(MAX_NAME_LEN + 1);
        let err = profiles
            .execute(&mut TestCtx::external(&alice, 3), &set_name(&overlong))
            .await
            .expect_err("over-long name rejects");
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("exceeds")),
            "{err:?}"
        );

        // a name AT the limit is accepted; the stored name is trimmed.
        let at_limit = "y".repeat(MAX_NAME_LEN);
        profiles
            .execute(
                &mut TestCtx::external(&alice, 4),
                &set_name(&format!("  {at_limit}  ")),
            )
            .await
            .expect("at-limit name after trim");
        profiles.commit_block().await.expect("commit");
        assert_eq!(get(&profiles, &alice).await.unwrap().display_name, at_limit);
    });
}

#[test]
fn non_external_origins_are_refused() {
    block_on(async {
        let mut profiles = Profiles::new(PROFILES);
        for origin in [
            Origin::System,
            Origin::Module("chat".into()),
            Origin::External(Vec::new()),
        ] {
            let err = profiles
                .execute(&mut TestCtx::with_origin(origin.clone()), &set_name("Nope"))
                .await
                .expect_err("only a non-empty external origin may write");
            assert!(matches!(err, Error::Module(_)), "{origin:?} -> {err:?}");
        }
        profiles.commit_block().await.expect("commit");
        assert!(all(&profiles).await.is_empty(), "no record was staged");
    });
}

#[test]
fn root_reflects_committed_state_only() {
    block_on(async {
        let mut profiles = Profiles::new(PROFILES);
        let root0 = profiles.root();
        let alice = b"alice".to_vec();

        profiles
            .execute(&mut TestCtx::external(&alice, 7), &set_name("Alice"))
            .await
            .expect("stage set");
        assert_eq!(
            profiles.root(),
            root0,
            "a staged write must not move the committed root"
        );
        assert_eq!(
            all(&profiles).await.len(),
            1,
            "queries read through the staged overlay"
        );

        profiles.commit_block().await.expect("commit");
        let root1 = profiles.root();
        assert_ne!(root1, root0, "commit moves the root");

        // stage then ABORT: root is byte-identical, the overlay is discarded.
        profiles
            .execute(&mut TestCtx::external(&alice, 8), &set_name("Alice2"))
            .await
            .expect("stage rename");
        assert_eq!(profiles.root(), root1, "root stays committed-only");
        profiles.abort_block().await.expect("abort");
        assert_eq!(
            profiles.root(),
            root1,
            "abort keeps the root byte-identical"
        );
        assert_eq!(get(&profiles, &alice).await.unwrap().display_name, "Alice");
    });
}

#[test]
fn all_paginates_ascending_by_key() {
    block_on(async {
        let mut profiles = Profiles::new(PROFILES);
        // keys chosen so byte-ascending order is a, b, c, d.
        for (i, k) in [b"a", b"b", b"c", b"d"].into_iter().enumerate() {
            profiles
                .execute(
                    &mut TestCtx::external(k, i as u64),
                    &set_name(&format!("name-{}", k[0] as char)),
                )
                .await
                .expect("set");
        }
        profiles.commit_block().await.expect("commit");

        let page = match decode_reply(
            &profiles
                .query(&encode_query(&ProfileQuery::All { from: 1, limit: 2 }))
                .await
                .expect("query"),
        )
        .expect("decode")
        {
            ProfileReply::Profiles(p) => p,
            other => panic!("{other:?}"),
        };
        let keys: Vec<Vec<u8>> = page.iter().map(|p| p.key.clone()).collect();
        assert_eq!(
            keys,
            vec![b"b".to_vec(), b"c".to_vec()],
            "offset 1 limit 2 is the ascending slice [b, c]"
        );
    });
}

#[test]
fn snapshot_install_reconstructs_and_matches_root() {
    block_on(async {
        let mut source = Profiles::new(PROFILES);
        source
            .execute(&mut TestCtx::external(b"alice", 5), &set_name("Alice"))
            .await
            .expect("set");
        source
            .execute(&mut TestCtx::external(b"bob", 6), &set_name("Bob"))
            .await
            .expect("set");
        source.commit_block().await.expect("commit");

        // the advertised handle is self-contained snapshot bytes...
        let handle = source.state_sync_handle().expect("handle");
        let bytes = match handle {
            StateSyncHandle::SnapshotBytes(bytes) => bytes,
            other => panic!("expected SnapshotBytes, got {other:?}"),
        };

        // ...that install verbatim on a joiner against the source root.
        let mut target = Profiles::new(PROFILES);
        target.install(&bytes, source.root()).expect("install");
        assert_eq!(target.root(), source.root());
        assert_eq!(all(&target).await, all(&source).await);

        // a corrupted image is refused against the honest root.
        let mut tampered = bytes.clone();
        *tampered.last_mut().unwrap() ^= 0xff;
        let mut victim = Profiles::new(PROFILES);
        assert!(
            victim.install(&tampered, source.root()).is_err(),
            "a root mismatch must refuse the snapshot"
        );
    });
}

// ---- through a REAL host, with authenticated origins ----------------------

async fn host_get(host: &Host, key: &[u8]) -> Option<Profile> {
    match decode_reply(
        &host
            .query(
                PROFILES,
                &encode_query(&ProfileQuery::Get { key: key.to_vec() }),
            )
            .await
            .expect("query"),
    )
    .expect("decode")
    {
        ProfileReply::Profile(p) => p,
        other => panic!("{other:?}"),
    }
}

async fn submit_as(host: &mut Host, who: &[u8], at: u64, msg: Msg) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
            height: at,
            consensus_time: at,
            origin: Origin::External(who.to_vec()),
        },
        msg,
    )
    .await
    .map(|_| ())
}

#[test]
fn host_binds_names_to_the_submit_origin_and_moves_the_app_hash() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Profiles::new(PROFILES))]).expect("genesis");
        let app0 = host.app_hash();
        let (alice, bob) = (b"alice".to_vec(), b"bob".to_vec());

        submit_as(&mut host, &alice, 1, set_name("Alice"))
            .await
            .expect("alice names herself");
        let app1 = host.app_hash();
        assert_ne!(app1, app0, "a committed name moves the app-hash");

        submit_as(&mut host, &bob, 2, set_name("Bob"))
            .await
            .expect("bob names himself");
        assert_ne!(host.app_hash(), app1, "a second identity moves it again");

        assert_eq!(host_get(&host, &alice).await.unwrap().display_name, "Alice");
        assert_eq!(host_get(&host, &bob).await.unwrap().display_name, "Bob");

        // a module/system origin cannot write: the block is rejected and the
        // app-hash is unchanged.
        let app2 = host.app_hash();
        let err = host
            .submit_at(
                BlockContext {
                    height: 3,
                    consensus_time: 3,
                    origin: Origin::System,
                },
                set_name("system"),
            )
            .await
            .expect_err("system origin is refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(_))),
            "{err:?}"
        );
        assert_eq!(
            host.app_hash(),
            app2,
            "a rejected block leaves the app-hash"
        );
    });
}
