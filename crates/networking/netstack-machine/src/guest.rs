//! The `ducktape:netstack` guest: the machine behind the component boundary
//! (`wit/netstack.wit`). Every value crosses as the contract's wire form;
//! the only imports are the identity signature and the log line. One
//! instance lives as long as the host keeps its store, with the machine's
//! state in guest memory between steps — the same state the native machine
//! holds, which is what makes the two traces identical.
//!
//! Compiled only under the `guest` feature, by guest-builder's synthesized
//! wasm32 workspace; a host-side dependent never enables it.

use std::cell::RefCell;
use std::fmt::Write as _;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use wireguard::IdentitySigner;

use crate::machine::Machine;
use crate::wire;

mod bindings {
    wit_bindgen::generate!({
        world: "netstack",
        path: "wit",
    });
}

use bindings::Guest;
use bindings::ducktape::netstack::host;

thread_local! {
    static MACHINE: RefCell<Option<Machine>> = const { RefCell::new(None) };
}

/// The node's identity, signing through the host: the private key stays
/// on the other side of the boundary.
struct HostSigner;

impl IdentitySigner for HostSigner {
    fn identity(&self) -> ed25519::PublicKey {
        let bytes = host::identity();
        ed25519::PublicKey::decode(bytes.as_slice())
            .expect("the host's identity is an ed25519 public key")
    }

    fn sign_message(&self, namespace: &[u8], message: &[u8]) -> ed25519::Signature {
        let bytes = host::sign(namespace, message);
        ed25519::Signature::decode(bytes.as_slice()).expect("the host signs with ed25519")
    }
}

/// The guest's tracing subscriber: every event the machine emits becomes
/// one host log line. Spans are accepted and ignored — the machine opens
/// none.
struct HostLog;

impl tracing::Subscriber for HostLog {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = LogLine::default();
        event.record(&mut line);
        let metadata = event.metadata();
        host::log(level_of(*metadata.level()), metadata.target(), &line.text);
    }

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

/// One event's fields rendered as the host ring shows them: the message
/// first, then `field=value` pairs.
#[derive(Default)]
struct LogLine {
    text: String,
}

impl LogLine {
    fn push_field(&mut self, field: &tracing::field::Field, value: std::fmt::Arguments<'_>) {
        let is_message = field.name() == "message";
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        match is_message {
            true => {
                let _ = self.text.write_fmt(value);
            }
            false => {
                let _ = write!(self.text, "{}=", field.name());
                let _ = self.text.write_fmt(value);
            }
        }
    }
}

impl tracing::field::Visit for LogLine {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.push_field(field, format_args!("{value:?}"));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.push_field(field, format_args!("{value}"));
    }
}

fn level_of(level: tracing::Level) -> host::Level {
    match level {
        tracing::Level::TRACE => host::Level::Trace,
        tracing::Level::DEBUG => host::Level::Debug,
        tracing::Level::INFO => host::Level::Info,
        tracing::Level::WARN => host::Level::Warn,
        tracing::Level::ERROR => host::Level::Error,
    }
}

struct Component;

impl Guest for Component {
    fn configure(config: Vec<u8>) -> Result<(), String> {
        let config = wire::decode_config(&config).map_err(|err| err.to_string())?;
        // the subscriber outlives every machine this instance builds; a
        // second configure finds it installed and keeps it.
        let _ = tracing::subscriber::set_global_default(HostLog);
        let machine = Machine::new(Box::new(HostSigner), config);
        MACHINE.with(|slot| *slot.borrow_mut() = Some(machine));
        Ok(())
    }

    fn step(event: Vec<u8>, now_ms: u64) -> Result<Vec<u8>, String> {
        let event = wire::decode_event(&event).map_err(|err| err.to_string())?;
        MACHINE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let machine = slot.as_mut().ok_or("step before configure")?;
            Ok(wire::encode_step(&machine.step(event, now_ms)))
        })
    }
}

bindings::export!(Component with_types_in bindings);
