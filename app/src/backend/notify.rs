//! NATIVE DESKTOP NOTIFICATIONS for the two arrivals a person cannot afford to
//! find later: being `@mentioned`, and a direct message.
//!
//! Everything a mention produced before this landed inside the app — an inbox
//! row and the bell's unread count — which is worth nothing to a reader whose
//! window is behind an editor. The decision of WHETHER to notify is pure and
//! lives in [`desktop_notice`]; posting is one platform call behind it, and
//! only that call is untestable.
//!
//! THE POSTING SIDE REFUSES OUTSIDE A BUNDLE. `UNUserNotificationCenter`
//! terminates a process that has no bundle identifier, and `cargo test` is
//! exactly such a process — so the bundle is checked before the framework is
//! touched, and a bare binary degrades to a `debug!` line.

use super::*;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

/// How much of a message body a notification carries. A banner shows two or
/// three lines; past that the excerpt is only paying for itself in memory.
const EXCERPT_CHARS: usize = 140;

/// One notification, already worded — the shape the platform call takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopNotice {
    /// the room, as the sidebar says it: `#general`, or the peer's name.
    pub title: String,
    /// who wrote it, by account name.
    pub subtitle: String,
    pub body: String,
    /// the room, for grouping every notice from one conversation together.
    pub thread: String,
}

/// WHY THIS ARRIVAL WOULD BE WORTH INTERRUPTING FOR. One discriminant, so a
/// third reason has to be routed rather than folded into a boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeReason {
    Mentioned,
    DirectMessage,
}

/// One arrived chat message, as the live fold reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub channel_id: String,
    /// the room's display name — `#general`, or the DM peer's name.
    pub room: String,
    /// the author's account name (or the short handle, unnamed).
    pub author: String,
    pub body: String,
    pub mentions_me: bool,
    pub in_my_dm: bool,
    pub authored_by_me: bool,
}

/// What the reader can already see for themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnScreen {
    pub app_focused: bool,
    pub active_channel: String,
}

/// The reason to notify about this arrival, or `None` for the arrivals that
/// are none: my own writing, and everything addressed to nobody in particular.
pub fn notice_reason(arrival: &Arrival) -> Option<NoticeReason> {
    if arrival.authored_by_me {
        return None;
    }
    // A mention outranks the room it was written in: being named in your own
    // DM is still being named.
    match (arrival.mentions_me, arrival.in_my_dm) {
        (true, _) => Some(NoticeReason::Mentioned),
        (false, true) => Some(NoticeReason::DirectMessage),
        (false, false) => None,
    }
}

/// THE WHOLE DECISION, pure: what to post for this arrival, or nothing.
///
/// Suppression is narrow ON PURPOSE — only a reader who is LOOKING AT the room
/// the message landed in has already been told. A focused window on another
/// room, or the Files tab, has not.
pub fn desktop_notice(
    arrival: &Arrival,
    enabled: bool,
    screen: &OnScreen,
) -> Option<DesktopNotice> {
    if !enabled {
        return None;
    }
    let reason = notice_reason(arrival)?;
    let already_read_it = screen.app_focused && screen.active_channel == arrival.channel_id;
    if already_read_it {
        return None;
    }
    Some(notice_text(arrival, reason))
}

/// The words. Split from the decision so both are checkable, and so the one
/// place a person's message is rendered for the lock screen is nameable.
pub fn notice_text(arrival: &Arrival, reason: NoticeReason) -> DesktopNotice {
    let subtitle = match reason {
        NoticeReason::Mentioned => format!("{} mentioned you", arrival.author),
        NoticeReason::DirectMessage => arrival.author.clone(),
    };
    DesktopNotice {
        title: arrival.room.clone(),
        subtitle,
        body: notice_excerpt(&arrival.body),
        thread: arrival.channel_id.clone(),
    }
}

/// A message body as one line of banner text: newlines collapse to spaces,
/// runs of whitespace collapse to one, and a long body is cut on a CHARACTER
/// boundary with an ellipsis.
pub fn notice_excerpt(body: &str) -> String {
    let flat: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= EXCERPT_CHARS {
        return flat;
    }
    let kept: String = flat.chars().take(EXCERPT_CHARS).collect();
    format!("{}…", kept.trim_end())
}

