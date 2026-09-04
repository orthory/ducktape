state
  members_answered = false
  agents_answered = false
  gov_answered = false
  members_rows:[MemberRow] = []
  members_generation:i64 = 0
  gov_rows:[ProposalRow] = []
  gov_generation:i64 = 0
  gov_voting = ""
  // The governance module's own view, when its `governance.view.wasm` is
  // beside the binary; `none` keeps the app's GovernanceScreen. `gov_view_wake`
  // moves once per served event, and a moved prop is what rebuilds the view.
  gov_view:WasmSurface? = none
  gov_view_wake:i64 = 0
  agents_rows:[AgentRow] = []
  agents_generation:i64 = 0
