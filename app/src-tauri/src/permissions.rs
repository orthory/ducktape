//! Who may use the camera, the microphone, the screen — and who may only ask.
//!
//! Every privileged Chromium permission request in the shell lands here:
//! `getUserMedia`, `getDisplayMedia`, and Chromium's permission prompts
//! (notifications, geolocation, clipboard, MIDI, …). The runtime denies by
//! default; this module is the only thing that can say yes.
//!
//! The decision is made about a PRINCIPAL, not a URL. The requesting origin
//! cannot carry that weight here: executable publisher content is proxied
//! through a random `<token>.localhost` gateway session, so its origin is
//! indistinguishable from the console's own — app-local, loopback, and
//! meaningless. What separates them is the webview they render in, which the
//! shell assigns and content cannot forge:
//!
//! - [`Principal::HostUi`] — the bundled Ducktape windows (console, tray
//!   popover, huddle, and this module's own consent window). Our code, our
//!   origin: it gets exactly the capabilities the product declares in
//!   [`HOST_UI_CAPABILITIES`] and nothing else — including the local-network
//!   permission its `tauri://localhost` origin needs to reach the node daemon
//!   on loopback. A console that starts asking for geolocation or MIDI is
//!   denied like anyone else.
//! - [`Principal::Gateway`] — executable content from a `.duck` site, in a
//!   capability-free webview. It may ASK for the huddle-class devices
//!   ([`GATEWAY_PROMPTABLE`]); only the user, through the native consent
//!   window, can grant them. Everything else is denied outright.
//! - [`Principal::Untrusted`] — an unknown webview label, or a bundled window
//!   showing an origin it has no business showing. Denied.
//!
//! A grant lasts, at most, as long as the surface that earned it: grants are
//! keyed by (webview, origin, permission), so navigating a gateway webview to a
//! different origin strands the old grant, and closing the webview drops its
//! grants entirely. Nothing is written to disk — there is deliberately no
//! persistent "always allow" yet.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tauri::{Manager as _, WebviewUrl, WebviewWindowBuilder};
use tauri_runtime_cef::{
    DeferredResponder, DenyReason, NormalizedOrigin, PermissionAudit, PermissionKind,
    PermissionRequest, PermissionResponder, Verdict, DEFAULT_PROMPT_TIMEOUT,
};

/// Bundled Ducktape UI windows: the console, the macOS menu-bar popover, the
/// popped-out huddle, and the consent window this module owns.
const HOST_UI_LABELS: &[&str] = &["main", "tray", "huddle", PROMPT_LABEL];

/// What the bundled UI may use without asking. The product declares these
/// because it ships the code that uses them: the huddle needs the microphone,
/// camera and screen; notifications and clipboard reads back the console's own
/// affordances.
const HOST_UI_CAPABILITIES: &[PermissionKind] = &[
    PermissionKind::LocalNetwork,
    PermissionKind::Microphone,
    PermissionKind::Camera,
    PermissionKind::ScreenCapture,
    PermissionKind::Notifications,
    PermissionKind::ClipboardRead,
];

/// What publisher content may ASK for. A `.duck` app can hold a call, so it may
/// request the huddle-class devices — with the user's explicit, per-site
/// consent. Everything outside this list is denied without a prompt: there is
/// no product reason for a page in the browser pane to read the clipboard,
/// track location, or drive MIDI.
const GATEWAY_PROMPTABLE: &[PermissionKind] = &[
    PermissionKind::Microphone,
    PermissionKind::Camera,
    PermissionKind::ScreenCapture,
];

const GATEWAY_PREFIX: &str = "gateway-";
const PROMPT_LABEL: &str = "permission-prompt";

/// Who is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Principal {
    /// A bundled Ducktape window, on the app's own origin.
    HostUi,
    /// Executable content from a `.duck` site, in its capability-free webview.
    /// `site` is the route the user opened (`app.demo.duck`) — the honest name
    /// for content whose session origin is a random loopback token.
    Gateway { site: String },
    /// An unknown label, or a bundled window on an origin it should never show.
    Untrusted,
}

