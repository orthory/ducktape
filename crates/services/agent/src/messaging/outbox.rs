//! the durable outbox: what this daemon took ownership of, and how far each
//! item got — written down BEFORE the thing it describes happens.
//!
//! ## the ordering, which is the whole point
//!
//! What matters is not the file format — an atomically rewritten state file
//! holding the same three-valued enum would do as well — but that one
//! particular write is DURABLE BEFORE an irreversible act, and that the check
//! and the claim are one critical section. An append-only journal is simply
//! the smallest thing that gives both.
//!
//! | last record for an item | what the provider may have seen | recovered as |
//! | --- | --- | --- |
//! | [`Record::Queued`] | nothing — we had not offered it yet | `Queued`, safe to attempt |
//! | [`Record::Attempting`] | possibly the whole message | **`DeliveryUnknown`** |
//! | [`Record::Settled`] | whatever the state says | that state |
//!
//! A crash between `Attempting` and `Settled` is exactly the spec's "crashes
//! after provider input but before recording acceptance". The instruction may
//! already have been read and acted on, so this daemon MUST NOT replay it on
//! its own initiative: it keeps `DeliveryUnknown` and waits for someone with
//! authority to decide. An explicit retry reuses the same message id, so the
//! receiver can tell it may have seen it before.
//!
//! ## what is NOT in here
//!
//! No body is durable beyond the delivery attempt it belongs to, no provider
//! token, no socket path, no session id. A journal line names the message and
//! its state; recovering one is enough to REPORT, never enough to re-send.
//! (That is deliberate, and it is what makes automatic replay impossible by
//! construction rather than by discipline: after a restart this daemon no
//! longer holds the bytes it would have to replay.)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;

use crate::wire::{MessageId, State};

/// one item's identity in the journal. The conversation's committed event
/// sequence is unique within it, so this is the receipt key the collaboration
/// module acknowledges on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub conversation: String,
    pub seq: u64,
}

/// what the journal knows about one item after a fold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub participant: String,
    pub binding_generation: u64,
    pub sender: String,
    pub message_id: MessageId,
    pub expires_at: u64,
    /// the canonical request digest, hex. What makes a retry telling apart
    /// from a forgery possible: the SAME id with the SAME bytes is the same
    /// message and gets the state it already has, while the same id with
    /// different bytes is refused rather than delivered.
    pub digest: String,
    pub state: State,
    pub reason: Option<String>,
}

/// what admitting an item decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// not seen before. The caller now owns it, durably.
    Fresh,
    /// seen before, byte for byte. The caller reports the state it already has
    /// and offers NOTHING to a provider — re-offering an accepted or unknown
    /// instruction is exactly the duplicate execution the retry contract
    /// exists to prevent.
    Duplicate { state: State, reason: Option<String> },
    /// seen before under this id, with different bytes. Refused: one id names
    /// one message, and a second body under it is not a retry.
    Conflict,
}

/// one line of the journal. Appended and fsynced before the act it describes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "r", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    /// this daemon has taken durable ownership of the item. Written and synced
    /// BEFORE the node is told `Queued`, so queue ownership is never claimed
    /// on the strength of memory alone.
    Queued {
        #[serde(flatten)]
        key: Key,
        participant: String,
        binding_generation: u64,
        sender: String,
        message_id: MessageId,
        expires_at: u64,
        digest: String,
    },
    /// the item is about to be offered to a provider. Written and synced
    /// BEFORE the adapter is called — the whole point being that a crash
    /// after this line and before the next is indistinguishable from success,
    /// and must therefore be reported as such.
    Attempting {
        #[serde(flatten)]
        key: Key,
    },
    /// the outcome, as the adapter reported it.
    Settled {
        #[serde(flatten)]
        key: Key,
        state: State,
        reason: Option<String>,
    },
}

/// the append-only delivery journal, and the state it folds to.
pub struct Outbox {
    path: PathBuf,
    /// the file AND the folded state under ONE lock, held across the whole
    /// append + fsync + publish.
    ///
    /// They are one lock because they are one fact. With the map behind its
    /// own lock, an admission could publish `Queued` the instant it decided to
    /// take an item and only then await the fsync — and a duplicate arriving
    /// in that window would be told, from memory, that this daemon durably
    /// owned something that was not on a disk yet and might never get there.
    /// Nothing is visible here until it is durable.
    journal: tokio::sync::Mutex<Journal>,
}

