// Ice owns UI discriminants as closed types. A new phase or destination must
// update every exhaustive match instead of silently becoming another string.
enum LiveKind
  retry
  tip
  ready
  chat
  bell
  pages
  forge
  plane
  resync

enum SearchPhase
  idle
  searching
  done

enum Appearance
  system
  light
  dark

// WHICH COMPOSER (ducktape-ui#697+#712). A handler-emitted event resolves to
// ONE app handler — the emitting handler cannot see which call site it is at —
// so the instance says which it is, and the app dispatches on the tag rather
// than on which of two near-identical handlers the route happened to name.
enum ComposerKind
  message
  reply

enum MessageAction
  toolbar
  more
  reactions
  editing
  delete

enum ForgePhase
  idle
  loading
  ready
  failed

enum HubStep
  loading
  create
  unlock
  reveal
  restore
  networks
  join
  provisioning
  live

enum NodeTab
  overview
  permissions
  activity
  modules

enum AutosaveStatus
  idle
  saving
  saved
  error

enum ShellTab
  chat
  shell
  pages
  forge
  agents
  files
  explorer
  node
  members
  governance
  settings

enum ShellMode
  raw
  chat

enum ForgeTab
  code
  pulls
  issues

enum ForgeReviewVerdict
  comment
  approve
  request_changes

enum MembersFilter
  all
  humans
  agents
  validators

enum MutationPhase
  idle
  recovering
  block_comment
  channel
  channel_archive
  channel_member
  channel_rename
  channel_unarchive
  comment_resolve
  forget_workspace
  huddle
  message_delete
  message_edit
  onboarding
  page
  page_delete
