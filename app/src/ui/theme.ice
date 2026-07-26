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
