// A sidebar channel row: 7px radius, 8px side inset, the hash in `label` ink
// and the name in `list` weight. Selected is the same tint capsule the rail uses.
component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)
  box w=fill pl=8.0 pr=8.0
    col w=fill
      if selected
        button label=channel.name w=fill p=0.0 @icon_action -> choose_channel(channel.id)
          box w=fill pl=10.0 pr=10.0 pt=7.0 pb=7.0
            row w=fill gap=7.0 align=center
              if channel.members_only
                text "◆" size=12.0 wrap=none @text-label
              if !channel.members_only
                text "#" size=13.0 wrap=none @text-label
              text channel.name w=fill size=13.0 wrap=none font=medium @text-fg
              if channel.archived
                text "archived" size=9.0 wrap=none font=code_semibold @text-label
              if channel.huddle_count > 0
                box w=7.0 h=7.0 bg=success_dot r=3.5
                  space w=1.0 h=1.0
          active bg=subtle text=fg border=transparent border-w=1.0 r=7.0
          hovered bg=subtle text=fg
          pressed bg=rail_hover text=fg
      if !selected
        button label=channel.name w=fill p=0.0 @icon_action -> choose_channel(channel.id)
          box w=fill pl=10.0 pr=10.0 pt=7.0 pb=7.0
            row w=fill gap=7.0 align=center
              if channel.members_only
                text "◆" size=12.0 wrap=none @text-label
              if !channel.members_only
                text "#" size=13.0 wrap=none @text-label
              if unread
                text channel.name w=fill size=13.0 wrap=none font=medium @text-fg
              if !unread
                text channel.name w=fill size=13.0 wrap=none font=medium @text-muted
              if channel.archived
                text "archived" size=9.0 wrap=none font=code_semibold @text-label
              if channel.huddle_count > 0
                box w=7.0 h=7.0 bg=success_dot r=3.5
                  space w=1.0 h=1.0
              if unread
                box w=7.0 h=7.0 bg=brand r=3.5
                  space w=1.0 h=1.0
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg

component ChatMemberRow(member:ChatMember, disabled:bool)
  row w=fill gap=6.0 align=center
    text member.label w=fill size=12.0 font=code @text-muted
    button "Remove" description=member.label disabled=disabled h=28.0 p=5.0 @danger_action -> remove_channel_member_submit(member.key)

component RichLine(block:ChatBlock)
  flex w=fill wrap=wrap gap-x=0.0 gap-y=4.0 items=start
    for span in block.spans
      if span.highlight
        text span.text size=13.5 line-h=1.55 font=medium @text-brand
      if !span.highlight && span.bold && span.italic
        text span.text size=13.5 line-h=1.55 font=strongitalic @text-accent_fg
      if !span.highlight && span.bold && !span.italic
        text span.text size=13.5 line-h=1.55 font=strong @text-accent_fg
      if !span.highlight && !span.bold && span.italic
        text span.text size=13.5 line-h=1.55 font=italic @text-accent_fg
      if !span.highlight && !span.bold && !span.italic
        text span.text size=13.5 line-h=1.55 @text-accent_fg

component MessageBody(message:ChatMessage)
  col w=fill gap=5.0
    for block in message.blocks
      if block.kind == "divider"
        Separator
      if block.kind == "code"
        box w=fill p=11.0 bg=black/26 border=fg/11 border-w=1.0 r=9.0
          col w=fill gap=5.0
            if !empty(block.lang)
              text block.lang size=10.5 wrap=none font=code_medium @text-muted
            text block.text w=fill size=12.0 line-h=1.5 font=code wrap=word @text-fg
      if block.kind == "quote"
        box w=fill p=9.0 pl=13.0 bg=muted_bg border=border border-w=1.0 r=8.0
          col w=fill
            if block.rich
              RichLine block=block
            if !block.rich
              text block.text w=fill size=13.5 line-h=1.45 wrap=word @text-fg
      if block.kind == "paragraph"
        if block.rich
          RichLine block=block
        if !block.rich
          text block.text w=fill size=13.5 line-h=1.55 wrap=word @text-accent_fg