// ============================================================================
// the preference
// ============================================================================

/// The prefs key. DEVICE-global like `appearance`: whether this machine may
/// raise a banner is a property of the machine, not of a workspace.
const NOTIFY_PREF: &str = "desktop_notifications";

/// Default ON — a person who installs a chat app expects to be told they were
/// named. Only an explicit `false` turns it off.
pub fn notifications_enabled() -> bool {
    read_prefs()[NOTIFY_PREF].as_bool().unwrap_or(true)
}

/// The Settings toggle's reading, at boot.
pub async fn load_desktop_notifications() -> bool {
    notifications_enabled()
}

/// Persist it. Best-effort like `save_appearance`: a failed write costs the
/// NEXT boot's default and nothing this session shows.
pub async fn save_desktop_notifications(enabled: bool) -> bool {
    let mut prefs = read_prefs();
    prefs[NOTIFY_PREF] = serde_json::json!(enabled);
    write_prefs(&prefs)
}

// ============================================================================
// what the live fold cannot see for itself
// ============================================================================

/// THE ROOM ON SCREEN, recorded where it is decided. The live decoder runs off
/// to the side of the reducer and is handed one op, so the two facts a
/// suppression needs — which room is open, and what the rooms are called —
/// reach it the way `ACCOUNT_NAMES` does: recorded by the load that learns
/// them.
static ACTIVE_CHANNEL: RwLock<String> = RwLock::new(String::new());
/// channel id → the name the sidebar draws.
static ROOM_NAMES: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());
/// the two-party rooms THIS reader is a party to.
static MY_DM_ROOMS: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());

/// Record the rooms and the one the reader is in — called by the chat load,
/// which is what decides both.
pub(crate) fn note_rooms(channels: &[ChatChannel], active: &str) {
    if let Ok(mut names) = ROOM_NAMES.write() {
        for channel in channels {
            names.insert(channel.id.clone(), format!("#{}", channel.name));
        }
    }
    if let Ok(mut open) = ACTIVE_CHANNEL.write() {
        active.clone_into(&mut open);
    }
}

/// Record this reader's own DM rooms and what to call them — the directory
/// load already derives both (`DmPeer.channel_id`, `DmPeer.name`).
pub(crate) fn note_dm_rooms(peers: &[DmPeer]) {
    let mine: BTreeSet<String> = peers
        .iter()
        .map(|peer| peer.channel_id.clone())
        .filter(|id| !id.is_empty())
        .collect();
    if let Ok(mut names) = ROOM_NAMES.write() {
        for peer in peers.iter().filter(|peer| !peer.channel_id.is_empty()) {
            names.insert(peer.channel_id.clone(), peer.name.clone());
        }
    }
    if let Ok(mut rooms) = MY_DM_ROOMS.write() {
        *rooms = mine;
    }
}

fn room_name(channel_id: &str) -> String {
    ROOM_NAMES
        .read()
        .ok()
        .and_then(|names| names.get(channel_id).cloned())
        .unwrap_or_else(|| channel_id.to_string())
}

fn in_my_dm(channel_id: &str) -> bool {
    MY_DM_ROOMS
        .read()
        .is_ok_and(|rooms| rooms.contains(channel_id))
}

fn active_channel() -> String {
    ACTIVE_CHANNEL
        .read()
        .map(|open| open.clone())
        .unwrap_or_default()
}

// ============================================================================
// the live trigger
// ============================================================================

