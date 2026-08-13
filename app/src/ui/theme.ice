font ui family="Geist" weight=normal stretch=normal style=normal default=true
font medium family="Geist" weight=medium stretch=normal style=normal
font display family="Geist" weight=semibold stretch=normal style=normal
font strong family="Geist" weight=bold stretch=normal style=normal
font italic family="Geist" weight=normal stretch=normal style=italic
font strongitalic family="Geist" weight=bold stretch=normal style=italic
font code family="Geist Mono" weight=normal stretch=normal style=normal
font code_medium family="Geist Mono" weight=medium stretch=normal style=normal
font code_semibold family="Geist Mono" weight=semibold stretch=normal style=normal

// ONE contract, TWO palettes: the kit's 49 semantic roles and the 74 steps the
// canonical artifact names live together here, in the app's own theme file,
// with a light and a dark reading of every token. The generic upstream recipes
// and log timeline are the only vendored Ice sources; app styling stays here.

// Icon actions take their size from the caller. Internal padding would collapse
// a fixed-size button's direct SVG child to a hairline.
recipe icon_action for button
  @p-0px bg-transparent text-fg rounded-7px hover:bg-accent pressed:bg-border disabled:opacity-50

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
  terminal_bg
  terminal_line
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
  selected_row
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
  forge_gutter_ink
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
  terminal_bg #090b0e
  terminal_line #242a33
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
  //
  // `selected_row` is the ONE plate that means "this is the row/tab you are
  // on", everywhere: chat channels, the selected chat MESSAGE and thread
  // reply, DMs, pages, both file trees, the object list, the repo switcher,
  // the nav rail, the member card, the Explorer blocks, the node matrix head.
  // It is NOT `subtle` — that is the track/pressed grey, and the two sat
  // 2.3/255 apart in dark until they were settled.
  //
  // A WASH CANNOT BE THE MARK ON ANYTHING NESTED INSIDE A ROW. Not a rule
  // about which wash: a wash an inner control rests on is a wash its row can
  // also wear, and no wash here stands off both `bg` and `selected_row` (the
  // best is `warning_plate`, 11.33/12.00, and it is the warning plate). So a
  // nested mark is INK or an EDGE. The reaction chip is where this was found:
  // mine rested on `brand_bg`, the selected message card's own plate at the
  // time, 0.00/255 apart in both themes, and now fills with `brand` ink at
  // 68.00/82.33 from the nearest plate any row wears.
  //
  // The quiet nested SLABS keep their washes — the un-reacted chip and the
  // code fence on `muted_bg`, the thread chip on `surface` — because the wash
  // was never their mark either. What draws them is the EDGE, and all three
  // were on lines that vanished on exactly the row this token paints:
  // `card_line` 2.33/3.00 and `border` 5.33/2.00 against `selected_row`. All
  // three now carry `control_line`, 13.00/7.67.
  //
  // `app/src/tests.rs` measures all four slabs' EDGES against every plate a
  // current-row arm anywhere in the app rests on, and holds every state on
  // them to the resting plate. Only the reacted chip's FILL is measured, and
  // only because it is the one of the four whose mark is a fill — the other
  // three rest on a wash on purpose, so a fill gate on them would be a gate
  // nothing is meant to pass.
  bg_wash #f7f6f2
  card_wash #faf9f6
  card_wash_hover #fcfbf9
  unread_wash #fbfaf7
  brand_wash #fdf8f3
  selected_row #f0ece1
  warning_plate #f4e7c8
  // Two more lines: the dashed outline of anything not yet settled, and the
  // step a control's border takes under the cursor.
  pending_line #d9d8d0
  control_line_hover #d5d3ca
  // Ink for quiet marks: chevrons and compact table headings.
  ink_soft #b6b4a8
  chevron_idle #c8c6bc
  gutter_ink #c2c0b6
  // People — the small avatar plate, and the dot that says a peer is there.
  presence_off #d0cec4
  avatar_bg_sm #dcdbd4
  avatar_fg_sm #7a7872
  agent_live #7e9e88
  // FORGE — the small mono gutter clears AA against every code/diff plate;
  // merged wears its own violet, and finalized wears the success plate.
  forge_gutter_ink #66645e
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

