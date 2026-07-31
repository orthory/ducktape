font ui family="Geist" weight=normal stretch=normal style=normal default=true
font medium family="Geist" weight=medium stretch=normal style=normal
font display family="Geist" weight=semibold stretch=normal style=normal
font strong family="Geist" weight=bold stretch=normal style=normal
font italic family="Geist" weight=normal stretch=normal style=italic
font strongitalic family="Geist" weight=bold stretch=normal style=italic
font code family="Geist Mono" weight=normal stretch=normal style=normal
font code_medium family="Geist Mono" weight=medium stretch=normal style=normal
font code_semibold family="Geist Mono" weight=semibold stretch=normal style=normal

// ONE contract, ONE palette: 2.0 allows exactly one of each, so the kit's 49
// semantic roles and the 74 steps the canonical artifact names live together
// here, in the app's own theme file. `ducktape-ui/default.ice` stays a verbatim
// vendor copy with its own theme deleted — a UI kit has no business naming
// `desk`, `rail`, or `diff_hunk_bg`.
theme contract AppTheme
  bg
  surface
  fg
  muted
  muted_bg
  primary
  primary_hover
  primary_fg
  disabled
  disabled_fg
  secondary
  secondary_fg
  accent
  accent_fg
  brand
  brand_fg
  brand_bg
  brand_line
  danger
  danger_fg
  danger_bg
  danger_line
  danger_dot
  success
  success_fg
  success_bg
  success_line
  success_dot
  warning
  warning_fg
  warning_bg
  warning_line
  warning_dot
  avatar_bg
  avatar_fg
  toast_bg
  toast_fg
  border
  control_line
  input
  ring
  glass_thin
  glass_regular
  glass_sheet
  shadow_popover
  shadow_toast
  shadow_modal
  shadow_window
  shadow_window_secondary
  desk
  desk_lit
  rail
  sidebar
  elevated
  subtle
  row_hover
  rail_hover
  window_line
  separator
  card_line
  danger_zone_line
  danger_zone_bg
  danger_solid
  danger_solid_hover
  danger_label
  ink_hover
  strong_ink
  caption
  meta
  hint
  label
  icon_idle
  success_tick
  info
  info_bg
  info_line
  info_dot
  alert_fg
  alert_bg
  alert_line
  alert_dot
  warning_bg_lit
  scrim
  bg_wash
  card_wash
  card_wash_hover
  unread_wash
  brand_wash
  tree_selected
  warning_plate
  pending_line
  control_line_hover
  ink_soft
  chevron_idle
  gutter_ink
  presence_off
  avatar_bg_sm
  avatar_fg_sm
  agent_live
  merged
  merged_bg
  merged_line
  final_bg
  final_line
  diff_add_bg
  diff_add_gutter
  diff_add_fg
  diff_del_bg
  diff_del_gutter
  diff_del_fg
  diff_hunk_bg
  panel_tile
  panel_tile_lit
  danger_soft
  tree_folder
  kind_page
  kind_page_bg
  kind_code
  kind_code_bg
  kind_file
  kind_file_bg
  kind_run
  kind_run_bg
  kind_task
  kind_task_bg

