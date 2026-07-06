//! the docs package proven end to end through `quack-harness` — the ADR's
//! Harness Requirements checklist, verbatim, against the REAL platform stack
//! (`packages/docs/` source capsule -> install -> pages events -> jobs ->
//! runs -> canned oracle -> probe/apply -> pages writes):
//!
//! - install seeds prompt records and registers package agents with expected
//!   hashes; action tags resolve to `docs-harness` as owner;
//! - a comment mentioning `docs.editor` creates exactly one
//!   `agent/docs.editor` job (idempotent on literal redelivery; non-mentions
//!   are no-ops; the harness's OWN apply-induced comments never re-engage —
//!   loop prevention);
//! - fake provider output requesting `pages.block.update_text` edits the
//!   intended block (guarded by `expected_hash`);
//! - unauthorized or malformed actions mutate nothing and record failure
//!   (runs drop log + the harness's committed error rows);
//! - malformed page events are no-op observations, not block aborts;
//! - suspend stops new jobs while preserving pages and comments; unplug
//!   removes hooks/action routes and tombstones agents while preserving user
//!   data;
//! - package module snapshots/state sync reproduce the same roots;
//! - and the capsule's `harness/golden.json` replays through the golden
//!   runner — the exact script `ducktape-node package test` drives.

use std::collections::BTreeMap;
use std::path::Path;

use agent::{AgentQuery, AgentReply, AgentStatus, encode_query as agent_encode_query};
use docs_harness::{
    ACTION_BLOCK_UPDATE_TEXT, ACTION_COMMENT_ADD, ACTION_THREAD_RESOLVE, DocsHarness,
};
use quack_harness::{
    GoldenFixture, InstallReport, PackageStatus, PackageTestBed, RoundtripKind,
    install_spec_from_capsule_defaulted, run_golden,
};
use saga::SagaOrigin;
use sdk::{Module, Origin};
use serde_json::json;

const PKG: &str = "org.ducktape.docs";
const HARNESS: &str = "docs-harness";
const AGENT: &str = "docs.editor";
const PROMPT_PATH: &str = "/packages/org.ducktape.docs/prompts/docs-editor.md";
/// sha256 of "Draft intro." — the golden block's seeded text.
const DRAFT_HASH: &str = "sha256:0b5330053f4c2f56493fa924fc97451eb462bb28e0f071502a0c5aaab14a7127";

fn docs_dir() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../packages/docs"
    ))
}

fn docs_capsule() -> quack::Capsule {
    quack::open_dir(docs_dir()).expect("the docs package source opens")
}

fn docs_modules() -> Vec<Box<dyn Module>> {
    vec![Box::new(DocsHarness::new(
        HARNESS, "package", "agent", "jobs", "memory", "pages", "runs",
    ))]
}

fn alice() -> Origin {
    Origin::External(b"alice".to_vec())
}

async fn install(bed: &mut PackageTestBed) -> InstallReport {
    bed.install_capsule(&docs_capsule(), HARNESS, &BTreeMap::new(), alice())
        .await
        .expect("the docs package installs")
}

/// page p1 with one paragraph block b1 ("Draft intro.") — the golden target.
async fn seed_page(bed: &mut PackageTestBed) {
    for payload in [
        json!({"create_page": {"page_id": "p1", "title": "Docs", "parent": null}}),
        json!({"insert_block": {"parent": "p1", "after": null,
               "block": {"id": "b1", "kind": "paragraph", "text": "Draft intro."}}}),
    ] {
        bed.submit_json(alice(), "pages", &payload)
            .await
            .expect("pages op commits");
    }
}

/// drop a comment on block b1 (each id pair opens its own thread).
async fn comment(bed: &mut PackageTestBed, thread_id: &str, comment_id: &str, text: &str) {
    bed.submit_json(
        alice(),
        "pages",
        &json!({"add_comment": {
            "thread_id": thread_id,
            "comment_id": comment_id,
            "target": "b1",
            "text": text,
        }}),
    )
    .await
    .expect("comment commits");
}

async fn block_text(bed: &PackageTestBed, block_id: &str) -> String {
    let reply = bed
        .query_json("pages", &json!({"get_block": {"block_id": block_id}}))
        .await
        .expect("pages query");
    reply["block"]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("block {block_id} does not exist: {reply}"))
        .to_string()
}

