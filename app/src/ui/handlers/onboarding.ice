// THE LAUNCH WINDOW'S HANDLERS. One `hub_step` machine inside the onboarding
// window: loading -> (password | wallets) -> [restore] -> networks ->
// [join -> provisioning -> live] -> [account] -> console window.
//
// The app is a strict CLIENT, and there is no create route: founding a network
// is `ducktape node init` on the node, where the coordinator and the rest of the
// network shape are chosen. `join_network` runs the join ceremony IN THIS
// PROCESS (the `workspace-config` library), materializing a workspace on disk.
// Nothing here starts a daemon, so `provision_progress` steps 4-5 are a real
// `/v1/status` poll and a stalled node reports `blocked` carrying the command
// that starts it, instead of a spinner on an 850ms fake clock.
//
// Sign-in is the SAME password every signing extern already threads: unlock
// opens the SELECTED wallet's key with the keystore library once, makes that
// wallet active, then stores the password in `password` for the session.
// Nothing new touches the wire.

// The launch window is up: register it, then load everything it renders —
// the keystore's wallets, the network list, and the persisted appearance.
on onboarding_opened(id)
  onboarding_win = some(id)
  parallel
    run replace lane=appearance_load load_appearance() -> appearance_loaded _
    run replace lane=hub_state hub_state() -> hub_booted _

// Boot answer: pick the entry step from the keystore and start probing the
// rows. `hub_booted` OWNS the step; the refresh route below never moves it.
// A keystore that could not be READ is not a keystore that is EMPTY: the
// error rides the same boot answer and lands on the create screen's own plate,
// where "Continue read-only" is the way past it.
on hub_booted(state)
  hub_hidden = state.hidden
  hub_networks = state.networks
  hub_selected = state.preselect
  hub_wallets = state.wallets
  hub_wallet_selected = preselect_wallet(state.wallets)
  hub_step = hub_entry_step(state.wallets)
  onboarding_error = state.wallets_error
  stream replace lane=network_probes probe_known_networks() -> network_probed _

// A refresh (after forget / after a join / on the way back to the wallet
// list) updates the rows where the user already is — the step stays put, and
// so does the row she picked while the refresh was in flight.
on hub_refreshed(state)
  hub_hidden = state.hidden
  hub_networks = state.networks
  hub_selected = refreshed_hub_selection(state.networks, hub_selected, state.preselect)
  hub_wallets = state.wallets
  hub_wallet_selected = refreshed_wallet_selection(state.wallets, hub_wallet_selected, preselect_wallet(state.wallets))
  onboarding_error = state.wallets_error
  stream replace lane=network_probes probe_known_networks() -> network_probed _

on network_probed(probe)
  hub_networks = apply_network_probe(hub_networks, probe)

on pick_wallet(name)
  hub_wallet_selected = name

// UNLOCK — verify the password opens the SELECTED wallet and make it the
// active one, then keep the password as the session's signing password.
// Optimistically stored: the failure arm clears it.
on unlock_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw) || empty(hub_wallet_selected)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every unlock_wallet(hub_wallet_selected, password) -> key_unlocked _ | login_failed _

on key_unlocked(_pubkey)
  mutation_phase = MutationPhase.idle
  onboarding_error = ""
  hub_step = HubStep.networks

// Reads never need the password — the quiet way past a forgotten one. The
// wallet selection goes with the password: a read-only session signs as
// NOBODY, and leaving a name behind made the network list claim otherwise.
on login_skip
  return if mutation_phase != MutationPhase.idle
  password = ""
  hub_wallet_selected = ""
  onboarding_error = ""
  hub_step = HubStep.networks

// PASSWORD — the device key is minted here, silently: no name to pick, no
// phrase to write down. Recovery is a passkey login from another device, so
// the words the keystore hands back are dropped before anything can show
// them. The confirm field is checked in the component (`password_problem`);
// this only fires clean.
on password_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every create_device_key(password) -> device_key_created _ | login_failed _

// The key exists; the list is refreshed so "signing as …" can name it.
on device_key_created(_pubkey)
  mutation_phase = MutationPhase.idle
  hub_step = HubStep.networks
  run replace lane=hub_state hub_state() -> hub_refreshed _

