// A DECORATIVE icon only: it sits in no control and names its own tone. An
// icon that IS a button's glyph is not this component — it is a direct
// `svg icon(…) memory color=inherit` child of the button, drawing the
// button's status-resolved text color (ducktape-ui#606), because a component
// call is a view-body boundary the inherit channel cannot cross.
component Icon(name:str, tone:str, px:f64)
  svg icon(name) #root memory
    with
      w=px
      h=px
      style=icon_tint(tone)
