//! the framework proven against its own dummy package: a `PackageTestBed`
//! boots the standard platform set plus the in-crate `DummyHarness`, installs
//! the `tests/fixtures/dummy` capsule, scripts oracle turns, and asserts the
//! ADR harness checklist — install seeds + registrations, engagement -> one
//! job, scripted provider output mutating the intended target, unauthorized
//! actions mutating nothing, suspend/unplug semantics, snapshot round-trips —
//! then replays the same script through the golden runner (the exact path the
//! CLI's `package test` drives).

use std::collections::BTreeMap;
use std::path::Path;

use agent::{AgentQuery, AgentReply, AgentStatus, encode_query as agent_encode_query};
use quack_harness::dummy::{ACTION_NOTE_ADD, ACTION_NOTE_SET_TEXT, DummyHarness};
use quack_harness::{
    GoldenFixture, InstallReport, PackageStatus, PackageTestBed, RoundtripKind, diff_json,
    parse_origin, run_golden,
};
use saga::SagaOrigin;
use sdk::{Module, Origin};
use serde_json::json;

const PKG: &str = "org.example.dummy";
const HARNESS: &str = "dummy-harness";
const AGENT: &str = "dummy.note-taker";
const PROMPT_TEXT: &str = "You are the dummy note taker. Reply ONLY with dummy.note actions.\n";

fn fixture_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dummy"))
}

fn dummy_capsule() -> quack::Capsule {
    quack::open_dir(fixture_dir()).expect("the dummy fixture dir opens")
}

fn dummy_modules() -> Vec<Box<dyn Module>> {
    vec![Box::new(DummyHarness::new(
        HARNESS, "package", "agent", "jobs", "memory",
    ))]
}

fn alice() -> Origin {
    Origin::External(b"alice".to_vec())
}

async fn install(bed: &mut PackageTestBed) -> InstallReport {
    bed.install_capsule(&dummy_capsule(), HARNESS, &BTreeMap::new(), alice())
        .await
        .expect("the dummy package installs")
}

fn page_op(payload: serde_json::Value) -> (String, serde_json::Value) {
    ("pages".to_string(), payload)
}

/// create the page + drop a mention comment — the standard engagement setup.
async fn engage(bed: &mut PackageTestBed, comment_id: &str, text: &str) {
    for (target, payload) in [
        page_op(json!({"create_page": {"page_id": "p1", "title": "Dummy", "parent": null}})),
        page_op(json!({"add_comment": {
            "thread_id": format!("t-{comment_id}"),
            "comment_id": comment_id,
            "target": "p1",
            "text": text,
        }})),
    ] {
        // page creation is idempotent, so re-running per comment is benign.
        bed.submit_json(alice(), &target, &payload)
            .await
            .expect("pages op commits");
    }
}

fn mention(text: &str) -> String {
    format!("@{AGENT} {text}")
}

// ---- install --------------------------------------------------------------------

#[test]
fn install_report_covers_the_adr_checklist_surface() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        let report = install(&mut bed).await;

        // the row activated in the install block (harness MarkActive ack).
        assert_eq!(report.package, PKG);
        assert_eq!(report.status, PackageStatus::Active);
        assert_eq!(report.harness, HARNESS);
        report.assert_active();

        // prompts seeded with expected hashes, generation pinned.
        assert_eq!(report.prompts.len(), 1);
        let seeded = &report.prompts[0];
        assert_eq!(seeded.path, format!("/packages/{PKG}/prompts/dummy.md"));
        assert_eq!(seeded.generation, 1);
        report.assert_prompt_seeded(&seeded.path.clone(), PROMPT_TEXT);

        // agents registered FROM THE HARNESS ORIGIN (harness-owned).
        assert_eq!(report.agents.len(), 1);
        let registered = &report.agents[0];
        assert_eq!(registered.agent_id, AGENT);
        assert_eq!(registered.owner, SagaOrigin::Module(HARNESS.into()));
        assert_eq!(registered.status, AgentStatus::Active);
        let prompt = registered.prompt.as_ref().expect("prompt pinned");
        assert_eq!(prompt.target, format!("{}@1", seeded.path));
        report.assert_agent_owned_by_harness(AGENT);

        // action tags resolve to the harness as owner.
        report.assert_route(ACTION_NOTE_ADD, HARNESS);
        report.assert_route(ACTION_NOTE_SET_TEXT, HARNESS);
        bed.assert_action_owner(ACTION_NOTE_ADD, Some(HARNESS))
            .await;

        // a second install of the same package id rejects (id stays claimed).
        let again = bed
            .install_capsule(&dummy_capsule(), HARNESS, &BTreeMap::new(), alice())
            .await;
        assert!(again.is_err(), "duplicate install must reject");
    });
}

