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
  // the Shell tab: the app's agent picks and the run it is watching, drawn
  // by the `shell` view; the composer, the terminal and the answer Markdown
  // are host surfaces, and a send arrives as the `send` intent
  component shell_view(dark:bool, connected:bool, surface:ShellSurface, setup_open:bool, identity_options:&[str], identity:&str, provider:&str, credential:&str, host_node_options:&[str], host_node:&str, credentials_loading:bool, terminal:&AgentTerminalSession, terminal_running:bool, terminal_busy:bool, terminal_title:&str, terminal_error:&str, entries:&[AgentChatEntry], activity:&[AgentActivity], chat_busy:bool, chat_status:&str, chat_detail:&str, live:&str, saga_id:&str, detached_saga:&str) -> ModuleViewEvent
  pure shell_intent(event:&ModuleViewEvent) -> ShellIntent
  pure shell_event_surface(event:&ModuleViewEvent) -> ShellSurface
  // empties the host-side composer (a new chat, a workspace reset)
  sync shell_composer_clear() -> bool
  // the document itself stays here: the fold stashes the buffer for the
  // `page_document` surface the view leaves a slot for
  component pages_view(dark:bool, connected:bool, loading:bool, mutation_phase:MutationPhase, network_chain_id:&str, pages:&[PageItem], page_create_open:bool, page_draft:&str, block_comment_draft:&str, seed_rev:i64, active_page:&str, active_page_title:&str, active_page_parent:&str, page_searching:bool, page_search_hits:&[PageSearchHit], page_search_query:&str, page_delete_armed:bool, autosave:AutosaveStatus, page_refusal:&str, doc_tabs:&[str], blocks:&[PageBlock], commented_block_hits:&[str], caret_comment_target:&str, active_thread_anchor:&str, orphaned_comment_drafts:&[str], page_editor:&editor, block_comments_open:bool, thread_total:i64, threads:&[PageCommentThread], comment_rows:&[PageCommentThreadRow], threads_loading:bool, threads_has_more:bool, active_thread:&str, comments:&[PageComment], comments_loading:bool, comments_has_more:bool) -> ModuleViewEvent
  pure pages_intent(event:&ModuleViewEvent) -> PagesIntent