fn kind() -> String {
    format!("agent/{AGENT}")
}

// ---- install --------------------------------------------------------------------

#[test]
fn install_covers_the_adr_install_checklist() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        let report = install(&mut bed).await;

        // the row activated in the install block (harness MarkActive ack).
        assert_eq!(report.package, PKG);
        assert_eq!(report.status, PackageStatus::Active);
        assert_eq!(report.harness, HARNESS);
        report.assert_active();

        // the prompt seeded with its expected hash, generation pinned — the
        // expected content is the REAL capsule file, never a copy.
        let capsule = docs_capsule();
        let prompt_bytes = capsule
            .files
            .get("prompts/docs-editor.md")
            .expect("the capsule ships the prompt");
        let prompt_text = std::str::from_utf8(prompt_bytes).unwrap();
        assert_eq!(report.prompts.len(), 1);
        let seeded = &report.prompts[0];
        assert_eq!(seeded.path, PROMPT_PATH);
        assert_eq!(seeded.generation, 1);
        report.assert_prompt_seeded(PROMPT_PATH, prompt_text);

        // the agent registered FROM THE HARNESS ORIGIN (harness-owned), its
        // pin pointing at the seeded generation, granted EXACTLY the three
        // pages tags (replies ride the job result — no chat.post).
        assert_eq!(report.agents.len(), 1);
        let registered = &report.agents[0];
        assert_eq!(registered.agent_id, AGENT);
        assert_eq!(registered.owner, SagaOrigin::Module(HARNESS.into()));
        assert_eq!(registered.status, AgentStatus::Active);
        assert_eq!(registered.capability, "codex");
        let prompt = registered.prompt.as_ref().expect("prompt pinned");
        assert_eq!(prompt.target, format!("{PROMPT_PATH}@1"));
        assert_eq!(
            registered.allowed_actions,
            vec![
                ACTION_BLOCK_UPDATE_TEXT.to_string(),
                ACTION_COMMENT_ADD.to_string(),
                ACTION_THREAD_RESOLVE.to_string(),
            ],
            "sorted grants: the three pages tags and NOTHING else"
        );
        report.assert_agent_owned_by_harness(AGENT);

        // action tags resolve to docs-harness as owner.
        for tag in [
            ACTION_COMMENT_ADD,
            ACTION_BLOCK_UPDATE_TEXT,
            ACTION_THREAD_RESOLVE,
        ] {
            report.assert_route(tag, HARNESS);
            bed.assert_action_owner(tag, Some(HARNESS)).await;
        }

        // the manifest's `harness` key resolves the mapping without an
        // explicit logical — what the CLI's `package test` leans on.
        let spec = install_spec_from_capsule_defaulted(&capsule, None, &BTreeMap::new())
            .expect("the manifest harness key resolves");
        assert_eq!(spec.harness, HARNESS);

        // a second install of the same package id rejects.
        let again = bed
            .install_capsule(&docs_capsule(), HARNESS, &BTreeMap::new(), alice())
            .await;
        assert!(again.is_err(), "duplicate install must reject");
    });
}

#[test]
fn a_squatted_prompt_path_cannot_brick_the_pin() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        // the squat: junk pre-published at the PREDICTABLE seed path, taking
        // generation 1 before the package ever installs.
        bed.submit(
            alice(),
            "memory",
            memory::encode_msg(&memory::MemoryMsg::Publish {
                path: PROMPT_PATH.into(),
                body: memory::PublishBody::Inline("squatted junk".into()),
                meta: memory::Meta::new(),
            }),
        )
        .await
        .expect("the squat commits");

        // install: the seed lands at generation 2 and the agent's PromptRef
        // must pin THAT — pinning an assumed generation 1 would fail every
        // run with pin-mismatch forever (agents are harness-owned; no repair).
        let report = install(&mut bed).await;
        report.assert_active();
        assert_eq!(
            report.prompts[0].generation, 2,
            "the seed rode above the squat"
        );
        let prompt = report.agents[0].prompt.as_ref().expect("prompt pinned");
        assert_eq!(prompt.target, format!("{PROMPT_PATH}@2"));

        // and a scripted run resolves the SEEDED content (pin match): the
        // guarded edit lands, which requires the composed prompt to have
        // hashed to the registered pin.
        seed_page(&mut bed).await;
        comment(&mut bed, "t1", "c1", "@docs.editor please tighten this").await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [
                {"action_id": "a1", "tag": ACTION_BLOCK_UPDATE_TEXT,
                 "payload": {"block_id": "b1", "expected_hash": DRAFT_HASH,
                             "text": "A crisp intro."}},
            ],
        }))
        .await
        .expect("oracle block commits");
        bed.deliver().await.expect("delivery block commits");
        assert_eq!(block_text(&bed, "b1").await, "A crisp intro.");
        bed.assert_job_status(
            &docs_harness::engagement_job_id(AGENT, "c1"),
            jobs::JobStatus::Done,
        )
        .await;
    });
}

