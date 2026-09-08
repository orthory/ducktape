// The module-owned views — `crate::module_view`: a screen that ships as an
// `ice:view` component (`crates/views`), loaded from a file beside the binary
// and drawn as one widget in its tab. Its props go in as the app has them;
// what the reader does in it comes back as a ModuleViewEvent the tab's
// handler acts on, so every write keeps going through the handler that
// signs it. The guest sees no key, no endpoint and no clock.
extern crate::module_view
  ModuleViewEvent(kind:str, detail:str)
  component governance_view(dark:bool, connected:bool, admin:bool, answered:bool, voting:&str, rows:&[ProposalRow]) -> ModuleViewEvent
  pure gov_intent(event:&ModuleViewEvent) -> GovIntent
  pure gov_event_proposal(event:&ModuleViewEvent) -> str
  pure gov_event_approves(event:&ModuleViewEvent) -> bool
  component members_view(dark:bool, connected:bool, admin:bool, answered:bool, rows:&[MemberRow]) -> ModuleViewEvent
  component agents_view(dark:bool, connected:bool, answered:bool, rows:&[AgentRow]) -> ModuleViewEvent
  pure roster_intent(event:&ModuleViewEvent) -> RosterIntent
  pure event_text(event:&ModuleViewEvent, field:&str) -> str
  pure event_flag(event:&ModuleViewEvent, field:&str) -> bool
  component node_view(dark:bool, connected:bool, admin:bool, tier:&str, status:&str, loading:bool, module_rows:&[ModuleRow], node_key:&str, node_data_dir:&str, node_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:&str, node_quorum_label:&str, node_version:&str, node_root_hash:&str, sync_line:&str, node_phase_since:i64, node_sync_retries:i64, node_sync_failures:i64, node_sync_last_error:&str, node_peers:&[PeerRow], wall_now:i64, timeline:&NodeLogTimelineState, source:&str) -> ModuleViewEvent
  pure node_intent(event:&ModuleViewEvent) -> NodeIntent
  pure node_event_tab(event:&ModuleViewEvent) -> NodeTab
  // what the reader did in the native log ring since the last drain,
  // applied to the timeline the app holds
  pure node_log_timeline_drain(state:NodeLogTimelineState) -> NodeLogTimelineState
  component explorer_view(dark:bool, connected:bool, loading:bool, blocks:&[ExplorerBlock], ops:&[ExplorerOp], head:i64, sync_line:&str, hits:&[ExplorerHit], kinds:&[KindCount], partial:&str, searching:bool, sent_query:&str) -> ModuleViewEvent
  pure explorer_intent(event:&ModuleViewEvent) -> ExplorerIntent
  component settings_view(dark:bool, connected:bool, loading:bool, status:&str, mutation_phase:MutationPhase, appearance:Appearance, desktop_notifications:bool, password:&str, account_name:&str, network_name:&str, connected_rpc:&str, account_ceremony_phase:&str, account_ceremony_qr:&str, account_ceremony_detail:&str, account_ceremony_left:&str, settings_key_state:&str, settings_key_path:&str, settings_open_tabs:i64, members_rows:&[MemberRow], members_answered:bool, account_number:&str, account_renaming:bool, account_exists:bool, account_keys:i64, account_key_rows:&[AccountKeyRow], account_busy:bool, account_ticket:&str, drafts_cleared:i64, drafts_scope:&str) -> ModuleViewEvent
  pure settings_intent(event:&ModuleViewEvent) -> SettingsIntent
  component files_view(dark:bool, connected:bool, path:&str, listed:bool, entries:&[FsEntry], loading:bool, preview_path:&str, preview_entry:&FsEntry, delete_target:&str, diff_from:&str, diff:&[FsDiffEntry], history:&[FsSnapshot], preview_truncated:bool, preview_binary:bool, preview_picture:bool, preview_width:i64, preview_height:i64, preview_text:&str, write_refusal:&str, writes:i64) -> ModuleViewEvent
  pure files_intent(event:&ModuleViewEvent) -> FilesIntent
  pure settings_event_tab(event:&ModuleViewEvent) -> ShellTab
  // Chat is a module-owned view: the screen's facts go in — the mutation
  // lock as `busy`, the enums by name — and every act comes back as an
  // intent; the two composers are host surfaces (`chat_composer`), whose
  // submit arrives as the `composer` intent. `chat_composer_unsent` hands a
  // refused or failed body back to the composer it came from.
  component chat_view(dark:bool, endpoint:&str, network_name:&str, network_chain_id:&str, status:&str, block_height:i64, search_phase:SearchPhase, search_query:&str, search_hits:&[ChatSearchHit], rooms:&[ChatSidebarRow], dm_rows:&[DmSidebarRow], channel_create_open:bool, connected:bool, loading:bool, mutation_phase:MutationPhase, active_channel:&str, active_dm_peer:&str, active_dm:&DmPeer, active_channel_name:&str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:&[ChatMember], post_refusal:&str, huddle_joined:bool, huddle_channel:&str, huddle_channel_name:&str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, messages:&[ChatMessage], has_older_history:bool, history_view:bool, at_live_tail:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, message_action:MessageAction, channel_settings_open:bool, active_thread_seq:i64, thread_target_seq:i64, thread_messages:&[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:MessageAction, thread_has_more:bool, thread_next_reply_seq:i64, thread_loading:bool, copy_anchor_seq:i64, copy_head_seq:i64, copy_surface:CopySurface, sent_serial:i64) -> ModuleViewEvent
  pure chat_intent(event:&ModuleViewEvent) -> ChatIntent
  pure chat_event_surface(event:&ModuleViewEvent) -> CopySurface
  pure chat_event_kind(event:&ModuleViewEvent) -> ComposerKind
  pure event_int(event:&ModuleViewEvent, field:&str) -> i64
  pure event_num(event:&ModuleViewEvent, field:&str) -> f64
  sync chat_composer_unsent(scope:&str, text:&str, committed:bool) -> bool