/// What the policy decided, before it is handed to the runtime. Split out from
/// [`evaluate`] so the rules can be tested without a live CEF callback.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decision {
    /// Grant every requested permission.
    Allow,
    Deny(DenyReason),
    /// Per-kind verdicts, parallel to the request's kinds. The runtime grants
    /// the request only if every one of them allows.
    PerKind(Vec<Verdict>),
    /// Put it to the user, naming this site.
    Prompt { site: String },
}

/// Install the process-global Chromium permission policy and its audit sink.
/// Call before `tauri::Builder::run`: the runtime ignores later calls, and any
/// request arriving before the policy is set is denied.
pub fn install_policy() {
    tauri_runtime_cef::set_permission_policy(evaluate);
    tauri_runtime_cef::set_permission_audit(audit);
}

/// Hand the policy the app handle it needs to raise the consent window.
pub fn attach(app: &crate::rt::AppHandle) {
    let _ = app_handle().set(app.clone());
}

/// One line per ENFORCED decision — what Chromium was actually told, emitted by
/// the runtime after the callback is answered, so it records what happened
/// rather than what the policy intended. Goes to the shell's stderr, which the
/// desktop launcher tees into the workspace log.
fn audit(event: &PermissionAudit) {
    let permissions = event
        .kinds
        .iter()
        .map(|kind| kind.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let origin = event
        .origin
        .as_ref()
        .map(|origin| origin.to_string())
        .unwrap_or_else(|| format!("opaque({:?})", event.raw_origin));
    let outcome = match event.reason {
        Some(reason) if !event.granted => format!("denied({reason})"),
        _ if event.granted => "granted".to_string(),
        _ => "denied".to_string(),
    };
    eprintln!(
        "permission {outcome}: request={} webview={:?} origin={origin} permissions=[{permissions}]",
        event.request_id, event.webview_label,
    );
}

/// The policy. Runs on a CEF thread: it decides, or it defers to the consent
/// window — it never blocks.
fn evaluate(request: PermissionRequest, responder: PermissionResponder) {
    match decide(&request) {
        Decision::Allow => responder.allow(),
        Decision::Deny(reason) => responder.deny(reason),
        Decision::PerKind(verdicts) => responder.decide(verdicts),
        Decision::Prompt { site } => {
            let deferred = responder.defer(DEFAULT_PROMPT_TIMEOUT);
            raise_prompt(&request, &site, deferred);
        }
    }
}

/// The rules, as a pure function of the request and the grants held so far.
fn decide(request: &PermissionRequest) -> Decision {
    // An origin that will not normalize (`null`, opaque, host-less) names
    // nobody, so no grant can be scoped to it.
    let Some(origin) = request.origin.as_ref() else {
        return Decision::Deny(DenyReason::InvalidOrigin);
    };
    // Permissions belong to the page the user navigated to, not to whatever it
    // embeds. Nothing in the product delegates a device to a subframe.
    if request.is_main_frame == Some(false) {
        return Decision::Deny(DenyReason::PolicyDenied);
    }
    if request.kinds.is_empty() {
        return Decision::Deny(DenyReason::UnsupportedPermission);
    }

    match principal(&request.webview_label, origin) {
        Principal::Untrusted => Decision::Deny(DenyReason::PolicyDenied),
        Principal::HostUi => Decision::PerKind(
            request
                .kinds
                .iter()
                .map(|kind| {
                    if HOST_UI_CAPABILITIES.contains(kind) {
                        Verdict::Allow
                    } else {
                        Verdict::Deny
                    }
                })
                .collect(),
        ),
        Principal::Gateway { site } => {
            // CEF grants a request whole or not at all, so a request mixing a
            // promptable device with something publisher content may never have
            // is denied outright — there is no subset left to ask about.
            if request
                .kinds
                .iter()
                .any(|kind| !GATEWAY_PROMPTABLE.contains(kind))
            {
                return Decision::Deny(DenyReason::PolicyDenied);
            }
            let held: Vec<Option<bool>> = request
                .kinds
                .iter()
                .map(|kind| grant_for(&request.webview_label, origin, *kind))
                .collect();
            if held.contains(&Some(false)) {
                return Decision::Deny(DenyReason::UserDenied);
            }
            if held.iter().all(|grant| *grant == Some(true)) {
                return Decision::Allow;
            }
            Decision::Prompt { site }
        }
    }
}

fn principal(webview_label: &str, origin: &NormalizedOrigin) -> Principal {
    if HOST_UI_LABELS.contains(&webview_label) {
        // A bundled window is trusted for what it *is* — but a bundled window
        // displaying remote content is not the thing that was trusted.
        return if origin.is_app_local() {
            Principal::HostUi
        } else {
            Principal::Untrusted
        };
    }
    if let Some(token) = webview_label.strip_prefix(GATEWAY_PREFIX) {
        // Fall back to the session token when the site was never recorded: a
        // gateway webview is gateway-class either way — worst case the consent
        // window names an ugly principal instead of a pretty one.
        let site = gateway_sites()
            .lock()
            .expect("gateway site registry poisoned")
            .get(webview_label)
            .cloned()
            .unwrap_or_else(|| token.to_string());
        return Principal::Gateway { site };
    }
    Principal::Untrusted
}

// ── registries ──────────────────────────────────────────

/// The `.duck` route each gateway webview was opened for, recorded by
/// [`crate::gateway_window`]. Without it a consent prompt could only name the
/// meaningless `<token>.localhost` session origin.
fn gateway_sites() -> &'static Mutex<HashMap<String, String>> {
    static SITES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    SITES.get_or_init(Default::default)
}

