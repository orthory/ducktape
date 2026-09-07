// The roster's one native component: the fact row `screens/node.ice`
// shares. The Members and Agents screens ship as module-owned views
// (`crates/views/members`, `crates/views/agents`).

// A bordered machine-fact line. The value takes the rest of the row and wraps,
// because a full 64-hex public key does not fit a 312px panel on one line.
component MemberFactRow(label:str, value:str)
  box #root
    with
      w=fill
      px=12.0
      py=9.0
      bg=surface
      border=card_line
      border-w=1.0
      r=8.0
    row
      with
        w=fill
        gap=10.0
        align=start
      text label
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-meta
      // `word-or-glyph`, because the value is usually a 64-character hex key
      // and word wrapping cannot break an unbroken token: the text's minimum
      // intrinsic width became the whole key, which pushed this card wider
      // than the 312px panel and let the pane clip cut the key mid-digit at
      // the window edge. A key you cannot read in full is not a key.
      text value
        with
          w=fill
          size=11.0
          wrap=word-or-glyph
          font=code_medium
          @text-secondary_fg