// People are round, agents are 8px-rounded squares. The artifact never mixes
// the two — the shape IS the authorship signal.
component MessageAvatar(initials:str, kind:str)
  stack #root w=30.0 h=30.0
    match kind
      "human"
        PersonAvatar initials=initials plate=30.0 ink=11.0
      "agent"
        AgentAvatar initials=initials plate=30.0 ink=11.0
      _
        AgentAvatar initials=initials plate=30.0 ink=11.0

// ONE reaction chip, on the artifact's warm pair. Mine is the tint plate the
// tree already tokenises at the artifact's own #f0ece1; not-mine is the flat
// wash with the card hairline. The count rides at the emoji's size, not one
// step above it.
// NOTE the two tokens theme.ice does not carry: the mine border (#e0d2bd) and
// the mine ink (#8a6a4a). Until they land this chip wears `brand_line`/`brand`,
// which is the same warm family one step hotter.
component ReactionChip(reaction:ChatReaction, seq:i64)
  col #root
    if reaction.reacted_by_me
      button label="Remove reaction" description=reaction.emoji p=0.0 @icon_action -> remove_reaction_at(seq, reaction.emoji)
        box pl=6.0 pr=8.0 pt=1.0 pb=1.0
          row gap=4.0 align=center
            text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
            text reaction.count size=11.0 wrap=none font=code_medium @text-brand
        active bg=tree_selected text=brand border=brand_line border-w=1.0 r=11.0
        hovered bg=tree_selected text=brand border=brand
        pressed bg=warning_plate text=brand border=brand
    if !reaction.reacted_by_me
      button label="Add reaction" description=reaction.emoji p=0.0 @icon_action -> add_reaction_at(seq, reaction.emoji)
        box pl=6.0 pr=8.0 pt=1.0 pb=1.0
          row gap=4.0 align=center
            text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
            text reaction.count size=11.0 wrap=none font=code_medium @text-muted
        active bg=muted_bg text=muted border=card_line border-w=1.0 r=11.0
        hovered bg=elevated text=fg border=control_line
        pressed bg=subtle text=fg

component MessageContents(message:ChatMessage)
  row w=fill gap=11.0 align=start
    if message.show_author
      MessageAvatar initials=message.initial kind=message.avatar_kind
    if !message.show_author
      space w=30.0
    col w=fill gap=2.0
      if message.show_author
        row w=fill gap=7.0 align=center
          text message.author size=13.0 wrap=none font=display @text-fg
          if message.avatar_kind == "agent"
            box px=5.0 py=2.0 bg=primary r=4.0
              text "AGENT" size=9.0 wrap=none font=code_semibold @text-primary_fg
          // The artifact stamps a wall clock here. This chain has none: the
          // validator writes `consensus_time = height`, so the honest stamp is
          // the block the message landed in. An unsettled row has no height
          // yet and simply carries none.
          if message.height > 0
            text height_label_short(message.height) size=11.0 wrap=none font=code_medium @text-hint
          if message.edited
            text "· edited" size=11.0 wrap=none font=code_medium @text-hint
          if !message.pending
            text "✓" size=10.0 wrap=none font=code_semibold @text-success_tick
          space w=fill
      MessageBody message=message
      // Reactions and the replies button STACK — the artifact gives each its
      // own line under the body, never one shared row.
      if !empty(message.reactions)
        flex w=fill wrap=wrap gap-x=5.0 gap-y=5.0 items=start pt=6.0
          for reaction in message.reactions
            ReactionChip reaction=reaction seq=message.seq
      if message.reply_count > 0
        row w=fill gap=6.0 align=center pt=6.0
          button label="Open thread" p=0.0 @icon_action -> open_thread_for(message.seq)
            box pl=7.0 pr=9.0 pt=3.0 pb=3.0
              row gap=6.0 align=center
                Icon name="nav-chat" tone="accent" px=12.0
                text message.reply_count size=11.0 wrap=none font=code_medium @text-brand
                text "replies" size=11.0 wrap=none font=code_medium @text-brand
            active bg=surface text=brand border=border border-w=1.0 r=8.0
            hovered bg=muted_bg text=brand border=control_line
            pressed bg=elevated text=brand

