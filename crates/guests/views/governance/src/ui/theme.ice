// Fira Sans is the one family a guest has: iced embeds it in the module, and
// the app embeds the same faces to replay the guest's lines where it laid
// them (`app/Cargo.toml`, the `fira-sans` feature).
font ui family="Fira Sans" weight=normal stretch=normal style=normal default=true
font strong family="Fira Sans" weight=bold stretch=normal style=normal

theme contract ApprovalsTheme
  bg
  fg
  primary
  primary_fg
  danger
  surface
  raised
  muted
  border

palette light for ApprovalsTheme
  bg #f6f7f9
  fg #14181f
  primary #0f766e
  primary_fg #ffffff
  danger #c2313d
  surface #ffffff
  raised #e9eef4
  muted #5f6b7a
  border #dfe4ea

palette dark for ApprovalsTheme
  bg #0f1318
  fg #e7ecf2
  primary #2dd4bf
  primary_fg #062b27
  danger #f2606a
  surface #171d25
  raised #212b38
  muted #8c9aab
  border #283140
