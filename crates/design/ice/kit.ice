component Card()
  container width=fill padding=12.0 style=card_style()
    slot

component InsetCard()
  container width=fill padding=8.0 style=inset_style()
    slot

component SectionLabel(title:str)
  text title size=15.0 wrapping=none font=medium @text-fg

component Chip(label:str)
  container height=22.0 padding-left=8.0 padding-right=8.0 align-y=center bg=elevated border=fg/10 border-w=1.0 r=11.0
    text label size=12.0 wrapping=none @text-muted

component AccentChip(label:str)
  container height=22.0 padding-left=8.0 padding-right=8.0 align-y=center bg=primary/12 border=primary/26 border-w=1.0 r=11.0
    text label size=12.0 wrapping=none font=medium @text-primary

component DangerChip(label:str)
  container height=22.0 padding-left=8.0 padding-right=8.0 align-y=center bg=danger/10 border=danger/26 border-w=1.0 r=11.0
    text label size=12.0 wrapping=none font=medium @text-fg