// ---- engagement -----------------------------------------------------------------

#[test]
fn a_mention_mints_exactly_one_job_with_idempotent_redelivery() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;

        comment(&mut bed, "t1", "c1", "@docs.editor please tighten this").await;
        bed.assert_job_count(&kind(), 1).await;
        bed.assert_pending_run_for_agent(AGENT, true).await;

        // a non-mention comment engages nothing.
        comment(&mut bed, "t2", "c2", "no robots were addressed here").await;
        bed.assert_job_count(&kind(), 1).await;

        // LITERAL redelivery of the first event (byte-identical, from the
        // pages origin): the committed idempotency key holds — no re-mint.
        let event = pages::encode_page_event(&pages::PageEvent::CommentAdded {
            page_id: "p1".into(),
            target: "b1".into(),
            thread_id: "t1".into(),
            comment_id: "c1".into(),
            author: pages::AuthorRef::User(b"alice".to_vec()),
            text: "@docs.editor please tighten this".into(),
        });
        bed.submit(Origin::Module("pages".into()), HARNESS, event)
            .await
            .expect("a redelivered event must not abort");
        bed.assert_job_count(&kind(), 1).await;
        bed.assert_failure_breadcrumb(HARNESS, "already minted");
    });
}

#[test]
fn a_near_cap_comment_mints_one_job_and_the_block_commits() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;

        // a mention comment near pages' 64 KiB cap, escape-heavy on purpose:
        // embedded verbatim in the job spec, its JSON escaping alone would
        // blow the jobs board's 64 KiB spec cap and abort the COMMENTER's
        // block from the no-fail intake arm. the bounded excerpt keeps the
        // Submit within the cap — the block commits and one job mints.
        let mut text = String::from("@docs.editor tighten this ");
        text.push_str(&"\"".repeat(pages::MAX_COMMENT_TEXT_BYTES - text.len()));
        assert_eq!(text.len(), pages::MAX_COMMENT_TEXT_BYTES);
        comment(&mut bed, "t1", "c1", &text).await; // panics if the block aborts
        bed.assert_job_count(&kind(), 1).await;
        bed.assert_pending_run_for_agent(AGENT, true).await;
    });
}

#[test]
fn malformed_page_events_are_no_op_observations() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        bed.submit(
            Origin::Module("pages".into()),
            HARNESS,
            b"not a page event".to_vec(),
        )
        .await
        .expect("a malformed event must NOT abort the block");
        bed.assert_failure_breadcrumb(HARNESS, "undecodable");
        bed.assert_job_count(&kind(), 0).await;
    });
}

// ---- the provider turn ------------------------------------------------------------

