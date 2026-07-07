//! the action-owner contract: shared Probe/Apply validation against
//! staged-or-committed state and the no-fail `Apply` arm (see the module
//! docs).

use package::PackageActionMsg;
use sdk::Ctx;

use super::state::Phase;
use super::{
    ACTION_NOTE_ADD, ACTION_NOTE_SET_TEXT, DummyHarness, MAX_NOTE_ID_BYTES, MAX_NOTE_TEXT_BYTES,
    MAX_NOTES, NotePayload,
};

/// what one probe/apply validated — shared so the verdict and the apply-time
/// re-check cannot drift (the tasks-module idiom).
pub(crate) enum ValidatedNote {
    Add { note_id: String, text: String },
    SetText { note_id: String, text: String },
}

impl DummyHarness {
    // ---- the action-owner contract ---------------------------------------------

    /// validate one owned action against STAGED-OR-COMMITTED state — the
    /// read-only half of `Probe` and the cheap re-check `Apply` runs.
    pub(crate) fn validate_action(
        &self,
        tag: &str,
        payload: &[u8],
    ) -> Result<ValidatedNote, String> {
        let active = self
            .store()
            .installed
            .as_ref()
            .is_some_and(|i| i.phase == Phase::Active);
        if !active {
            return Err("the dummy package is not active".into());
        }
        let note: NotePayload =
            serde_json::from_slice(payload).map_err(|e| format!("malformed {tag} payload: {e}"))?;
        if note.note_id.is_empty() || note.note_id.len() > MAX_NOTE_ID_BYTES {
            return Err("note_id must be 1..=64 bytes".into());
        }
        if note.text.is_empty() || note.text.len() > MAX_NOTE_TEXT_BYTES {
            return Err(format!("text must be 1..={MAX_NOTE_TEXT_BYTES} bytes"));
        }
        match tag {
            ACTION_NOTE_ADD => {
                if self.store().notes.contains_key(&note.note_id) {
                    return Err(format!("note already exists: {}", note.note_id));
                }
                if self.store().notes.len() >= MAX_NOTES {
                    return Err("note cap reached".into());
                }
                Ok(ValidatedNote::Add {
                    note_id: note.note_id,
                    text: note.text,
                })
            }
            ACTION_NOTE_SET_TEXT => {
                if !self.store().notes.contains_key(&note.note_id) {
                    return Err(format!("unknown note: {}", note.note_id));
                }
                Ok(ValidatedNote::SetText {
                    note_id: note.note_id,
                    text: note.text,
                })
            }
            other => Err(format!("dummy-harness does not own action tag: {other}")),
        }
    }

    /// NO-FAIL: an accepted action's `Apply` rides the runs module's delivery
    /// block — decode-or-drop, re-check, breadcrumb on late conflict.
    pub(crate) fn apply_action(&mut self, ctx: &mut dyn Ctx, apply: &PackageActionMsg) {
        let PackageActionMsg::Apply {
            action_id,
            tag,
            payload,
            ..
        } = apply;
        match self.validate_action(tag, payload) {
            Ok(ValidatedNote::Add { note_id, text })
            | Ok(ValidatedNote::SetText { note_id, text }) => {
                self.store_mut().notes.insert(note_id, text);
            }
            Err(reason) => {
                self.breadcrumb(ctx, format!("action {action_id} ({tag}) dropped: {reason}"));
            }
        }
    }
}

// ---- the action-owner contract tests ---------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use package::{PackageActionQuery, PackageActionReply, encode_action_query};
    use sdk::{Module, Origin};

    use crate::dummy::testutil::*;

    #[test]
    fn apply_is_no_fail_and_probe_shares_its_validation() {
        let mut m = module();
        installed(&mut m);

        // probe: accept a fresh create, reject set_text on a missing note.
        let probe = |m: &DummyHarness, tag: &str, payload: serde_json::Value| {
            let ctx = TestCtx::at(Origin::Module("runs".into()));
            let req = encode_action_query(&PackageActionQuery::Probe {
                action_id: "a1".into(),
                tag: tag.into(),
                payload: serde_json::to_vec(&payload).unwrap(),
                run_context: b"{}".to_vec(),
            });
            package::decode_action_reply(&block_on(m.query_with(&ctx, &req)).unwrap()).unwrap()
        };
        assert_eq!(
            probe(
                &m,
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "x"})
            ),
            PackageActionReply::Accepted
        );
        assert!(matches!(
            probe(
                &m,
                ACTION_NOTE_SET_TEXT,
                serde_json::json!({"note_id": "n1", "text": "x"})
            ),
            PackageActionReply::Rejected { .. }
        ));

        // apply the create, then a LATE-CONFLICT duplicate: breadcrumb, Ok.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "x"}),
            ),
        )
        .unwrap();
        let mut dup = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut dup,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "y"}),
            ),
        )
        .expect("a late conflict must not abort the delivery block");
        assert!(dup.events.iter().any(|e| e.contains("already exists")));

        // malformed payload: breadcrumb, Ok, nothing staged.
        let mut bad = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut bad,
            apply(ACTION_NOTE_ADD, serde_json::json!({"bogus": true})),
        )
        .expect("a malformed apply must not abort the delivery block");
        assert!(bad.events.iter().any(|e| e.contains("dropped")));

        commit(&mut m);
        assert_eq!(m.committed.notes.get("n1").map(String::as_str), Some("x"));
    }
}
