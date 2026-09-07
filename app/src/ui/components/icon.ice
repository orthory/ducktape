// A DECORATIVE icon only: it sits in no control and names its own tone. An
// icon that IS a button's glyph is not this component — it is a direct
// `svg icon(…) memory color=inherit` child of the button, drawing the
// button's status-resolved text color (ducktape-ui#606), because a component
// call is a view-body boundary the inherit channel cannot cross.
//
// A tone is a palette token, matched here rather than resolved by a style
// callback: the palette already holds the ink ramp in both readings, and a
// declared color is what crosses the tree wire when this file is `use`d by
// a module view. The tones no token names (later, running, pending, next,
// done) read muted, as the ramp's default always did.
component Icon(name:str, tone:str, px:f64)
  col #root
    match tone
      "ink"
        svg icon(name) memory
          with
            w=px
            h=px
            color=primary
      "label"
        svg icon(name) memory
          with
            w=px
            h=px
            color=label
      "meta"
        svg icon(name) memory
          with
            w=px
            h=px
            color=meta
      "caption"
        svg icon(name) memory
          with
            w=px
            h=px
            color=caption
      "hint"
        svg icon(name) memory
          with
            w=px
            h=px
            color=hint
      "idle"
        svg icon(name) memory
          with
            w=px
            h=px
            color=icon_idle
      "accent"
        svg icon(name) memory
          with
            w=px
            h=px
            color=brand
      "success"
        svg icon(name) memory
          with
            w=px
            h=px
            color=success
      "warning"
        svg icon(name) memory
          with
            w=px
            h=px
            color=warning
      "danger"
        svg icon(name) memory
          with
            w=px
            h=px
            color=danger
      "paper"
        svg icon(name) memory
          with
            w=px
            h=px
            color=toast_fg
      _
        svg icon(name) memory
          with
            w=px
            h=px
            color=muted
