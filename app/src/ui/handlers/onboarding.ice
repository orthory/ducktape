// THE LAUNCH WINDOW'S HANDLERS. One `hub_step` machine inside the onboarding
// window: loading -> (create | unlock) -> [reveal | restore] -> networks ->
// [join -> provisioning -> live] -> console window.
//
// The app is a strict CLIENT, and there is no create route: founding a network
// is `ducktape node init` on the node, where the coordinator and the rest of the
// network shape are chosen. `join_network` shells out to `ducktape node join`,
// which materializes a workspace on disk and then EXITS. Nothing here starts a
// daemon, so `provision_progress` steps 4-5 are a real `/v1/status` poll and a
// stalled node reports `blocked` carrying the command that starts it, instead
// of a spinner on an 850ms fake clock.
//
// Sign-in is the SAME password every signing extern already threads: unlock
// verifies it against user.key once (`ducktape user key unlock`), then stores
// it in `password` for the session. Nothing new touches the wire.

state
  // The steps the provisioning stream has actually reported. Ice has no list
  // append and no `sync` helper exists to merge one, so this holds the CURRENT
  // step rather than the artifact's five-row history — see the report.
  provision_steps:[ProvisionStep] = []
  provision_index:i64 = 0
  // The chain id `node join` minted, which is also the `-n`
  // workspace selector every later CLI call needs.
  onboarding_chain = ""

// The launch window is up: register it, then load everything it renders —
// the key state, the network list, and the persisted appearance.
on onboarding_opened(id)
  onboarding_win = some(id)
  parallel
    run load_appearance() -> appearance_loaded _
    run hub_state() -> hub_booted _

// Boot answer: pick the entry step from the key state and start probing the
// rows. `hub_booted` OWNS the step; the refresh route below never moves it.
on hub_booted(state)
  hub_key_state = state.key_state
  hub_networks = state.networks
  hub_selected = state.preselect
  hub_step = hub_entry_step(state.key_state)
  hub_probe_generation = hub_probe_generation + 1
  stream probe_known_networks(hub_probe_generation) -> network_probed _

// A refresh (after forget / after a join) updates the rows where the user
// already is — the step stays put.
on hub_refreshed(state)
  hub_key_state = state.key_state
  hub_networks = state.networks
  hub_selected = refreshed_hub_selection(state.networks, hub_selected, state.preselect)
  hub_probe_generation = hub_probe_generation + 1
  stream probe_known_networks(hub_probe_generation) -> network_probed _

on network_probed(probe)
  return if probe.generation != hub_probe_generation
  hub_networks = apply_network_probe(hub_networks, probe)

// UNLOCK — verify the password opens user.key, then keep it as the session's
// signing password. Optimistically stored: the failure arm clears it.
on unlock_submit(pw)
  return if mutation_phase != "idle" || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = "onboarding"
  run unlock_user_key(password) -> key_unlocked _ | login_failed _

on key_unlocked(_pubkey)
  mutation_phase = "idle"
  onboarding_error = ""
  hub_step = "networks"

// Reads never need the password — the quiet way past a forgotten one.
on login_skip
  return if mutation_phase != "idle"
  password = ""
  onboarding_error = ""
  hub_step = "networks"

// CREATE — mint user.key under the new password. The confirm field is
// checked in the component (`password_problem`); this only fires clean.
on create_submit(pw)
  return if mutation_phase != "idle" || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = "onboarding"
  run create_user_key(password) -> key_created _ | login_failed _

on key_created(created)
  mutation_phase = "idle"
  hub_key_state = "encrypted"
  reveal_words = created.words
  hub_step = "reveal"

// The one moment the 24 words exist on screen ends here.
on reveal_confirm
  reveal_words = ""
  hub_step = "networks"

on go_restore
  return if mutation_phase != "idle"
  onboarding_error = ""
  hub_step = "restore"

on go_login
  return if mutation_phase != "idle"
  onboarding_error = ""
  hub_step = hub_entry_step(hub_key_state)

on restore_submit(words, pw)
  return if mutation_phase != "idle" || empty(trim(words)) || empty(pw)
  onboarding_error = ""
  password = pw
  mutation_phase = "onboarding"
  run restore_user_key(words, password) -> key_restored _ | login_failed _

