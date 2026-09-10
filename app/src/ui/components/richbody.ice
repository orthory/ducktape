// THE RICH BODY — one message's blocks, as the chat stream and the forge
// discussion both draw them: the paragraph/quote/code arms, the inline span
// template, and the author plate. Shared by the app (screens/forge.ice) and
// the chat view (crates/views/chat), which `use`s this file by path; it
// calls no app extern, so both sides compile it as it is.
// ONE PARAGRAPH, NOT A FLEX OF TOKENS (ducktape-ui#639 collected by #1096).
// The whole span list lowers into a single native rich-text widget: real word
// wrapping (`word-or-glyph` still, so an unbroken hash or invite one run wide
// breaks instead of clipping), native selection across the line, and `link=`
// handled by the widget's own route instead of a per-token button.
//
// THE ARM CHOICE IS DATA. A rich-text `for` expands a fixed span template per
// item with no conditionals, so the module files every run under exactly one
// `ChatSpan` text field (`span_arm` in chat's client) and this template emits
// every arm for every run — an empty span draws no glyphs, so the five silent
// arms of each run cost nothing.
//
// A MENTION IS A TOKEN, NOT JUST A TINT: the `brand_bg` plate (68.00+/255
// from every row plate it can sit on) makes it an entity you can find at a
// glance, and ONLY a mention wears it — a posted URL is a destination, not a
// person. A LINK wears the underline instead, the one convention every reader
// already knows, and ONLY the link arm draws that rule: it marks a
// destination, not an emphasis. Both keep brand ink; the plate versus the
// rule is what tells a human from a destination.
component RichLine(block:ChatBlock, size:f64)
  emits
    open_message_link(str)
  rich-text -> emit(open_message_link, _)
    with
      w=fill
      size=size
      line-h=1.55
      wrap=word-or-glyph
      color=accent_fg
    for span in block.spans
      // Span padding expands only the paint, not the text layout. Keep it
      // below a prose space so ordinary surrounding spaces stay visible.
      span span.mention bg=brand_bg px=1.0 r=4.0 font=medium color=brand
      span span.link_text link=span.link underline font=medium color=brand
      span span.bold_italic font=strongitalic
      span span.bold font=strong
      span span.italic font=italic
      span span.plain

// max-w=760.0 STOPS THE LINE, not the pane. Nothing here bounded a line's
// measure at all — the default window ran ~130 characters a line and a
// maximized one ~320, both well past the 45–75 characters a reader's eye can
// find its way back across reliably. 760px at 13.5px Geist is ~110 chars:
// generous, and it leaves the narrow rail/min-window case (≤300px) untouched.
// Everything that hangs off this column — the reaction row, the reply pill —
// inherits the cap through `w=fill`, which is correct: neither belongs 2000px
// right of the text it is about.
component MessageBody(message:ChatMessage)
  emits
    open_message_link(str)
  col w=fill max-w=760.0
    RichBody blocks=message.blocks size=13.5
      forward
        open_message_link

// ONE MARKDOWN RENDERER FOR THE WHOLE APP. A body is a body: chat rows, forge
// issue/PR descriptions and review comments all run the same tokenizer
// (`chat::client::paragraph_blocks`) and land here, so a `duck://` ref, a
// `[label](url)` and a bare `https://` are openable wherever prose is shown —
// through the ONE open plane, never a second renderer with its own idea of
// what a link is. `size` is the prose scale; a code fence keeps its own
// monospace scale, which does not track the surrounding prose.
component RichBody(blocks:[ChatBlock], size:f64)
  emits
    open_message_link(str)
  col w=fill gap=5.0
    for block in blocks
      if block.kind == "divider"
        Separator
      // A code fence is a QUIET slab: the near-surface tint + hairline reads
      // as "preformatted" in both themes (the old black/26 was a dark slab in
      // light mode and vanished in dark). The lang tag is an eyebrow label,
      // not a code line.
      //
      // Same shape as the un-reacted reaction chip, and the same fix: this is
      // nested INSIDE a message row, `muted_bg` was never what drew it, and
      // its `border` edge measured 5.33/2.00 against `selected_row` — inside a
      // message you had a menu open on, the fence had no outline at all.
      // `control_line` is 13.00/7.67 there and 31.67/31.00 against `bg`.
      if block.kind == "code"
        box
          with
            w=fill
            p=11.0
            bg=muted_bg
            border=control_line
            border-w=1.0
            r=9.0
          col w=fill gap=6.0
            if !empty(block.lang)
              text block.lang
                with
                  size=10.0
                  wrap=none
                  font=code_semibold
                  @text-label
            text block.text
              with
                w=fill
                size=12.0
                line-h=1.5
                font=code
                wrap=word-or-glyph
                @text-fg
      // A quote is a LEFT BAR, not a box — boxed it was indistinguishable
      // from a code slab at a glance. The bar wears the warm accent hairline
      // and fills the CONTENT's height (zstack resolves fill layers against
      // the content union — ducktape-ui 1231692; the earlier row form
      // degenerated in the infinite-height scroll and ate the whole row).
      if block.kind == "quote"
        stack w=fill
          col
            with
              w=fill
              pl=13.0
              pt=2.0
              pb=2.0
            if block.rich
              RichLine block=block size=size
                forward
                  open_message_link
            // QUIETER THAN THE PROSE QUOTING IT, at the SAME leading the rich
            // arm renders it at — a quote is cited material, and it should
            // recede from the author's own words, not out-darken them. This
            // was the darkest ink in the message (`fg`, 13.4:1); `muted`
            // (5.37:1) is still comfortably readable and visibly quieter.
            if !block.rich
              text block.text
                with
                  w=fill
                  size=size
                  line-h=1.55
                  wrap=word-or-glyph
                  @text-muted
          box
            with
              w=3.0
              h=fill
              bg=brand_line
              r=1.5
            space w=1.0 h=1.0
      if block.kind == "paragraph"
        if block.rich
          RichLine block=block size=size
            forward
              open_message_link
        if !block.rich
          text block.text
            with
              w=fill
              size=size
              line-h=1.55
              wrap=word-or-glyph
              @text-accent_fg

// People are round, agents are 8px-rounded squares. The artifact never mixes
// the two — the shape IS the authorship signal.
component MessageAvatar(initials:str, kind:str)
  stack #root w=30.0 h=30.0
    match kind
      "human"
        PersonAvatar
          with
            initials
            plate=30.0
            ink=11.0
      "agent"
        AgentAvatar
          with
            initials
            plate=30.0
            ink=11.0
      _
        AgentAvatar
          with
            initials
            plate=30.0
            ink=11.0
