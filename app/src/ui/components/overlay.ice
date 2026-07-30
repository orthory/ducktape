// The overlay and control primitives no single screen owns. Every shape here
// is chrome only: none of them route, so the button, the handler and the state
// stay at the call site and each screen keeps ownership of its own decisions.

// The modal CARD — r=14 over the artifact's 24/60 shadow, 418px at the call
// site. It paints no scrim and binds no dismiss on purpose: both come free
// from the `overlay` widget, which is also the only way to close on scrim
// click.
//
//   overlay when=channel_create_open dismiss=close_channel_modal backdrop=scrim
//           p=30.0 align-x=center align-y=center
//     content
//       space w=fill h=fill
//     layer
//       ModalShell title="Invite to testnet" width=418.0
//         close:
//           button label="Close" ... -> close_invite_modal
//         body:
//           col w=fill gap=10.0
//             ...
component ModalShell(title:str, width:f64)
  box #root w=width bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0
    col w=fill pl=22.0 pr=22.0 pt=20.0 pb=22.0 gap=13.0
      row w=fill gap=10.0 align=center
        text title w=fill size=16.0 wrap=none font=display @text-primary
        slot close
      slot body

// The only confirmation surface the artifact has: an ink pill on the bottom
// edge. The stack that positions it and the timer that retires it belong to
// the view; this is one toast.
// NAME: the contract froze this as `Toast`, which is already taken by the
// vendored `Toast(title, description)` in ducktape-ui/components.ice — a file
// no item in this campaign owns. `Toast.Confirm` is the same component under
// the family's own dotted convention (Badge.Success, Alert.Warning).
component Toast.Confirm(message:str, tone:str)
  box #root px=16.0 py=10.0 bg=toast_bg r=10.0 shadow=shadow_toast shadow-y=6.0 shadow-blur=18.0
    row gap=9.0 align=center
      match tone
        "error"
          box w=6.0 h=6.0 bg=danger_dot r=3.0
            space w=1.0 h=1.0
        "warning"
          box w=6.0 h=6.0 bg=warning_dot r=3.0
            space w=1.0 h=1.0
        _
          box w=6.0 h=6.0 bg=success_dot r=3.0
            space w=1.0 h=1.0
      text message size=12.5 wrap=none @text-toast_fg

// `Switch` (a 38x22 track with an 18px knob sliding 2px -> 18px, over the
// label/note pair) was built here and is DELETED — this console has nothing to
// switch. The app ships exactly ONE device preference, `receipts`, and view.ice
// already records why it has no surface: every finality mark renders
// unconditionally, so the toggle wrote a value nothing read, and it painted ON
// from the state default before the loader answered `false` a beat later. Every
// other boolean on screen is a disclosure (`channel_settings_open`,
// `fs_history_open`, `bell_open`) whose control is the thing it opens, not a
// track and a knob.
// The label/note half of it is not lost: the Settings THIS DEVICE card draws
// exactly that pair — 12.5 `@text-accent_fg` over 12.5 `@text-meta` — beside a
// value and a button, which is the honest shape for a reading with an action.
// The geometry to restore the day a real preference exists: 38x22 track, r11,
// `primary` on / `pending_line` off, an 18px `surface` knob at y=2 sliding
// x=2 -> x=18, and the press target on the CALLER's button, never here.

// The floating card every dropdown wears — repo switcher, mention autocomplete,
// message ⋯ menu, bell. The caller pins it (stack + pin, or an `overlay` layer)
// and fills it with its own rows.
component Popover(width:f64)
  box #root w=width p=5.0 bg=surface border=border border-w=1.0 r=11.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
    col w=fill
      slot

// Tab-bar chrome: the label, its count chip, and the 2px underline that carries
// selection. The button and its route stay at the call site so the forge tabs,
// the PR detail tabs and the node tabs each pick with their own handler.
component TabLabel(label:str, count:i64, active:bool)
  col #root
    row gap=7.0 pt=10.0 pb=10.0 align=center
      if active
        text label size=13.0 wrap=none font=display @text-primary
      if !active
        text label size=13.0 wrap=none font=display @text-meta
      if count > 0
        box px=7.0 py=1.0 bg=elevated r=9.0
          text count size=10.0 wrap=none font=code_semibold @text-meta
    if active
      box w=fill h=2.0 bg=primary
        space w=1.0 h=1.0
    if !active
      box w=fill h=2.0 bg=transparent
        space w=1.0 h=1.0

// A filter chip with its matched count: the Explorer kind strip and the
// members All/Humans/Agents/Validators strip. Selected inverts to ink.
component FilterChip(label:str, count:i64, selected:bool)
  col #root
    if selected
      box px=11.0 py=6.0 bg=primary border=primary border-w=1.0 r=8.0
        row gap=6.0 align=center
          text label size=12.0 wrap=none font=display @text-primary_fg
          text count size=10.0 wrap=none font=code_semibold @text-meta
    if !selected
      box px=11.0 py=6.0 bg=surface border=border border-w=1.0 r=8.0
        row gap=6.0 align=center
          text label size=12.0 wrap=none font=display @text-secondary_fg
          text count size=10.0 wrap=none font=code_semibold @text-label

// The 9px mono section label, with the artifact's optional trailing note
// (`needs quorum to change`) hung beside it.
// NOTE: the artifact sets letter-spacing .1em on this label and iced exposes
// none. The per-glyph row that would fake it needs a chars-splitting helper in
// backend.rs, which this file does not own — it renders tight until that lands.
component Eyebrow(label:str, note:str)
  row #root gap=8.0 align=center
    text label size=9.0 wrap=none font=code_semibold @text-label
    if note != ""
      text note size=9.0 wrap=none font=code_semibold @text-label

// NO SPINNER COMPONENT. The artifact's 2px ring with a transparent quarter is
// a CSS keyframe; the ice equivalent is a `canvas` arc turned by
// `animation.value(spin)`. Nothing in this app drives `spin` — state.ice
// declares it (.8s linear forever) but no handler ever assigns it, so an
// `animation` that is never transitioned never ticks. A ring mounted on it
// would paint a FROZEN arc: a spinner that claims work is in flight and then
// stands still. Both running markers therefore ship static and say so —
// `ProvisionMark` (onboarding.ice) is a solid amber ring, and every live
// signal elsewhere is `PulseDot`. Reviving the ring costs a `spin = 1.0`
// driver alongside `pulse` in handlers/lifecycle.ice plus one canvas; until
// that driver exists, the honest marker is the static one.

// One NETWORK stat card: a mono caps label over the machine reading, with an
// optional unit suffix (`ms`) beside it.
component StatCard(label:str, value:str, note:str)
  box #root w=fill px=13.0 py=11.0 bg=surface border=card_line border-w=1.0 r=10.0
    col w=fill gap=3.0
      text label size=9.0 wrap=none font=code_semibold @text-label
      row gap=4.0 align=center
        text value size=14.0 wrap=none font=code_semibold @text-primary
        if note != ""
          text note size=11.0 wrap=none font=code_medium @text-meta