component MessageCard(message:ChatMessage, selected:bool, hovered:bool, disabled:bool)
  mouse enter=message_entered(message.seq) exit=message_exited(message.seq)
    stack w=fill
      if message.deleted
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=transparent border=transparent border-w=1.0 r=9.0
          MessageContents message=message
      if !message.deleted && selected
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=brand_bg border=brand_line border-w=1.0 r=9.0
          MessageContents message=message
      if !message.deleted && !selected && hovered
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=row_hover border=transparent border-w=1.0 r=9.0
          MessageContents message=message
      if !message.deleted && !selected && !hovered
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=transparent border=transparent border-w=1.0 r=9.0
          MessageContents message=message
      if !message.deleted && !message.pending && !hovered
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_message_actions_accessibly(message.seq, message.body, message.rev)
            active bg=transparent text=muted r=7.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/13 text=fg
      if !message.deleted && !message.pending && hovered
        box w=fill align-x=end align-y=start pr=8.0
          // The designer's own opaque answer for this bar: white card, 1px
          // border, r9 over the 3/12 popover shadow. The glass file's r13 and
          // its heavier drop are the blurred variant, which this app has no
          // compositor for. The artifact hangs it 12px above the row's top
          // edge; ice has no negative offset, so it sits inside the row.
          box p=2.0 bg=surface border=border border-w=1.0 r=9.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
            row gap=1.0 align=center
              // Three one-tap reactions, exactly as the artifact wires them:
              // one click, no intermediate surface. `♡` still opens the full
              // picker for the emojis these three do not cover.
              button "👍" label="React with 👍" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> add_reaction_at(message.seq, "👍")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "✅" label="React with ✅" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> add_reaction_at(message.seq, "✅")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "👀" label="React with 👀" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> add_reaction_at(message.seq, "👀")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              box w=1.0 h=16.0 bg=subtle
                space w=1.0 h=1.0
              button "♡" label="Manage reactions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> open_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button label="Open thread" disabled=disabled p=5.0 @icon_action -> open_thread_for(message.seq)
                Icon name="nav-chat" tone="muted" px=15.0
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button label="Inspect event" disabled=disabled p=5.0 @icon_action -> open_message_inspector(message.seq)
                Icon name="shield" tone="muted" px=15.0
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "⋯" label="More message actions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> open_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg

// A reply reads one notch SMALLER than the same message in the timeline: 26px
// plate, 12px author, and the stamp a step down. The body keeps the shared
// block renderer — `MessageBody` is also mounted by the forge discussion, so
// its scale is not this component's to change.
component ThreadMessageBody(message:ChatMessage)
  row w=fill gap=10.0 align=start
    PrincipalAvatar initials=message.initial is_agent=(message.avatar_kind == "agent") plate=26.0 ink=9.0 ring=""
    col w=fill gap=1.0
      row w=fill gap=6.0 align=center
        text message.author size=12.0 wrap=none font=display @text-fg
        if message.height > 0
          text height_label_short(message.height) size=10.0 wrap=none font=code_semibold @text-hint
        if message.edited
          text "· edited" size=10.0 wrap=none font=code_semibold @text-hint
        space w=fill
      MessageBody message=message
      if !empty(message.reactions)
        flex w=fill wrap=wrap gap-x=5.0 gap-y=5.0 items=start pt=5.0
          for reaction in message.reactions
            ReactionChip reaction=reaction seq=message.seq

