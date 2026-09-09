// THE LAUNCH WINDOW'S HANDLERS. One `hub_step` machine inside the onboarding
// window: loading -> networks -> (password | wallets) -> [restore] ->
// [account] -> console window, with [join -> provisioning -> live] off the
// network list.
//
// THE NETWORK COMES FIRST. A wallet is an identity ON a network, kept in that
// network's keystore on this device (`<workspace>/keys/` when this device
// hosts the node, `<ducktape home>/remotes/<chain id>/keys/` when it only
// reaches it), so there is nothing to unlock until a network is picked: the
// pick loads THAT keystore (`load_wallets`), and its rows decide whether the
// next screen unlocks one or mints the device key.
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
// opens the SELECTED wallet's key with the keystore library once, seats it as
// the session's signer, makes that wallet active, then stores the password in
// `password` for the session. Nothing new touches the wire.

// The launch window is up: register it, then load everything it renders —
// the network list and the persisted appearance.
on onboarding_opened(id)
  onboarding_win = some(id)
  parallel
    run replace lane=appearance_load load_appearance() -> appearance_loaded _
    run replace lane=notify_load load_desktop_notifications() -> desktop_notifications_loaded _
    run replace lane=hub_state hub_state() -> hub_booted _

// Boot answer: the network list, and probes for its rows. `hub_booted` OWNS
// the step; the refresh route below never moves it.
on hub_booted(state)
  hub_networks = state.networks
  hub_selected = state.preselect
  hub_step = HubStep.networks
  onboarding_error = ""
  stream replace lane=network_probes probe_known_networks() -> network_probed _

// A refresh (after a forget, after a join, on the way back to the list)
// updates the rows where the user already is — the step stays put, and so
// does the row she picked while the refresh was in flight.
on hub_refreshed(state)
  hub_networks = state.networks
  hub_selected = refreshed_hub_selection(state.networks, hub_selected, state.preselect)
  stream replace lane=network_probes probe_known_networks() -> network_probed _

on network_probed(probe)
  hub_networks = apply_network_probe(hub_networks, probe)

on pick_wallet(name)
  hub_wallet_selected = name

// UNLOCK — verify the password opens the SELECTED wallet of the picked
// network, seat it, and make it the active one, then keep the password as the
// session's signing password. Optimistically stored: the failure arm clears
// it.
on unlock_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw) || empty(hub_wallet_selected)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every unlock_wallet(rpc, hub_wallet_selected, password) -> key_unlocked _ | login_failed _

// The wallet is open: now the account probe, for THIS key on THIS chain. The
// console opens only for a key that has an account there; one with none lands
// on the welcome step. The probe block is inlined wherever a key becomes the
// session's — a handler cannot call a handler.
on key_unlocked(pubkey)
  onboarding_error = ""
  signer_key = pubkey
  live_agents = []
  parallel
    run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
    run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | chain_probe_failed _

// Reads never need the password — the quiet way past a forgotten one. A
// read-only session signs as NOBODY, so the wallet selection goes with the
// password, and the console opens on the picked network without an account
// probe: there is no key to look an account up for.
on login_skip
  return if mutation_phase != MutationPhase.idle
  password = ""
  hub_wallet_selected = ""
  onboarding_error = ""
  task window open console -> console_opened _

// PASSWORD — the device key is BEGUN here, in the picked network's keystore:
// a name and 24 words, and nothing on disk yet. The password field's confirm
// is checked in the component (`password_problem`); this only fires clean.
on password_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every create_device_key(rpc, password) -> device_key_created _ | login_failed _

// The words exist and the key does not. Straight into the ceremony.
on device_key_created(_name)
  mutation_phase = MutationPhase.idle
  hub_step = HubStep.phrase

// THE CEREMONY. Written down -> confirm -> the network list. There is no
// skip: the only way out of these two steps is three correct words, because
// the confirm is what seals the key — quit here and this device has no
// identity to have lost. Going back to the phrase is the way past a typo,
// and once the confirm passes this app never shows the words again.
on phrase_written_down
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  hub_step = HubStep.confirm

on show_phrase_again
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  hub_step = HubStep.phrase

on confirm_phrase_submit(answer)
  return if mutation_phase != MutationPhase.idle || empty(trim(answer))
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  run every confirm_recovery_phrase(rpc, answer, password) -> phrase_confirmed _ | phrase_confirm_failed _

