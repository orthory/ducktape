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

// The door a picked network's keystore opens: its rows unlock, an empty
// keystore mints the device key, and a keystore that could not be NAMED (a
// remote whose node never said which network it serves) keeps the pick on
// screen with the error. A remote has a keystore like any network — under the
// ducktape home, by chain id — so there is no read-only door here; the only
// read-only entry is the button on the key screens.
enum WalletDoor
  wallets
  password
  unreached

// Open-or-raise for a window whose open state means "put it in front of me",
// nothing more — the huddle's call window. The status item's own "Open" row
// needs a third answer (`TrayOpen`, below): a connected network with nothing
// tracked must reopen the CONSOLE, not this pair, so it gets its own type
// instead of forcing a meaningless third arm onto this match.
enum WindowSummon
  open
  raise

// What the status item's "Open" row has to do. Closing a window no longer
// ends the process, so the daemon can be connected with nothing tracked — an
// ordinary state, not "never signed in". Routing that through the LAUNCH
// window resets `hub_step` to the network picker (`onboarding_opened` always
// re-runs `hub_state()`), sending a connected user back to network selection
// for merely closing the console (#1782); routing it through the console
// instead reconnects from `rpc` the way a fresh pick does.
enum TrayOpen
  launch
  console
  raise

// WHICH PLATE A MESSAGE ROW WEARS. `selected_row` is the one plate in the app
// that means "the row you are on"; `brand_wash` is deliberately lighter,
// because a copy range is a RUN and painting five rows in the you-are-here
// plate would say the reader is on all five.
enum RowPlate
  plain
  selected
  ranged

// WHERE A COPY RANGE WAS DRAWN. A thread reply and a timeline row take their
// seqs from the SAME channel sequence, so a reply can fall numerically inside
// a range the reader drew in the stream behind it. The surface is what keeps
// the two apart: a row lights up only for a range drawn where it lives, and
// the chord lifts the same rows the bar is counting.
enum CopySurface
  nowhere
  timeline
  thread

// The command chords this app answers, as one discriminant. macOS binds ⌘Q and
// ⌘W through an app menu this app does not have, so it reads them itself — and
// they differ only in the letter, which is exactly the shape one enum and one
// match are for.
enum CommandChord
  ignored
  quit
  close_window

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

// The code browse's two reads — the directory listing and the file — as the
// app tracks them for the Forge view.
enum ForgeTreePhase
  loading
  ready
  failed

enum ForgeFilePhase
  idle
  loading
  ready
  failed

// what the Forge view asks of the app: one variant per act the screen
// offers, each carrying only what the reader picked or typed (a repo, an
// item, a directory or file, a review body, a line comment) — the review
// and comment drafts themselves are the view's
enum ForgeIntent
  open_repo
  close_repo
  toggle_repo_menu
  tab
  open_item
  close_item
  merge
  review_pick
  review_submit
  comment_stage
  comment_drop
  tree
  blob
  open_link
  copy
  composer

// What a `governance` module view asks of the app: the two writes the
// Approvals screen makes, routed to the handlers that sign them.
enum GovIntent
  vote
  execute

// what the Members view asks of the app: a clipboard write, an agent's
// paused state, or a membership ballot
enum RosterIntent
  copy
  agent_status
  propose

// what the Agents view asks of the app: an agent's paused state, a record
// rewritten from the editor's draft, a new agent registered from one, the
// journal of a run the reader opened, or the messaging panel opening a
// conversation, paging it, and sending on it
enum AgentsIntent
  status
  save
  register
  open_run
  messaging_open
  messaging_page
  messaging_send

// what the Node view asks of the app: a clipboard write, the tab it is on,
// the log filter, or the native log ring reporting what the reader did
enum NodeIntent
  copy
  tab
  log_filter
  log_timeline

// what the Explorer view asks of the app: reload the ledger, a clipboard
// write, a workspace search, or dropping the standing answer
enum ExplorerIntent
  refresh
  copy
  search
  clear

// what the Settings view asks of the app: one variant per act the screen
// offers, each carrying only what the reader typed (a name, a key, a ticket,
// the key password) — the drafts themselves are the view's
// what the shell view asks of the app — `crate::module_view::shell_intent`
enum ShellIntent
  surface
  setup
  identity
  host_node
  refresh
  terminal_start
  terminal_stop
  send
  reset
  detach
  reopen
  discard
  open_link

// what the Files view asks of the app: one variant per act the browser
// offers, from a navigation to a write
enum FilesIntent
  open_dir
  open_file
  open_parent
  mkdir
  new_file
  arm_delete
  disarm_delete
  delete
  save
  show_diff
  close_diff
  open_link

enum SettingsIntent
  tab
  reconnect
  switch_network
  unlock
  lock
  rename
  create
  key_add
  join
  key_remove
  passkey
  passkey_desktop
  ceremony_cancel
  wallet
  login
  copy
  clear_tabs
  light
  dark
  notifications

// What the pages view asks of the app, one per act the screen offers.
// The drafts are the view's: a create carries its title, a search its
// query, a post its text, and the acts that abandon the rail's comment
// carry it for the recovered-drafts plate.
enum PagesIntent
  toggle_create
  create
  choose
  search
  clear_search
  arm_delete
  disarm_delete
  delete
  close_tab
  open_hit
  use_draft
  discard_draft
  edited
  toggle_comments
  close_comments
  open_thread
  resolve
  more_threads
  close_thread
  more_comments
  post
  copy
// what the Chat view asks of the app: one variant per act the screen offers,
// each carrying only what the reader chose or typed — the drafts are the view's
enum ChatIntent
  search
  clear_search
  open_hit
  toggle_create
  choose_channel
  choose_dm
  toggle_settings
  show_huddle
  leave_huddle
  join_huddle
  load_history
  scrolled
  open_link
  copy
  copy_link
  add_reaction
  remove_reaction
  open_thread
  message_actions
  message_reactions
  begin_edit
  arm_delete
  clear_selection
  press
  clear_range
  copy_range
  reaction_submit
  edit
  delete
  rename
  archive
  unarchive
  add_member
  remove_member
  close_thread
  thread_actions
  thread_reactions
  thread_begin_edit
  thread_arm_delete
  thread_clear_selection
  thread_edit
  thread_delete
  load_thread
  composer

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
  huddle
  message_delete
  message_edit
  onboarding
  page
  page_delete

// Whether a closed window owns the authentication being abandoned.
enum CeremonyRetirement
  keep
  welcome
  account