/// the journal's two halves, kept in step because nothing can see one without
/// the other.
struct Journal {
    file: tokio::fs::File,
    /// the folded state, live. Recovering it and then throwing it away would
    /// leave every duplicate to be re-executed: this IS the dedup map, and it
    /// is the same one the journal replays into at boot.
    state: BTreeMap<Key, Entry>,
}

impl Journal {
    /// append one line and make it durable. Writes nothing to `state` — the
    /// caller publishes, and only after this returns `Ok`.
    async fn append(&mut self, record: &Record) -> Result<(), String> {
        let mut line = serde_json::to_string(record)
            .map_err(|error| format!("encode outbox record: {error}"))?;
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .await
            .map_err(|error| format!("append outbox record: {error}"))?;
        self.file
            .sync_data()
            .await
            .map_err(|error| format!("sync outbox record: {error}"))
    }
}

/// the ceiling on a journal this daemon will read back at boot.
///
/// An append-only file with no bound is a boot that gets slower forever and
/// eventually a read that will not fit in memory. 64 MiB is far past any real
/// mailbox (the network's own cap is 2 MiB of queued payload per participant)
/// and small enough to read at boot without thinking about it.
const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;

/// the ceiling on tracked items. Reached only if compaction cannot keep up,
/// at which point admitting more would trade dedup for throughput — so
/// admission refuses instead, retryably.
pub const MAX_TRACKED: usize = 4096;

