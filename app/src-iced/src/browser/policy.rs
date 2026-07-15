use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cef::{
    ImplMediaAccessCallback as _, ImplPermissionPromptCallback as _, MediaAccessCallback,
    MediaAccessPermissionTypes, PermissionPromptCallback, PermissionRequestResult,
    PermissionRequestTypes,
};

use crate::browser_chrome::validate_duck_host;

const MAX_URL_BYTES: usize = 2 * 1024 * 1024;
const MAX_NET_DATA_IMAGE_URL_BYTES: usize = 64 * 1024 * 1024;
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// Runtime-neutral permissions the iced shell may show in native consent UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserPermission {
    Microphone,
    Camera,
    ScreenCapture,
    LocalNetwork,
}

impl BrowserPermission {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::ScreenCapture => "screen-capture",
            Self::LocalNetwork => "local-network",
        }
    }
}

/// One live all-or-nothing permission question. The shell renders this in its
/// own trusted iced UI and answers it with `BrowserRuntime::decide_permission`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPrompt {
    pub id: u64,
    pub origin: String,
    pub permissions: Vec<BrowserPermission>,
}

#[derive(Clone)]
pub(crate) struct PermissionBroker(Arc<Mutex<PermissionState>>);

struct PermissionState {
    next_id: u64,
    pending: Option<PendingPermission>,
    grants: HashMap<(String, BrowserPermission), bool>,
}

struct PendingPermission {
    prompt: PermissionPrompt,
    opened: Instant,
    completion: Completion,
}

enum Completion {
    Media {
        callback: MediaAccessCallback,
        requested: u32,
    },
    Prompt(PermissionPromptCallback),
    #[cfg(test)]
    Test(std::sync::mpsc::Sender<bool>),
}

impl Completion {
    fn finish(self, allowed: bool) {
        match self {
            Self::Media {
                callback,
                requested,
            } => callback.cont(if allowed {
                requested
            } else {
                MediaAccessPermissionTypes::NONE.get_raw()
            }),
            Self::Prompt(callback) => callback.cont(if allowed {
                PermissionRequestResult::ACCEPT
            } else {
                PermissionRequestResult::DENY
            }),
            #[cfg(test)]
            Self::Test(sender) => {
                let _ = sender.send(allowed);
            }
        }
    }
}

impl Default for PermissionBroker {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(PermissionState {
            next_id: 1,
            pending: None,
            grants: HashMap::new(),
        })))
    }
}

impl PermissionBroker {
    pub(crate) fn request_media(
        &self,
        origin: &str,
        is_main_frame: bool,
        requested: u32,
        callback: MediaAccessCallback,
    ) {
        let Some(permissions) = media_permissions(requested) else {
            Completion::Media {
                callback,
                requested,
            }
            .finish(false);
            return;
        };
        self.request(
            origin,
            is_main_frame,
            permissions,
            Completion::Media {
                callback,
                requested,
            },
            false,
        );
    }

    pub(crate) fn request_prompt(
        &self,
        origin: &str,
        requested: u32,
        callback: PermissionPromptCallback,
        allow_minted_loopback: bool,
    ) {
        let Some(permissions) = prompt_permissions(requested) else {
            Completion::Prompt(callback).finish(false);
            return;
        };
        self.request(
            origin,
            true,
            permissions,
            Completion::Prompt(callback),
            allow_minted_loopback,
        );
    }

    fn request(
        &self,
        raw_origin: &str,
        is_main_frame: bool,
        permissions: Vec<BrowserPermission>,
        completion: Completion,
        allow_minted_loopback: bool,
    ) {
        self.expire();
        let Some(origin) = normalize_duck_origin(raw_origin) else {
            completion.finish(false);
            return;
        };
        if !is_main_frame || permissions.is_empty() {
            completion.finish(false);
            return;
        }
        if permissions.contains(&BrowserPermission::LocalNetwork) {
            completion.finish(
                allow_minted_loopback
                    && permissions.len() == 1
                    && permissions[0] == BrowserPermission::LocalNetwork,
            );
            return;
        }

        let mut completion = Some(completion);
        let immediate = {
            let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
            let remembered: Vec<_> = permissions
                .iter()
                .filter_map(|permission| state.grants.get(&(origin.clone(), *permission)).copied())
                .collect();
            if remembered.len() == permissions.len() {
                Some(remembered.into_iter().all(|allowed| allowed))
            } else if state.pending.is_some() {
                Some(false)
            } else {
                let prompt = PermissionPrompt {
                    id: state.next_id,
                    origin,
                    permissions,
                };
                state.next_id = state.next_id.saturating_add(1);
                state.pending = Some(PendingPermission {
                    prompt,
                    opened: Instant::now(),
                    completion: completion.take().expect("completion moves once"),
                });
                None
            }
        };
        if let Some(allowed) = immediate {
            completion
                .expect("immediate completion remains owned")
                .finish(allowed);
        }
    }

