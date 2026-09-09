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
  // THE MESSAGING PANEL. `messaging` is one authenticated reading of ONE
  // conversation — it carries the endpoint and chain it was read from, and
  // the handler installs it only while the app is still on both. The pair
  // beside it is what the panel ASKED for, which is what a late answer is
  // checked against.
  messaging:MessagingView = messaging_none()
  messaging_participant = ""
  messaging_conversation = ""
  // THE OPERATION'S OWN NAME. The pair above is the operation's SUBJECT, and a
  // subject is not an identity: leaving this conversation and coming back
  // restores every id, so the first read's answer would match on the way back
  // in and overwrite what the second one found. These count dispatches, and an
  // answer installs only under the number it was sent with — alongside
  // `connect_generation` and `account_number`, which move when the link or the
  // seat does.
  messaging_load_op:i64 = 0
  messaging_send_op:i64 = 0
  messaging_loading = false
  messaging_sending = false
  // the last send's refusal — the composer keeps its draft under it
  messaging_send_error = ""
  // one per admitted send: the panel clears the draft it wrote on the bump
  messaging_sent:i64 = 0