#[test]
fn the_scripted_editor_turn_edits_replies_resolves_and_never_loops() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;
        comment(
            &mut bed,
            "t1",
            "c1",
            "@docs.editor please tighten this intro",
        )
        .await;
        bed.assert_job_count(&kind(), 1).await;

        // one canned response, three granted actions: the guarded edit, a
        // reply INTO the mentioning thread (whose text mentions the agent —
        // the loop-prevention bait), and the resolve.
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [
                {"action_id": "a1", "tag": ACTION_BLOCK_UPDATE_TEXT,
                 "payload": {"block_id": "b1", "expected_hash": DRAFT_HASH,
                             "text": "A crisp intro."}},
                {"action_id": "a2", "tag": ACTION_COMMENT_ADD,
                 "payload": {"target": "b1", "thread_id": "t1",
                             "text": "Rewrote block b1 as asked. — @docs.editor"}},
                {"action_id": "a3", "tag": ACTION_THREAD_RESOLVE,
                 "payload": {"thread_id": "t1", "resolved": true}},
            ],
        }))
        .await
        .expect("oracle block commits");
        // nothing reaches pages until the delivery block (never-pop-stack).
        assert_eq!(block_text(&bed, "b1").await, "Draft intro.");

        bed.deliver().await.expect("delivery block commits");

        // the intended block was edited.
        assert_eq!(block_text(&bed, "b1").await, "A crisp intro.");

        // the reply landed in the thread (authored by the harness module)
        // and the thread is resolved.
        let thread = bed
            .query_json("pages", &json!({"comment_thread": {"thread_id": "t1"}}))
            .await
            .expect("thread query");
        assert_eq!(thread["comment_thread"]["thread"]["resolved"], json!(true));
        let comments = thread["comment_thread"]["comments"]
            .as_array()
            .expect("comments");
        assert_eq!(comments.len(), 2, "the opener plus the agent's reply");
        assert_eq!(comments[1]["author"], json!({"module": HARNESS}));
        assert!(
            comments[1]["text"]
                .as_str()
                .unwrap()
                .contains("@docs.editor")
        );

        // LOOP PREVENTION: the agent's own apply-induced comment mentioned
        // @docs.editor and was fanned back to the harness — it must NOT have
        // minted a second job.
        bed.assert_job_count(&kind(), 1).await;
        bed.assert_job_status(
            &docs_harness::engagement_job_id(AGENT, "c1"),
            jobs::JobStatus::Done,
        )
        .await;
        bed.assert_pending_run_for_agent(AGENT, false).await;
    });
}

#[test]
fn unauthorized_and_malformed_actions_mutate_nothing_and_record_failure() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;

        // turn 1 — an UNGRANTED tag (tasks.create): dropped by runs before
        // any probe; nothing mutates; the run fails.
        comment(&mut bed, "t1", "c1", "@docs.editor sneak a task in").await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": "tasks.create",
                         "payload": {"task_id": "sneaky", "title": "not granted"}}],
        }))
        .await
        .expect("the disallowed response must not abort the oracle block");
        bed.deliver().await.expect("delivery block");
        let tasks = bed.query_json("tasks", &json!("list")).await.unwrap();
        assert_eq!(tasks, json!({"tasks": []}), "nothing mutated");
        bed.assert_failure_breadcrumb("runs", "is not allowed to tasks.create");
        bed.assert_job_status(
            &docs_harness::engagement_job_id(AGENT, "c1"),
            jobs::JobStatus::Failed,
        )
        .await;

        // turn 2 — a GRANTED tag with a malformed payload (the retired
        // page_id field): the owner's probe rejects; the block is untouched.
        comment(&mut bed, "t2", "c2", "@docs.editor malformed please").await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": ACTION_BLOCK_UPDATE_TEXT,
                         "payload": {"block_id": "b1", "text": "clobber",
                                     "page_id": "p1"}}],
        }))
        .await
        .expect("oracle block");
        bed.deliver().await.expect("delivery block");
        assert_eq!(block_text(&bed, "b1").await, "Draft intro.");
        bed.assert_failure_breadcrumb("runs", "rejected by docs-harness");

        // turn 3 — a STALE expected_hash: the guard bites at probe time.
        let stale = format!("sha256:{}", "ab".repeat(32));
        comment(&mut bed, "t3", "c3", "@docs.editor stale edit").await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [{"action_id": "a1", "tag": ACTION_BLOCK_UPDATE_TEXT,
                         "payload": {"block_id": "b1", "text": "clobber",
                                     "expected_hash": stale}}],
        }))
        .await
        .expect("oracle block");
        bed.deliver().await.expect("delivery block");
        assert_eq!(block_text(&bed, "b1").await, "Draft intro.");
        bed.assert_failure_breadcrumb("runs", "expected_hash mismatch");

        // turn 4 — a DUPLICATED action_id (the one same-block conflict the
        // probe cannot see): the first comment lands, the duplicate drops
        // with a COMMITTED error row on the harness.
        comment(&mut bed, "t4", "c4", "@docs.editor reply twice").await;
        bed.oracle_response_json(&json!({
            "reply_blocks": [],
            "actions": [
                {"action_id": "dup", "tag": ACTION_COMMENT_ADD,
                 "payload": {"target": "b1", "text": "first"}},
                {"action_id": "dup", "tag": ACTION_COMMENT_ADD,
                 "payload": {"target": "b1", "text": "second"}},
            ],
        }))
        .await
        .expect("oracle block");
        bed.deliver()
            .await
            .expect("the duplicate must not abort the delivery block");
        bed.assert_failure_breadcrumb(HARNESS, "duplicate action_id");
        let failures = bed.query_json(HARNESS, &json!("failures")).await.unwrap();
        let rows = failures["failures"].as_array().expect("failure rows");
        assert_eq!(rows.len(), 1, "one committed error row");
        assert_eq!(rows[0]["action_id"], json!("dup"));
        assert_eq!(rows[0]["tag"], json!(ACTION_COMMENT_ADD));
        bed.assert_job_status(
            &docs_harness::engagement_job_id(AGENT, "c4"),
            jobs::JobStatus::Done,
        )
        .await;
    });
}