    pub(crate) fn prompt(&self) -> Option<PermissionPrompt> {
        self.expire();
        self.0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .pending
            .as_ref()
            .map(|pending| pending.prompt.clone())
    }

    pub(crate) fn decide(&self, id: u64, allow: bool, session: bool) -> Result<(), String> {
        self.expire();
        let pending = {
            let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
            if state.pending.as_ref().map(|pending| pending.prompt.id) != Some(id) {
                return Err("permission prompt is no longer active".into());
            }
            let pending = state.pending.take().expect("matching prompt exists");
            if session {
                for permission in &pending.prompt.permissions {
                    state
                        .grants
                        .insert((pending.prompt.origin.clone(), *permission), allow);
                }
            }
            pending
        };
        pending.completion.finish(allow);
        Ok(())
    }

    pub(crate) fn expire(&self) {
        let expired = {
            let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
            if state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.opened.elapsed() >= PROMPT_TIMEOUT)
            {
                state.pending.take()
            } else {
                None
            }
        };
        if let Some(expired) = expired {
            tracing::warn!(
                target: "ducktape::browser",
                reason = "permission_timeout",
                "browser permission request expired"
            );
            expired.completion.finish(false);
        }
    }

    pub(crate) fn close(&self) {
        let pending = {
            let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
            state.grants.clear();
            state.pending.take()
        };
        if let Some(pending) = pending {
            pending.completion.finish(false);
        }
    }
}

fn normalize_duck_origin(raw: &str) -> Option<String> {
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "duck"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    validate_duck_host(&host).ok()?;
    Some(format!("duck://{host}"))
}

fn media_permissions(mask: u32) -> Option<Vec<BrowserPermission>> {
    let table = [
        (
            MediaAccessPermissionTypes::DEVICE_AUDIO_CAPTURE.get_raw(),
            BrowserPermission::Microphone,
        ),
        (
            MediaAccessPermissionTypes::DEVICE_VIDEO_CAPTURE.get_raw(),
            BrowserPermission::Camera,
        ),
        (
            MediaAccessPermissionTypes::DESKTOP_VIDEO_CAPTURE.get_raw(),
            BrowserPermission::ScreenCapture,
        ),
    ];
    permissions_from_mask(mask, &table)
}

fn prompt_permissions(mask: u32) -> Option<Vec<BrowserPermission>> {
    let table = [
        (
            PermissionRequestTypes::CAMERA_STREAM.get_raw(),
            BrowserPermission::Camera,
        ),
        (
            PermissionRequestTypes::MIC_STREAM.get_raw(),
            BrowserPermission::Microphone,
        ),
        (
            PermissionRequestTypes::LOCAL_NETWORK_ACCESS.get_raw(),
            BrowserPermission::LocalNetwork,
        ),
        (
            PermissionRequestTypes::LOCAL_NETWORK.get_raw(),
            BrowserPermission::LocalNetwork,
        ),
        (
            PermissionRequestTypes::LOOPBACK_NETWORK.get_raw(),
            BrowserPermission::LocalNetwork,
        ),
    ];
    permissions_from_mask(mask, &table)
}

fn permissions_from_mask(
    mask: u32,
    table: &[(u32, BrowserPermission)],
) -> Option<Vec<BrowserPermission>> {
    let known = table.iter().fold(0, |known, (bit, _)| known | bit);
    if mask == 0 || mask & !known != 0 {
        return None;
    }
    let mut permissions = Vec::new();
    for (bit, permission) in table {
        if mask & bit != 0 && !permissions.contains(permission) {
            permissions.push(*permission);
        }
    }
    Some(permissions)
}

#[derive(Debug, Clone)]
pub(crate) enum NavigationPolicy {
    Exact(String),
    Origin { scheme: String, authority: String },
}

impl NavigationPolicy {
    pub(crate) fn new(url: &str) -> Result<Self, String> {
        match parse(url, true)? {
            Parsed::Exact => Ok(Self::Exact(url.to_string())),
            Parsed::Origin { scheme, authority } => Ok(Self::Origin { scheme, authority }),
        }
    }

    pub(crate) fn allows(&self, url: &str) -> bool {
        match self {
            Self::Exact(initial) => url == initial,
            Self::Origin { scheme, authority } => matches!(
                parse(url, false),
                Ok(Parsed::Origin {
                    scheme: candidate_scheme,
                    authority: candidate_authority,
                }) if candidate_scheme == *scheme && candidate_authority == *authority
            ),
        }
    }

    pub(crate) fn allows_resource(&self, url: &str) -> bool {
        self.allows(url)
            || matches!(
                self,
                Self::Origin { authority, .. }
                    if authority == "net.duck" && allowed_data_image(url)
            )
    }

