//! The Pages capability boundary: stable app source, one bounded transfer, and
//! accepted canonical editor state. No Markdown or document editing lives here.
use super::{Guest, MAX_PAYLOAD_BYTES, wire};
use crate::pages::guest_document::{DocumentIdentity, SourceStore, Transfer};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct Context {
    connection: u64,
    network: String,
    page: String,
}
#[derive(Default)]
struct Session {
    context: Option<Context>,
    marker: Vec<u8>,
    reset: u64,
    transfer: u64,
    sources: SourceStore,
}
fn session() -> &'static Mutex<Session> {
    static SESSION: OnceLock<Mutex<Session>> = OnceLock::new();
    SESSION.get_or_init(Mutex::default)
}

pub(crate) fn source_changed() {
    let mut session = session().lock().unwrap();
    session.context = None;
    session.marker.clear();
    session.sources.clear();
}

pub(crate) fn source(
    connection: u64,
    network: &str,
    page: &str,
    text: &str,
) -> Result<Vec<u8>, &'static str> {
    let context = Context {
        connection,
        network: network.into(),
        page: page.into(),
    };
    let mut session = session().lock().unwrap();
    if session.context.as_ref() == Some(&context) {
        return Ok(session.marker.clone());
    }
    let reset = session
        .reset
        .checked_add(1)
        .ok_or("Document source exhausted")?;
    let identity = DocumentIdentity {
        document: format!("pages/{}", hex::encode(wire::encode(&context))),
        reset,
    };
    let marker = session
        .sources
        .show(identity, text, wire::EditorCursor::default())?;
    session.context = Some(context);
    session.reset = reset;
    session.marker = marker.clone();
    Ok(marker)
}

pub(super) fn request(guest: &mut Guest, id: u64, payload: &[u8]) {
    if let Some((previous, _)) = guest.pages_document.take() {
        guest.reply(previous, Err("Document transfer replaced".into()));
    }
    let result = (|| {
        let mut session = session().lock().unwrap();
        session.transfer = session
            .transfer
            .checked_add(1)
            .ok_or("Document transfer exhausted")?;
        session.sources.transfer(payload, session.transfer, id)
    })();
    match result {
        Ok(transfer) => guest.pages_document = Some((id, transfer)),
        Err(error) => guest.reply(id, Err(error.into())),
    }
}

/// Emit one existing transfer frame per redraw. The guest receives no partial
/// String, and the host never queues an entire document as one capability item.
pub(super) fn drive(guest: &mut Guest) {
    let Some((id, mut transfer)) = guest.pages_document.take() else {
        return;
    };
    let result = transfer.next(&session().lock().unwrap().sources);
    match result {
        Ok(Some(frame)) => {
            let done = matches!(
                frame,
                wire::editor_document::EditorTransfer::Complete { .. }
                    | wire::editor_document::EditorTransfer::Abort { .. }
            );
            let bytes = wire::encode(&frame);
            if bytes.len() > MAX_PAYLOAD_BYTES {
                guest.reply(id, Err("Document transfer frame is too large".into()));
                return;
            }
            guest.pending.push(wire::Event::Response {
                id,
                result: Ok(bytes),
                done,
            });
            if !done {
                guest.pages_document = Some((id, transfer));
            }
        }
        Ok(None) => guest.reply(id, Err("Document transfer ended early".into())),
        Err(error) => guest.reply(id, Err(error.into())),
    }
}

pub(super) type Pending = Option<(u64, Transfer)>;


#[derive(Clone, Debug, Default)]
pub struct AcceptedDocument {
    pub accepted: bool,
    pub text: String,
    pub cursor_line: i64,
    pub comment_line: i64,
    pub link: String,
}

/// Resolve only the canonical editor revision named by this accepted intent.
/// The final app handler supplies its current page/network; a queued old edit
/// cannot replace the buffer after navigation or a connection replacement.
pub fn accept_page_document(event: super::ModuleViewEvent, network: String, page: String) -> AcceptedDocument {
    let Some(next) = accept_inner(&event, &network, &page) else {
        return AcceptedDocument::default();
    };
    next
}
fn accept_inner(event: &super::ModuleViewEvent, network: &str, page: &str) -> Option<AcceptedDocument> {
    if event.kind != "edited" || event.detail.len() > 16 * 1024 { return None; }
    let envelope: Envelope = serde_json::from_str(&event.detail).ok()?;
    let accepted = envelope.accepted;
    let reference: wire::editor_document::EditorDocumentRef = wire::decode(&accepted.reference).ok()?;
    let registry = super::registry().lock().ok()?;
    let connection = super::connection().lock().ok()?;
    let mounted = registry.get("pages")?.lock().ok()?;
    let super::Slot::Ready(guest) = &mounted.slot else { return None; };
    if guest.pages_instance.as_deref() != Some(envelope.instance.as_str()) { return None; }
    let document = guest.inputs.editor_document("PagesView/document")?;
    if document.reference() != reference { return None; }
    let mut session = session().lock().ok()?;
    if session.context.as_ref() != Some(&Context { connection: connection.rev, network: network.into(), page: page.into() })
        || session.marker != accepted.source { return None; }
    let identity: DocumentIdentity = wire::decode(&accepted.source).ok()?;
    let navigation = if accepted.navigation.is_empty() { crate::pages::guest_document::Navigation::default() }
        else { wire::decode(&accepted.navigation).ok()? };
    session.sources.show(identity, document.text(), reference.cursor).ok()?;
    Some(AcceptedDocument {
        accepted: true,
        text: document.text().to_owned(),
        cursor_line: i64::from(reference.cursor.position.line),
        comment_line: navigation.comment_line.map_or(-1, i64::from),
        link: navigation.link,
    })
}


#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope {
    instance: String,
    accepted: crate::pages::guest_document::Accepted,
}

/// This stamp is added by the host after decoding the guest payload. An old
/// queued intent cannot impersonate the successor of a same-document reload.
pub(super) fn emit(guest: &mut Guest, id: u64, payload: &[u8]) {
    let Some(instance) = guest.pages_instance.as_ref() else { return; };
    let Ok(accepted) = serde_json::from_slice(payload) else {
        guest.reply(id, Err("Invalid document notification".into()));
        return;
    };
    let detail = serde_json::to_string(&Envelope { instance: instance.clone(), accepted }).expect("document envelope");
    if detail.len() > 16 * 1024 {
        guest.reply(id, Err("Document notification is too large".into()));
        return;
    }
    guest.intents.push(super::ModuleViewEvent { kind: "edited".into(), detail });
}
