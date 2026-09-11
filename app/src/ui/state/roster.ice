state
  members_answered = false
  members_rows:[MemberRow] = []
  members_generation:i64 = 0
  // the Approvals tab badge, as the governance view last reported it
  gov_open:i64 = 0
  // the membership ballot the Members view opened, while its write is out
  gov_voting = ""
  // the run the app has opened for the reader, by dispatch id, and one per
  // door it was opened through — a chat hint, a bell, a duck://run link.
  // The Agents view reads its own register; this is the one thing the app
  // knows that the view cannot, so it rides the session push.
  agents_open_run = ""
  agents_opened:i64 = 0
  // an agent holds a run in flight, as the Agents view last counted them:
  // the rail's pulse dot
  agents_live = false