// The key is sealed, seated, and the words are gone from this process. A
// fresh key has no account on the picked chain yet, which the probe is about
// to say: the welcome step is where it lands.
on phrase_confirmed(pubkey)
  onboarding_error = ""
  signer_key = pubkey
  live_agents = []
  parallel
    run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
    run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | chain_probe_failed _

// A miss keeps the phrase AND the step: the retry is the point.
on phrase_confirm_failed(cause)
  mutation_phase = MutationPhase.idle
  onboarding_error = cause.message

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
// pubkey, and the session names the wallet by name.
on restore_submit(name, pw)
  return if mutation_phase != MutationPhase.idle || empty(restore_words) || empty(pw) || empty(name)
  onboarding_error = ""
  password = pw
  hub_wallet_selected = name
  mutation_phase = MutationPhase.onboarding
  run every restore_user_key(rpc, name, restore_words, password) -> key_restored _ | login_failed _

// Restored and seated: the same probe an unlock runs.
on key_restored(pubkey)
  restore_words = ""
  onboarding_error = ""
  signer_key = pubkey
  live_agents = []
  parallel
    run replace lane=account_probe load_account(rpc, account_generation) -> account_probed _ | account_probe_failed _
    run replace lane=chain_probe chain_id_of(rpc) -> chain_named _ | chain_probe_failed _

on login_failed(cause)
  mutation_phase = MutationPhase.idle
  password = ""
  onboarding_error = cause.message

// NETWORKS — select, open, forget.
on pick_network(id)
  hub_selected = id

// A NETWORK PICK LOADS ITS KEYSTORE. The picked network's wallets decide the
// next step (`wallets_loaded`): rows are the unlock surface, an empty keystore
// mints the device key. A remote has its keystore too (by chain id, under the
// ducktape home), so the same two screens open for it; only a remote whose
// node cannot say which network it serves stays here with the error. The
// account probe comes AFTER the wallet, not before: which account to look for
// depends on which key signs. A previous pick's password names a wallet on
// another network, so it goes. The wallet screens name the network they are
// about, so its name is settled here, on the pick.
on open_network_submit
  return if mutation_phase != MutationPhase.idle || empty(selected_network_endpoint(hub_networks, hub_selected))
  rpc = selected_network_endpoint(hub_networks, hub_selected)
  network_name = selected_network_name(hub_networks, hub_selected)
  onboarding_error = ""
  password = ""
  hub_wallet_selected = ""
  mutation_phase = MutationPhase.onboarding
  run replace lane=hub_wallets load_wallets(rpc) -> wallets_loaded _

// A remote endpoint this device holds no workspace for — unless it does: the
// same keystore load answers both. On a successful connect `remember_network`
// saves a remote, which is how a `saved_remotes` row is born.
on connect_remote_submit(endpoint)
  return if mutation_phase != MutationPhase.idle || empty(trim(endpoint))
  rpc = canonical_endpoint(endpoint)
  network_name = network_label("", rpc)
  onboarding_error = ""
  password = ""
  hub_wallet_selected = ""
  mutation_phase = MutationPhase.onboarding
  run replace lane=hub_wallets load_wallets(rpc) -> wallets_loaded _

// The picked network's keystore, and the door it opens. A keystore that could
// not be READ is not a keystore that is EMPTY: the error rides the same answer
// and lands on the create screen's own plate, where "Continue read-only" is
// the way past it. A keystore that could not be NAMED — a remote whose node
// never answered which network it serves — is neither: the pick stays on
// screen with the error, and nothing opens.
on wallets_loaded(list)
  let door = wallet_door(list)
  mutation_phase = MutationPhase.idle
  hub_wallets = list.wallets
  hub_wallet_selected = preselect_wallet(list.wallets)
  onboarding_error = list.error
  match door
    WalletDoor.wallets
      hub_step = HubStep.wallets
    WalletDoor.password
      hub_step = HubStep.password
    WalletDoor.unreached
      hub_step = hub_step

on chain_named(id)
  hub_chain_id = id

// A node that serves no chain yet cannot take a key consent; the welcome
// still shows, and the ceremonies refuse on the empty chain id.
on chain_probe_failed(_cause)
  hub_chain_id = ""

on account_probed(next)
  mutation_phase = MutationPhase.idle
  let probe = account_probe(next.exists)
  match probe
    AccountProbe.found
      task window open console -> console_opened _
    AccountProbe.missing
      network_name = network_label(hub_chain_id, rpc)
      ceremony_phase = ""
      ceremony_qr = ""
      ceremony_detail = ""
      ceremony_left = ""
      hub_step = HubStep.account

