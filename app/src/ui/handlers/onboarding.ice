// ONBOARDING HANDLERS. The phase machine in front of the console: welcome ->
// join -> provisioning -> live -> console.
//
// The app is a strict CLIENT, and there is no create route: founding a network
// is `ducktape node init` on the node, where the coordinator and the rest of the
// network shape are chosen. `join_network` shells out to `ducktape node join`,
// which materializes a workspace on disk and then EXITS. Nothing here starts a
// daemon, so `provision_progress` steps 4-5 are a real `/v1/status` poll and a
// stalled node reports `blocked` carrying the command that starts it, instead
// of a spinner on an 850ms fake clock.

state
  // The steps the provisioning stream has actually reported. Ice has no list
  // append and no `sync` helper exists to merge one, so this holds the CURRENT
  // step rather than the artifact's five-row history — see the report.
  provision_steps:[ProvisionStep] = []
  provision_index:i64 = 0
  // The chain id `node join` minted, which is also the `-n`
  // workspace selector every later CLI call needs.
  onboarding_chain = ""

on go_welcome
  return if mutation_phase != "idle"
  phase = "welcome"
  onboarding_error = ""

on go_join
  return if mutation_phase != "idle"
  phase = "join"
  onboarding_error = ""

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
  phase = "provisioning"
  stream provision_progress(init.chain_id, init.rpc) -> provision_stepped _

// Every yielded step replaces the reading. The screen only leaves this phase
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
  phase = "live"
  run mint_invite(onboarding_chain, "resident", 7) -> onboarding_invite_minted _ | onboarding_failed _

on onboarding_invite_minted(blob)
  invite_link = blob
  onboarding_error = ""

on copy_onboarding_invite
  return if empty(invite_link)
  toast = "Invite link copied"
  toast_tone = "info"
  task clipboard write invite_link

// Leaving onboarding is the first real connect: the console mounts against the
// endpoint the new workspace published, on lifecycle's own routes.
on enter_console
  return if mutation_phase != "idle"
  phase = "console"
  onboarding_error = ""
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  connected = false
  loading = true
  connected_rpc = rpc
  run connect(rpc) -> workspace_connected _ | failed _

// A refusal here is recoverable — the workspace is already on disk — so the
// screen keeps its controls and says what happened.
on onboarding_failed(cause)
  mutation_phase = "idle"
  onboarding_error = cause.message
