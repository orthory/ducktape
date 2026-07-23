//! the inline continuation lane ([`Host::submit_block_ops`]): an envelope op's
//! continuation is released in the SAME consensus unit, immediately after the
//! parent's disposition settles, as its own isolated unit. properties:
//!
//! 1. applied parent → the continuation fires with `Ok(output)` (the parent's
//!    `set_output` bytes; empty when it declared nothing), dispatched
//!    `Origin::Module(parent_target)` with the relay's `author` = the
//!    envelope's verified origin — the authorization rule's two identities;
//! 2. rejected parent → the continuation STILL fires, with `Err(reason)`;
//! 3. a rejecting continuation is isolated: the parent's commit survives;
//! 4. read-your-writes: the continuation sees the parent's staged writes;
//! 5. an oversized `set_output` is a deterministic rejection of the parent
//!    (and its continuation then fires with the `Err` relay);
//! 6. determinism: two hosts fed identical envelope ops land identical
//!    root-hashes and traces;
//! 7. a bare op (no continuation) produces no derived unit — `submit_block`
//!    over pairs is byte-identical to before.
//!
//! the `probe` module is a stateless fixture: byte 0 of the payload selects
//! the behavior (set an output, reject, or ECHO the relay slot it observed
//! into an event the test can read out of the [`BatchOutcome`]).

use directory::{DirMsg, Directory, encode_msg as dir_encode};
use futures::executor::block_on;
use host::{BlockContext, BlockOp, Host, MemberOutcome};
use sdk::{Continuation, Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot};

const DIR: &str = "directory";
const PROBE: &str = "probe";

/// stateless fixture module. payload = one opcode byte + rest:
/// - `o`: `set_output(rest)`
/// - `O`: set an OVER-CAP output (deterministic rejection at the drain)
/// - `r`: reject with `probe_reject`
/// - `e`: echo `env.origin` / `author_origin()` / `relay()` into an event
struct Probe;

#[async_trait::async_trait(?Send)]
impl Module for Probe {
    fn id(&self) -> ModuleId {
        PROBE.into()
    }

    fn root(&self) -> StateRoot {
        StateRoot([0; 32])
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let Some((op, rest)) = msg.payload.split_first() else {
            return Err(Error::Module("probe: empty payload".into()));
        };
        match op {
            b'o' => {
                ctx.set_output(rest.to_vec());
                Ok(())
            }
            b'O' => {
                ctx.set_output(vec![0u8; sdk::MAX_OUTPUT_BYTES + 1]);
                Ok(())
            }
            b'r' => Err(Error::Module("probe_reject".into())),
            b'e' => {
                let seen = format!(
                    "origin={:?} author={:?} relay={:?}",
                    ctx.env().origin,
                    ctx.author_origin(),
                    ctx.relay()
                );
                ctx.emit_event(Event {
                    source: PROBE.into(),
                    payload: seen.into_bytes(),
                });
                Ok(())
            }
            other => Err(Error::Module(format!("probe: unknown opcode {other}"))),
        }
    }
}

fn host() -> Host {
    Host::genesis(vec![Box::new(Directory::new(DIR)), Box::new(Probe)]).expect("genesis")
}