component ThreadMessageCard(message:ChatMessage, selected:bool, hovered:bool, disabled:bool)
  mouse enter=thread_message_entered(message.seq) exit=thread_message_exited(message.seq)
    stack w=fill
      if message.deleted
        box w=fill p=8.0 bg=transparent border=transparent border-w=1.0 r=9.0
          ThreadMessageBody message=message
      if !message.deleted && selected
        box w=fill p=8.0 bg=accent border=border border-w=1.0 r=9.0
          ThreadMessageBody message=message
      if !message.deleted && !selected && hovered
        box w=fill p=8.0 bg=fg/4 border=fg/7 border-w=1.0 r=9.0
          ThreadMessageBody message=message
      if !message.deleted && !selected && !hovered
        box w=fill p=8.0 bg=transparent border=transparent border-w=1.0 r=9.0
          ThreadMessageBody message=message
      if !message.deleted && !message.pending && !hovered
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_thread_message_actions(message.seq, message.body, message.rev)
            active bg=transparent text=muted r=7.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/13 text=fg
      if !message.deleted && !message.pending && hovered
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          box p=2.0 style=raised_style()
            row gap=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_thread_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg
              button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_thread_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg

component ChatSearchResult(hit:ChatSearchHit)
  button label=hit.text w=fill p=8.0 @ghost_action -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
    col w=fill gap=3.0
      row w=fill gap=7.0 align=center
        text hit.author w=fill size=13.0 font=medium @text-fg
        text hit.meta size=11.0 font=code_medium @text-muted
      text hit.text w=fill size=13.5 wrap=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
    hovered bg=fg/6 text=fg border=fg/9
    pressed bg=fg/10 text=fg border=fg/13

// ============================================================================
// MY OWN POSTS — the artifact branches the message list on authorship, so this
// is a sibling of MessageCard rather than a fifth arm inside it. A post of mine
// is a right-aligned ink bubble: no avatar, no hover bar, and a meta line that
// carries the send state instead of the small tick.
// ============================================================================

component OwnMessageRow(message:ChatMessage)
  row #root w=fill pt=3.0 pb=3.0 align=start
    space w=fill
    // the artifact caps the bubble at 74% of the pane; ice lengths are absolute,
    // so the cap is a fixed measure at the pane's design width.
    box max-w=520.0 pl=13.0 pr=13.0 pt=9.0 pb=9.0 bg=toast_bg r-tl=13.0 r-tr=13.0 r-br=4.0 r-bl=13.0
      col gap=4.0
        text message.body w=fill size=13.5 line-h=1.5 wrap=word @text-toast_fg
        // The artifact's meta row is `time` then the send state. Our stamp IS
        // the height, and FinalityChip already spells `✓ finalized · h N`, so
        // the chip is the whole line rather than the height printed twice.
        row w=fill gap=6.0 align=center
          space w=fill
          if message.pending
            FinalityChip phase="finalizing" height=0
          if !message.pending
            button label="Inspect event" p=0.0 @icon_action -> open_message_inspector(message.seq)
              FinalityChip phase="finalized" height=message.height
              active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
              hovered bg=fg/9 text=fg
              pressed bg=fg/13 text=fg

// One row of the message ⋯ menu: a 14px stroke glyph and its label. The button
// and its route stay at the call site so each item picks its own handler.
component MessageMenuItem(icon:str, label:str)
  row #root w=fill gap=9.0 align=center
    Icon name=icon tone="muted" px=14.0
    text label w=fill size=12.5 wrap=none @text-accent_fg

// THE REFUSED COMPOSER. `post_gate` names the refusal; this turns the token
// into the sentence, and every sentence names the move that clears it. An
// empty reason renders nothing at all — the composer itself is the else arm.
component ComposerGate(reason:str)
  col #root w=fill
    match reason
      "channel_archived"
        GateNote reason="This channel is archived — new messages are refused." next="Unarchive it from Channel details to post here again."
      "members_only"
        GateNote reason="This channel is members-only and your key is not on its roster." next="Ask a member to add your key from Channel details."
      _
        space w=1.0 h=1.0

// ============================================================================
// THREAD RAIL — a 50px header bar, the parent message in its own divided
// block, then the replies under their own count rule.
// ============================================================================

