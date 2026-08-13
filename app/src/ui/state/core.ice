state
  app_palette:palette[AppTheme] = AppTheme.app
  appearance = ""
  wall_now:i64 = current_wall_seconds()
  rpc = ""
  connected_rpc = ""
  password = ""
  status = "Connecting…"
  connected = false
  loading = false
  block_height:i64 = -1
  hydration_generation:i64 = 0
  connect_generation:i64 = 0
  hydration_retry_attempt:i64 = 0
  mutation_phase:MutationPhase = MutationPhase.idle
  error = ""
