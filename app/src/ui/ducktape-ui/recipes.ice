recipe section for col
  @w-full gap-3

recipe field for col
  @w-full gap-2

recipe panel for box
  @w-full p-5 bg-surface border border-border rounded-11px overflow-hidden

recipe section_title for text
  @text-16px leading-snug font-semibold text-primary

recipe caption for text
  @text-12.5px leading-normal text-muted

recipe meta for text
  @text-11px leading-normal font-medium text-muted

recipe field_label for text
  @text-12.5px leading-normal font-semibold text-fg

recipe badge_label for text
  @text-9px leading-snug font-semibold text-fg

recipe control for input
  @w-full px-13px py-11px bg-surface border border-border rounded-10px focus:border-ring

// Every action recipe styles its keyboard focus ring `focus-visible:border-ring`
// (ducktape-ui#611): the ring paints ONLY on keyboard/AT-acquired focus — a
// mouse click no longer wears it (orthory#804's cosmetic item) — and it draws
// as an overlay against the page, not the button fill, so the paint is the
// same `ring` token inputs use for `focus:border-ring`, never the button's own
// text ink (a primary button's light `primary_fg` would vanish on a light
// page). The styled ring also takes each button's rounded-* radius in place
// of the default 3px.
recipe primary_action for button
  @text-12.5px font-semibold px-16px py-11px bg-primary text-primary_fg rounded-9px hover:bg-primary_hover pressed:bg-primary/80 disabled:bg-disabled disabled:text-disabled_fg focus-visible:border-ring

recipe secondary_action for button
  @text-12.5px font-semibold px-16px py-11px bg-secondary text-secondary_fg border border-control_line rounded-9px hover:bg-accent pressed:bg-muted_bg disabled:opacity-50 focus-visible:border-ring

recipe outline_action for button
  @text-12.5px font-semibold px-12px py-8px bg-surface text-accent_fg border border-border rounded-8px hover:bg-muted_bg pressed:bg-accent disabled:opacity-50 focus-visible:border-ring

recipe ghost_action for button
  @text-12.5px font-semibold px-12px py-7px bg-transparent text-fg rounded-8px hover:bg-accent pressed:bg-border disabled:opacity-50 focus-visible:border-ring

recipe danger_action for button
  @text-12.5px font-semibold px-16px py-11px bg-danger text-danger_fg rounded-9px hover:bg-danger/90 pressed:bg-danger/80 disabled:opacity-50 focus-visible:border-ring