fn probe(payload: &[u8]) -> Msg {
    Msg {
        target: PROBE.into(),
        payload: payload.to_vec(),
    }
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: DIR.into(),
        payload: dir_encode(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

fn author() -> Origin {
    Origin::External(vec![0xAA; 32])
}

fn echo_cont() -> Continuation {
    Continuation {
        target: PROBE.into(),
        payload: b"e".to_vec(),
    }
}

/// the one probe echo event in an outcome's aggregate events, as a string.
fn echoed(events: &[Event]) -> String {
    let hits: Vec<&Event> = events.iter().filter(|e| e.source == PROBE).collect();
    assert_eq!(hits.len(), 1, "exactly one probe echo event");
    String::from_utf8(hits[0].payload.clone()).expect("utf8 echo")
}

// (1) applied parent: continuation fires same unit, Ok(output), module lane +
// authenticated author distinct.
#[test]
fn applied_parent_releases_continuation_with_output() {
    block_on(async {
        let mut h = host();
        let out = h
            .submit_block_ops(
                BlockContext::default(),
                vec![BlockOp {
                    origin: author(),
                    msg: probe(b"ohello"),
                    continuation: Some(echo_cont()),
                    frame: [7; 32],
                }],
            )
            .await
            .expect("batch applies");

        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert_eq!(out.continuations.len(), 1, "one derived unit");
        let (parent_idx, ref cont_outcome) = out.continuations[0];
        assert_eq!(parent_idx, 0);
        assert!(
            matches!(cont_outcome, MemberOutcome::Applied { dispatches } if dispatches.len() == 1),
            "continuation applied with its own trace: {cont_outcome:?}"
        );
        // the continuation dispatch rides the MODULE lane of the parent target...
        if let MemberOutcome::Applied { dispatches } = cont_outcome {
            assert_eq!(dispatches[0].origin, Origin::Module(PROBE.into()));
            assert_eq!(dispatches[0].module, PROBE.to_string());
        }
        // ...while the relay carries the authenticated AUTHOR and the parent's
        // declared output.
        let echo = echoed(&out.events);
        assert!(
            echo.contains("author=External") && echo.contains("170"),
            "author_origin is the envelope's external key (0xAA=170): {echo}"
        );
        assert!(
            echo.contains("Ok([104, 101, 108, 108, 111])"),
            "relay outcome carries the parent's `hello` output: {echo}"
        );
        assert!(
            echo.contains("parent_target: \"probe\""),
            "relay names the parent target: {echo}"
        );
    });
}

// (2) rejected parent: the continuation STILL fires, with the Err relay.
#[test]
fn rejected_parent_still_releases_continuation_with_err() {
    block_on(async {
        let mut h = host();
        let out = h
            .submit_block_ops(
                BlockContext::default(),
                vec![BlockOp {
                    origin: author(),
                    msg: probe(b"r"),
                    continuation: Some(echo_cont()),
                    frame: [7; 32],
                }],
            )
            .await
            .expect("batch applies (the member rejection is folded)");

        assert!(matches!(out.members[0], MemberOutcome::Rejected { .. }));
        assert!(
            matches!(out.continuations[0].1, MemberOutcome::Applied { .. }),
            "continuation fires despite the parent rejection"
        );
        let echo = echoed(&out.events);
        assert!(
            echo.contains("Err(") && echo.contains("probe_reject"),
            "relay carries the parent's deterministic rejection: {echo}"
        );
    });
}

// (3) a rejecting continuation is isolated — the parent's commit survives.
#[test]
fn rejecting_continuation_never_takes_parent_down() {
    block_on(async {
        let mut h = host();
        let out = h
            .submit_block_ops(
                BlockContext::default(),
                vec![BlockOp {
                    origin: author(),
                    msg: set("a", "1"),
                    continuation: Some(Continuation {
                        target: PROBE.into(),
                        payload: b"r".to_vec(),
                    }),
                    frame: [7; 32],
                }],
            )
            .await
            .expect("batch applies");

        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(
            out.continuations[0].1,
            MemberOutcome::Rejected { .. }
        ));

        // the parent's write committed: state equals a bare `set a=1` block.
        let mut reference = host();
        reference
            .submit_at(BlockContext::default(), set("a", "1"))
            .await
            .expect("reference applies");
        assert_eq!(
            h.module_root(DIR).unwrap(),
            reference.module_root(DIR).unwrap(),
            "parent commit survives its continuation's rejection"
        );
    });
}

// (4) read-your-writes: a continuation targeting the same module sees the
// parent's staged writes (both Sets commit in one block).
#[test]
fn continuation_sees_parent_staged_writes() {
    block_on(async {
        let mut h = host();
        h.submit_block_ops(
            BlockContext::default(),
            vec![BlockOp {
                origin: author(),
                msg: set("a", "1"),
                continuation: Some(Continuation {
                    target: DIR.into(),
                    payload: dir_encode(&DirMsg::Set {
                        key: "b".into(),
                        value: "2".into(),
                    }),
                }),
                frame: [7; 32],
            }],
        )
        .await
        .expect("batch applies");

        let mut reference = host();
        reference
            .submit_at(BlockContext::default(), set("a", "1"))
            .await
            .expect("apply a");
        reference
            .submit_at(BlockContext::default(), set("b", "2"))
            .await
            .expect("apply b");
        assert_eq!(
            h.module_root(DIR).unwrap(),
            reference.module_root(DIR).unwrap(),
            "parent + continuation writes both committed, in order"
        );
    });
}

// (5) an oversized declared output deterministically REJECTS the parent —
// and the continuation then fires on the Err arm.
#[test]
fn oversized_output_rejects_parent_continuation_gets_err() {
    block_on(async {
        let mut h = host();
        let out = h
            .submit_block_ops(
                BlockContext::default(),
                vec![BlockOp {
                    origin: author(),
                    msg: probe(b"O"),
                    continuation: Some(echo_cont()),
                    frame: [7; 32],
                }],
            )
            .await
            .expect("batch applies");

        let MemberOutcome::Rejected { reason } = &out.members[0] else {
            panic!("oversized output must reject the parent");
        };
        assert!(reason.contains("exceeds cap"), "reason names the cap: {reason}");
        let echo = echoed(&out.events);
        assert!(
            echo.contains("Err(") && echo.contains("exceeds cap"),
            "continuation relays the capped rejection: {echo}"
        );
    });
}

// (6) determinism: two hosts, same envelope ops → identical root-hash.
#[test]
fn continuation_lane_is_deterministic() {
    block_on(async {
        let ops = || {
            vec![
                BlockOp {
                    origin: author(),
                    msg: set("a", "1"),
                    continuation: Some(Continuation {
                        target: DIR.into(),
                        payload: dir_encode(&DirMsg::Set {
                            key: "b".into(),
                            value: "2".into(),
                        }),
                    }),
                    frame: [1; 32],
                },
                BlockOp {
                    origin: author(),
                    msg: probe(b"r"),
                    continuation: Some(echo_cont()),
                    frame: [2; 32],
                },
            ]
        };
        let mut h1 = host();
        let mut h2 = host();
        let o1 = h1
            .submit_block_ops(BlockContext::default(), ops())
            .await
            .expect("h1");
        let o2 = h2
            .submit_block_ops(BlockContext::default(), ops())
            .await
            .expect("h2");
        assert_eq!(o1.root_hash, o2.root_hash, "identical root-hashes");
        assert_eq!(
            o1.continuations.len(),
            o2.continuations.len(),
            "identical derived-unit counts"
        );
    });
}

// (8) the replay-path trace fold: `into_trace` interleaves each parent's
// applied continuation right after it (the index order), and a block whose
// ONLY real work is a released continuation still counts as ran — the seal
// disposition rule that keeps a rejected-parent/applied-continuation block
// reproducible on recovery.
#[test]
fn into_trace_interleaves_continuations_and_counts_them_as_work() {
    block_on(async {
        // parent 0 applies (1 dispatch) + applied continuation (1 dispatch);
        // parent 1 REJECTS + applied continuation (1 dispatch).
        let mut h = host();
        let out = h
            .submit_block_ops(
                BlockContext::default(),
                vec![
                    BlockOp {
                        origin: author(),
                        msg: set("a", "1"),
                        continuation: Some(Continuation {
                            target: DIR.into(),
                            payload: dir_encode(&DirMsg::Set {
                                key: "b".into(),
                                value: "2".into(),
                            }),
                        }),
                        frame: [1; 32],
                    },
                    BlockOp {
                        origin: author(),
                        msg: probe(b"r"),
                        continuation: Some(echo_cont()),
                        frame: [2; 32],
                    },
                ],
            )
            .await
            .expect("batch applies");

        let (ran, trace) = out.into_trace();
        assert!(ran, "applied members and continuations are real work");
        let modules: Vec<&str> = trace.iter().map(|d| d.module.as_str()).collect();
        assert_eq!(
            modules,
            vec![DIR, DIR, PROBE],
            "parent0, its continuation, then parent1's continuation (the \
             rejected parent leaves no trace): {modules:?}"
        );
        assert_eq!(trace[1].origin, Origin::Module(DIR.into()));
        assert_eq!(trace[2].origin, Origin::Module(PROBE.into()));

        // a block whose ONLY work is a released continuation still ran.
        let mut h2 = host();
        let out2 = h2
            .submit_block_ops(
                BlockContext::default(),
                vec![BlockOp {
                    origin: author(),
                    msg: probe(b"r"),
                    continuation: Some(Continuation {
                        target: DIR.into(),
                        payload: dir_encode(&DirMsg::Set {
                            key: "only".into(),
                            value: "cont".into(),
                        }),
                    }),
                    frame: [3; 32],
                }],
            )
            .await
            .expect("batch applies");
        let (ran2, trace2) = out2.into_trace();
        assert!(ran2, "a continuation-only block ran real work");
        assert_eq!(trace2.len(), 1);
        assert_eq!(trace2[0].module, DIR.to_string());
    });
}

// (7) bare pairs are unchanged: no continuation → no derived units.
#[test]
fn bare_ops_produce_no_continuations() {
    block_on(async {
        let mut h = host();
        let out = h
            .submit_block(
                BlockContext::default(),
                vec![(author(), set("a", "1")), (author(), probe(b"e"))],
            )
            .await
            .expect("batch applies");
        assert!(out.continuations.is_empty(), "no derived units for bare ops");
        let echo = echoed(&out.events);
        assert!(
            echo.contains("relay=None"),
            "a non-continuation dispatch carries no relay: {echo}"
        );
        assert!(
            echo.contains("author=External"),
            "author_origin falls back to the dispatch origin: {echo}"
        );
    });
}