/// Record the site a gateway webview is about to show.
pub fn note_gateway_site(label: &str, site: &str) {
    gateway_sites()
        .lock()
        .expect("gateway site registry poisoned")
        .insert(label.to_string(), site.to_string());
}

/// A gateway webview is gone: forget its site and drop every grant it earned.
pub fn forget_webview(label: &str) {
    gateway_sites()
        .lock()
        .expect("gateway site registry poisoned")
        .remove(label);
    session_grants()
        .lock()
        .expect("session grant registry poisoned")
        .retain(|grant, _| grant.webview_label != label);
}

/// A grant the user gave through the consent window, for this run only.
///
/// The origin is part of the key, so a gateway webview that navigates to a
/// different origin cannot inherit the previous site's camera: the key stops
/// matching and the request goes back to the user.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GrantKey {
    webview_label: String,
    origin: String,
    permission: PermissionKind,
}

fn session_grants() -> &'static Mutex<HashMap<GrantKey, bool>> {
    static GRANTS: OnceLock<Mutex<HashMap<GrantKey, bool>>> = OnceLock::new();
    GRANTS.get_or_init(Default::default)
}

fn grant_for(
    webview_label: &str,
    origin: &NormalizedOrigin,
    permission: PermissionKind,
) -> Option<bool> {
    session_grants()
        .lock()
        .expect("session grant registry poisoned")
        .get(&GrantKey {
            webview_label: webview_label.to_string(),
            origin: origin.to_string(),
            permission,
        })
        .copied()
}

fn remember_grant(
    webview_label: &str,
    origin: &str,
    permission: PermissionKind,
    allowed: bool,
) {
    session_grants()
        .lock()
        .expect("session grant registry poisoned")
        .insert(
            GrantKey {
                webview_label: webview_label.to_string(),
                origin: origin.to_string(),
                permission,
            },
            allowed,
        );
}

// ── the consent window ──────────────────────────────────

/// The consent window's live request. At most one is open at a time.
struct OpenPrompt {
    responder: DeferredResponder,
    webview_label: String,
    origin: String,
    site: String,
    permissions: Vec<PermissionKind>,
}

fn open_prompt() -> &'static Mutex<Option<OpenPrompt>> {
    static PROMPT: OnceLock<Mutex<Option<OpenPrompt>>> = OnceLock::new();
    PROMPT.get_or_init(Default::default)
}