#[test]
fn install_spec_mapping_rejects_a_tampered_capsule() {
    // a digest mismatch fails BEFORE any op is submitted.
    let mut tampered = dummy_capsule();
    tampered.insert("prompts/dummy.md", b"tampered".to_vec());
    let err = quack_harness::install_spec_from_capsule(&tampered, HARNESS, &BTreeMap::new())
        .expect_err("tampered prompt bytes must fail digest verification");
    assert!(err.contains("digest"), "readable failure, got: {err}");

    // an unbound harness logical fails the mapping too.
    let err = quack_harness::install_spec_from_capsule(&dummy_capsule(), "ghost", &BTreeMap::new())
        .expect_err("an undeclared harness logical must fail");
    assert!(err.contains("ghost"), "names the bad logical, got: {err}");
}

// ---- the engagement -> job -> oracle -> apply loop ---------------------------------

#[test]
fn a_mention_comment_mints_exactly_one_job_and_the_scripted_action_lands() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        install(&mut bed).await;
        engage(&mut bed, "c1", &mention("please note this")).await;

        // exactly one job, claimed by the runs worker in the submit cascade.
        bed.assert_job_count(&format!("agent/{AGENT}"), 1).await;
        bed.assert_pending_run_for_agent(AGENT, true).await;

        // a non-mention comment engages nothing.
        engage(&mut bed, "c2", "no robots were addressed here").await;
        bed.assert_job_count(&format!("agent/{AGENT}"), 1).await;

        // the scripted oracle turn: the canned response's note action.
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": ACTION_NOTE_ADD,
                         "payload": {"note_id": "n1", "text": "noted"}}],
        }))
        .await
        .expect("oracle block commits");
        // nothing reaches the owner until the delivery block (never-pop-stack).
        let notes = bed.query_json(HARNESS, &json!("notes")).await.unwrap();
        assert_eq!(notes, json!({"notes": []}));

        bed.deliver().await.expect("delivery block commits");
        let notes = bed.query_json(HARNESS, &json!("notes")).await.unwrap();
        assert_eq!(
            notes,
            json!({"notes": [{"note_id": "n1", "text": "noted"}]})
        );
        bed.assert_pending_run_for_agent(AGENT, false).await;
        bed.assert_job_status(&format!("dummy:{AGENT}:c1"), jobs::JobStatus::Done)
            .await;
    });
}

#[test]
fn an_ungranted_action_mutates_nothing_and_records_failure() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        install(&mut bed).await;
        engage(&mut bed, "c1", &mention("do something sneaky")).await;

        // the oracle asks for a tag the agent was never granted.
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": "tasks.create",
                         "payload": {"task_id": "sneaky", "title": "not granted"}}],
        }))
        .await
        .expect("the disallowed response must not abort the oracle block");
        bed.deliver()
            .await
            .expect("the disallowed response must not abort the delivery block");

        // nothing mutated; the failure is recorded as a breadcrumb + failed job.
        let tasks = bed.query_json("tasks", &json!("list")).await.unwrap();
        assert_eq!(tasks, json!({"tasks": []}));
        bed.assert_failure_breadcrumb("runs", "is not allowed to tasks.create");
        bed.assert_job_status(&format!("dummy:{AGENT}:c1"), jobs::JobStatus::Failed)
            .await;
    });
}

#[test]
fn a_sibling_action_targeting_a_same_response_create_probe_rejects() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        install(&mut bed).await;
        engage(&mut bed, "c1", &mention("note then edit")).await;

        // the documented caveat: an owner's probe sees staged-or-committed
        // state, NEVER sibling actions from the same response — the set_text
        // targeting the same response's create drops; the create lands.
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [
                {"action_id": "a1", "tag": ACTION_NOTE_ADD,
                 "payload": {"note_id": "n1", "text": "first"}},
                {"action_id": "a2", "tag": ACTION_NOTE_SET_TEXT,
                 "payload": {"note_id": "n1", "text": "second"}},
            ],
        }))
        .await
        .expect("oracle block");
        bed.deliver().await.expect("delivery block");

        let notes = bed.query_json(HARNESS, &json!("notes")).await.unwrap();
        assert_eq!(
            notes,
            json!({"notes": [{"note_id": "n1", "text": "first"}]}),
            "the create landed; the sibling edit was probe-rejected"
        );
        bed.assert_failure_breadcrumb("runs", "rejected by dummy-harness");
    });
}

// ---- lifecycle ------------------------------------------------------------------

