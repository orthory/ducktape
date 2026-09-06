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
// The one decision a submitted body faces at delivery. See `submit_verdict`.
enum SubmitVerdict
  admitted
  refused

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
  password
  // The mint's two steps, and the only door out of them is the confirm —
  // which is also what writes the key file. There is no skip: a key whose 24
  // words were never written down has no recovery at all, so this app never
  // creates one.
  phrase
  confirm
  wallets
  restore
  networks
  join
  provisioning
  live
  account

// A network pick's gate: a read-only session (no password, no key) opens the
// console outright; a signing session probes the account first.
enum PickGate
  read_only
  probe

// The account probe's answer for the picked network's chain.
enum AccountProbe
  found
  missing

// A QR ceremony stream's reading, as the handlers branch on it.
enum CeremonyPhase
  working
  show_qr
  done
  failed

// Which welcome door a ceremony came through — the name draft is the tell.
enum WelcomeDoor
  create
  login

// SETTINGS' GROUPS, as the one thing the screen branches on. Settings was a
// single reflowing grid of eight cards, so one topic (identity, its keys, the
// seat that signs with them) landed in whichever column the width happened to
// give it, and the destructive act sat at the bottom of the same list as the
// theme switch. Each variant is one group of settings, and the danger zone is
// a place you go rather than a card you scroll past.
enum SettingsPane
  general
  network
  account
  security
  danger

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

// The duck:// module table's verdict on a clicked or embedded link
// (`resolve_duck_link`): which existing navigation it maps onto. `unknown` is
// a malformed/unknown ref; `web` is an http(s) URL for the OS opener;
// `foreign_network` is a well-formed link whose `?net=` names a network this
// app is not on, so its ids address a store that is not this one.
enum DuckKind
  unknown
  web
  foreign_network
  page
  files
  forge_repo
  forge_item
  forge_blob
  channel
  channel_message

// The second step a forge deep link still owes once its repo is open.
enum ForgeFocus
  idle
  item
  blob

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

// The Shell tab's two SURFACES, not two modes: a durable task conversation and
// an interactive terminal. Both can be live at once — the node holds a saga and
// a pty session independently — so this only says which one is on screen.
enum ShellSurface
  tasks
  terminal

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
