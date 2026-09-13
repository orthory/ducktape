use ducktape_view_guest::{kit as native, wire};
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SearchPhase {
    Idle,
    Searching,
    Done,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MessageAction {
    Toolbar,
    More,
    Reactions,
    Editing,
    Delete,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CopySurface {
    Nowhere,
    Timeline,
    Thread,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RowPlate {
    Plain,
    Selected,
    Ranged,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tone {
    Light,
    Dark,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RoomMove {
    Stayed,
    Moved,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SearchOutcome {
    Answered,
    Refused,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LandingThread {
    Absent,
    Seated,
}
#[allow(dead_code)]
pub(crate) struct ChatScreenState {
    message_action_focus: String,
    chat_pointer_y: f64,
    chat_height: f64,
    thread_pointer_y: f64,
    thread_height: f64,
}
impl ::std::default::Default for ChatScreenState {
    fn default() -> Self {
        Self {
            message_action_focus: "".to_owned(),
            chat_pointer_y: 0.0,
            chat_height: 720.0,
            thread_pointer_y: 0.0,
            thread_height: 720.0,
        }
    }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct ChatScreenStateSnapshot {
    pub(crate) message_action_focus: String,
    pub(crate) chat_pointer_y: f64,
    pub(crate) chat_height: f64,
    pub(crate) thread_pointer_y: f64,
    pub(crate) thread_height: f64,
}
#[cfg(test)]
#[allow(dead_code)]
impl ChatView {
    pub(crate) fn test_state_chat_screen(&self, scope: &str) -> Option<ChatScreenStateSnapshot> {
        let view = |state: &ChatScreenState| ChatScreenStateSnapshot {
            message_action_focus: state.message_action_focus.clone(),
            chat_pointer_y: state.chat_pointer_y.clone(),
            chat_height: state.chat_height.clone(),
            thread_pointer_y: state.thread_pointer_y.clone(),
            thread_height: state.thread_height.clone(),
        };
        self.chat_screen_states.get(scope).map(view)
    }
    pub(crate) fn test_message_chat_screen_chat_pointer_pressed(
        scope: String,
        p0: f64,
        p1: f64,
    ) -> Message {
        Message::ChatScreenChatPointerPressed(scope, p0, p1)
    }
    pub(crate) fn test_message_chat_screen_chat_resized(
        scope: String,
        p0: f64,
        p1: f64,
    ) -> Message {
        Message::ChatScreenChatResized(scope, p0, p1)
    }
    pub(crate) fn test_message_chat_screen_thread_pointer_pressed(
        scope: String,
        p0: f64,
        p1: f64,
    ) -> Message {
        Message::ChatScreenThreadPointerPressed(scope, p0, p1)
    }
    pub(crate) fn test_message_chat_screen_thread_resized(
        scope: String,
        p0: f64,
        p1: f64,
    ) -> Message {
        Message::ChatScreenThreadResized(scope, p0, p1)
    }
}
#[allow(dead_code)]
pub struct ChatView {
    pub(crate) active_palette: AppTheme,
    pub(crate) endpoint: String,
    pub(crate) network_name: String,
    pub(crate) network_chain_id: String,
    pub(crate) status: String,
    pub(crate) block_height: i64,
    pub(crate) connected: bool,
    pub(crate) session_loading: bool,
    pub(crate) session_busy: bool,
    pub(crate) rooms: Vec<crate::host::ChatSidebarRow>,
    pub(crate) dm_rows: Vec<crate::host::DmSidebarRow>,
    pub(crate) channel_create_open: bool,
    pub(crate) active_channel: String,
    pub(crate) active_dm_peer: String,
    pub(crate) active_dm: crate::host::DmPeer,
    pub(crate) huddle_joined: bool,
    pub(crate) huddle_channel: String,
    pub(crate) huddle_channel_name: String,
    pub(crate) huddle_joined_at: i64,
    pub(crate) huddle_now: i64,
    pub(crate) call_muted: bool,
    pub(crate) unread_boundary: i64,
    pub(crate) live_agents: Vec<crate::host::LiveRunHint>,
    pub(crate) shift_held: bool,
    pub(crate) copy_chord_serial: i64,
    pub(crate) sent_serial: i64,
    pub(crate) pending_sends: Vec<crate::host::PendingSend>,
    pub(crate) me: String,
    pub(crate) me_key: String,
    pub(crate) names_serial: i64,
    pub(crate) land_seq: i64,
    pub(crate) connection_serial: i64,
    pub(crate) room_serial: i64,
    pub(crate) history_pages: i64,
    pub(crate) room_key: crate::host::RoomKey,
    pub(crate) room_channel: String,
    pub(crate) room_messages: Vec<crate::host::ChatMessage>,
    pub(crate) messages: Vec<crate::host::ChatMessage>,
    pub(crate) channel_members: Vec<crate::host::ChatMember>,
    pub(crate) active_channel_name: String,
    pub(crate) active_channel_archived: bool,
    pub(crate) active_channel_members_only: bool,
    pub(crate) post_refusal: String,
    pub(crate) has_older_history: bool,
    pub(crate) loading: bool,
    pub(crate) busy: bool,
    pub(crate) timeline: crate::host::Timeline,
    pub(crate) thread_pages: i64,
    pub(crate) thread_key: crate::host::ThreadKey,
    pub(crate) active_thread_seq: i64,
    pub(crate) thread_target_seq: i64,
    pub(crate) thread_reveal_key: i64,
    pub(crate) stream_reveal_key: i64,
    pub(crate) thread_messages: Vec<crate::host::ChatMessage>,
    pub(crate) thread_has_more: bool,
    pub(crate) thread_next_reply_seq: i64,
    pub(crate) thread_loading: bool,
    pub(crate) search_key: crate::host::SearchKey,
    pub(crate) search_phase: SearchPhase,
    pub(crate) search_query: String,
    pub(crate) search_hits: Vec<crate::host::ChatSearchHit>,
    pub(crate) history_view: bool,
    pub(crate) at_live_tail: bool,
    pub(crate) history_loading: bool,
    pub(crate) unread_marker_seq: i64,
    pub(crate) selected_message_seq: i64,
    pub(crate) selected_message_rev: i64,
    pub(crate) message_action: MessageAction,
    pub(crate) channel_settings_open: bool,
    pub(crate) thread_selected_seq: i64,
    pub(crate) thread_selected_rev: i64,
    pub(crate) thread_message_action: MessageAction,
    pub(crate) copy_anchor_seq: i64,
    pub(crate) copy_head_seq: i64,
    pub(crate) copy_surface: CopySurface,
    pub(crate) chat_viewport_width: f64,
    pub(crate) sidebar_width: f64,
    pub(crate) details_width: f64,
    pub(crate) thread_width: f64,
    pub(crate) search_draft: String,
    pub(crate) message_edit_draft: String,
    pub(crate) channel_name_draft: String,
    pub(crate) member_key_draft: String,
    pub(crate) thread_edit_draft: String,
    pub(crate) host_error: String,
    pub(crate) sent: bool,
    pub(crate) timeline_revision: u64,
    pub(crate) thread_messages_revision: u64,
    pub(crate) chat_screen_states: ::std::collections::HashMap<String, ChatScreenState>,
    pub(crate) chat_screen_initial: ChatScreenState,
}
impl ::std::fmt::Debug for ChatView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("ChatView")
    }
}
#[derive(Clone)]
pub enum Message {
    SidebarResized(f64, f64),
    DetailsResized(f64, f64),
    ThreadResized(f64, f64),
    ChatViewportChanged(f64, f64),
    SessionArrived(crate::host::SessionItem),
    ToneChanged(bool),
    SessionSettled(bool),
    SnapStream(bool),
    RevealStream(i64),
    RevealThread(i64),
    RoomArrived(crate::host::RoomItem),
    ThreadArrived(crate::host::ThreadItem),
    SearchArrived(crate::host::SearchItem),
    ActDone(crate::host::ActItem),
    SearchChatSubmit,
    ClearChatSearch,
    OpenChatSearchHit(String, i64, i64),
    ToggleChannelCreate,
    ChooseChannel(String),
    ChooseDm(String),
    ToggleChannelSettings,
    ShowHuddle,
    LeaveHuddleHere,
    JoinHuddleSubmit,
    OpenMessageLink(String),
    CopyToClipboard(String, String),
    CopyMessageLink(String),
    CancelRun(String),
    OpenRun(String),
    ChatScrolled(f64, f64, f64, f64),
    LoadMoreHistory,
    OpenMessageActions(i64, String, i64),
    OpenMessageReactions(i64, String, i64),
    BeginMessageEdit(i64, String, i64),
    ArmMessageDelete(i64, String, i64),
    ClearMessageSelection,
    OpenThreadMessageActions(i64, String, i64),
    OpenThreadMessageReactions(i64, String, i64),
    BeginThreadMessageEdit(i64, String, i64),
    ArmThreadMessageDelete(i64, String, i64),
    ClearThreadMessageSelection,
    OpenThreadFor(i64),
    CloseThread,
    LoadMoreThread,
    AddReactionSubmit(String),
    AddReactionAt(i64, String),
    RemoveReactionAt(i64, String),
    DeleteMessageSubmit,
    DeleteThreadMessageSubmit,
    RenameChannelSubmit,
    ArchiveChannelSubmit,
    UnarchiveChannelSubmit,
    AddChannelMemberSubmit,
    RemoveChannelMemberSubmit(String),
    PressMessage(i64, CopySurface),
    ClearCopyRange,
    CopySelectedMessages,
    CopyChord(bool),
    ChatScreenChatPointerPressed(String, f64, f64),
    ChatScreenChatResized(String, f64, f64),
    ChatScreenThreadPointerPressed(String, f64, f64),
    ChatScreenThreadResized(String, f64, f64),
    ChatScreenMessageActionFocusChanged(String, String),
    SearchDraftChanged(String),
    ChannelNameDraftChanged(String),
    MemberKeyDraftChanged(String),
    Ignore,
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl ChatView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            endpoint: "".to_owned(),
            network_name: "".to_owned(),
            network_chain_id: "".to_owned(),
            status: "".to_owned(),
            block_height: 0,
            connected: false,
            session_loading: false,
            session_busy: false,
            rooms: Vec::new(),
            dm_rows: Vec::new(),
            channel_create_open: false,
            active_channel: "".to_owned(),
            active_dm_peer: "".to_owned(),
            active_dm: crate::host::no_dm_peer(),
            huddle_joined: false,
            huddle_channel: "".to_owned(),
            huddle_channel_name: "".to_owned(),
            huddle_joined_at: 0,
            huddle_now: 0,
            call_muted: false,
            unread_boundary: 0,
            live_agents: Vec::new(),
            shift_held: false,
            copy_chord_serial: 0,
            sent_serial: 0,
            pending_sends: Vec::new(),
            me: "".to_owned(),
            me_key: "".to_owned(),
            names_serial: 0,
            land_seq: 0,
            connection_serial: 0,
            room_serial: 0,
            history_pages: 0,
            room_key: crate::host::room_key(0, 0, ::std::convert::AsRef::as_ref(&("")), 0, 0),
            room_channel: "".to_owned(),
            room_messages: Vec::new(),
            messages: Vec::new(),
            channel_members: Vec::new(),
            active_channel_name: "".to_owned(),
            active_channel_archived: false,
            active_channel_members_only: false,
            post_refusal: "".to_owned(),
            has_older_history: false,
            loading: false,
            busy: false,
            timeline: crate::host::timeline_of(
                ::std::convert::AsRef::as_ref(&(Vec::new())),
                ::std::convert::AsRef::as_ref(&(Vec::new())),
            ),
            thread_pages: 0,
            thread_key: crate::host::thread_key(
                0,
                0,
                ::std::convert::AsRef::as_ref(&("")),
                0,
                0,
                0,
            ),
            active_thread_seq: 0,
            thread_target_seq: 0,
            thread_reveal_key: 0,
            stream_reveal_key: 0,
            thread_messages: Vec::new(),
            thread_has_more: false,
            thread_next_reply_seq: 0,
            thread_loading: false,
            search_key: crate::host::search_key(0, 0, ::std::convert::AsRef::as_ref(&(""))),
            search_phase: SearchPhase::Idle,
            search_query: "".to_owned(),
            search_hits: Vec::new(),
            history_view: false,
            at_live_tail: true,
            history_loading: false,
            unread_marker_seq: 0,
            selected_message_seq: 0,
            selected_message_rev: 0,
            message_action: MessageAction::Toolbar,
            channel_settings_open: false,
            thread_selected_seq: 0,
            thread_selected_rev: 0,
            thread_message_action: MessageAction::Toolbar,
            copy_anchor_seq: 0,
            copy_head_seq: 0,
            copy_surface: CopySurface::Nowhere,
            chat_viewport_width: 1280.0,
            sidebar_width: 236.0,
            details_width: 320.0,
            thread_width: 330.0,
            search_draft: "".to_owned(),
            message_edit_draft: "".to_owned(),
            channel_name_draft: "".to_owned(),
            member_key_draft: "".to_owned(),
            thread_edit_draft: "".to_owned(),
            host_error: "".to_owned(),
            sent: false,
            timeline_revision: ::ducktape_view_guest::rev::seed(),
            thread_messages_revision: ::ducktape_view_guest::rev::seed(),
            chat_screen_states: ::std::collections::HashMap::new(),
            chat_screen_initial: ::std::default::Default::default(),
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "cf1516e075a6b4b32487abe6afbfe3678c29fde066a7987abc12f3d3b256ef2b";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: wire::SnapshotValue::Record {
                name: String::from("ChatView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("endpoint"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.endpoint))), (String::from("network_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.network_name))), (String::from("network_chain_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.network_chain_id))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.status))), (String::from("block_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .block_height))), (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("session_loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .session_loading))), (String::from("session_busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .session_busy))), (String::from("rooms"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.rooms)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSidebarRow"), fields :
                    ::std::vec![(String::from("channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatChannel"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).channel).id))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).channel).name))), (String::from("archived"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& (item)
                    .channel).archived))), (String::from("members_only"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& (item)
                    .channel).members_only))), (String::from("huddle_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& (item)
                    .channel).huddle_count))), (String::from("head_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& (item)
                    .channel).head_seq)))] }), (String::from("unread"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .unread)))] }).collect())), (String::from("dm_rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.dm_rows)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("DmSidebarRow"), fields :
                    ::std::vec![(String::from("peer"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("DmPeer"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).peer).key))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).peer).name))), (String::from("initials"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).peer).initials))), (String::from("is_agent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& (item).peer)
                    .is_agent))), (String::from("channel_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).peer).channel_id)))] }), (String::from("unread"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .unread)))] }).collect())), (String::from("channel_create_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .channel_create_open))), (String::from("active_channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_channel))), (String::from("active_dm_peer"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_dm_peer))), (String::from("active_dm"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("DmPeer"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.active_dm).key))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.active_dm).name))), (String::from("initials"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.active_dm).initials))), (String::from("is_agent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& self
                    .active_dm).is_agent))), (String::from("channel_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.active_dm).channel_id)))] }), (String::from("huddle_joined"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .huddle_joined))), (String::from("huddle_channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.huddle_channel))), (String::from("huddle_channel_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.huddle_channel_name))), (String::from("huddle_joined_at"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .huddle_joined_at))), (String::from("huddle_now"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .huddle_now))), (String::from("call_muted"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .call_muted))), (String::from("unread_boundary"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .unread_boundary))), (String::from("live_agents"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.live_agents)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("LiveRunHint"), fields :
                    ::std::vec![(String::from("anchor_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .anchor_seq))), (String::from("thread_root"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_root))), (String::from("run_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).run_id))), (String::from("dispatch_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).dispatch_id))), (String::from("agent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).agent))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).status)))] }).collect())), (String::from("shift_held"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .shift_held))), (String::from("copy_chord_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .copy_chord_serial))), (String::from("sent_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .sent_serial))), (String::from("pending_sends"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .pending_sends).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PendingSend"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_seq)))] }).collect())), (String::from("me"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.me))), (String::from("me_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.me_key))), (String::from("names_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .names_serial))), (String::from("land_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .land_seq))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("room_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .room_serial))), (String::from("history_pages"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .history_pages))), (String::from("room_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("RoomKey"), fields :
                    ::std::vec![(String::from("serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .room_key).serial))), (String::from("names"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .room_key).names))), (String::from("channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.room_key).channel))), (String::from("land"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .room_key).land))), (String::from("pages"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .room_key).pages)))] }), (String::from("room_channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.room_channel))), (String::from("room_messages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .room_messages).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("view_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .view_key))), (String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("edit_body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).edit_body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("pending"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .pending))), (String::from("rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).rev))),
                    (String::from("edited"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .edited))), (String::from("deleted"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .deleted))), (String::from("reply_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .reply_count))), (String::from("thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_seq))), (String::from("show_author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .show_author))), (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("time"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).time))),
                    (String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).reactions)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatReaction"), fields :
                    ::std::vec![(String::from("emoji"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).emoji))), (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count))),
                    (String::from("reacted_by_me"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .reacted_by_me)))] }).collect())), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("messages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.messages)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("view_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .view_key))), (String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("edit_body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).edit_body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("pending"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .pending))), (String::from("rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).rev))),
                    (String::from("edited"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .edited))), (String::from("deleted"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .deleted))), (String::from("reply_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .reply_count))), (String::from("thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_seq))), (String::from("show_author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .show_author))), (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("time"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).time))),
                    (String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).reactions)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatReaction"), fields :
                    ::std::vec![(String::from("emoji"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).emoji))), (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count))),
                    (String::from("reacted_by_me"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .reacted_by_me)))] }).collect())), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("channel_members"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .channel_members).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMember"), fields :
                    ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).key))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label)))] }).collect())),
                    (String::from("active_channel_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_channel_name))),
                    (String::from("active_channel_archived"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .active_channel_archived))),
                    (String::from("active_channel_members_only"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .active_channel_members_only))), (String::from("post_refusal"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.post_refusal))), (String::from("has_older_history"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .has_older_history))), (String::from("loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .loading))), (String::from("busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.busy))),
                    (String::from("timeline"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("Timeline"), fields :
                    ::std::vec![(String::from("messages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& self.timeline)
                    .messages).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("view_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .view_key))), (String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("edit_body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).edit_body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("pending"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .pending))), (String::from("rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).rev))),
                    (String::from("edited"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .edited))), (String::from("deleted"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .deleted))), (String::from("reply_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .reply_count))), (String::from("thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_seq))), (String::from("show_author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .show_author))), (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("time"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).time))),
                    (String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).reactions)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatReaction"), fields :
                    ::std::vec![(String::from("emoji"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).emoji))), (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count))),
                    (String::from("reacted_by_me"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .reacted_by_me)))] }).collect())), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("live_agents"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& self.timeline)
                    .live_agents).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("LiveRunHint"), fields :
                    ::std::vec![(String::from("anchor_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .anchor_seq))), (String::from("thread_root"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_root))), (String::from("run_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).run_id))), (String::from("dispatch_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).dispatch_id))), (String::from("agent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).agent))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).status)))] }).collect()))] }), (String::from("thread_pages"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_pages))), (String::from("thread_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ThreadKey"), fields :
                    ::std::vec![(String::from("serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .thread_key).serial))), (String::from("names"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .thread_key).names))), (String::from("channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.thread_key).channel))), (String::from("root"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .thread_key).root))), (String::from("target"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .thread_key).target))), (String::from("pages"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .thread_key).pages)))] }), (String::from("active_thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .active_thread_seq))), (String::from("thread_target_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_target_seq))), (String::from("thread_reveal_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_reveal_key))), (String::from("stream_reveal_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .stream_reveal_key))), (String::from("thread_messages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .thread_messages).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("view_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .view_key))), (String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("edit_body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).edit_body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("pending"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .pending))), (String::from("rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).rev))),
                    (String::from("edited"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .edited))), (String::from("deleted"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .deleted))), (String::from("reply_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .reply_count))), (String::from("thread_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .thread_seq))), (String::from("show_author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .show_author))), (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("time"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).time))),
                    (String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).reactions)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatReaction"), fields :
                    ::std::vec![(String::from("emoji"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).emoji))), (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count))),
                    (String::from("reacted_by_me"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .reacted_by_me)))] }).collect())), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("thread_has_more"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .thread_has_more))), (String::from("thread_next_reply_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_next_reply_seq))), (String::from("thread_loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .thread_loading))), (String::from("search_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SearchKey"), fields :
                    ::std::vec![(String::from("serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .search_key).serial))), (String::from("names"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .search_key).names))), (String::from("query"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.search_key).query)))] }), (String::from("search_phase"),
                    match & self.search_phase { SearchPhase::Idle =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SearchPhase"), fields : vec![(String::from("idle"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SearchPhase::Searching =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SearchPhase"), fields :
                    vec![(String::from("searching"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SearchPhase::Done =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SearchPhase"), fields : vec![(String::from("done"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("search_query"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.search_query))), (String::from("search_hits"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.search_hits)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSearchHit"), fields :
                    ::std::vec![(String::from("channel_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).channel_id))), (String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("root_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .root_seq))), (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta)))] }).collect())), (String::from("history_view"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .history_view))), (String::from("at_live_tail"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .at_live_tail))), (String::from("history_loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .history_loading))), (String::from("unread_marker_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .unread_marker_seq))), (String::from("selected_message_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .selected_message_seq))), (String::from("selected_message_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .selected_message_rev))), (String::from("message_action"), match &
                    self.message_action { MessageAction::Toolbar =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("toolbar"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::More =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields : vec![(String::from("more"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Reactions =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Editing =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("editing"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Delete =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields : vec![(String::from("delete"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("channel_settings_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .channel_settings_open))), (String::from("thread_selected_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_selected_seq))), (String::from("thread_selected_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_selected_rev))), (String::from("thread_message_action"),
                    match & self.thread_message_action { MessageAction::Toolbar =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("toolbar"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::More =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields : vec![(String::from("more"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Reactions =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("reactions"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Editing =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields :
                    vec![(String::from("editing"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MessageAction::Delete =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MessageAction"), fields : vec![(String::from("delete"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("copy_anchor_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .copy_anchor_seq))), (String::from("copy_head_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .copy_head_seq))), (String::from("copy_surface"), match & self
                    .copy_surface { CopySurface::Nowhere =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("CopySurface"), fields : vec![(String::from("nowhere"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    CopySurface::Timeline =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("CopySurface"), fields : vec![(String::from("timeline"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    CopySurface::Thread =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("CopySurface"), fields : vec![(String::from("thread"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("chat_viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .chat_viewport_width))), (String::from("sidebar_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .sidebar_width))), (String::from("details_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .details_width))), (String::from("thread_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .thread_width))), (String::from("search_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.search_draft))), (String::from("message_edit_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.message_edit_draft))), (String::from("channel_name_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.channel_name_draft))), (String::from("member_key_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.member_key_draft))), (String::from("thread_edit_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.thread_edit_draft))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent))),
                    (String::from("chat_screen_states"), { let values = & self
                    .chat_screen_states; let mut scopes = values.keys().collect:: < Vec <
                    _ > > (); scopes.sort();
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatScreen instances"), fields : scopes.into_iter()
                    .map(| scope | { let component = & values[scope]; (scope.clone(),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatScreen"), fields :
                    vec![(String::from("message_action_focus"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    component.message_action_focus))), (String::from("chat_pointer_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .chat_pointer_y))), (String::from("chat_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .chat_height))), (String::from("thread_pointer_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .thread_pointer_y))), (String::from("thread_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .thread_height)))] }) }).collect() } }),
                    (String::from("chat_screen_initial"), { let component = & self
                    .chat_screen_initial;
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatScreen"), fields :
                    vec![(String::from("message_action_focus"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    component.message_action_focus))), (String::from("chat_pointer_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .chat_pointer_y))), (String::from("chat_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .chat_height))), (String::from("thread_pointer_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .thread_pointer_y))), (String::from("thread_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& component
                    .thread_height)))] } })
                ],
            },
        }
            .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err(String::from("snapshot schema mismatch"));
        }
        let value = snapshot.state;
        ((|| {
            let wire::SnapshotValue::Record { name, fields } = value else {
                return None;
            };
            if name != "ChatView" || fields.len() != 88 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "AppTheme" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "app" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::App)
                    }
                    "app_dark" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::AppDark)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "endpoint" {
                return None;
            }
            let endpoint: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "network_name" {
                return None;
            }
            let network_name: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "network_chain_id" {
                return None;
            }
            let network_chain_id: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "status" {
                return None;
            }
            let status: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "block_height" {
                return None;
            }
            let block_height: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "session_loading" {
                return None;
            }
            let session_loading: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "session_busy" {
                return None;
            }
            let session_busy: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "rooms" {
                return None;
            }
            let rooms: Vec<crate::host::ChatSidebarRow> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatSidebarRow" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "channel" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "unread" {
                                return None;
                            }
                            Some(crate::host::ChatSidebarRow {
                                channel: ((|| {
                                    let wire::SnapshotValue::Record { name, fields } = field_0
                                    else {
                                        return None;
                                    };
                                    if name != "ChatChannel" || fields.len() != 6 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "id" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "name" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "archived" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "members_only" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "huddle_count" {
                                        return None;
                                    }
                                    let (name, field_5) = fields.next()?;
                                    if name != "head_seq" {
                                        return None;
                                    }
                                    Some(crate::host::ChatChannel {
                                        id: (match field_0 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        name: (match field_1 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        archived: (match field_2 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        members_only: (match field_3 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        huddle_count: (match field_4 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        head_seq: (match field_5 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                    })
                                })())?,
                                unread: (match field_1 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "dm_rows" {
                return None;
            }
            let dm_rows: Vec<crate::host::DmSidebarRow> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "DmSidebarRow" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "peer" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "unread" {
                                return None;
                            }
                            Some(crate::host::DmSidebarRow {
                                peer: ((|| {
                                    let wire::SnapshotValue::Record { name, fields } = field_0
                                    else {
                                        return None;
                                    };
                                    if name != "DmPeer" || fields.len() != 5 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "key" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "name" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "initials" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "is_agent" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "channel_id" {
                                        return None;
                                    }
                                    Some(crate::host::DmPeer {
                                        key: (match field_0 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        name: (match field_1 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        initials: (match field_2 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        is_agent: (match field_3 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        channel_id: (match field_4 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                    })
                                })())?,
                                unread: (match field_1 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "channel_create_open" {
                return None;
            }
            let channel_create_open: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_channel" {
                return None;
            }
            let active_channel: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_dm_peer" {
                return None;
            }
            let active_dm_peer: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_dm" {
                return None;
            }
            let active_dm: crate::host::DmPeer = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "DmPeer" || fields.len() != 5 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "key" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "name" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "initials" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "is_agent" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "channel_id" {
                    return None;
                }
                Some(crate::host::DmPeer {
                    key: (match field_0 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    name: (match field_1 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    initials: (match field_2 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    is_agent: (match field_3 {
                        wire::SnapshotValue::Bool(item) => Some(item),
                        _ => None,
                    })?,
                    channel_id: (match field_4 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "huddle_joined" {
                return None;
            }
            let huddle_joined: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "huddle_channel" {
                return None;
            }
            let huddle_channel: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "huddle_channel_name" {
                return None;
            }
            let huddle_channel_name: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "huddle_joined_at" {
                return None;
            }
            let huddle_joined_at: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "huddle_now" {
                return None;
            }
            let huddle_now: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "call_muted" {
                return None;
            }
            let call_muted: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "unread_boundary" {
                return None;
            }
            let unread_boundary: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "live_agents" {
                return None;
            }
            let live_agents: Vec<crate::host::LiveRunHint> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "LiveRunHint" || fields.len() != 6 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "anchor_seq" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "thread_root" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "run_id" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "dispatch_id" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "agent" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "status" {
                                return None;
                            }
                            Some(crate::host::LiveRunHint {
                                anchor_seq: (match field_0 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                thread_root: (match field_1 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                run_id: (match field_2 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                dispatch_id: (match field_3 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                agent: (match field_4 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                status: (match field_5 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "shift_held" {
                return None;
            }
            let shift_held: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "copy_chord_serial" {
                return None;
            }
            let copy_chord_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent_serial" {
                return None;
            }
            let sent_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pending_sends" {
                return None;
            }
            let pending_sends: Vec<crate::host::PendingSend> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "PendingSend" || fields.len() != 3 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "thread_seq" {
                                return None;
                            }
                            Some(crate::host::PendingSend {
                                id: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                body: (match field_1 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                thread_seq: (match field_2 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "me" {
                return None;
            }
            let me: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "me_key" {
                return None;
            }
            let me_key: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "names_serial" {
                return None;
            }
            let names_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "land_seq" {
                return None;
            }
            let land_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connection_serial" {
                return None;
            }
            let connection_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "room_serial" {
                return None;
            }
            let room_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "history_pages" {
                return None;
            }
            let history_pages: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "room_key" {
                return None;
            }
            let room_key: crate::host::RoomKey = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "RoomKey" || fields.len() != 5 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "serial" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "names" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "channel" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "land" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "pages" {
                    return None;
                }
                Some(crate::host::RoomKey {
                    serial: (match field_0 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    names: (match field_1 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    channel: (match field_2 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    land: (match field_3 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    pages: (match field_4 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "room_channel" {
                return None;
            }
            let room_channel: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "room_messages" {
                return None;
            }
            let room_messages: Vec<crate::host::ChatMessage> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatMessage" || fields.len() != 21 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "view_key" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "edit_body" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "pending" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "rev" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "edited" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "deleted" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "reply_count" {
                                return None;
                            }
                            let (name, field_13) = fields.next()?;
                            if name != "thread_seq" {
                                return None;
                            }
                            let (name, field_14) = fields.next()?;
                            if name != "show_author" {
                                return None;
                            }
                            let (name, field_15) = fields.next()?;
                            if name != "initial" {
                                return None;
                            }
                            let (name, field_16) = fields.next()?;
                            if name != "avatar_kind" {
                                return None;
                            }
                            let (name, field_17) = fields.next()?;
                            if name != "height" {
                                return None;
                            }
                            let (name, field_18) = fields.next()?;
                            if name != "time" {
                                return None;
                            }
                            let (name, field_19) = fields.next()?;
                            if name != "reactions" {
                                return None;
                            }
                            let (name, field_20) = fields.next()?;
                            if name != "render_rev" {
                                return None;
                            }
                            Some(crate::host::ChatMessage {
                                id: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                view_key: (match field_1 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                seq: (match field_2 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                author: (match field_3 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                meta: (match field_4 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                body: (match field_5 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                edit_body: (match field_6 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                blocks: (match field_7 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let wire::SnapshotValue::Record { name, fields } = item
                                                                    else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                    })
                                                                })())
                                                                .collect::<Option<Vec<_>>>()
                                                        }
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                pending: (match field_8 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                rev: (match field_9 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                edited: (match field_10 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                deleted: (match field_11 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                reply_count: (match field_12 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                thread_seq: (match field_13 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                show_author: (match field_14 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                initial: (match field_15 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                avatar_kind: (match field_16 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                height: (match field_17 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                time: (match field_18 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                reactions: (match field_19 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatReaction" || fields.len() != 3 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "emoji" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "count" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "reacted_by_me" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatReaction {
                                                    emoji: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    count: (match field_1 {
                                                        wire::SnapshotValue::I64(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    reacted_by_me: (match field_2 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                render_rev: (match field_20 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "messages" {
                return None;
            }
            let messages: Vec<crate::host::ChatMessage> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatMessage" || fields.len() != 21 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "view_key" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "edit_body" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "pending" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "rev" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "edited" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "deleted" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "reply_count" {
                                return None;
                            }
                            let (name, field_13) = fields.next()?;
                            if name != "thread_seq" {
                                return None;
                            }
                            let (name, field_14) = fields.next()?;
                            if name != "show_author" {
                                return None;
                            }
                            let (name, field_15) = fields.next()?;
                            if name != "initial" {
                                return None;
                            }
                            let (name, field_16) = fields.next()?;
                            if name != "avatar_kind" {
                                return None;
                            }
                            let (name, field_17) = fields.next()?;
                            if name != "height" {
                                return None;
                            }
                            let (name, field_18) = fields.next()?;
                            if name != "time" {
                                return None;
                            }
                            let (name, field_19) = fields.next()?;
                            if name != "reactions" {
                                return None;
                            }
                            let (name, field_20) = fields.next()?;
                            if name != "render_rev" {
                                return None;
                            }
                            Some(crate::host::ChatMessage {
                                id: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                view_key: (match field_1 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                seq: (match field_2 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                author: (match field_3 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                meta: (match field_4 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                body: (match field_5 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                edit_body: (match field_6 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                blocks: (match field_7 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let wire::SnapshotValue::Record { name, fields } = item
                                                                    else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                    })
                                                                })())
                                                                .collect::<Option<Vec<_>>>()
                                                        }
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                pending: (match field_8 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                rev: (match field_9 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                edited: (match field_10 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                deleted: (match field_11 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                reply_count: (match field_12 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                thread_seq: (match field_13 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                show_author: (match field_14 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                initial: (match field_15 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                avatar_kind: (match field_16 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                height: (match field_17 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                time: (match field_18 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                reactions: (match field_19 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatReaction" || fields.len() != 3 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "emoji" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "count" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "reacted_by_me" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatReaction {
                                                    emoji: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    count: (match field_1 {
                                                        wire::SnapshotValue::I64(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    reacted_by_me: (match field_2 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                render_rev: (match field_20 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "channel_members" {
                return None;
            }
            let channel_members: Vec<crate::host::ChatMember> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatMember" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "key" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "label" {
                                return None;
                            }
                            Some(crate::host::ChatMember {
                                key: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                label: (match field_1 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_channel_name" {
                return None;
            }
            let active_channel_name: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_channel_archived" {
                return None;
            }
            let active_channel_archived: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_channel_members_only" {
                return None;
            }
            let active_channel_members_only: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "post_refusal" {
                return None;
            }
            let post_refusal: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "has_older_history" {
                return None;
            }
            let has_older_history: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "loading" {
                return None;
            }
            let loading: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "busy" {
                return None;
            }
            let busy: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "timeline" {
                return None;
            }
            let timeline: crate::host::Timeline = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "Timeline" || fields.len() != 2 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "messages" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "live_agents" {
                    return None;
                }
                Some(crate::host::Timeline {
                    messages: (match field_0 {
                        wire::SnapshotValue::List(items) => {
                            items
                                .into_iter()
                                .map(|item| (|| {
                                    let wire::SnapshotValue::Record { name, fields } = item
                                    else {
                                        return None;
                                    };
                                    if name != "ChatMessage" || fields.len() != 21 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "id" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "view_key" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "seq" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "author" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "meta" {
                                        return None;
                                    }
                                    let (name, field_5) = fields.next()?;
                                    if name != "body" {
                                        return None;
                                    }
                                    let (name, field_6) = fields.next()?;
                                    if name != "edit_body" {
                                        return None;
                                    }
                                    let (name, field_7) = fields.next()?;
                                    if name != "blocks" {
                                        return None;
                                    }
                                    let (name, field_8) = fields.next()?;
                                    if name != "pending" {
                                        return None;
                                    }
                                    let (name, field_9) = fields.next()?;
                                    if name != "rev" {
                                        return None;
                                    }
                                    let (name, field_10) = fields.next()?;
                                    if name != "edited" {
                                        return None;
                                    }
                                    let (name, field_11) = fields.next()?;
                                    if name != "deleted" {
                                        return None;
                                    }
                                    let (name, field_12) = fields.next()?;
                                    if name != "reply_count" {
                                        return None;
                                    }
                                    let (name, field_13) = fields.next()?;
                                    if name != "thread_seq" {
                                        return None;
                                    }
                                    let (name, field_14) = fields.next()?;
                                    if name != "show_author" {
                                        return None;
                                    }
                                    let (name, field_15) = fields.next()?;
                                    if name != "initial" {
                                        return None;
                                    }
                                    let (name, field_16) = fields.next()?;
                                    if name != "avatar_kind" {
                                        return None;
                                    }
                                    let (name, field_17) = fields.next()?;
                                    if name != "height" {
                                        return None;
                                    }
                                    let (name, field_18) = fields.next()?;
                                    if name != "time" {
                                        return None;
                                    }
                                    let (name, field_19) = fields.next()?;
                                    if name != "reactions" {
                                        return None;
                                    }
                                    let (name, field_20) = fields.next()?;
                                    if name != "render_rev" {
                                        return None;
                                    }
                                    Some(crate::host::ChatMessage {
                                        id: (match field_0 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        view_key: (match field_1 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        seq: (match field_2 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        author: (match field_3 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        meta: (match field_4 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        body: (match field_5 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        edit_body: (match field_6 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        blocks: (match field_7 {
                                            wire::SnapshotValue::List(items) => {
                                                items
                                                    .into_iter()
                                                    .map(|item| (|| {
                                                        let wire::SnapshotValue::Record { name, fields } = item
                                                        else {
                                                            return None;
                                                        };
                                                        if name != "ChatBlock" || fields.len() != 5 {
                                                            return None;
                                                        }
                                                        let mut fields = fields.into_iter();
                                                        let (name, field_0) = fields.next()?;
                                                        if name != "kind" {
                                                            return None;
                                                        }
                                                        let (name, field_1) = fields.next()?;
                                                        if name != "text" {
                                                            return None;
                                                        }
                                                        let (name, field_2) = fields.next()?;
                                                        if name != "lang" {
                                                            return None;
                                                        }
                                                        let (name, field_3) = fields.next()?;
                                                        if name != "rich" {
                                                            return None;
                                                        }
                                                        let (name, field_4) = fields.next()?;
                                                        if name != "spans" {
                                                            return None;
                                                        }
                                                        Some(crate::host::ChatBlock {
                                                            kind: (match field_0 {
                                                                wire::SnapshotValue::Str(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            text: (match field_1 {
                                                                wire::SnapshotValue::Str(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            lang: (match field_2 {
                                                                wire::SnapshotValue::Str(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            rich: (match field_3 {
                                                                wire::SnapshotValue::Bool(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            spans: (match field_4 {
                                                                wire::SnapshotValue::List(items) => {
                                                                    items
                                                                        .into_iter()
                                                                        .map(|item| (|| {
                                                                            let wire::SnapshotValue::Record { name, fields } = item
                                                                            else {
                                                                                return None;
                                                                            };
                                                                            if name != "ChatSpan" || fields.len() != 8 {
                                                                                return None;
                                                                            }
                                                                            let mut fields = fields.into_iter();
                                                                            let (name, field_0) = fields.next()?;
                                                                            if name != "mention" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_1) = fields.next()?;
                                                                            if name != "mention_link" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_2) = fields.next()?;
                                                                            if name != "link_text" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_3) = fields.next()?;
                                                                            if name != "link" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_4) = fields.next()?;
                                                                            if name != "bold_italic" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_5) = fields.next()?;
                                                                            if name != "bold" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_6) = fields.next()?;
                                                                            if name != "italic" {
                                                                                return None;
                                                                            }
                                                                            let (name, field_7) = fields.next()?;
                                                                            if name != "plain" {
                                                                                return None;
                                                                            }
                                                                            Some(crate::host::ChatSpan {
                                                                                mention: (match field_0 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                mention_link: (match field_1 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                link_text: (match field_2 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                link: (match field_3 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                bold_italic: (match field_4 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                bold: (match field_5 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                italic: (match field_6 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                                plain: (match field_7 {
                                                                                    wire::SnapshotValue::Str(item) => Some(item),
                                                                                    _ => None,
                                                                                })?,
                                                                            })
                                                                        })())
                                                                        .collect::<Option<Vec<_>>>()
                                                                }
                                                                _ => None,
                                                            })?,
                                                        })
                                                    })())
                                                    .collect::<Option<Vec<_>>>()
                                            }
                                            _ => None,
                                        })?,
                                        pending: (match field_8 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        rev: (match field_9 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        edited: (match field_10 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        deleted: (match field_11 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        reply_count: (match field_12 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        thread_seq: (match field_13 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        show_author: (match field_14 {
                                            wire::SnapshotValue::Bool(item) => Some(item),
                                            _ => None,
                                        })?,
                                        initial: (match field_15 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        avatar_kind: (match field_16 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        height: (match field_17 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        time: (match field_18 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        reactions: (match field_19 {
                                            wire::SnapshotValue::List(items) => {
                                                items
                                                    .into_iter()
                                                    .map(|item| (|| {
                                                        let wire::SnapshotValue::Record { name, fields } = item
                                                        else {
                                                            return None;
                                                        };
                                                        if name != "ChatReaction" || fields.len() != 3 {
                                                            return None;
                                                        }
                                                        let mut fields = fields.into_iter();
                                                        let (name, field_0) = fields.next()?;
                                                        if name != "emoji" {
                                                            return None;
                                                        }
                                                        let (name, field_1) = fields.next()?;
                                                        if name != "count" {
                                                            return None;
                                                        }
                                                        let (name, field_2) = fields.next()?;
                                                        if name != "reacted_by_me" {
                                                            return None;
                                                        }
                                                        Some(crate::host::ChatReaction {
                                                            emoji: (match field_0 {
                                                                wire::SnapshotValue::Str(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            count: (match field_1 {
                                                                wire::SnapshotValue::I64(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                            reacted_by_me: (match field_2 {
                                                                wire::SnapshotValue::Bool(item) => Some(item),
                                                                _ => None,
                                                            })?,
                                                        })
                                                    })())
                                                    .collect::<Option<Vec<_>>>()
                                            }
                                            _ => None,
                                        })?,
                                        render_rev: (match field_20 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                    })
                                })())
                                .collect::<Option<Vec<_>>>()
                        }
                        _ => None,
                    })?,
                    live_agents: (match field_1 {
                        wire::SnapshotValue::List(items) => {
                            items
                                .into_iter()
                                .map(|item| (|| {
                                    let wire::SnapshotValue::Record { name, fields } = item
                                    else {
                                        return None;
                                    };
                                    if name != "LiveRunHint" || fields.len() != 6 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "anchor_seq" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "thread_root" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "run_id" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "dispatch_id" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "agent" {
                                        return None;
                                    }
                                    let (name, field_5) = fields.next()?;
                                    if name != "status" {
                                        return None;
                                    }
                                    Some(crate::host::LiveRunHint {
                                        anchor_seq: (match field_0 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        thread_root: (match field_1 {
                                            wire::SnapshotValue::I64(item) => Some(item),
                                            _ => None,
                                        })?,
                                        run_id: (match field_2 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        dispatch_id: (match field_3 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        agent: (match field_4 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                        status: (match field_5 {
                                            wire::SnapshotValue::Str(item) => Some(item),
                                            _ => None,
                                        })?,
                                    })
                                })())
                                .collect::<Option<Vec<_>>>()
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "thread_pages" {
                return None;
            }
            let thread_pages: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_key" {
                return None;
            }
            let thread_key: crate::host::ThreadKey = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "ThreadKey" || fields.len() != 6 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "serial" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "names" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "channel" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "root" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "target" {
                    return None;
                }
                let (name, field_5) = fields.next()?;
                if name != "pages" {
                    return None;
                }
                Some(crate::host::ThreadKey {
                    serial: (match field_0 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    names: (match field_1 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    channel: (match field_2 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    root: (match field_3 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    target: (match field_4 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    pages: (match field_5 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "active_thread_seq" {
                return None;
            }
            let active_thread_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_target_seq" {
                return None;
            }
            let thread_target_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_reveal_key" {
                return None;
            }
            let thread_reveal_key: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "stream_reveal_key" {
                return None;
            }
            let stream_reveal_key: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_messages" {
                return None;
            }
            let thread_messages: Vec<crate::host::ChatMessage> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatMessage" || fields.len() != 21 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "view_key" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "edit_body" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "pending" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "rev" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "edited" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "deleted" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "reply_count" {
                                return None;
                            }
                            let (name, field_13) = fields.next()?;
                            if name != "thread_seq" {
                                return None;
                            }
                            let (name, field_14) = fields.next()?;
                            if name != "show_author" {
                                return None;
                            }
                            let (name, field_15) = fields.next()?;
                            if name != "initial" {
                                return None;
                            }
                            let (name, field_16) = fields.next()?;
                            if name != "avatar_kind" {
                                return None;
                            }
                            let (name, field_17) = fields.next()?;
                            if name != "height" {
                                return None;
                            }
                            let (name, field_18) = fields.next()?;
                            if name != "time" {
                                return None;
                            }
                            let (name, field_19) = fields.next()?;
                            if name != "reactions" {
                                return None;
                            }
                            let (name, field_20) = fields.next()?;
                            if name != "render_rev" {
                                return None;
                            }
                            Some(crate::host::ChatMessage {
                                id: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                view_key: (match field_1 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                seq: (match field_2 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                author: (match field_3 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                meta: (match field_4 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                body: (match field_5 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                edit_body: (match field_6 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                blocks: (match field_7 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let wire::SnapshotValue::Record { name, fields } = item
                                                                    else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
                                                                            wire::SnapshotValue::Str(item) => Some(item),
                                                                            _ => None,
                                                                        })?,
                                                                    })
                                                                })())
                                                                .collect::<Option<Vec<_>>>()
                                                        }
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                pending: (match field_8 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                rev: (match field_9 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                edited: (match field_10 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                deleted: (match field_11 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                reply_count: (match field_12 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                thread_seq: (match field_13 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                show_author: (match field_14 {
                                    wire::SnapshotValue::Bool(item) => Some(item),
                                    _ => None,
                                })?,
                                initial: (match field_15 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                avatar_kind: (match field_16 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                height: (match field_17 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                time: (match field_18 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                reactions: (match field_19 {
                                    wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let wire::SnapshotValue::Record { name, fields } = item
                                                else {
                                                    return None;
                                                };
                                                if name != "ChatReaction" || fields.len() != 3 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "emoji" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "count" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "reacted_by_me" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatReaction {
                                                    emoji: (match field_0 {
                                                        wire::SnapshotValue::Str(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    count: (match field_1 {
                                                        wire::SnapshotValue::I64(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                    reacted_by_me: (match field_2 {
                                                        wire::SnapshotValue::Bool(item) => Some(item),
                                                        _ => None,
                                                    })?,
                                                })
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                render_rev: (match field_20 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_has_more" {
                return None;
            }
            let thread_has_more: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_next_reply_seq" {
                return None;
            }
            let thread_next_reply_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_loading" {
                return None;
            }
            let thread_loading: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "search_key" {
                return None;
            }
            let search_key: crate::host::SearchKey = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "SearchKey" || fields.len() != 3 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "serial" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "names" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "query" {
                    return None;
                }
                Some(crate::host::SearchKey {
                    serial: (match field_0 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    names: (match field_1 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    query: (match field_2 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "search_phase" {
                return None;
            }
            let search_phase: SearchPhase = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "SearchPhase" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "idle" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(SearchPhase::Idle)
                    }
                    "searching" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(SearchPhase::Searching)
                    }
                    "done" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(SearchPhase::Done)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "search_query" {
                return None;
            }
            let search_query: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "search_hits" {
                return None;
            }
            let search_hits: Vec<crate::host::ChatSearchHit> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let wire::SnapshotValue::Record { name, fields } = item else {
                                return None;
                            };
                            if name != "ChatSearchHit" || fields.len() != 6 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "channel_id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "root_seq" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "text" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            Some(crate::host::ChatSearchHit {
                                channel_id: (match field_0 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                seq: (match field_1 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                root_seq: (match field_2 {
                                    wire::SnapshotValue::I64(item) => Some(item),
                                    _ => None,
                                })?,
                                author: (match field_3 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                text: (match field_4 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                                meta: (match field_5 {
                                    wire::SnapshotValue::Str(item) => Some(item),
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "history_view" {
                return None;
            }
            let history_view: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "at_live_tail" {
                return None;
            }
            let at_live_tail: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "history_loading" {
                return None;
            }
            let history_loading: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "unread_marker_seq" {
                return None;
            }
            let unread_marker_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "selected_message_seq" {
                return None;
            }
            let selected_message_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "selected_message_rev" {
                return None;
            }
            let selected_message_rev: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "message_action" {
                return None;
            }
            let message_action: MessageAction = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "MessageAction" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "toolbar" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Toolbar)
                    }
                    "more" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::More)
                    }
                    "reactions" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Reactions)
                    }
                    "editing" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Editing)
                    }
                    "delete" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Delete)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "channel_settings_open" {
                return None;
            }
            let channel_settings_open: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_selected_seq" {
                return None;
            }
            let thread_selected_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_selected_rev" {
                return None;
            }
            let thread_selected_rev: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_message_action" {
                return None;
            }
            let thread_message_action: MessageAction = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "MessageAction" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "toolbar" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Toolbar)
                    }
                    "more" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::More)
                    }
                    "reactions" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Reactions)
                    }
                    "editing" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Editing)
                    }
                    "delete" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MessageAction::Delete)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "copy_anchor_seq" {
                return None;
            }
            let copy_anchor_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "copy_head_seq" {
                return None;
            }
            let copy_head_seq: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "copy_surface" {
                return None;
            }
            let copy_surface: CopySurface = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "CopySurface" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "nowhere" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(CopySurface::Nowhere)
                    }
                    "timeline" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(CopySurface::Timeline)
                    }
                    "thread" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(CopySurface::Thread)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "chat_viewport_width" {
                return None;
            }
            let chat_viewport_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sidebar_width" {
                return None;
            }
            let sidebar_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "details_width" {
                return None;
            }
            let details_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_width" {
                return None;
            }
            let thread_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "search_draft" {
                return None;
            }
            let search_draft: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "message_edit_draft" {
                return None;
            }
            let message_edit_draft: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "channel_name_draft" {
                return None;
            }
            let channel_name_draft: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "member_key_draft" {
                return None;
            }
            let member_key_draft: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_edit_draft" {
                return None;
            }
            let thread_edit_draft: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "host_error" {
                return None;
            }
            let host_error: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "chat_screen_states" {
                return None;
            }
            let chat_screen_states: ::std::collections::HashMap<
                String,
                ChatScreenState,
            > = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "ChatScreen instances" {
                    return None;
                }
                let mut values = ::std::collections::HashMap::new();
                for (scope, value) in fields {
                    let component = ((|| {
                        let wire::SnapshotValue::Record { name, fields } = value else {
                            return None;
                        };
                        if name != "ChatScreen" || fields.len() != 5 {
                            return None;
                        }
                        let mut fields = fields.into_iter();
                        let (name, value) = fields.next()?;
                        if name != "message_action_focus" {
                            return None;
                        }
                        let message_action_focus: String = (match value {
                            wire::SnapshotValue::Str(item) => Some(item),
                            _ => None,
                        })?;
                        let (name, value) = fields.next()?;
                        if name != "chat_pointer_y" {
                            return None;
                        }
                        let chat_pointer_y: f64 = (match value {
                            wire::SnapshotValue::F64(item) if item.is_finite() => {
                                Some(item)
                            }
                            _ => None,
                        })?;
                        let (name, value) = fields.next()?;
                        if name != "chat_height" {
                            return None;
                        }
                        let chat_height: f64 = (match value {
                            wire::SnapshotValue::F64(item) if item.is_finite() => {
                                Some(item)
                            }
                            _ => None,
                        })?;
                        let (name, value) = fields.next()?;
                        if name != "thread_pointer_y" {
                            return None;
                        }
                        let thread_pointer_y: f64 = (match value {
                            wire::SnapshotValue::F64(item) if item.is_finite() => {
                                Some(item)
                            }
                            _ => None,
                        })?;
                        let (name, value) = fields.next()?;
                        if name != "thread_height" {
                            return None;
                        }
                        let thread_height: f64 = (match value {
                            wire::SnapshotValue::F64(item) if item.is_finite() => {
                                Some(item)
                            }
                            _ => None,
                        })?;
                        Some(ChatScreenState {
                            message_action_focus: message_action_focus,
                            chat_pointer_y: chat_pointer_y,
                            chat_height: chat_height,
                            thread_pointer_y: thread_pointer_y,
                            thread_height: thread_height,
                        })
                    })())?;
                    if values.insert(scope, component).is_some() {
                        return None;
                    }
                }
                Some(values)
            })())?;
            let (name, value) = fields.next()?;
            if name != "chat_screen_initial" {
                return None;
            }
            let chat_screen_initial: ChatScreenState = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "ChatScreen" || fields.len() != 5 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, value) = fields.next()?;
                if name != "message_action_focus" {
                    return None;
                }
                let message_action_focus: String = (match value {
                    wire::SnapshotValue::Str(item) => Some(item),
                    _ => None,
                })?;
                let (name, value) = fields.next()?;
                if name != "chat_pointer_y" {
                    return None;
                }
                let chat_pointer_y: f64 = (match value {
                    wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                    _ => None,
                })?;
                let (name, value) = fields.next()?;
                if name != "chat_height" {
                    return None;
                }
                let chat_height: f64 = (match value {
                    wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                    _ => None,
                })?;
                let (name, value) = fields.next()?;
                if name != "thread_pointer_y" {
                    return None;
                }
                let thread_pointer_y: f64 = (match value {
                    wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                    _ => None,
                })?;
                let (name, value) = fields.next()?;
                if name != "thread_height" {
                    return None;
                }
                let thread_height: f64 = (match value {
                    wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                    _ => None,
                })?;
                Some(ChatScreenState {
                    message_action_focus: message_action_focus,
                    chat_pointer_y: chat_pointer_y,
                    chat_height: chat_height,
                    thread_pointer_y: thread_pointer_y,
                    thread_height: thread_height,
                })
            })())?;
            Some(Self {
                active_palette: active_palette,
                endpoint: endpoint,
                network_name: network_name,
                network_chain_id: network_chain_id,
                status: status,
                block_height: block_height,
                connected: connected,
                session_loading: session_loading,
                session_busy: session_busy,
                rooms: rooms,
                dm_rows: dm_rows,
                channel_create_open: channel_create_open,
                active_channel: active_channel,
                active_dm_peer: active_dm_peer,
                active_dm: active_dm,
                huddle_joined: huddle_joined,
                huddle_channel: huddle_channel,
                huddle_channel_name: huddle_channel_name,
                huddle_joined_at: huddle_joined_at,
                huddle_now: huddle_now,
                call_muted: call_muted,
                unread_boundary: unread_boundary,
                live_agents: live_agents,
                shift_held: shift_held,
                copy_chord_serial: copy_chord_serial,
                sent_serial: sent_serial,
                pending_sends: pending_sends,
                me: me,
                me_key: me_key,
                names_serial: names_serial,
                land_seq: land_seq,
                connection_serial: connection_serial,
                room_serial: room_serial,
                history_pages: history_pages,
                room_key: room_key,
                room_channel: room_channel,
                room_messages: room_messages,
                messages: messages,
                channel_members: channel_members,
                active_channel_name: active_channel_name,
                active_channel_archived: active_channel_archived,
                active_channel_members_only: active_channel_members_only,
                post_refusal: post_refusal,
                has_older_history: has_older_history,
                loading: loading,
                busy: busy,
                timeline: timeline,
                thread_pages: thread_pages,
                thread_key: thread_key,
                active_thread_seq: active_thread_seq,
                thread_target_seq: thread_target_seq,
                thread_reveal_key: thread_reveal_key,
                stream_reveal_key: stream_reveal_key,
                thread_messages: thread_messages,
                thread_has_more: thread_has_more,
                thread_next_reply_seq: thread_next_reply_seq,
                thread_loading: thread_loading,
                search_key: search_key,
                search_phase: search_phase,
                search_query: search_query,
                search_hits: search_hits,
                history_view: history_view,
                at_live_tail: at_live_tail,
                history_loading: history_loading,
                unread_marker_seq: unread_marker_seq,
                selected_message_seq: selected_message_seq,
                selected_message_rev: selected_message_rev,
                message_action: message_action,
                channel_settings_open: channel_settings_open,
                thread_selected_seq: thread_selected_seq,
                thread_selected_rev: thread_selected_rev,
                thread_message_action: thread_message_action,
                copy_anchor_seq: copy_anchor_seq,
                copy_head_seq: copy_head_seq,
                copy_surface: copy_surface,
                chat_viewport_width: chat_viewport_width,
                sidebar_width: sidebar_width,
                details_width: details_width,
                thread_width: thread_width,
                search_draft: search_draft,
                message_edit_draft: message_edit_draft,
                channel_name_draft: channel_name_draft,
                member_key_draft: member_key_draft,
                thread_edit_draft: thread_edit_draft,
                host_error: host_error,
                sent: sent,
                timeline_revision: ::ducktape_view_guest::rev::seed(),
                thread_messages_revision: ::ducktape_view_guest::rev::seed(),
                chat_screen_states: chat_screen_states,
                chat_screen_initial: chat_screen_initial,
            })
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl ChatView {
    pub(crate) fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::room(
                    self.room_key.clone(),
                )
                .map(move |value| Message::RoomArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (self.active_thread_seq > 0)) {
                ::ducktape_view_guest::Subscription::batch([crate::host::thread(
                    self.thread_key.clone(),
                )
                .map(move |value| Message::ThreadArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.search_query).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([crate::host::search(
                    self.search_key.clone(),
                )
                .map(move |value| Message::SearchArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = ChatView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
mod app_update;
mod app_view;
mod chat;
mod components;
mod dm;
mod icon;
mod kit;