#[test]
fn suspend_stops_minting_and_unplug_tombstones_while_preserving_user_data() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        install(&mut bed).await;
        engage(&mut bed, "c1", &mention("note this")).await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": ACTION_NOTE_ADD,
                         "payload": {"note_id": "n1", "text": "kept"}}],
        }))
        .await
        .expect("oracle block");
        bed.deliver().await.expect("delivery block");

        // suspend: no new jobs, existing data preserved.
        bed.submit_json(alice(), "package", &json!({"suspend": {"package": PKG}}))
            .await
            .expect("suspend commits");
        engage(&mut bed, "c2", &mention("anyone home?")).await;
        bed.assert_job_count(&format!("agent/{AGENT}"), 1).await;

        // resume: minting restarts.
        bed.submit_json(alice(), "package", &json!({"resume": {"package": PKG}}))
            .await
            .expect("resume commits");
        engage(&mut bed, "c3", &mention("welcome back")).await;
        bed.assert_job_count(&format!("agent/{AGENT}"), 2).await;

        // unplug: routes gone, agent tombstoned, notes (user data) intact.
        bed.submit_json(alice(), "package", &json!({"unplug": {"package": PKG}}))
            .await
            .expect("unplug commits");
        bed.assert_action_owner(ACTION_NOTE_ADD, None).await;
        let reply = bed
            .query(
                "agent",
                &agent_encode_query(&AgentQuery::Agent {
                    agent_id: AGENT.into(),
                }),
            )
            .await
            .unwrap();
        let AgentReply::Agent(Some(record)) = agent::decode_reply(&reply).unwrap() else {
            panic!("the tombstoned agent record must survive for audit");
        };
        assert_eq!(record.status, AgentStatus::Tombstoned);
        let notes = bed.query_json(HARNESS, &json!("notes")).await.unwrap();
        assert_eq!(notes, json!({"notes": [{"note_id": "n1", "text": "kept"}]}));

        // engagement after unplug is a dead letter: the hook is unregistered.
        engage(&mut bed, "c4", &mention("ghost")).await;
        bed.assert_job_count(&format!("agent/{AGENT}"), 2).await;
    });
}

// ---- snapshot round-trips -----------------------------------------------------------

#[test]
fn snapshot_roundtrip_all_covers_every_registered_module() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        install(&mut bed).await;
        engage(&mut bed, "c1", &mention("note this")).await;

        let report = bed
            .snapshot_roundtrip_all()
            .await
            .expect("every module round-trips");
        let kind_of = |id: &str| {
            report
                .iter()
                .find(|m| m.id == id)
                .unwrap_or_else(|| panic!("module {id} missing from the sweep"))
                .kind
                .clone()
        };
        // qmdb substrates verify their served resolver target against the root.
        assert_eq!(kind_of("chat"), RoundtripKind::ResolverVerified);
        assert_eq!(kind_of("pages"), RoundtripKind::ResolverVerified);
        // platform snapshot-bytes modules re-install into a FRESH instance.
        for id in [
            "tagging",
            "saga",
            "dispatch",
            "agent",
            "runs",
            "tasks",
            "jobs",
            "memory",
            "package",
            "capability",
        ] {
            assert_eq!(kind_of(id), RoundtripKind::Reinstalled, "{id}");
        }
        // caller-supplied modules verify the snapshot-preimage convention.
        assert_eq!(kind_of(HARNESS), RoundtripKind::PreimageVerified);
        assert_eq!(report.len(), 13, "one entry per registered module");
    });
}

// ---- golden fixtures -----------------------------------------------------------------

#[test]
fn the_golden_fixture_replays_end_to_end() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        let capsule = dummy_capsule();
        let fixture = GoldenFixture::from_capsule(&capsule).expect("fixture parses");
        assert_eq!(fixture.package, PKG);
        let run = run_golden(&mut bed, &capsule, &fixture)
            .await
            .unwrap_or_else(|e| panic!("golden run failed: {e}"));
        assert_eq!(run.steps.len(), fixture.steps.len());
        assert!(run.install.is_some(), "the install step reports");
    });
}