/// One applied chat op → at most one banner. Called from the live decoder with
/// the payload it already decoded the delta from.
///
/// Only a POST notifies: an edit, a reaction or a membership change is not an
/// arrival. Everything this reads is either in the op or in a warm cache — no
/// query runs here, for the reason the decoder's other reads are cached
/// (a `/v1/query` inside the fold freezes every subscriber).
pub(crate) fn notify_chat_op(payload: &[u8], origin_id: Option<&str>, names: &NameDirectory) {
    let Ok(::chat::ChatMsg::PostMessage {
        channel_id, blocks, ..
    }) = ::chat::decode_msg(payload)
    else {
        return;
    };
    let Some(author_key) = origin_id else {
        return;
    };
    let me = rpc::cached_user_key();
    let my_keys = me
        .as_deref()
        .map(|key| names.account_keys_of(key))
        .unwrap_or_default();
    let arrival = Arrival {
        room: room_name(&channel_id),
        in_my_dm: in_my_dm(&channel_id),
        channel_id,
        author: ::chat::client::author_display(&format!("user:{author_key}"), names),
        body: ::chat::client::message_body(&blocks),
        mentions_me: ::chat::client::mentions_reach(&blocks, &my_keys),
        authored_by_me: me
            .as_deref()
            .is_some_and(|key| hex_encode(key).eq_ignore_ascii_case(author_key)),
    };
    let screen = OnScreen {
        app_focused: platform::app_is_frontmost(),
        active_channel: active_channel(),
    };
    let Some(notice) = desktop_notice(&arrival, notifications_enabled(), &screen) else {
        return;
    };
    platform::post(&notice);
}

// ============================================================================
// the platform call — the one part no test reaches
// ============================================================================