// A node that cannot answer the probe is a node the console cannot use
// either: say so where the user is, keep the pick.
on account_probe_failed(cause)
  mutation_phase = MutationPhase.idle
  onboarding_error = cause.message

// THE WELCOME'S DOORS. Skipping opens the console without an account (the
// banner there is the way back); cancel drops a ceremony mid-flight — the
// lane invalidation aborts the owned future and drops its browser session.
on welcome_skip
  return if mutation_phase != MutationPhase.idle
  onboarding_error = ""
  task window open console -> console_opened _

on welcome_cancel
  invalidate lane=ceremony
  invalidate lane=desktop_ceremony
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  ceremony_left = ""

// THE CEREMONIES. Each is a stream on ONE lane: the first reading is the QR
// to show, `working` lines fill the gaps between touches, and `done` /
// `failed` close it. The chain id is the pick's probe answer; a node that
// named none refuses here, before any phone is involved.
on welcome_create_submit(name)
  return if mutation_phase != MutationPhase.idle || empty(name) || empty(hub_chain_id)
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  stream replace lane=ceremony create_account_by_qr(rpc, password, hub_chain_id, name) -> ceremony_stepped _

on welcome_login_submit
  return if mutation_phase != MutationPhase.idle || empty(hub_chain_id)
  onboarding_error = ""
  mutation_phase = MutationPhase.onboarding
  stream replace lane=ceremony login_by_qr(rpc, password, hub_chain_id) -> ceremony_stepped _

// The desktop path, from under the QR: the browser ceremonies the Settings
// card runs. A non-empty name draft means the user was creating — the
// account exists by the time a QR shows, so registering the passkey is the
// right continuation; otherwise it is a login.
on welcome_desktop
  return if ceremony_phase != "show_qr"
  invalidate lane=ceremony
  ceremony_phase = "working"
  ceremony_qr = ""
  ceremony_detail = "Continue in the browser…"
  let door = welcome_door(welcome_name_draft)
  match door
    WelcomeDoor.create
      run replace lane=desktop_ceremony register_passkey(rpc, password, hub_chain_id, "") -> welcome_desktop_done _ | welcome_failed _
    WelcomeDoor.login
      run replace lane=desktop_ceremony login_with_passkey(rpc, password, hub_chain_id, "") -> welcome_desktop_done _ | welcome_failed _

// Same landing as a `done` step (inlined: a handler cannot call a handler).
on welcome_desktop_done(_ok)
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  ceremony_left = ""
  task window open console -> console_opened _

on ceremony_stepped(next)
  let phase = ceremony_phase(next)
  ceremony_phase = next.phase
  ceremony_qr = next.qr
  ceremony_detail = next.detail
  ceremony_left = next.left
  match phase
    CeremonyPhase.done
      mutation_phase = MutationPhase.idle
      ceremony_phase = ""
      ceremony_qr = ""
      task window open console -> console_opened _
    CeremonyPhase.failed
      mutation_phase = MutationPhase.idle
      ceremony_phase = ""
      ceremony_qr = ""
      onboarding_error = next.detail
    CeremonyPhase.show_qr
      onboarding_error = ""
    CeremonyPhase.working
      onboarding_error = ""

on welcome_failed(cause)
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  ceremony_left = ""
  onboarding_error = cause.message

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
  fs_generation = fs_generation + 1
  fs_preview_path = ""
  fs_preview_entry = no_fs_entry()
  fs_preview_text = ""
  fs_preview_base = ""
  fs_write_pending = ""
  fs_loading = false
  invalidate lane=ceremony
  invalidate lane=desktop_ceremony
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  ceremony_left = ""
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = false
  account_banner_dismissed = false
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
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
  // Which chain this endpoint serves is the NODE's answer (`node_facts_loaded`);
  // the previous connection's is not it. Until it lands the title is the host.
  network_chain_id = ""
  network_name = network_label(network_chain_id, connected_rpc)
  hydration_generation = hydration_generation + 1
  connect_generation = connect_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.idle
  channels = []
  rooms = []
  dm_rows = []
  messages = []
  // A new network mounts a fresh timeline at its tail — see `state/chat.ice`.
  chat_at_tail = true
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
  shell_draft_cleared = shell_composer_clear()
  shell_chat_busy = false
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  shell_chat_saga = ""
  shell_detached_saga = ""
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
  forge_comment_staged = []
  forge_drafts_cleared = forge_drafts_cleared + 1
  forge_drafts_scope = "item"
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries = []
  forge_tree_born = false
  forge_tree_truncated = false
  forge_tree_phase = ForgeTreePhase.loading
  forge_file_path = ""
  forge_file_text = ""
  forge_file_note = ""
  forge_opened_dir = ""
  forge_opened_rev = ""
  forge_file_phase = ForgeFilePhase.idle
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
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