#[test]
fn golden_parsing_is_strict() {
    // unknown step kinds reject.
    let err = GoldenFixture::parse(
        br#"{"schema":1,"package":"p","harness":"h","steps":[{"explode":{}}]}"#,
    )
    .expect_err("unknown step kind must reject");
    assert!(err.contains("explode") || err.contains("unknown"), "{err}");

    // unknown fields inside a known step reject.
    assert!(
        GoldenFixture::parse(
            br#"{"schema":1,"package":"p","harness":"h","steps":[{"deliver":{"bogus":1}}]}"#,
        )
        .is_err(),
        "unknown fields must reject"
    );

    // a wrong schema rejects.
    assert!(
        GoldenFixture::parse(br#"{"schema":2,"package":"p","harness":"h","steps":[]}"#,).is_err()
    );

    // origins parse strictly: only external:<hex> / module:<id>.
    assert_eq!(
        parse_origin("external:616c696365").unwrap(),
        Origin::External(b"alice".to_vec())
    );
    assert_eq!(
        parse_origin("module:dummy-harness").unwrap(),
        Origin::Module("dummy-harness".into())
    );
    for bad in [
        "external:",
        "external:6",
        "external:GG",
        "external:6A",
        "module:",
        "system",
        "user:alice",
        "",
    ] {
        assert!(parse_origin(bad).is_err(), "{bad:?} must reject");
    }
}

#[test]
fn every_golden_step_kind_fails_readably() {
    PackageTestBed::run(dummy_modules(), |mut bed| async move {
        let capsule = dummy_capsule();

        let fixture_of = |steps: serde_json::Value| {
            GoldenFixture::parse(
                serde_json::to_vec(&json!({
                    "schema": 1, "package": PKG, "harness": HARNESS, "steps": steps,
                }))
                .unwrap()
                .as_slice(),
            )
            .expect("fixture shape parses")
        };

        // install: a fixture whose package id disagrees with the capsule.
        let bad = GoldenFixture::parse(
            serde_json::to_vec(&json!({
                "schema": 1, "package": "org.example.other", "harness": HARNESS,
                "steps": [{"install": {"origin": "external:616c696365"}}],
            }))
            .unwrap()
            .as_slice(),
        )
        .unwrap();
        let err = run_golden(&mut bed, &capsule, &bad)
            .await
            .expect_err("package mismatch fails");
        assert!(err.to_string().contains("org.example.other"), "{err}");

        // oracle with no pending dispatch.
        let fixture =
            fixture_of(json!([{"oracle": {"response": {"reply_blocks": [], "actions": []}}}]));
        let err = run_golden(&mut bed, &capsule, &fixture)
            .await
            .expect_err("no pending oracle request");
        assert!(err.to_string().contains("no pending"), "{err}");

        // a submit that the block rejects (unknown package lifecycle op).
        let fixture = fixture_of(json!([{"submit": {
            "origin": "external:616c696365", "target": "package",
            "payload": {"suspend": {"package": "ghost"}},
        }}]));
        let err = run_golden(&mut bed, &capsule, &fixture)
            .await
            .expect_err("a rejected submit fails the step");
        assert!(err.to_string().contains("rejected"), "{err}");

        // ...unless the fixture EXPECTS the rejection.
        let fixture = fixture_of(json!([{"submit": {
            "origin": "external:616c696365", "target": "package",
            "payload": {"suspend": {"package": "ghost"}}, "expect": "rejected",
        }}]));
        run_golden(&mut bed, &capsule, &fixture)
            .await
            .expect("an expected rejection passes");

        // expect_query mismatch carries a readable JSON diff.
        let fixture = fixture_of(json!([{"expect_query": {
            "module": "tasks", "query": "list", "expect": {"tasks": [{"id": "ghost"}]},
        }}]));
        let err = run_golden(&mut bed, &capsule, &fixture)
            .await
            .expect_err("query mismatch fails");
        let msg = err.to_string();
        assert!(msg.contains("$.tasks"), "diff names the path: {msg}");

        // expect_job with the wrong count.
        let fixture = fixture_of(json!([{"expect_job": {"kind_prefix": "agent/", "count": 7}}]));
        let err = run_golden(&mut bed, &capsule, &fixture)
            .await
            .expect_err("job count mismatch fails");
        assert!(err.to_string().contains("7"), "{err}");

        // expect_run for a run that does not exist.
        let fixture = fixture_of(json!([{"expect_run": {"agent": "ghost", "exists": true}}]));
        assert!(run_golden(&mut bed, &capsule, &fixture).await.is_err());

        // expect_failure_row with no matching breadcrumb.
        let fixture =
            fixture_of(json!([{"expect_failure_row": {"source": "runs", "contains": "nope"}}]));
        assert!(run_golden(&mut bed, &capsule, &fixture).await.is_err());

        // an expect_run selector-free step is a fixture error.
        let fixture = fixture_of(json!([{"expect_run": {"exists": true}}]));
        assert!(run_golden(&mut bed, &capsule, &fixture).await.is_err());
    });
}

#[test]
fn diff_json_names_the_first_divergent_path() {
    let expected = json!({"a": [{"b": 1}, {"b": 2}]});
    let actual = json!({"a": [{"b": 1}, {"b": 3}]});
    let diff = diff_json(&expected, &actual).expect("differs");
    assert!(diff.contains("$.a[1].b"), "{diff}");
    assert!(diff_json(&expected, &expected).is_none());
}