/// The policy is installed before (and lives outside) the Tauri app, so `setup`
/// hands it the app handle once the app is up.
fn app_handle() -> &'static OnceLock<crate::rt::AppHandle> {
    static APP: OnceLock<crate::rt::AppHandle> = OnceLock::new();
    &APP
}

/// Put the request in front of the user, in a window the page cannot reach: its
/// own OS window, on the app's own origin, holding only the two commands below.
/// Any failure to do so denies — the question is never silently dropped.
fn raise_prompt(request: &PermissionRequest, site: &str, deferred: DeferredResponder) {
    let Some(app) = app_handle().get() else {
        eprintln!("permission prompt not raised: the app is not up yet");
        deferred.deny(DenyReason::PolicyDenied);
        return;
    };
    {
        let mut prompt = open_prompt().lock().expect("prompt registry poisoned");
        // ponytail: one prompt at a time — a second request while the user is
        // deciding is denied rather than queued. getUserMedia asks for the mic
        // and the camera in ONE request, so this only bites a page firing
        // overlapping requests. Queue them if that turns out to be real.
        if prompt.as_ref().is_some_and(|open| open.responder.is_live()) {
            drop(prompt);
            eprintln!("permission prompt not raised: another request is already open");
            deferred.deny(DenyReason::PolicyDenied);
            return;
        }
        *prompt = Some(OpenPrompt {
            responder: deferred,
            webview_label: request.webview_label.clone(),
            origin: request
                .origin
                .as_ref()
                .map(|origin| origin.to_string())
                .unwrap_or_default(),
            site: site.to_string(),
            permissions: request.kinds.clone(),
        });
    }

    // The consent window is built once and HIDDEN between requests, never
    // closed: a closed webview stays resolvable in tauri's registry as a dead
    // husk, so the next request would `show()` a window that no longer exists
    // and hang until it expired.
    //
    // Reusing it means RELOADING it — Chromium freezes a hidden page's timers,
    // so a window that is merely shown again keeps rendering the request it was
    // last opened for. Navigating gives each request its own document, which
    // cannot show the user one site's name over another site's question.
    if let Some(window) = app.get_webview_window(PROMPT_LABEL) {
        match window.url() {
            Ok(mut url) => {
                url.set_query(Some(&format!("request={}", request.id)));
                let _ = window.navigate(url);
                let _ = window.show();
                let _ = window.set_focus();
            }
            Err(error) => {
                eprintln!("permission prompt not raised: {error}");
                resolve_prompt(None);
            }
        }
        return;
    }
    let built = WebviewWindowBuilder::new(app, PROMPT_LABEL, WebviewUrl::App("permission.html".into()))
        .title("Permission request")
        .inner_size(460.0, 360.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        // The page must not be able to cover, hide, or outlive the question it
        // provoked: the consent window floats above everything and takes focus.
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .devtools(false)
        .build();
    match built {
        Ok(window) => {
            window.on_window_event(move |event| {
                // Dismissing the question is not consenting to it.
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    resolve_prompt(Some((false, false)));
                }
            });
        }
        Err(error) => {
            eprintln!("permission prompt not raised: {error}");
            resolve_prompt(None);
        }
    }
}

/// What the consent window renders. `None` once the request is gone — answered,
/// expired, or its webview closed — which tells the window to shut itself.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptState {
    /// The `.duck` route that is asking.
    site: String,
    /// The session origin it is actually served from — shown so the window
    /// never hides where the content really comes from.
    origin: String,
    /// Runtime-neutral permission names, e.g. `["camera", "microphone"]`.
    permissions: Vec<String>,
}

/// The consent window's view of the request it must ask about.
#[tauri::command]
pub fn permission_prompt_state(
    window: crate::rt::WebviewWindow,
) -> Result<Option<PromptState>, String> {
    require_prompt_window(&window)?;
    let prompt = open_prompt().lock().expect("prompt registry poisoned");
    let Some(open) = prompt.as_ref().filter(|open| open.responder.is_live()) else {
        // The request died on its own (the runtime's timeout, or the webview
        // that asked went away). Nothing left to consent to.
        drop(prompt);
        hide_prompt_window();
        return Ok(None);
    };
    Ok(Some(PromptState {
        site: open.site.clone(),
        origin: open.origin.clone(),
        permissions: open
            .permissions
            .iter()
            .map(|permission| permission.to_string())
            .collect(),
    }))
}

