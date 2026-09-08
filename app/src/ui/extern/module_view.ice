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
