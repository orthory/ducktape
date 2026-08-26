//! Text an application takes but never keeps: the buffer behind a `secret`
//! input.
//!
//! The whole point of this module is what it does *not* offer. There is no
//! `Clone`, no `Display`, no `serde`, and no accessor returning an owned
//! `String`. A [`Secret`] can be looked at once, through [`Secret::expose`],
//! by the Rust function an Ice `secret` extern parameter names — and Ice
//! itself can only ask whether the buffer is empty, how long it is, and to
//! clear it.
//!
//! Zeroing is delegated to `zeroize` rather than hand-rolled: this workspace
//! forbids `unsafe`, and a `String` cannot be overwritten in place without it.
//! A hand-written `Drop` that assigned an empty string would be a comment
//! rather than an erasure — the compiler is free to drop a write nobody reads.

use std::collections::BTreeMap;

use zeroize::Zeroizing;

/// One reading of a secret buffer, owned by the Rust function it was handed to
/// and wiped when that function returns.
///
/// The generated code produces one of these only where an extern declared a
/// `secret` parameter. Ice has no expression that produces a `Secret`, no
/// state that can hold one, and no route that can carry one.
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Build a reading. Called by generated code, and by Rust tests that need
    /// to drive a `secret` extern directly.
    pub fn new(text: impl Into<String>) -> Self {
        Self(Zeroizing::new(text.into()))
    }

    /// Look at the text. The borrow cannot outlive the `Secret`, so the caller
    /// cannot keep the reading without deciding to copy it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether anything was typed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many characters were typed — the same count the masked field is
    /// already drawing one bullet per.
    pub fn chars(&self) -> usize {
        self.0.chars().count()
    }
}

/// Redacted on purpose. A struct holding a `Secret` can still derive `Debug`,
/// and a panic message or a log line that formats one prints this instead of
/// the phrase.
impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

/// The buffers behind an application's `secret` inputs, keyed by the declared
/// slot name.
///
/// This is a field on the generated application struct rather than a global,
/// for the reason every other per-app store is: tests run in parallel in one
/// process, and a process-wide buffer would let one test read another's field.
/// It is not application state — nothing in Ice can name it, no preset can set
/// it, and the generated `Debug` for the application does not reach it.
#[derive(Default)]
pub struct SecretStore {
    slots: BTreeMap<&'static str, Zeroizing<String>>,
}

impl SecretStore {
    /// The text a masked field draws. Borrowed rather than cloned: the widget
    /// needs a `&str` and this is the one place the buffer is read without
    /// producing a second copy.
    pub fn text(&self, slot: &'static str) -> &str {
        self.slots.get(slot).map_or("", |held| held.as_str())
    }

    /// Replace a slot with what was just typed. The previous buffer is wiped
    /// as it drops, so a phrase does not survive in a reallocated block after
    /// a keystroke lengthened it.
    pub fn set(&mut self, slot: &'static str, text: String) {
        self.slots.insert(slot, Zeroizing::new(text));
    }

    /// Wipe a slot. This is what `slot = ""` lowers to.
    pub fn clear(&mut self, slot: &'static str) {
        self.slots.remove(slot);
    }

    /// Whether a slot holds nothing.
    pub fn is_empty(&self, slot: &'static str) -> bool {
        self.text(slot).is_empty()
    }

    /// How many characters a slot holds.
    pub fn chars(&self, slot: &'static str) -> usize {
        self.text(slot).chars().count()
    }

    /// Hand a slot's content to a `secret` extern parameter. The copy this
    /// makes is wiped when the receiving function returns; the slot itself is
    /// untouched, because a derivation that failed should not cost the owner
    /// their typing.
    pub fn read(&self, slot: &'static str) -> Secret {
        Secret::new(self.text(slot))
    }
}

/// Never print the buffers. The generated application's own `Debug` prints
/// only its name, and this keeps that true for anything that reaches the store
/// another way.
impl std::fmt::Debug for SecretStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SecretStore")
            .field("slots", &self.slots.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Secret, SecretStore};

    #[test]
    fn a_secret_never_prints_itself() {
        let secret = Secret::new("abandon abandon about");
        let printed = format!("{secret:?}");
        assert_eq!(printed, "Secret(<redacted>)");
        assert!(!printed.contains("abandon"));
        assert_eq!(secret.expose(), "abandon abandon about");
    }

    #[test]
    fn a_store_answers_facts_and_hands_over_content() {
        let mut store = SecretStore::default();
        assert!(store.is_empty("phrase"));
        assert_eq!(store.chars("phrase"), 0);
        assert_eq!(store.text("phrase"), "");

        store.set("phrase", "abandon about".to_owned());
        assert!(!store.is_empty("phrase"));
        assert_eq!(store.chars("phrase"), 13);
        assert_eq!(store.read("phrase").expose(), "abandon about");
        assert!(!format!("{store:?}").contains("abandon"));

        // A read leaves the slot alone: a derivation that refused the phrase
        // must not cost the owner their typing.
        assert_eq!(store.read("phrase").expose(), "abandon about");

        store.clear("phrase");
        assert!(store.is_empty("phrase"));
        assert_eq!(store.text("phrase"), "");
        assert!(store.read("phrase").expose().is_empty());
    }

    #[test]
    fn slots_do_not_leak_into_each_other() {
        let mut store = SecretStore::default();
        store.set("phrase", "words".to_owned());
        store.set("passphrase", "extra".to_owned());
        store.clear("phrase");
        assert!(store.is_empty("phrase"));
        assert_eq!(store.text("passphrase"), "extra");
    }

    #[test]
    fn characters_are_counted_rather_than_bytes() {
        let mut store = SecretStore::default();
        store.set("phrase", "pässwörd".to_owned());
        assert_eq!(store.chars("phrase"), 8);
        assert_eq!(store.read("phrase").chars(), 8);
    }
}
