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
  // the run tracker: every run the journal lists, the run the reader has
  // open, and that run's journal as last read
  agents_runs:[RunRow] = []
  agents_open_run = ""
  agents_journal:RunJournal = empty_run_journal()
  // what the network announces and what a grant may name — the editor's
  // pick lists
  agents_capabilities:[str] = []
  agents_actions:[str] = []
  // one per committed agent write: the view re-seeds its drafts on the bump
  agents_committed:i64 = 0