#[cfg(target_os = "macos")]
mod platform {
    use super::DesktopNotice;
    use std::sync::Once;

    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNNotificationSound, UNUserNotificationCenter,
    };

    /// UNUserNotificationCenter TERMINATES a process with no bundle
    /// identifier — it is not an error a caller can catch, the process dies.
    /// `cargo test`, `cargo run` and any bare binary are exactly that process,
    /// so the identifier is read first and its absence is a `debug!` line.
    fn bundled() -> bool {
        // SAFETY: reading the main bundle's identifier is valid on any thread.
        unsafe { NSBundle::mainBundle().bundleIdentifier().is_some() }
    }

    /// Ask once per process. macOS remembers the answer per bundle id; asking
    /// again is a no-op, but doing it once keeps the prompt off every message.
    fn authorize(center: &UNUserNotificationCenter) {
        static ASKED: Once = Once::new();
        ASKED.call_once(|| {
            let options = UNAuthorizationOptions::UNAuthorizationOptionAlert
                | UNAuthorizationOptions::UNAuthorizationOptionSound;
            let handler = block2::RcBlock::new(|granted: Bool, _error: *mut NSError| {
                tracing::debug!(
                    target: "ducktape::app",
                    granted = granted.as_bool(),
                    "desktop notification authorization answered"
                );
            });
            // SAFETY: the center is a live object and the block outlives the call.
            unsafe { center.requestAuthorizationWithOptions_completionHandler(options, &handler) };
        });
    }

    /// True when this app is the one the person is looking at.
    pub(super) fn app_is_frontmost() -> bool {
        // SAFETY: `currentApplication` and `isActive` are documented as
        // callable from any thread.
        unsafe { NSRunningApplication::currentApplication().isActive() }
    }

    pub(super) fn post(notice: &DesktopNotice) {
        if !bundled() {
            tracing::debug!(
                target: "ducktape::app",
                reason = "no_bundle_identifier",
                "skipped a desktop notification"
            );
            return;
        }
        // SAFETY: every call below is a plain framework call on objects this
        // function owns; the center is thread-safe by contract.
        unsafe {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            authorize(&center);
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&notice.title));
            content.setSubtitle(&NSString::from_str(&notice.subtitle));
            content.setBody(&NSString::from_str(&notice.body));
            content.setThreadIdentifier(&NSString::from_str(&notice.thread));
            content.setSound(Some(&UNNotificationSound::defaultSound()));
            let id = NSString::from_str(&format!("ducktape-{}", fresh_notice_id()));
            let request: Retained<UNNotificationRequest> =
                UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
            center.addNotificationRequest_withCompletionHandler(&request, None);
        }
    }

    /// A fresh identifier per banner — reusing one REPLACES the standing
    /// notification instead of adding to it.
    fn fresh_notice_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::DesktopNotice;

    /// No native notifier is wired outside macOS yet; the in-app bell is still
    /// the whole story there.
    pub(super) fn post(notice: &DesktopNotice) {
        tracing::debug!(
            target: "ducktape::app",
            reason = "unsupported_platform",
            room = %notice.thread,
            "skipped a desktop notification"
        );
    }

    /// With no way to ask, assume the reader is elsewhere: a banner they did
    /// not need costs less than the mention they never saw.
    pub(super) fn app_is_frontmost() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrival() -> Arrival {
        Arrival {
            channel_id: "general".into(),
            room: "#general".into(),
            author: "orthory".into(),
            body: "ping @eddy about the deploy".into(),
            mentions_me: true,
            in_my_dm: false,
            authored_by_me: false,
        }
    }

    fn elsewhere() -> OnScreen {
        OnScreen {
            app_focused: false,
            active_channel: String::new(),
        }
    }

    #[test]
    fn a_mention_notifies_and_names_the_room_the_author_and_the_words() {
        let notice = desktop_notice(&arrival(), true, &elsewhere()).expect("a mention notifies");
        assert_eq!(notice.title, "#general");
        assert_eq!(notice.subtitle, "orthory mentioned you");
        assert_eq!(notice.body, "ping @eddy about the deploy");
        assert_eq!(notice.thread, "general", "banners group per room");
    }

    #[test]
    fn a_dm_notifies_without_a_mention() {
        let dm = Arrival {
            channel_id: "dm-1".into(),
            room: "orthory".into(),
            mentions_me: false,
            in_my_dm: true,
            ..arrival()
        };
        let notice = desktop_notice(&dm, true, &elsewhere()).expect("a DM notifies");
        assert_eq!(notice.title, "orthory");
        assert_eq!(
            notice.subtitle, "orthory",
            "a DM is already addressed to me — 'mentioned you' would be a lie"
        );
    }

    /// A mention in my own DM is still a mention.
    #[test]
    fn a_mention_outranks_the_room_it_landed_in() {
        let both = Arrival {
            in_my_dm: true,
            ..arrival()
        };
        assert_eq!(notice_reason(&both), Some(NoticeReason::Mentioned));
    }

    #[test]
    fn an_ordinary_room_message_never_notifies() {
        let chatter = Arrival {
            mentions_me: false,
            ..arrival()
        };
        assert_eq!(notice_reason(&chatter), None);
        assert!(desktop_notice(&chatter, true, &elsewhere()).is_none());
    }

    #[test]
    fn my_own_writing_never_notifies_me() {
        let mine = Arrival {
            authored_by_me: true,
            ..arrival()
        };
        assert_eq!(notice_reason(&mine), None);
    }

    /// The one suppression: the reader is LOOKING AT the room it landed in.
    #[test]
    fn the_room_on_screen_in_a_focused_window_suppresses_it() {
        let watching = OnScreen {
            app_focused: true,
            active_channel: "general".into(),
        };
        assert!(desktop_notice(&arrival(), true, &watching).is_none());

        let other_room = OnScreen {
            app_focused: true,
            active_channel: "random".into(),
        };
        assert!(
            desktop_notice(&arrival(), true, &other_room).is_some(),
            "a focused window on ANOTHER room has not shown me this"
        );

        let behind = OnScreen {
            app_focused: false,
            active_channel: "general".into(),
        };
        assert!(
            desktop_notice(&arrival(), true, &behind).is_some(),
            "the right room behind an editor is not a room anyone has read"
        );
    }

    #[test]
    fn the_preference_is_the_first_gate() {
        assert!(desktop_notice(&arrival(), false, &elsewhere()).is_none());
    }

    #[test]
    fn an_excerpt_is_one_flat_bounded_line() {
        assert_eq!(notice_excerpt("one\ntwo   three\n"), "one two three");
        assert_eq!(notice_excerpt(""), "");

        let long = "duck ".repeat(80);
        let excerpt = notice_excerpt(&long);
        assert!(excerpt.chars().count() <= EXCERPT_CHARS + 1, "…is the +1");
        assert!(excerpt.ends_with('…'));
        assert!(!excerpt.contains("  "));

        // A multi-byte body is cut on a CHARACTER boundary, never a byte one.
        let wide = "한글 ".repeat(80);
        assert!(notice_excerpt(&wide).ends_with('…'));
    }
}