    pub(crate) fn is_idle(&self) -> bool {
        matches!(self, Self::Exact(url) if url.eq_ignore_ascii_case("about:blank"))
    }
}

fn allowed_data_image(url: &str) -> bool {
    if !net_data_image_size_is_allowed(url.len())
        || url
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }

    [
        "data:image/gif;base64,",
        "data:image/jpeg;base64,",
        "data:image/png;base64,",
        "data:image/webp;base64,",
    ]
    .iter()
    .any(|prefix| {
        url.get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
    })
}

const fn net_data_image_size_is_allowed(size: usize) -> bool {
    size <= MAX_NET_DATA_IMAGE_URL_BYTES
}

enum Parsed {
    Exact,
    Origin { scheme: String, authority: String },
}

fn parse(url: &str, initial: bool) -> Result<Parsed, String> {
    if url.is_empty()
        || url.len() > MAX_URL_BYTES
        || url
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("URL is empty, oversized, or contains whitespace/control bytes".into());
    }

    let (scheme, rest) = url
        .split_once(':')
        .ok_or_else(|| "URL has no scheme".to_string())?;
    if !valid_scheme(scheme) {
        return Err("URL has an invalid scheme".into());
    }
    let scheme = scheme.to_ascii_lowercase();

    match scheme.as_str() {
        "data" => {
            let lower = rest
                .get(..rest.len().min(32))
                .unwrap_or(rest)
                .to_ascii_lowercase();
            if lower.starts_with("text/html,") || lower.starts_with("text/html;charset=utf-8,") {
                Ok(Parsed::Exact)
            } else {
                Err("only HTML data URLs are allowed".into())
            }
        }
        "about" if rest == "blank" => Ok(Parsed::Exact),
        "duck" => {
            let (authority, suffix) = authority(rest)?;
            if initial && suffix.contains('#') && !authority.eq_ignore_ascii_case("net.duck") {
                return Err("initial duck URL must not contain a fragment".into());
            }
            validate_duck_host(authority)?;
            Ok(Parsed::Origin {
                scheme,
                authority: authority.to_ascii_lowercase(),
            })
        }
        _ => Err("URL scheme is not allowed".into()),
    }
}

