state
  members_answered = false
  agents_answered = false
  gov_answered = false
  members_rows:[MemberRow] = []
  members_generation:i64 = 0
  gov_rows:[ProposalRow] = []
  gov_generation:i64 = 0
  gov_voting = ""
  agents_rows:[AgentRow] = []
  agents_generation:i64 = 0
  // what the network announces and what a grant may name — the editor's
  // pick lists
  agents_capabilities:[str] = []
  agents_actions:[str] = []
  // one per committed agent write: the view re-seeds its drafts on the bump
  agents_committed:i64 = 0