impl Outbox {
    /// open (creating) the journal at `dir/outbox.jsonl` and fold whatever is
    /// already there into the recovered state.
    ///
    /// Returns the recovered entries alongside the handle: a caller that skips
    /// them is a caller that lost the crash boundary, so they are not
    /// available any other way.
    pub async fn open(dir: &Path) -> Result<(Self, BTreeMap<Key, Entry>), String> {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| format!("create outbox dir: {error}"))?;
        let path = dir.join("outbox.jsonl");
        let oversized = tokio::fs::metadata(&path)
            .await
            .is_ok_and(|meta| meta.len() > MAX_JOURNAL_BYTES);
        if oversized {
            // refused rather than truncated: the journal IS the dedup record,
            // and dropping it to keep booting would license re-executing every
            // instruction in it.
            return Err(format!(
                "outbox journal exceeds {MAX_JOURNAL_BYTES} bytes; it must be inspected, not discarded"
            ));
        }
        let existing = match tokio::fs::read_to_string(&path).await {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("read outbox: {error}")),
        };
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|error| format!("open outbox: {error}"))?;

        // A crash mid-append leaves an unterminated last line. It is the ONE
        // tear a crash can produce, and it is REPAIRED HERE — by dropping it —
        // rather than tolerated for the rest of the file's life.
        //
        // Repairing by appending a newline instead would be worse than doing
        // nothing: it turns an incomplete tail, which recovery knows how to
        // forgive, into a COMPLETE line that will not parse — and the next
        // boot would then fail closed on a file that had just been recovered
        // from cleanly. Truncating to the last record boundary preserves every
        // complete record and leaves nothing to forgive.
        let complete = existing.rfind('\n').map_or(0, |end| end + 1);
        let torn_tail = complete < existing.len();
        if torn_tail {
            file.set_len(complete as u64)
                .await
                .map_err(|error| format!("truncate torn outbox tail: {error}"))?;
            file.sync_all()
                .await
                .map_err(|error| format!("sync truncated outbox: {error}"))?;
            tracing::warn!(
                target: "ducktape::collab",
                reason = "outbox_tail_torn",
                dropped_bytes = existing.len() - complete,
                "dropped an incomplete outbox record left by a crash"
            );
        }
        // folded from the COMPLETE prefix only, so `fold` never has to guess
        // which failure it is looking at.
        let recovered = fold(&existing[..complete])?;
        Ok((
            Self {
                path,
                journal: tokio::sync::Mutex::new(Journal {
                    file,
                    state: recovered.clone(),
                }),
            },
            recovered,
        ))
    }

    /// take durable ownership of one item, or say why not.
    ///
    /// The check and the claim are one critical section, so two deliveries of
    /// the same message — a node retrying, a duplicate frame — cannot both
    /// find it absent and both offer it to a provider.
    ///
    /// This is the seam that makes a retry safe: an identical one is answered
    /// from the record, and only a genuinely new item ever reaches a provider.
    pub async fn admit(&self, key: &Key, entry: &Entry) -> Result<Admission, String> {
        let mut journal = self.journal.lock().await;
        if let Some(existing) = journal.state.get(key) {
            // one id, one message. Different bytes under it are not a retry of
            // anything, and delivering them would let a second instruction
            // inherit the first one's identity.
            if existing.digest != entry.digest {
                return Ok(Admission::Conflict);
            }
            return Ok(Admission::Duplicate {
                state: existing.state,
                reason: existing.reason.clone(),
            });
        }
        if journal.state.len() >= MAX_TRACKED {
            return Err("outbox is at its tracked-item ceiling".to_string());
        }
        // durable FIRST, published second, both inside this one critical
        // section. A concurrent duplicate either waits here and then sees a
        // record that is already on the disk, or arrives after a failure and
        // finds nothing — never a `Queued` that only memory believes in.
        journal
            .append(&Record::Queued {
                key: key.clone(),
                participant: entry.participant.clone(),
                binding_generation: entry.binding_generation,
                sender: entry.sender.clone(),
                message_id: entry.message_id,
                expires_at: entry.expires_at,
                digest: entry.digest.clone(),
            })
            .await?;
        journal.state.insert(key.clone(), entry.clone());
        Ok(Admission::Fresh)
    }

    /// what the journal currently says about one item.
    pub async fn state_of(&self, key: &Key) -> Option<State> {
        self.journal
            .lock()
            .await
            .state
            .get(key)
            .map(|entry| entry.state)
    }

    /// where the journal lives — for the operator-facing log line at boot, and
    /// for the tests that reopen it.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// mark an item as about to reach a provider. Returns once durable; the
    /// caller must not touch the provider until it has.
    ///
    /// The in-memory state becomes `DeliveryUnknown` at the same moment,
    /// because from here until an outcome is recorded that IS what is true: if
    /// this process died right now, recovery would say exactly this.
    pub async fn attempting(&self, key: &Key) -> Result<(), String> {
        let mut journal = self.journal.lock().await;
        journal.append(&Record::Attempting { key: key.clone() }).await?;
        remember(&mut journal.state, key, State::DeliveryUnknown, Some("in_flight"));
        Ok(())
    }

    /// record the outcome.
    ///
    /// Refuses to walk a state BACKWARDS. An item that reached a provider is
    /// never returned to `Queued` by a later report, because `Queued` is the
    /// one state that licenses offering it again — and the whole point of
    /// `DeliveryUnknown` is that offering it again may repeat something a
    /// model already did.
    pub async fn settled(
        &self,
        key: &Key,
        state: State,
        reason: Option<&str>,
    ) -> Result<(), String> {
        // the check and the write are one critical section, so the state this
        // decided against cannot have moved by the time the record lands.
        let mut journal = self.journal.lock().await;
        let downgrades = state == State::Queued
            && journal
                .state
                .get(key)
                .is_some_and(|held| held.state != State::Queued);
        if downgrades {
            tracing::warn!(
                target: "ducktape::collab",
                conversation = %key.conversation,
                seq = key.seq,
                reason = "would_downgrade_to_queued",
                "refused to move a delivery back to queued"
            );
            return Ok(());
        }
        journal
            .append(&Record::Settled {
                key: key.clone(),
                state,
                reason: reason.map(str::to_string),
            })
            .await?;
        remember(&mut journal.state, key, state, reason);
        Ok(())
    }
}

/// publish a transition into the live map, after it is durable.
fn remember(
    state: &mut BTreeMap<Key, Entry>,
    key: &Key,
    to: State,
    reason: Option<&str>,
) {
    if let Some(entry) = state.get_mut(key) {
        entry.state = to;
        entry.reason = reason.map(str::to_string);
    }
}