// ---- lifecycle ------------------------------------------------------------------

#[test]
fn suspend_stops_new_jobs_and_unplug_tombstones_while_preserving_user_data() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;
        comment(&mut bed, "t1", "c1", "@docs.editor first").await;
        bed.assert_job_count(&kind(), 1).await;

        // suspend: no new jobs; pages and comments preserved.
        bed.submit_json(alice(), "package", &json!({"suspend": {"package": PKG}}))
            .await
            .expect("suspend commits");
        comment(&mut bed, "t2", "c2", "@docs.editor anyone?").await;
        bed.assert_job_count(&kind(), 1).await;
        assert_eq!(block_text(&bed, "b1").await, "Draft intro.");

        // resume: minting restarts.
        bed.submit_json(alice(), "package", &json!({"resume": {"package": PKG}}))
            .await
            .expect("resume commits");
        comment(&mut bed, "t3", "c3", "@docs.editor welcome back").await;
        bed.assert_job_count(&kind(), 2).await;

        // unplug: routes gone, agent tombstoned, user data intact.
        bed.submit_json(alice(), "package", &json!({"unplug": {"package": PKG}}))
            .await
            .expect("unplug commits");
        for tag in [
            ACTION_COMMENT_ADD,
            ACTION_BLOCK_UPDATE_TEXT,
            ACTION_THREAD_RESOLVE,
        ] {
            bed.assert_action_owner(tag, None).await;
        }
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
        // pages content and comments — user data — are untouched.
        assert_eq!(block_text(&bed, "b1").await, "Draft intro.");
        let threads = bed
            .query_json("pages", &json!({"comment_thread": {"thread_id": "t1"}}))
            .await
            .unwrap();
        assert!(
            threads["comment_thread"].is_object(),
            "comments preserved: {threads}"
        );

        // engagement after unplug is a dead letter: the hook is gone.
        comment(&mut bed, "t5", "c5", "@docs.editor ghost").await;
        bed.assert_job_count(&kind(), 2).await;
    });
}

// ---- snapshots ------------------------------------------------------------------

#[test]
fn snapshot_roundtrip_all_covers_every_registered_module() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        install(&mut bed).await;
        seed_page(&mut bed).await;
        comment(&mut bed, "t1", "c1", "@docs.editor snapshot this").await;

        let report = bed
            .snapshot_roundtrip_all()
            .await
            .expect("every module round-trips");
        let harness = report
            .iter()
            .find(|m| m.id == HARNESS)
            .expect("docs-harness is in the sweep");
        assert_eq!(harness.kind, RoundtripKind::PreimageVerified);
        assert_eq!(report.len(), 13, "one entry per registered module");
    });
}

// ---- the golden fixture -----------------------------------------------------------

#[test]
fn the_docs_golden_fixture_replays_end_to_end() {
    PackageTestBed::run(docs_modules(), |mut bed| async move {
        let capsule = docs_capsule();
        let fixture = GoldenFixture::from_capsule(&capsule).expect("fixture parses");
        assert_eq!(fixture.package, PKG);
        assert_eq!(
            fixture.harness, None,
            "the fixture leans on the manifest's harness key"
        );
        let run = run_golden(&mut bed, &capsule, &fixture)
            .await
            .unwrap_or_else(|e| panic!("golden run failed: {e}"));
        assert_eq!(run.steps.len(), fixture.steps.len());
        let report = run.install.expect("the install step reports");
        report.assert_active();
    });
}
