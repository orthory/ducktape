component Icon(name:str, tone:str, px:f64)
  svg icon(name) #root memory
    with
      w=px
      h=px
      style=icon_tint(tone)

// THE SAME ICON, CARRYING THE INK ITS BUTTON CANNOT HAND IT. A button's
// `hovered … text=fg` reaches its content as an INHERITED text color and an svg
// reads none, so an icon-only control lit its plate under the cursor and left
// the glyph muted, next to string-label buttons that brightened — and
// `disabled:opacity-50` never dimmed it either, for the same reason.
//
// `disabled` IS A PARAMETER, not the svg's own `hover=` arm, because that arm
// keys on the SVG's OWN bounds: iced computes `svg::Status::Hovered` inside
// `Button::update`, which forwards to its content BEFORE it checks `on_press`,
// so a dead control's glyph would light up under the cursor. The button's own
// disabled term is the only thing that knows, so the mount passes it down: a
// mount that forgets is `E123 missing prop`, and one that silences it with a
// literal is `assert_icon_controls_inherit_ink`.
//
// NOT folded into `Icon`: a decorative icon sits in no control, has no disabled
// term to read, and must not answer a cursor that can press nothing.
component IconAction(name:str, tone:str, px:f64, disabled:bool)
  svg icon(name) #root memory
    with
      w=px
      h=px
      style=icon_action_tint(tone, disabled)
