component Icon(name:str, tone:str, px:f64)
  svg icon(name) #root memory
    with
      w=px
      h=px
      style=icon_tint(tone)