component ThreadPanelHeader(channel_name:str)
  col #root w=fill
    box w=fill h=50.0 pl=16.0 pr=16.0
      row w=fill h=fill gap=8.0 align=center
        text "Thread" size=13.0 wrap=none font=display @text-fg
        row w=fill gap=0.0 align=center
          text "#" size=11.0 wrap=none font=code_medium @text-caption
          text channel_name size=11.0 wrap=none font=code_medium @text-caption
        button "×" label="Close thread" w=24.0 h=24.0 p=0.0 @ghost_action -> close_thread
          active bg=transparent text=meta border=transparent border-w=1.0 r=6.0
          hovered bg=separator text=fg
          pressed bg=subtle text=fg
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

component ThreadParentBlock(message:ChatMessage)
  col #root w=fill
    row w=fill gap=10.0 align=start pb=14.0
      PrincipalAvatar initials=message.initial is_agent=(message.avatar_kind == "agent") plate=28.0 ink=10.0 ring=""
      col w=fill gap=2.0
        row w=fill gap=6.0 align=center
          text message.author size=12.5 wrap=none font=display @text-fg
          if message.height > 0
            text height_label_short(message.height) size=10.0 wrap=none font=code_semibold @text-hint
          space w=fill
        MessageBody message=message
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

component ThreadRepliesRule(count:i64)
  row #root w=fill gap=5.0 align=center pt=13.0 pb=4.0
    text count size=10.5 wrap=none font=code_medium @text-label
    text "replies" size=10.5 wrap=none font=code_medium @text-label

// ============================================================================
// EVENT INSPECTOR — the reduced panel. It answers the four questions the app
// can answer from what it holds: who wrote this, into which channel, at which
// height, and has it settled.
//
// DELIBERATELY ABSENT, and not stubbed: the 5-step consensus timeline (no
// per-height batch id, merkle root or round is reachable from any RPC), the
// quorum signer chips (no block certificate on the wire), Copy proof and
// Verify (nothing to copy and nothing to check it against), and the raw wire
// JSON — `ChatMessage` is a projection of the op, not the op, so printing a
// JSON-shaped block here would be a drawing of a record we never fetched.
// ============================================================================

component EventInspector(message:ChatMessage, channel_name:str)
  col #root w=fill h=fill
    box w=fill h=50.0 pl=16.0 pr=16.0
      row w=fill h=fill gap=8.0 align=center
        text "Event Inspector" w=fill size=13.0 wrap=none font=display @text-fg
        button "×" label="Close inspector" w=24.0 h=24.0 p=0.0 @ghost_action -> close_message_inspector
          active bg=transparent text=meta border=transparent border-w=1.0 r=6.0
          hovered bg=separator text=fg
          pressed bg=subtle text=fg
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0
    scroll dir=vertical w=fill h=fill
      col w=fill gap=7.0 p=16.0
        row w=fill gap=8.0 align=center
          text "chat.message" size=14.0 wrap=none font=code_semibold @text-primary
          if message.avatar_kind == "agent"
            box px=5.0 py=2.0 bg=primary r=4.0
              text "AGENT" size=9.0 wrap=none font=code_semibold @text-primary_fg
        row w=fill gap=5.0 align=center
          row gap=0.0 align=center
            text "#" size=10.5 wrap=none font=code_medium @text-hint
            text message.seq size=10.5 wrap=none font=code_medium @text-hint
          text "·" size=10.5 wrap=none font=code_medium @text-hint
          row gap=0.0 align=center
            text "#" size=10.5 wrap=none font=code_medium @text-hint
            text channel_name size=10.5 wrap=none font=code_medium @text-hint
        row w=fill gap=5.0 align=center
          text "author" size=11.0 wrap=none font=code_medium @text-input
          text message.author size=11.0 wrap=none font=code_medium @text-secondary_fg
        row w=fill gap=6.0 align=center pt=4.0
          if message.pending
            FinalityChip phase="finalizing" height=0
          if !message.pending
            FinalityChip phase="finalized" height=message.height
