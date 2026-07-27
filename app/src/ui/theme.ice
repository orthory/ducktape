font ui family="Geist" weight=normal stretch=normal style=normal default=true
font medium family="Geist" weight=medium stretch=normal style=normal
font display family="Geist" weight=semibold stretch=normal style=normal
font strong family="Geist" weight=bold stretch=normal style=normal
font italic family="Geist" weight=normal stretch=normal style=italic
font strongitalic family="Geist" weight=bold stretch=normal style=italic
font code family="Geist Mono" weight=normal stretch=normal style=normal
font code_medium family="Geist Mono" weight=medium stretch=normal style=normal
font code_semibold family="Geist Mono" weight=semibold stretch=normal style=normal

// The steps the canonical artifact names that no semantic role in
// `ducktape-ui/default.ice` has a word for. Surfaces first, then the lines that
// separate them, then the ink ramp, then the functional-layer glass.
theme
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
