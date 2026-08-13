// Ice owns UI discriminants as closed types. A new phase or destination must
// update every exhaustive match instead of silently becoming another string.
enum SearchPhase
  idle
  searching
  done

enum ComposerFocus
  unfocused
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

enum ForgeCodePhase
  idle
  tree_loading
  file_loading
  ready
  tree_failed
  file_failed

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