/// The user's answer. `allow` grants every permission in the request (CEF
/// cannot grant a subset); `session` keeps the grant for this webview and
/// origin until it closes or navigates away.
#[tauri::command]
pub fn permission_prompt_decide(
    window: crate::rt::WebviewWindow,
    allow: bool,
    session: bool,
) -> Result<(), String> {
    require_prompt_window(&window)?;
    resolve_prompt(Some((allow, session)));
    Ok(())
}

/// Answer the open request (if it is still live), clear it, and put the consent
/// window away. `answer` is `None` when the prompt could not be shown at all —
/// which denies, because a question that never reached the user is not consent.
fn resolve_prompt(answer: Option<(bool, bool)>) {
    let open = open_prompt()
        .lock()
        .expect("prompt registry poisoned")
        .take();
    hide_prompt_window();
    let Some(open) = open else {
        return;
    };
    let Some((allow, session)) = answer else {
        open.responder.deny(DenyReason::PolicyDenied);
        return;
    };
    if session {
        for permission in &open.permissions {
            remember_grant(&open.webview_label, &open.origin, *permission, allow);
        }
    }
    if allow {
        open.responder.allow();
    } else {
        open.responder.deny(DenyReason::UserDenied);
    }
}

fn hide_prompt_window() {
    if let Some(window) = app_handle()
        .get()
        .and_then(|app| app.get_webview_window(PROMPT_LABEL))
    {
        let _ = window.hide();
    }
}