// The dark reading of the same contract — the same warm family, inverted:
// deep warm charcoal surfaces, off-white ink, semantic hues lifted one step
// so they carry on ink-dark plates. The dots keep their light-palette values
// (they were tuned as signals, not surfaces), and every `fg/N` composite in
// the screens adapts automatically because `fg` itself flips.
palette app_dark for AppTheme
  bg         #1b1a16
  surface    #22211d
  fg         #e8e6df
  muted      #a8a69c
  muted_bg   #26251f
  primary    #e8e6df
  primary_hover #f4f2ea
  primary_fg #1b1a16
  disabled   #33322c
  disabled_fg #6b6a61
  secondary  #2a2925
  secondary_fg #b5b3a9
  accent     #2e2d27
  accent_fg  #cfcdc4
  brand      #c98a63
  brand_fg   #1b1a16
  brand_bg   #33261d
  brand_line #4a382b
  danger     #d97b72
  danger_fg  #1b1a16
  danger_bg  #33211f
  danger_line #4d2f2c
  danger_dot #e0655c
  success    #7fb894
  success_fg #151410
  success_bg #1e2a22
  success_line #32473a
  success_dot #5cb45f
  warning    #d4a94e
  warning_fg #151410
  warning_bg #2e2717
  warning_line #4d3f22
  warning_dot #e3b443
  avatar_bg  #3a3931
  avatar_fg  #cfcdc4
  toast_bg   #f3f1ea
  toast_fg   #26251f
  border     #35342e
  control_line #3b3a33
  input      #85837b
  ring       #e8e6df
  glass_thin #1b1a1680
  glass_regular #1b1a169e
  glass_sheet #1b1a16db
  shadow_popover #00000040
  shadow_toast #00000059
  shadow_modal #00000073
  shadow_window #00000059
  shadow_window_secondary #00000026
  // SURFACES — the desk sits DEEPER than the content in the dark reading.
  desk #121110
  desk_lit #191815
  rail #201f1b
  sidebar #1e1d19
  elevated #2a2925
  subtle #31302b
  row_hover #24231e
  rail_hover #282722
  // LINES
  window_line #0e0d0b
  separator #2c2b26
  terminal_bg #090b0e
  terminal_line #242a33
  card_line #302f29
  danger_zone_line #4d2f2c
  danger_zone_bg #2a1d1b
  danger_solid #c25a4f
  danger_solid_hover #d3685c
  danger_label #8a5a4d
  // INK — the ramp fades toward the surface, mirroring the light order.
  ink_hover #f4f2ea
  strong_ink #dcdad2
  caption #8f8d84
  meta #7c7a71
  hint #6b6a61
  label #605f56
  icon_idle #55544c
  // STATE inks
  success_tick #7ba78c
  info #7f9ab8
  info_bg #1e2530
  info_line #303e52
  info_dot #7f9ab8
  alert_fg #d3685c
  alert_bg #301f1c
  alert_line #4d2f2c
  alert_dot #cf6a5e
  warning_bg_lit #2a2517
  scrim #00000080
  // WASHES — one notch off the dark surface each sits on.
  bg_wash #201f1a
  card_wash #23221d
  card_wash_hover #262520
  unread_wash #262418
  brand_wash #2a221b
  selected_row #35322a
  warning_plate #453a1e
  pending_line #3f3e36
  control_line_hover #45443c
  ink_soft #6e6d63
  chevron_idle #5b5a52
  gutter_ink #62615a
  presence_off #4a4941
  avatar_bg_sm #33322c
  avatar_fg_sm #a3a198
  agent_live #7e9e88
  // FORGE
  forge_gutter_ink #9d9b92
  merged #a89ac9
  merged_bg #2a2633
  merged_line #443c57
  final_bg #1e2a22
  final_line #32473a
  diff_add_bg #1d2a20
  diff_add_gutter #24352a
  diff_add_fg #8fc9a2
  diff_del_bg #2f1f1c
  diff_del_gutter #3d2723
  diff_del_fg #de8b7f
  diff_hunk_bg #262330
  // Panel tiles read one step LIGHTER than their surface in the dark reading.
  panel_tile #2e2d28
  danger_soft #a05c55
  tree_folder #c0a86e
  // EXPLORER
  kind_page #7fb894
  kind_page_bg #1e2a22
  kind_code #a89ac9
  kind_code_bg #2a2633
  kind_file #cfcdc4
  kind_file_bg #2e2d28
  kind_run #d09068
  kind_run_bg #33261d
  kind_task #d4a94e
  kind_task_bg #2e2717