/// replay the journal into per-item state.
///
/// Pure, so the crash boundaries are unit-testable without a disk: this is the
/// function the recovery contract actually lives in.
///
/// **Every line here is a COMPLETE record, and every one must read.** The
/// caller has already dropped an unterminated tail, so anything unreadable at
/// this point is corruption of a record that was finished — and this fails
/// closed on it rather than skipping.
///
/// Skipping is the tempting mistake and the dangerous one: the record it
/// cannot read might be the `Attempting` that is the only thing standing
/// between an already-executed instruction and an automatic replay, and
/// skipping it silently recovers that item as `Queued` — deliverable again.
fn fold(text: &str) -> Result<BTreeMap<Key, Entry>, String> {
    let mut entries: BTreeMap<Key, Entry> = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str::<Record>(line).map_err(|error| {
            format!(
                "outbox record {} is unreadable ({error}); refusing to recover past it",
                index + 1
            )
        })?;
        match record {
            Record::Queued {
                key,
                participant,
                binding_generation,
                sender,
                message_id,
                expires_at,
                digest,
            } => {
                entries.insert(
                    key,
                    Entry {
                        participant,
                        binding_generation,
                        sender,
                        message_id,
                        expires_at,
                        digest,
                        state: State::Queued,
                        reason: None,
                    },
                );
            }
            // the crash boundary. An item recorded as attempted and never
            // settled reaches here as `DeliveryUnknown` and stays there: the
            // provider may have read it, so nothing may claim it did not.
            Record::Attempting { key } => {
                if let Some(entry) = entries.get_mut(&key) {
                    entry.state = State::DeliveryUnknown;
                    entry.reason = Some("crashed_after_provider_input".to_string());
                }
            }
            Record::Settled { key, state, reason } => {
                if let Some(entry) = entries.get_mut(&key) {
                    entry.state = state;
                    entry.reason = reason;
                }
            }
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn key(seq: u64) -> Key {
        Key {
            conversation: "conv-1".to_string(),
            seq,
        }
    }

    fn entry() -> Entry {
        entry_with("digest-aaaa")
    }

    fn entry_with(digest: &str) -> Entry {
        Entry {
            participant: "p-recipient".to_string(),
            binding_generation: 3,
            sender: "p-sender".to_string(),
            message_id: MessageId {
                generation: 2,
                sequence: 1,
            },
            expires_at: 1_200,
            digest: digest.to_string(),
            state: State::Queued,
            reason: None,
        }
    }

    async fn scratch(name: &str) -> (PathBuf, Outbox, BTreeMap<Key, Entry>) {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-outbox-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (outbox, recovered) = Outbox::open(&dir).await.expect("opens");
        (dir, outbox, recovered)
    }

    /// crash boundary 1: we took ownership but never reached the provider.
    /// Nothing was read, so the item is still deliverable.
    #[tokio::test]
    async fn a_crash_before_provider_input_recovers_as_queued() {
        let (dir, outbox, _) = scratch("before-input").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        drop(outbox); // the crash

        let (_reopened, recovered) = Outbox::open(&dir).await.expect("reopens");
        assert_eq!(recovered[&key(7)].state, State::Queued);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// crash boundary 2: THE one that matters. We wrote the message into a
    /// provider and died before its outcome was recorded, so the model may
    /// already have read and acted on it. Anything other than
    /// `DeliveryUnknown` here is a fabricated status — and a `Queued` in
    /// particular would license an automatic replay of an instruction that has
    /// already been carried out.
    #[tokio::test]
    async fn a_crash_after_provider_input_recovers_as_delivery_unknown() {
        let (dir, outbox, _) = scratch("after-input").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        outbox.attempting(&key(7)).await.expect("attempting");
        drop(outbox); // the crash, between the provider write and its receipt

        let (_reopened, recovered) = Outbox::open(&dir).await.expect("reopens");
        let entry = &recovered[&key(7)];
        assert_eq!(
            entry.state,
            State::DeliveryUnknown,
            "an attempt with no recorded outcome must never recover as anything else"
        );
        assert_eq!(
            entry.reason.as_deref(),
            Some("crashed_after_provider_input")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// crash boundary 3: the outcome was durable before we died, so it stands.
    #[tokio::test]
    async fn a_crash_after_the_receipt_recovers_that_receipt() {
        let (dir, outbox, _) = scratch("after-receipt").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        outbox.attempting(&key(7)).await.expect("attempting");
        outbox
            .settled(&key(7), State::AdapterAccepted, None)
            .await
            .expect("settled");
        drop(outbox);

        let (_reopened, recovered) = Outbox::open(&dir).await.expect("reopens");
        assert_eq!(recovered[&key(7)].state, State::AdapterAccepted);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// the identity that survives a crash is the SENDER's, not a fresh one: an
    /// explicit retry has to reuse it so the receiver can be told it may have
    /// seen this before.
    #[tokio::test]
    async fn recovery_preserves_the_senders_message_id() {
        let (dir, outbox, _) = scratch("identity").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        outbox.attempting(&key(7)).await.expect("attempting");
        drop(outbox);

        let (_reopened, recovered) = Outbox::open(&dir).await.expect("reopens");
        let entry = &recovered[&key(7)];
        assert_eq!(entry.message_id.generation, 2);
        assert_eq!(entry.message_id.sequence, 1);
        assert_eq!(entry.sender, "p-sender");
        assert_eq!(entry.binding_generation, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn queued_line(seq: u64) -> String {
        serde_json::to_string(&Record::Queued {
            key: key(seq),
            participant: "p-recipient".to_string(),
            binding_generation: 3,
            sender: "p-sender".to_string(),
            message_id: MessageId {
                generation: 2,
                sequence: seq,
            },
            expires_at: 1_200,
            digest: "digest-aaaa".to_string(),
        })
        .unwrap()
    }

    /// The hole a "skip whatever will not parse" fold leaves: the damaged
    /// record can be the `Attempting` that is the ONLY thing standing between
    /// an already-executed instruction and an automatic replay. Skipping it
    /// recovers that item as `Queued` — deliverable again. A record that was
    /// complete and is now unreadable therefore fails closed.
    #[test]
    fn a_corrupt_record_is_never_skipped() {
        let corrupt_attempting = r#"{"r":"attem GARBAGE "seq":7}"#;
        let journal = format!(
            "{}\n{corrupt_attempting}\n{}\n",
            queued_line(7),
            queued_line(8)
        );
        let error = fold(&journal).expect_err("a corrupt committed record must fail closed");
        assert!(error.contains("unreadable"), "{error}");
    }

    /// two items in one journal do not read each other's transitions.
    #[test]
    fn each_item_folds_independently() {
        let mut text = String::new();
        for seq in [7, 8] {
            text.push_str(&queued_line(seq));
            text.push('\n');
        }
        text.push_str(&serde_json::to_string(&Record::Attempting { key: key(7) }).unwrap());
        text.push('\n');
        text.push_str(
            &serde_json::to_string(&Record::Settled {
                key: key(7),
                state: State::Refused,
                reason: Some("auth_rejected".to_string()),
            })
            .unwrap(),
        );
        text.push('\n');
        text.push_str(&serde_json::to_string(&Record::Attempting { key: key(8) }).unwrap());

        let recovered = fold(&text).expect("folds");
        assert_eq!(recovered[&key(7)].state, State::Refused);
        assert_eq!(recovered[&key(7)].reason.as_deref(), Some("auth_rejected"));
        assert_eq!(recovered[&key(8)].state, State::DeliveryUnknown);
    }

    /// THE duplicate-execution guard. A node retrying, or a frame arriving
    /// twice, must not put the same instruction into a provider twice — the
    /// second admission answers from the record instead.
    #[tokio::test]
    async fn an_identical_retry_is_answered_from_the_record_not_re_offered() {
        let (dir, outbox, _) = scratch("dedup").await;
        assert_eq!(
            outbox.admit(&key(7), &entry()).await.expect("admits"),
            Admission::Fresh
        );
        outbox.attempting(&key(7)).await.expect("attempting");
        outbox
            .settled(&key(7), State::AdapterAccepted, Some("queued_by_cli"))
            .await
            .expect("settled");

        let again = outbox.admit(&key(7), &entry()).await.expect("admits");
        assert_eq!(
            again,
            Admission::Duplicate {
                state: State::AdapterAccepted,
                reason: Some("queued_by_cli".to_string()),
            },
            "an identical retry reports what already happened"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One id names one message. A second body under it is not a retry of the
    /// first, and delivering it would let a different instruction inherit an
    /// identity the network already accounted for.
    #[tokio::test]
    async fn a_conflicting_body_under_the_same_id_is_refused() {
        let (dir, outbox, _) = scratch("conflict").await;
        assert_eq!(
            outbox
                .admit(&key(7), &entry_with("digest-aaaa"))
                .await
                .expect("admits"),
            Admission::Fresh
        );
        assert_eq!(
            outbox
                .admit(&key(7), &entry_with("digest-bbbb"))
                .await
                .expect("admits"),
            Admission::Conflict
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Dedup must survive the restart, or a crash becomes a licence to replay.
    #[tokio::test]
    async fn dedup_survives_a_restart() {
        let (dir, outbox, _) = scratch("dedup-restart").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        outbox.attempting(&key(7)).await.expect("attempting");
        drop(outbox);

        let (reopened, _) = Outbox::open(&dir).await.expect("reopens");
        let again = reopened.admit(&key(7), &entry()).await.expect("admits");
        assert_eq!(
            again,
            Admission::Duplicate {
                state: State::DeliveryUnknown,
                reason: Some("crashed_after_provider_input".to_string()),
            },
            "a message that may already have been read must not be re-offered after a restart"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `Queued` is the one state that licenses offering an item to a provider.
    /// Nothing may move an item back into it once the provider has been
    /// touched — that would turn a "we do not know" into "deliver it again".
    #[tokio::test]
    async fn nothing_moves_a_delivery_back_to_queued() {
        let (dir, outbox, _) = scratch("no-downgrade").await;
        outbox.admit(&key(7), &entry()).await.expect("admits");
        outbox.attempting(&key(7)).await.expect("attempting");
        assert_eq!(outbox.state_of(&key(7)).await, Some(State::DeliveryUnknown));

        outbox
            .settled(&key(7), State::Queued, Some("turn_busy"))
            .await
            .expect("accepted but ignored");
        assert_eq!(
            outbox.state_of(&key(7)).await,
            Some(State::DeliveryUnknown),
            "an in-flight delivery must not be walked back to queued"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A crash mid-append leaves a half-written record. Recovery must survive
    /// it, keep every complete record before it, and — the part a
    /// newline-repair gets wrong — still be recoverable the NEXT time.
    ///
    /// Appending a newline behind the tear would make it a complete line that
    /// does not parse, so this same file would fail closed on the second boot.
    /// Reopening twice is the assertion that says so.
    #[tokio::test]
    async fn a_torn_tail_is_dropped_and_the_file_reopens_cleanly_twice() {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-outbox-torn-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let attempting = serde_json::to_string(&Record::Attempting { key: key(7) }).unwrap();
        std::fs::write(
            dir.join("outbox.jsonl"),
            // two complete records, then a record cut off mid-write.
            format!(
                "{}\n{attempting}\n{{\"r\":\"settl",
                queued_line(7)
            ),
        )
        .expect("a journal with a half-written tail");

        // first boot: the complete prefix stands, so the item is still the
        // cautious `DeliveryUnknown` its `Attempting` record says it is.
        let (outbox, recovered) = Outbox::open(&dir).await.expect("recovers");
        assert_eq!(recovered[&key(7)].state, State::DeliveryUnknown);
        outbox
            .settled(&key(7), State::Refused, Some("auth_rejected"))
            .await
            .expect("settles");
        drop(outbox);

        // second boot: still readable. This is what fails if the tear was
        // "repaired" into a complete unparseable line instead of dropped.
        let (outbox, recovered) = Outbox::open(&dir).await.expect("reopens once");
        assert_eq!(recovered[&key(7)].state, State::Refused);
        drop(outbox);

        // third, for the same reason: repair must be idempotent, not a state
        // the file passes through on its way to being unreadable.
        let (_again, recovered) = Outbox::open(&dir).await.expect("reopens twice");
        assert_eq!(recovered[&key(7)].state, State::Refused);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ownership becomes visible only once it is on the disk. With the map
    /// behind its own lock, an admission could publish `Queued` and only then
    /// await the fsync — and a duplicate arriving in that window would be told
    /// this daemon durably owned something that was not written yet.
    #[tokio::test]
    async fn a_duplicate_never_sees_ownership_that_is_not_yet_durable() {
        let (dir, outbox, _) = scratch("durable-before-visible").await;
        let outbox = Arc::new(outbox);

        // two admissions of the same item, racing.
        let first = {
            let outbox = outbox.clone();
            tokio::spawn(async move { outbox.admit(&key(7), &entry()).await })
        };
        let second = {
            let outbox = outbox.clone();
            tokio::spawn(async move { outbox.admit(&key(7), &entry()).await })
        };
        let outcomes = [
            first.await.expect("joins").expect("admits"),
            second.await.expect("joins").expect("admits"),
        ];

        // exactly one takes it; the other is told what the record says.
        let fresh = outcomes
            .iter()
            .filter(|admission| **admission == Admission::Fresh)
            .count();
        assert_eq!(fresh, 1, "exactly one admission may own the item: {outcomes:?}");

        // and whatever the duplicate was told is what is actually on the disk.
        drop(outbox);
        let (_reopened, recovered) = Outbox::open(&dir).await.expect("reopens");
        assert_eq!(recovered[&key(7)].state, State::Queued);
        let journal =
            std::fs::read_to_string(dir.join("outbox.jsonl")).expect("the journal");
        assert_eq!(
            journal.matches(r#""r":"queued""#).count(),
            1,
            "one admission, one record: {journal}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