/// These commands answer for someone else's request, so only the window this
/// module opened may call them. (The ACL already restricts them to the
/// `permission-prompt` capability; this is the same rule enforced in Rust,
/// where it cannot drift.)
fn require_prompt_window(window: &crate::rt::WebviewWindow) -> Result<(), String> {
    if window.label() == PROMPT_LABEL {
        Ok(())
    } else {
        Err("this command is restricted to the permission prompt window".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_runtime_cef::RequestSource;

    fn request(label: &str, origin: &str, kinds: Vec<PermissionKind>) -> PermissionRequest {
        PermissionRequest {
            id: 1,
            webview_label: label.into(),
            origin: NormalizedOrigin::parse(origin),
            raw_origin: origin.into(),
            kinds,
            is_main_frame: Some(true),
            source: RequestSource::MediaAccess,
        }
    }

    #[test]
    fn the_console_gets_what_the_product_declares_and_nothing_else() {
        let allowed = decide(&request(
            "main",
            "http://localhost:1430",
            vec![PermissionKind::Microphone, PermissionKind::Camera],
        ));
        assert_eq!(
            allowed,
            Decision::PerKind(vec![Verdict::Allow, Verdict::Allow])
        );

        assert_eq!(
            decide(&request(
                "main",
                "tauri://localhost",
                vec![PermissionKind::LocalNetwork]
            )),
            Decision::PerKind(vec![Verdict::Allow]),
            "the packaged console needs Chromium local-network access to reach its loopback node"
        );

        let mixed = decide(&request(
            "main",
            "tauri://localhost",
            vec![PermissionKind::Microphone, PermissionKind::Geolocation],
        ));
        assert_eq!(
            mixed,
            Decision::PerKind(vec![Verdict::Allow, Verdict::Deny]),
            "an undeclared permission is denied even for the console"
        );
    }

    #[test]
    fn a_console_window_showing_remote_content_is_not_the_console() {
        assert_eq!(
            decide(&request(
                "main",
                "https://evil.example",
                vec![PermissionKind::Camera]
            )),
            Decision::Deny(DenyReason::PolicyDenied)
        );
    }

    #[test]
    fn gateway_content_may_only_ask_and_only_for_devices() {
        assert_eq!(
            decide(&request(
                "gateway-inline",
                "http://0123456789abcdef0123456789abcdef.localhost:49152",
                vec![PermissionKind::Camera]
            )),
            Decision::Prompt {
                site: "inline".into()
            },
            "an unrecorded site falls back to the label's own token"
        );

        note_gateway_site("gateway-abc", "app.demo.duck");
        assert_eq!(
            decide(&request(
                "gateway-abc",
                "http://abc.localhost:49152",
                vec![PermissionKind::Microphone, PermissionKind::Camera]
            )),
            Decision::Prompt {
                site: "app.demo.duck".into()
            },
            "the prompt names the .duck route, not the loopback session origin"
        );

        for forbidden in [
            PermissionKind::LocalNetwork,
            PermissionKind::Geolocation,
            PermissionKind::ClipboardRead,
            PermissionKind::Notifications,
            PermissionKind::MidiSysex,
            PermissionKind::Unknown(1 << 30),
        ] {
            assert_eq!(
                decide(&request(
                    "gateway-abc",
                    "http://abc.localhost:49152",
                    vec![forbidden]
                )),
                Decision::Deny(DenyReason::PolicyDenied),
                "{forbidden} must not even reach the user"
            );
        }

        assert_eq!(
            decide(&request(
                "gateway-abc",
                "http://abc.localhost:49152",
                vec![PermissionKind::Camera, PermissionKind::Geolocation]
            )),
            Decision::Deny(DenyReason::PolicyDenied),
            "a request mixing a device with a forbidden permission cannot be split"
        );
        forget_webview("gateway-abc");
    }

    #[test]
    fn an_unknown_webview_is_untrusted() {
        assert_eq!(
            decide(&request(
                "some-other-window",
                "http://localhost:1430",
                vec![PermissionKind::Camera]
            )),
            Decision::Deny(DenyReason::PolicyDenied)
        );
    }

    #[test]
    fn opaque_origins_and_subframes_are_denied() {
        assert_eq!(
            decide(&request("main", "null", vec![PermissionKind::Camera])),
            Decision::Deny(DenyReason::InvalidOrigin)
        );

        let mut subframe = request(
            "gateway-sub",
            "http://sub.localhost:49152",
            vec![PermissionKind::Camera],
        );
        subframe.is_main_frame = Some(false);
        assert_eq!(
            decide(&subframe),
            Decision::Deny(DenyReason::PolicyDenied),
            "an embedded frame does not inherit its host page's devices"
        );
    }

    #[test]
    fn a_session_grant_is_bound_to_its_webview_and_origin() {
        let origin = "http://feed.localhost:49152";
        note_gateway_site("gateway-feed", "feed.demo.duck");
        remember_grant("gateway-feed", origin, PermissionKind::Camera, true);

        assert_eq!(
            decide(&request("gateway-feed", origin, vec![PermissionKind::Camera])),
            Decision::Allow,
            "the site the user allowed does not get asked twice"
        );
        assert!(matches!(
            decide(&request(
                "gateway-feed",
                "http://feed.localhost:49999",
                vec![PermissionKind::Camera]
            )),
            Decision::Prompt { .. }
        ), "navigating to another origin strands the grant");
        assert!(matches!(
            decide(&request(
                "gateway-feed",
                origin,
                vec![PermissionKind::Camera, PermissionKind::Microphone]
            )),
            Decision::Prompt { .. }
        ), "a permission that was never granted is still asked about");

        // A denial the user gave is remembered too, and is not re-asked.
        remember_grant("gateway-feed", origin, PermissionKind::Microphone, false);
        assert_eq!(
            decide(&request(
                "gateway-feed",
                origin,
                vec![PermissionKind::Microphone]
            )),
            Decision::Deny(DenyReason::UserDenied)
        );

        forget_webview("gateway-feed");
        assert!(matches!(
            decide(&request("gateway-feed", origin, vec![PermissionKind::Camera])),
            Decision::Prompt { .. }
        ), "closing the webview drops its grants");
    }
}