// Surfaces first, then the lines that separate them, then the ink ramp, then
// the functional layers.
palette app for AppTheme
  bg         #fdfdfb
  surface    #ffffff
  fg         #2c2b27
  muted      #6b6962
  muted_bg   #f6f5f2
  primary    #26251f
  primary_hover #322f28
  primary_fg #ffffff
  disabled   #ecebe6
  disabled_fg #b3b1a8
  secondary  #ffffff
  secondary_fg #5e5c55
  accent     #f3f2ef
  accent_fg  #3f3e39
  brand      #a05a3c
  brand_fg   #ffffff
  brand_bg   #f9f1ea
  brand_line #e7d2c4
  danger     #b8544c
  danger_fg  #ffffff
  danger_bg  #fdf4f3
  danger_line #efd6d3
  danger_dot #e0655c
  success    #5f9e74
  success_fg #151410
  success_bg #eef5f0
  success_line #cfe3d7
  success_dot #5cb45f
  warning    #a07b32
  warning_fg #151410
  warning_bg #fbf4e6
  warning_line #ecdcae
  warning_dot #e3b443
  avatar_bg  #d2d0c7
  avatar_fg  #4f4d47
  toast_bg   #26251f
  toast_fg   #f3f1ea
  border     #e7e6e2
  control_line #e0dfd7
  input      #8a8983
  ring       #26251f
  glass_thin #fdfcfa80
  glass_regular #fdfcfa9e
  glass_sheet #fdfcfadb
  shadow_popover #28262221
  shadow_toast #28262238
  shadow_modal #2826224d
  shadow_window #28262238
  shadow_window_secondary #2826221a
  // SURFACES — the content layer stays opaque paper, all the way down.
  desk #e3e1d9
  desk_lit #eceae3
  rail #fafaf8
  sidebar #fbfbf9
  elevated #f3f2ef
  subtle #ecebe6
  row_hover #f8f7f3
  rail_hover #f0efea
  // LINES — window, control, default (`border`), divider, track (`subtle`).
  window_line #d6d4cc
  separator #efeee9
  card_line #ece9e1
  danger_zone_line #ecd6d0
  danger_zone_bg #fdf6f4
  danger_solid #a35248
  danger_solid_hover #8f463d
  danger_label #c79a8a
  // INK — body copy forward, each step fades one notch further back.
  ink_hover #322f28
  strong_ink #3a3934
  caption #9a988f
  meta #a7a59b
  hint #b3b1a8
  label #bdbbb1
  icon_idle #cbc9bf
  // STATE inks the semantic roles do not carry.
  success_tick #7ba78c
  info #5f7a9e
  info_bg #eef2f7
  info_line #dae2ec
  info_dot #7f9ab8
  alert_fg #a35248
  alert_bg #fbecea
  alert_line #eccfc9
  alert_dot #cf6a5e
  warning_bg_lit #fbf8f0
  // The scrim a modal drops over the content it takes focus from.
  scrim #28262257
  // WASHES — the tint a row or card takes when it is hovered, unread, or
  // selected. Each is one notch off the surface it sits on, never a grey.
  bg_wash #f7f6f2
  card_wash #faf9f6
  card_wash_hover #fcfbf9
  unread_wash #fbfaf7
  brand_wash #fdf8f3
  tree_selected #f0ece1
  warning_plate #f4e7c8
  // Two more lines: the dashed outline of anything not yet settled, and the
  // step a control's border takes under the cursor.
  pending_line #d9d8d0
  control_line_hover #d5d3ca
  // Ink for the marks that are not text: chevrons and the diff gutter.
  ink_soft #b6b4a8
  chevron_idle #c8c6bc
  gutter_ink #c2c0b6
  // People — the small avatar plate, and the dot that says a peer is there.
  presence_off #d0cec4
  avatar_bg_sm #dcdbd4
  avatar_fg_sm #7a7872
  agent_live #7e9e88
  // FORGE — merged wears its own violet; finalized wears the success plate.
  merged #7a6f9e
  merged_bg #f1edf5
  merged_line #ddd2e6
  final_bg #f0f5f1
  final_line #dcebe0
  // DIFF — a sign column, a gutter, and the line tint behind each side.
  diff_add_bg #eef6ef
  diff_add_gutter #e1efe3
  diff_add_fg #2f6b41
  diff_del_bg #fbeeec
  diff_del_gutter #f4ddd8
  diff_del_fg #a14338
  diff_hunk_bg #f6f3f9
  // The dark tiles a panel header and a file tree fold use.
  panel_tile #4a4843
  panel_tile_lit #57554d
  danger_soft #e0918a
  tree_folder #a08a5a
  // EXPLORER — one ink + one plate per result kind, so kind reads before text.
  kind_page #5f8a72
  kind_page_bg #edf4ef
  kind_code #7a6f9e
  kind_code_bg #f1eff7
  kind_file #4a4843
  kind_file_bg #f2f1ed
  kind_run #b9714e
  kind_run_bg #faf0e9
  kind_task #c08a3e
  kind_task_bg #faf3e6
