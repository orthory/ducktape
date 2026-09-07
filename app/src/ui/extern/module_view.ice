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
