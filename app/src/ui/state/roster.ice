state
  members_answered = false
  agents_answered = false
  members_rows:[MemberRow] = []
  members_generation:i64 = 0
  // the Approvals tab badge, as the governance view last reported it
  gov_open:i64 = 0
  // the membership ballot the Members view opened, while its write is out
  gov_voting = ""
  agents_rows:[AgentRow] = []
  agents_generation:i64 = 0
  // the run tracker: every run the journal lists, the run the reader has
  // open, and that run's journal as last read
  agents_runs:[RunRow] = []
  agents_open_run = ""
  // one per door a run was opened through: the view lands on its tracker at
  // every bump, onto the run already open as much as onto a new one
  agents_opened:i64 = 0
  agents_journal:RunJournal = empty_run_journal()
  // the journal read's own name. The run id is its SUBJECT, not its identity:
  // two networks can carry the same run id, so an answer about A would match
  // on the way back into A after a switch to B and back. Counts dispatches.
  agents_journal_op:i64 = 0
  // what the network announces and what a grant may name — the editor's
  // pick lists
  agents_capabilities:[str] = []
  agents_actions:[str] = []
  // one per committed agent write: the view re-seeds its drafts on the bump
  agents_committed:i64 = 0
