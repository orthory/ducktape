// The module-owned views — `crate::module_view`: a screen that ships as an
// `ice:view` component (`crates/views`), loaded from a file beside the binary
// and drawn as one widget in its tab. Its props go in as the app has them;
// what the reader does in it comes back as a ModuleViewEvent the tab's
// handler acts on, so every write keeps going through the handler that
// signs it. The guest sees no key, no endpoint and no clock.
extern crate::module_view
  ModuleViewEvent(kind:str, detail:str)
  // Test seam: Ice reads extern structs but cannot construct one, and a
  // scenario that presses a view's control has no view to press it in.
  pure view_event(kind:str, detail:str) -> ModuleViewEvent
  // Approvals speaks the KERNEL CONTRACT: session facts go in, the view
  // reads and writes the node through the kernel (`rpc.*`, `op.submit`),
  // and the one event back is the kernel's `badge` (`count`).
  component governance_view(dark:bool, connected:bool, admin:bool) -> ModuleViewEvent
  // a block moved a module's plane: every `rpc.live` subscription its view
  // holds is told; the serial moves when one was, so the redraw follows
  sync view_live_hit(module:&str, serial:i64) -> i64
  // the node's height as the app last heard it: a height that moved is a
  // hit on the `block` plane every view reading the feed subscribes to
  sync view_block_hit(height:i64, serial:i64) -> i64
  // Members speaks the KERNEL CONTRACT too: session facts go in, the view
  // reads the roster off the node and signs its writes through `op.submit`;
  // the one event back is the clipboard intent
  component members_view(dark:bool, connected:bool, admin:bool) -> ModuleViewEvent
  // Agents speaks it as well: session facts go in — the run another tab
  // opened for the reader among them, which the kernel has no other door
  // for — and the view reads and writes the node itself.
  component agents_view(dark:bool, connected:bool, account:&str, open_run:&str, opened:i64) -> ModuleViewEvent
  pure agents_intent(event:&ModuleViewEvent) -> AgentsIntent
  pure event_text(event:&ModuleViewEvent, field:&str) -> str
  pure event_flag(event:&ModuleViewEvent, field:&str) -> bool
  // Node speaks the KERNEL CONTRACT too: only what no `/v1` route publishes
  // goes in — this seat's standing, the app's connection reading, the
  // daemon's workspace directory and the clock — and the view reads the
  // node's own status, peers, registry and log ring for itself. The one
  // intent back is the clipboard.
  component node_view(dark:bool, connected:bool, admin:bool, tier:&str, status:&str, data_dir:&str, wall_now:i64) -> ModuleViewEvent
  // The Explorer speaks the KERNEL CONTRACT: session facts go in (the two
  // node facts are the titlebar's own, so the screen cannot disagree with
  // it), the view reads the block window and runs its search through the
  // kernel, and the one intent back is `copy`.
  component explorer_view(dark:bool, connected:bool, head:i64, sync_line:&str) -> ModuleViewEvent
  // Settings speaks the KERNEL CONTRACT: what goes in is session facts —
  // the seat, the connection the titlebar names, and the wallet/account
  // machinery that is the kernel's alone. This node's standing and the
  // account's key associations the view reads for itself, and every act
  // comes back as one intent the kernel signs.
  component settings_view(dark:bool, connected:bool, loading:bool, status:&str, mutation_phase:MutationPhase, appearance:Appearance, desktop_notifications:bool, password:&str, seat_key:&str, account_name:&str, network_name:&str, connected_rpc:&str, account_ceremony_phase:&str, account_ceremony_qr:&str, account_ceremony_detail:&str, account_ceremony_left:&str, settings_key_state:&str, settings_key_path:&str, account_number:&str, account_exists:bool, account_busy:bool, account_ticket:&str) -> ModuleViewEvent
  pure settings_intent(event:&ModuleViewEvent) -> SettingsIntent
  // Files speaks the KERNEL CONTRACT: session facts go in, the view reads
  // and writes duckfs through the kernel (`files.get`, `op.submit`), and the
  // events back are the app's own doors — a link to open, and the directory
  // a dropped file lands in.
  component files_view(dark:bool, connected:bool, chain:&str, route:&str, route_serial:i64) -> ModuleViewEvent
  pure settings_event_tab(event:&ModuleViewEvent) -> ShellTab
  // The guest owns the document editor. The app supplies a bounded source
  // stream and reconciles accepted edits with persistence and navigation.
  component pages_view(dark:bool, connected:bool, loading:bool, mutation_phase:MutationPhase, network_chain_id:&str, pages:&[PageItem], page_create_open:bool, page_draft:&str, block_comment_draft:&str, seed_rev:i64, active_page:&str, active_page_title:&str, active_page_parent:&str, page_searching:bool, page_search_hits:&[PageSearchHit], page_search_query:&str, page_delete_armed:bool, autosave:AutosaveStatus, page_refusal:&str, blocks:&[PageBlock], commented_block_hits:&[str], orphaned_comment_drafts:&[str], page_text:&str, buffer_page:&str, block_comments_open:bool, scope_target:&str, scope_pinned:bool, thread_total:i64, comment_rows:&[PageCommentThreadRow], threads_loading:bool) -> ModuleViewEvent
  pure pages_intent(event:&ModuleViewEvent) -> PagesIntent
  // The Forge tab: the register the app holds and the item it has open,
  // the code browse's listing and file, and the discussion — whose note
  // composer is the chat composer as a host surface (`forge_composer`),
  // keyed by the item's channel; its send arrives as the `composer` intent.
  component forge_view(dark:bool, connected:bool, org:&str, about:&str, tier:&str, network_chain_id:&str, connected_rpc:&str, link:&str, link_tick:i64) -> ModuleViewEvent
  pure forge_intent(event:&ModuleViewEvent) -> ForgeIntent
  pure event_number(event:&ModuleViewEvent, field:&str) -> i64
  // Chat is a module-owned view: the screen's facts go in — the mutation
  // lock as `busy`, the enums by name — and every act comes back as an
  // intent; the two composers are host surfaces (`chat_composer`), whose
  // submit arrives as the `composer` intent. `chat_composer_unsent` hands a
  // refused or failed body back to the composer it came from.
  component chat_view(dark:bool, endpoint:&str, network_name:&str, network_chain_id:&str, status:&str, block_height:i64, search_phase:SearchPhase, search_query:&str, search_hits:&[ChatSearchHit], rooms:&[ChatSidebarRow], dm_rows:&[DmSidebarRow], channel_create_open:bool, connected:bool, loading:bool, mutation_phase:MutationPhase, active_channel:&str, active_dm_peer:&str, active_dm:&DmPeer, active_channel_name:&str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:&[ChatMember], post_refusal:&str, huddle_joined:bool, huddle_channel:&str, huddle_channel_name:&str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, messages:&[ChatMessage], has_older_history:bool, history_view:bool, at_live_tail:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, message_action:MessageAction, channel_settings_open:bool, active_thread_seq:i64, thread_target_seq:i64, thread_messages:&[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:MessageAction, thread_has_more:bool, thread_next_reply_seq:i64, thread_loading:bool, copy_anchor_seq:i64, copy_head_seq:i64, copy_surface:CopySurface, sent_serial:i64, live_agents:&[LiveAgentRow]) -> ModuleViewEvent
  pure chat_intent(event:&ModuleViewEvent) -> ChatIntent
  pure chat_event_surface(event:&ModuleViewEvent) -> CopySurface
  pure chat_event_kind(event:&ModuleViewEvent) -> ComposerKind
  pure event_int(event:&ModuleViewEvent, field:&str) -> i64
  pure event_num(event:&ModuleViewEvent, field:&str) -> f64
  sync chat_composer_unsent(scope:&str, text:&str, committed:bool) -> bool
  sync chat_composer_edit(scope:&str, messages:&[ChatMessage], seq:i64, rev:i64) -> bool
  // the room's roster for the composers over it: what `@` may complete to,
  // by the same rule the send resolves
  sync chat_composer_roster(scope:&str, members:&[ChatMember]) -> bool