on go_restore
  return if mutation_phase != MutationPhase.idle
  restore_words = ""
  onboarding_error = ""
  hub_step = HubStep.restore

on go_login
  return if mutation_phase != MutationPhase.idle
  restore_words = ""
  onboarding_error = ""
  hub_step = hub_entry_step(hub_wallets)

// Same stash as `create_submit`, same reason: `key_restored` carries only a
// pubkey, and the list it lands on names the wallet by name.
on restore_submit(name, pw)
  return if mutation_phase != MutationPhase.idle || empty(restore_words) || empty(pw) || empty(name)
  onboarding_error = ""
  password = pw
  hub_wallet_selected = name
  mutation_phase = MutationPhase.onboarding
  run every restore_user_key(name, restore_words, password) -> key_restored _ | login_failed _

on key_restored(_pubkey)
  restore_words = ""
  mutation_phase = MutationPhase.idle
  hub_step = HubStep.networks

on login_failed(cause)
  mutation_phase = MutationPhase.idle
  password = ""
  onboarding_error = cause.message

// NETWORKS — select, open, forget.
on pick_network(id)
  hub_selected = id

on open_network_submit
  return if mutation_phase != MutationPhase.idle || empty(selected_network_endpoint(hub_networks, hub_selected))
  rpc = selected_network_endpoint(hub_networks, hub_selected)
  onboarding_error = ""
  task window open console -> console_opened _

// A remote endpoint this device holds no workspace for. On a successful
// connect `remember_network` saves it, which is how a `saved_remotes` row is
// born — the old Settings endpoint field was the only source before.
on connect_remote_submit(endpoint)
  return if mutation_phase != MutationPhase.idle || empty(trim(endpoint))
  rpc = canonical_endpoint(endpoint)
  onboarding_error = ""
  task window open console -> console_opened _