fn valid_scheme(scheme: &str) -> bool {
    let mut bytes = scheme.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn authority(rest: &str) -> Result<(&str, &str), String> {
    let rest = rest
        .strip_prefix("//")
        .ok_or_else(|| "URL requires an authority".to_string())?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') {
        return Err("URL authority is empty or contains credentials".into());
    }
    Ok((authority, &rest[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duck_navigation_is_pinned_to_one_valid_origin() {
        let policy = NavigationPolicy::new("duck://app.demo.duck/start").unwrap();
        assert!(policy.allows("duck://app.demo.duck/next?q=1#section"));
        assert!(policy.allows("DUCK://APP.DEMO.DUCK/next"));
        assert!(!policy.allows("duck://other.demo.duck/next"));
        assert!(!policy.allows("duck://user@app.demo.duck/next"));
        assert!(NavigationPolicy::new("duck://app.demo.duck/#secret").is_err());
        assert!(NavigationPolicy::new("duck://net.duck/index.html#part").is_ok());
        assert!(NavigationPolicy::new("duck://agents.duck/").is_err());
        assert!(NavigationPolicy::new("duck://api.net.duck/").is_err());
        assert!(NavigationPolicy::new("duck://a.b.c.demo.duck/").is_err());
    }

    #[test]
    fn direct_network_and_local_file_urls_are_refused() {
        assert!(NavigationPolicy::new("http://127.0.0.1:3210/index").is_err());
        assert!(NavigationPolicy::new("https://[::1]:8443/").is_err());
        assert!(NavigationPolicy::new("https://example.com/").is_err());
        assert!(NavigationPolicy::new("file:///etc/passwd").is_err());
        assert!(NavigationPolicy::new("javascript:alert(1)").is_err());
    }

    #[test]
    fn local_documents_are_exact() {
        let data = "data:text/html,%3Ch1%3Eok%3C/h1%3E";
        let policy = NavigationPolicy::new(data).unwrap();
        assert!(policy.allows(data));
        assert!(!policy.allows("data:text/html,%3Ch1%3Echanged%3C/h1%3E"));
        assert!(NavigationPolicy::new("data:text/plain,nope").is_err());
        assert!(NavigationPolicy::new("about:blank").is_ok());
        assert!(NavigationPolicy::new("about:blank").unwrap().is_idle());
        assert!(!NavigationPolicy::new(data).unwrap().is_idle());
        assert!(!NavigationPolicy::new("duck://net.duck/").unwrap().is_idle());
    }

    #[test]
    fn net_resources_allow_only_bounded_raster_data_images() {
        let policy = NavigationPolicy::new("duck://net.duck/index.html").unwrap();
        for image in [
            "data:image/gif;base64,R0lGODlhAQABAAAAACw=",
            "DATA:IMAGE/JPEG;BASE64,/9j/2Q==",
            "data:image/png;base64,iVBORw0KGgo=",
            "data:image/webp;base64,UklGRg==",
        ] {
            assert!(policy.allows_resource(image), "refused {image}");
            assert!(!policy.allows(image), "data image became a navigation");
        }

        for rejected in [
            "data:image/svg+xml;base64,PHN2Zz4=",
            "data:text/plain;base64,bm9wZQ==",
            "data:text/html;base64,PGgxPm5vcGU8L2gxPg==",
            "javascript:alert(1)",
        ] {
            assert!(!policy.allows_resource(rejected), "allowed {rejected}");
        }

        let larger_than_navigation_limit =
            format!("data:image/png;base64,{}", "A".repeat(MAX_URL_BYTES));
        assert!(policy.allows_resource(&larger_than_navigation_limit));
        assert!(net_data_image_size_is_allowed(MAX_NET_DATA_IMAGE_URL_BYTES));
        assert!(!net_data_image_size_is_allowed(
            MAX_NET_DATA_IMAGE_URL_BYTES + 1
        ));
    }

    #[test]
    fn account_resources_reject_data_and_cross_origin_urls() {
        let policy = NavigationPolicy::new("duck://app.demo.duck/index.html").unwrap();
        assert!(!policy.allows_resource("data:image/png;base64,iVBORw0KGgo="));
        assert!(!policy.allows_resource("duck://other.demo.duck/image.png"));
        assert!(policy.allows_resource("duck://app.demo.duck/image.png"));
    }

    #[test]
    fn permission_masks_are_closed_and_deduplicated() {
        let screen = MediaAccessPermissionTypes::DESKTOP_VIDEO_CAPTURE.get_raw();
        assert_eq!(
            media_permissions(screen),
            Some(vec![BrowserPermission::ScreenCapture])
        );
        assert!(
            media_permissions(screen | MediaAccessPermissionTypes::DESKTOP_AUDIO_CAPTURE.get_raw())
                .is_none()
        );
        assert!(media_permissions(screen | (1 << 31)).is_none());

        let local = PermissionRequestTypes::LOCAL_NETWORK.get_raw();
        assert_eq!(
            prompt_permissions(local),
            Some(vec![BrowserPermission::LocalNetwork])
        );
        assert!(prompt_permissions(PermissionRequestTypes::GEOLOCATION.get_raw()).is_none());
    }

    #[test]
    fn consent_is_main_frame_origin_scoped_and_session_only() {
        let broker = PermissionBroker::default();
        let camera = vec![BrowserPermission::Camera];

        let (first_tx, first_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://app.demo.duck",
            true,
            camera.clone(),
            Completion::Test(first_tx),
            false,
        );
        let first = broker.prompt().unwrap();
        broker.decide(first.id, true, true).unwrap();
        assert!(first_rx.recv().unwrap());

        let (remembered_tx, remembered_rx) = std::sync::mpsc::channel();
        broker.request(
            "DUCK://APP.DEMO.DUCK",
            true,
            camera.clone(),
            Completion::Test(remembered_tx),
            false,
        );
        assert!(remembered_rx.recv().unwrap());
        assert!(broker.prompt().is_none());

        let (other_tx, other_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://other.demo.duck",
            true,
            camera.clone(),
            Completion::Test(other_tx),
            false,
        );
        assert!(broker.prompt().is_some());
        broker.close();
        assert!(!other_rx.recv().unwrap());

        let (subframe_tx, subframe_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://app.demo.duck",
            false,
            camera,
            Completion::Test(subframe_tx),
            false,
        );
        assert!(!subframe_rx.recv().unwrap());
    }

    #[test]
    fn local_network_requires_an_exact_minted_loopback_capability() {
        let broker = PermissionBroker::default();
        let (local_tx, local_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://app.demo.duck",
            true,
            vec![BrowserPermission::LocalNetwork],
            Completion::Test(local_tx),
            false,
        );
        assert!(!local_rx.recv().unwrap());

        let (minted_tx, minted_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://app.demo.duck",
            true,
            vec![BrowserPermission::LocalNetwork],
            Completion::Test(minted_tx),
            true,
        );
        assert!(minted_rx.recv().unwrap());

        let (mixed_tx, mixed_rx) = std::sync::mpsc::channel();
        broker.request(
            "duck://app.demo.duck",
            true,
            vec![BrowserPermission::LocalNetwork, BrowserPermission::Camera],
            Completion::Test(mixed_tx),
            true,
        );
        assert!(!mixed_rx.recv().unwrap());
    }
}