// Only a saved remote can be forgotten: a local network is a directory under
// the ducktape home, and this app deletes none.
on forget_network_submit(id)
  return if mutation_phase != MutationPhase.idle
  run every forget_network(id) -> network_forgotten _

on network_forgotten(_written)
  run replace lane=hub_state hub_state() -> hub_refreshed _

// JOIN — unchanged plumbing, new seams: it starts from the network list and
// settles back into it through the provisioning/live screens.
on go_join
  return if mutation_phase != MutationPhase.idle
  join_invite = ""
  hub_step = HubStep.join
  onboarding_error = ""

// Back to the network list from anywhere past it. The pick is undone with
// it: the wallets shown were the picked network's, and the password named
// one of them.
on go_networks
  let unrelated_mutation = mutation_phase != MutationPhase.idle && hub_step != HubStep.account
  return if unrelated_mutation
  invalidate lane=ceremony
  invalidate lane=desktop_ceremony
  invalidate lane=hub_wallets
  mutation_phase = MutationPhase.idle
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  ceremony_left = ""
  restore_words = ""
  join_invite = ""
  onboarding_error = ""
  password = ""
  hub_wallets = []
  hub_wallet_selected = ""
  hub_step = HubStep.networks
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
  // the TTL is the node's default (`workspace_config::DEFAULT_INVITE_TTL_DAYS`),
  // applied in the backend: Ice cannot import a Rust constant.
  run every mint_invite(onboarding_name) -> onboarding_invite_minted _ | onboarding_failed _

on onboarding_invite_minted(blob)
  invite_link = blob
  onboarding_error = ""

on copy_onboarding_invite
  return if empty(invite_link)
  toast = "Invite copied"
  toast_age = 0
  task clipboard write invite_link

// Leaving the live screen is the network-pick handoff with the pick pre-made:
// `rpc` already points at the workspace the join materialized, whose fresh
// keystore is empty — so this lands on the password step and mints the
// device key for the new network.
on enter_console
  return if mutation_phase != MutationPhase.idle
  network_name = network_label(onboarding_name, rpc)
  onboarding_error = ""
  password = ""
  hub_wallet_selected = ""
  mutation_phase = MutationPhase.onboarding
  run replace lane=hub_wallets load_wallets(rpc) -> wallets_loaded _

// A refusal here is recoverable — the workspace is already on disk — so the
// screen keeps its controls and says what happened.
on onboarding_failed(cause)
  mutation_phase = MutationPhase.idle
  onboarding_error = cause.message

// THE WAY BACK — the titlebar chip and Settings' Switch network land here:
// reopen the launch window; once it is registered, the console closes behind
// it — and the popped huddle with it, since the huddle it showed belongs to
// the network being left. The network list is where it lands, and the session
// ends with the network: its password named a wallet on the network being
// left, so the signer seat is dropped with it and the next pick unlocks anew.
on switch_network
  return if mutation_phase != MutationPhase.idle
  fs_generation = fs_generation + 1
  fs_preview_path = ""
  fs_preview_entry = no_fs_entry()
  fs_preview_text = ""
  fs_preview_base = ""
  fs_write_pending = ""
  fs_loading = false
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = account_busy && empty(account_ceremony_phase)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
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
  password = ""
  hub_wallets = []
  hub_wallet_selected = ""
  // the seat goes with the network, and every lane keyed on it re-keys.
  signer_key = ""
  live_agents = []
  parallel
    task window close target=window_target(console_win)
    task window close target=window_target(huddle_win)
    flow
      from run lock_signer()
      discard
    run replace lane=hub_state hub_state() -> hub_refreshed _

// THE BANNER'S WAY BACK — the launch window at the welcome step for THIS
// network. Same teardown as `switch_network` (a handler cannot call a
// handler, so the lines are repeated), different landing.
on dismiss_account_banner
  account_banner_dismissed = true

on open_account_welcome
  return if mutation_phase != MutationPhase.idle
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = account_busy && empty(account_ceremony_phase)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
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
  rpc = connected_rpc
  hub_chain_id = network_chain_id
  task window open onboarding -> welcome_reopened _

on welcome_reopened(id)
  onboarding_win = some(id)
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""
  onboarding_error = ""
  hub_step = HubStep.account
  parallel
    task window close target=window_target(console_win)
    task window close target=window_target(huddle_win)