// The console window exists: point it at the picked endpoint, remember the
// pick, close the launch window BY ID, and run the same connect boot the
// single-window app ran on mount. By id, not the targetless `task window
// close` this used to be: that compiles to "the oldest window", and the
// daemon's third window (the popped huddle) can outlive the console, so
// oldest stopped meaning predecessor.
//
// EVERY per-network reading and draft resets to its default here — the pick
// may name a DIFFERENT network than the last console, and a channel list,
// half-typed draft, or open thread from the previous one leaking into this
// console is a lie about where the user is. The lane invalidations and the
// remaining scoped generation bumps are the other half: an in-flight load
// from the previous network must land dead.
// (`reconnect` is the same-endpoint sibling that deliberately KEEPS drafts.)
on console_opened(id)
  invalidate lane=chat_search
  invalidate lane=page_search
  invalidate lane=palette_search
  invalidate lane=chat_load
  invalidate lane=page_load
  invalidate lane=history
  invalidate lane=thread
  invalidate lane=live_thread
  invalidate lane=block_threads
  invalidate lane=block_comments
  invalidate lane=live_resync
  invalidate lane=forge_load
  invalidate lane=forge_live
  invalidate lane=forge_repo
  invalidate lane=forge_item
  invalidate lane=forge_discussion
  invalidate lane=files_preview
  invalidate lane=shell_credentials
  invalidate lane=shell_terminal
  invalidate lane=shell_chat
  invalidate lane=page_autosave
  console_win = some(id)
  wall_now = current_wall_seconds()
  connected = false
  loading = true
  status = "Connecting…"
  error = ""
  connected_rpc = rpc
  network_name = network_label(account_name, connected_rpc)
  hydration_generation = hydration_generation + 1
  connect_generation = connect_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.idle
  channels = []
  rooms = []
  dm_rows = []
  messages = []
  node_log_filter = ""
  node_log_timeline = node_log_timeline_reset()
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials = []
  shell_identities = []
  shell_identity_options = []
  shell_identity = ""
  shell_provider = "codex"
  shell_credential = ""
  shell_credentials_loading = false
  shell_setup_open = false
  shell_host_nodes = []
  shell_host_node_options = ["This node"]
  shell_host_node = "This node"
  shell_host_node_key = ""
  shell_terminal = idle_agent_terminal()
  shell_terminal_running = false
  shell_terminal_busy = false
  shell_terminal_title = ""
  shell_terminal_error = ""
  shell_chat_entries = []
  shell_chat_activity = []
  shell_chat_draft = editor("")
  shell_chat_busy = false
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  shell_chat_saga = ""
  shell_detached_saga = ""
  shell_steps_open = 0
  // The old network's history lane was invalidated above, so a socket that
  // never answers cannot keep "Load older" disabled in the new network.
  history_loading = false
  channel_reads = []
  unread_boundary = 0
  unread_marker_seq = 0
  active_channel = ""
  // Same two readings of the room as `reconnect`, and this one points at a
  // DIFFERENT node: a peer from the network she left names nothing here.
  active_dm_peer = ""
  active_dm = no_dm_peer()
  history_view = false
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  channel_members = []
  post_refusal = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  channel_draft = ""
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = MessageAction.toolbar
  message_edit_draft = ""
  // NO COMPOSER LINES HERE, AND THE NETWORK IS WHY THEY ARE NOT NEEDED: every
  // composer instance keys on `(endpoint, room)` (ducktape-ui#697), so network
  // A's `#general` and network B's `#general` are two instances. The park store
  // this replaced shared one key per channel id and had to be emptied by hand
  // right here, or a sentence typed on one node was handed back on ANOTHER.
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_seq = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  invalidate lane=live_thread
  thread_loading = false
  pending_channel = ""
  chat_search_draft = ""
  chat_search_hits = []
  chat_search_phase = SearchPhase.idle
  chat_search_query = ""
  pages = []
  doc_tabs = []
  blocks = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_draft = ""
  pending_page = ""
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  commented_block_hits = []
  caret_comment_target = ""
  page_editor = editor("")
  page_saved_text = ""
  buffer_page = ""
  page_inflight_text = ""
  page_refusal = ""
  block_autosave_status = AutosaveStatus.idle
  orphaned_comment_drafts = []
  page_delete_armed = false
  page_search_draft = ""
  page_search_hits = []
  page_searching = false
  page_search_query = ""
  // The palette's readings name the network being left too: hits from the
  // previous network are live, clickable rows that would render — and route —
  // on the new one if the palette is up. Scrubbed like its chat and pages
  // siblings; the open flag is not touched, matching this handler's doctrine
  // of resetting readings rather than closing surfaces.
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_search_phase = SearchPhase.idle
  // Forge's open repo, tracker item, code reading and drafts all name
  // the network being left. The new endpoint may even have the same repo/item
  // names, so identity strings cannot make any of these safe to retain.
  forge_generation = forge_generation + 1
  forge_list_phase = ForgePhase.idle
  forge_repos = []
  forge_repo = ""
  forge_repo_phase = ForgePhase.idle
  forge_repo_menu = false
  forge_branches = []
  forge_items = []
  forge_item_number = 0
  forge_item_phase = ForgePhase.idle
  forge_item_diff = ""
  forge_item_channel = ""
  forge_review_draft = ""
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""
  forge_comment_staged = []
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  // The huddle and its media session belong to the PREVIOUS network.
  // `call_session` is subscribed `when huddle_joined`, so this clear IS the
  // teardown — the stream drops and the old node's presence gate reaps the
  // seat when the socket dies.
  huddle_joined = false
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at = 0
  huddle_roster = []
  huddle_rows = []
  call_status = ""
  call_muted = false
  // The video source is reset by the session's own teardown
  // (`crate::video::reset`), so the readings of it must go with it — a
  // "sharing" button left lit for the previous network's call has nothing
  // behind it.
  call_camera = false
  call_sharing = false
  call_video_live = false
  huddle_stage = ""
  call_peers = []
  // An empty endpoint names no node: keep the adopted window and the
  // reset above, but launch nothing a "" could never answer.
  return if empty(connected_rpc)
  parallel
    task window close target=window_target(onboarding_win)
    flow
      from run remember_network(connected_rpc)
      discard
    run replace lane=connect connect(connected_rpc, 0, connect_generation) -> workspace_connected _ | connect_failed _

on forget_network_submit(id, kind)
  return if mutation_phase != MutationPhase.idle
  run every forget_network(id, kind) -> network_forgotten _