on key_restored(_pubkey)
  mutation_phase = "idle"
  hub_key_state = "encrypted"
  hub_step = "networks"

on login_failed(cause)
  mutation_phase = "idle"
  password = ""
  onboarding_error = cause.message

// NETWORKS — select, open, forget.
on pick_network(id)
  hub_selected = id

on open_network_submit
  return if mutation_phase != "idle" || empty(selected_network_endpoint(hub_networks, hub_selected))
  rpc = selected_network_endpoint(hub_networks, hub_selected)
  onboarding_error = ""
  task window open console -> console_opened _

// The console window exists: point it at the picked endpoint, remember the
// pick, close the launch window (the OLDEST window — it opened first), and
// run the same connect boot the single-window app ran on mount.
on console_opened(id)
  console_win = some(id)
  connected = false
  loading = true
  status = "Connecting…"
  error = ""
  connected_rpc = rpc
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  parallel
    task window close
    run remember_network(connected_rpc) -> network_remembered _
    run connect(connected_rpc) -> workspace_connected _ | failed _

on network_remembered(_written)
  error = error

on forget_network_submit(id, kind)
  return if mutation_phase != "idle"
  run forget_network(id, kind) -> network_forgotten _

on network_forgotten(_written)
  run hub_state() -> hub_refreshed _

// JOIN — unchanged plumbing, new seams: it starts from the network list and
// settles back into it through the provisioning/live screens.
on go_join
  return if mutation_phase != "idle"
  hub_step = "join"
  onboarding_error = ""

on go_networks
  return if mutation_phase != "idle"
  onboarding_error = ""
  hub_step = "networks"
  run hub_state() -> hub_refreshed _

on join_network_submit(blob)
  return if mutation_phase != "idle" || empty(trim(blob))
  onboarding_invite = trim(blob)
  onboarding_error = ""
  mutation_phase = "onboarding"
  run join_network(onboarding_invite) -> workspace_materialized _ | onboarding_failed _

// The workspace now exists on disk. Point the app at its endpoint and start
// watching for the node that will serve it.
on workspace_materialized(init)
  mutation_phase = "idle"
  onboarding_chain = init.chain_id
  onboarding_name = init.chain_id
  workspace_slug = network_slug(init.chain_id)
  rpc = init.rpc
  invite_link = ""
  provision_steps = []
  provision_index = 0
  onboarding_error = ""
  hub_step = "provisioning"
  stream provision_progress(init.chain_id, init.rpc) -> provision_stepped _

// Every yielded step replaces the reading. The screen only leaves this step
// when the LAST step actually settled — a blocked or running step keeps it
// here, showing what is wrong.
on provision_stepped(step)
  // Copy fields first, then the move: `provision_steps = [step]` takes the
  // step whole, so any read of `index`/`state` after it is a use-after-move.
  // `settled` exists precisely so this decision needs no String.
  provision_index = step.index
  provision_settled = step.settled
  provision_steps = [step]
  return if provision_index != 5 || !provision_settled
  hub_step = "live"
  run mint_invite(onboarding_chain, "resident", 7) -> onboarding_invite_minted _ | onboarding_failed _

on onboarding_invite_minted(blob)
  invite_link = blob
  onboarding_error = ""

on copy_onboarding_invite
  return if empty(invite_link)
  toast = "Invite link copied"
  toast_tone = "info"
  toast_age = 0
  task clipboard write invite_link

// Leaving the live screen is the first real connect for the fresh network:
// `rpc` already points at the workspace it materialized, so this is the
// network-pick handoff with the pick pre-made.
on enter_console
  return if mutation_phase != "idle"
  onboarding_error = ""
  task window open console -> console_opened _

// A refusal here is recoverable — the workspace is already on disk — so the
// screen keeps its controls and says what happened.
on onboarding_failed(cause)
  mutation_phase = "idle"
  onboarding_error = cause.message

// THE WAY BACK — `Leave workspace` (handlers/node.ice) reopens the launch
// window; once it is registered, the console closes behind it.
on onboarding_reopened(id)
  onboarding_win = some(id)
  hub_step = "loading"
  parallel
    task window close
    run hub_state() -> hub_booted _
