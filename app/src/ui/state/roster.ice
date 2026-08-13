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
  // Members and agents share one screen and therefore one filter/selection.
  members_filter:MembersFilter = MembersFilter.all
  members_selected = ""
