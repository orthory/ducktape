component Icon(name:str, tone:str, px:f64)
  svg icon(name) #root memory
    with
      w=px
      h=px
      style=icon_tint(tone)

// THE SAME ICON, CARRYING THE HOVER INK ITS BUTTON CANNOT HAND IT. A button's
// `hovered … text=fg` reaches its content as an INHERITED text color, and an
// svg reads none — so an icon-only control lit its plate under the cursor and
// left the glyph muted, next to string-label buttons that brightened. `hover=`
// is the svg node's own status arm, layered over the resting tone.
//
// NOT the default on `Icon`: that arm keys on the SVG's OWN bounds, so every
// decorative icon in the app would light up under a cursor that can press
// nothing. An icon opts in by sitting in a control.
component IconAction(name:str, tone:str, px:f64)
  svg icon(name) #root memory
    with
      w=px
      h=px
      hover=fg
      style=icon_tint(tone)