on network_forgotten(_written)
  run replace lane=hub_state hub_state() -> hub_refreshed _

on restore_hidden_submit
  return if mutation_phase != MutationPhase.idle
  run every restore_hidden_networks() -> network_forgotten _

// JOIN — unchanged plumbing, new seams: it starts from the network list and
// settles back into it through the provisioning/live screens.
on go_join
  return if mutation_phase != MutationPhase.idle
  join_invite = ""
  hub_step = HubStep.join
  onboarding_error = ""

on go_networks
  return if mutation_phase != MutationPhase.idle
  restore_words = ""
  join_invite = ""
  onboarding_error = ""
  hub_step = HubStep.networks
  run replace lane=hub_state hub_state() -> hub_refreshed _

// Back to the wallet list from the network picker. Refreshed on the way in:
// a wallet minted since boot is on disk but not in `hub_wallets`, and a list
// that cannot show it is a list you cannot switch to it with.
on go_wallets
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  hub_step = HubStep.wallets
  run replace lane=hub_state hub_state() -> hub_refreshed _

on join_network_submit
  return if mutation_phase != MutationPhase.idle || empty(join_invite)
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  run every join_network(join_invite) -> workspace_materialized _ | onboarding_failed _

// The workspace now exists on disk. Point the app at its endpoint and start
// watching for the node that will serve it.
on workspace_materialized(init)
  join_invite = ""
  mutation_phase = MutationPhase.idle
  onboarding_name = init.chain_id
  rpc = init.rpc
  invite_link = ""
  provision_steps = []
  provision_index = 0
  onboarding_error = ""
  hub_step = HubStep.provisioning
  stream replace lane=provision provision_progress(init.chain_id, init.rpc) -> provision_stepped _

// Every yielded step replaces the reading. The screen only leaves this step
// when the LAST step actually settled — a blocked or running step keeps it
// here, showing what is wrong.
on provision_stepped(step)
  // Copy fields first, then the move: `provision_steps = [step]` takes the
  // step whole, so any read of `index`/`state` after it is a use-after-move.
  // `settled` exists precisely so this decision needs no String.
  let settled = step.settled
  provision_index = step.index
  provision_steps = [step]
  return if provision_index != 5 || !settled
  hub_step = HubStep.live
  run every mint_invite(onboarding_name, 7) -> onboarding_invite_minted _ | onboarding_failed _

on onboarding_invite_minted(blob)
  invite_link = blob
  onboarding_error = ""

on copy_onboarding_invite
  return if empty(invite_link)
  toast = "Invite copied"
  toast_age = 0
  task clipboard write invite_link

// Leaving the live screen is the first real connect for the fresh network:
// `rpc` already points at the workspace it materialized, so this is the
// network-pick handoff with the pick pre-made.
on enter_console
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  task window open console -> console_opened _

// A refusal here is recoverable — the workspace is already on disk — so the
// screen keeps its controls and says what happened.
on onboarding_failed(cause)
  mutation_phase = MutationPhase.idle
  onboarding_error = cause.message

// THE WAY BACK — the titlebar chip, Settings' Switch network, and Danger
// Zone's forget all land here: reopen the launch window; once it is
// registered, the console closes behind it — and the popped huddle with it,
// since the huddle it showed belongs to the network being left. The network
// list is where it lands — never the wallet list again; the session's password
// (or the user's deliberate read-only skip) survives a network switch.
on switch_network
  return if mutation_phase != MutationPhase.idle
  invalidate lane=page_autosave
  invalidate lane=shell_credentials
  invalidate lane=shell_terminal
  invalidate lane=shell_chat
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials_loading = false
  shell_terminal = idle_agent_terminal()
  shell_terminal_running = false
  shell_terminal_busy = false
  shell_terminal_title = ""
  shell_chat_busy = false
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  task window open onboarding -> onboarding_reopened _

on onboarding_reopened(id)
  onboarding_win = some(id)
  hub_step = HubStep.networks
  parallel
    task window close target=window_target(console_win)
    task window close target=window_target(huddle_win)
    run replace lane=hub_state hub_state() -> hub_refreshed _
