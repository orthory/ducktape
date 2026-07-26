// A sidebar channel row: 7px radius, 8px side inset, the hash in `label` ink
// and the name in `list` weight. Selected is the same tint capsule the rail uses.
component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)
  box w=fill pl=8.0 pr=8.0
    col w=fill
      if selected
        button label=channel.name w=fill p=0.0 @ghost_action -> choose_channel(channel.id)
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
        button label=channel.name w=fill p=0.0 @ghost_action -> choose_channel(channel.id)
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
          text message.meta size=11.0 wrap=none font=code_medium @text-hint
          if !message.pending
            text "✓" size=10.0 wrap=none font=code_semibold @text-success_tick
          space w=fill
      MessageBody message=message
      if message.reply_count > 0 || !empty(message.reactions)
        row w=fill gap=6.0 align=center
          if message.reply_count > 0
            button label="Open thread" p=0.0 @ghost_action -> open_thread_for(message.seq)
              box pl=7.0 pr=9.0 pt=3.0 pb=3.0
                row gap=6.0 align=center
                  Icon name="nav-chat" tone="accent" px=12.0
                  text message.reply_count size=12.0 wrap=none font=code_medium @text-brand
                  text "replies" size=12.0 wrap=none @text-brand
              active bg=surface text=brand border=border border-w=1.0 r=8.0
              hovered bg=muted_bg text=brand border=control_line
              pressed bg=elevated text=brand
          for reaction in message.reactions
            if reaction.reacted_by_me
              button label="Remove reaction" description=reaction.emoji p=0.0 @ghost_action -> remove_reaction_at(message.seq, reaction.emoji)
                box pl=6.0 pr=8.0 pt=1.0 pb=1.0
                  row gap=4.0 align=center
                    text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
                    text reaction.count size=12.0 wrap=none font=code_medium @text-brand
                active bg=brand_bg text=brand border=brand_line border-w=1.0 r=11.0
                hovered bg=brand_bg text=brand border=brand
                pressed bg=elevated text=brand border=brand
            if !reaction.reacted_by_me
              button label="Add reaction" description=reaction.emoji p=0.0 @ghost_action -> add_reaction_at(message.seq, reaction.emoji)
                box pl=6.0 pr=8.0 pt=1.0 pb=1.0
                  row gap=4.0 align=center
                    text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
                    text reaction.count size=12.0 wrap=none font=code_medium @text-muted
                active bg=surface text=muted border=border border-w=1.0 r=11.0
                hovered bg=muted_bg text=fg border=control_line
                pressed bg=elevated text=fg

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
          box p=2.0 bg=surface border=border border-w=1.0 r=13.0 shadow=shadow_toast shadow-y=6.0 shadow-blur=18.0
            row gap=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> open_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button label="Open thread" disabled=disabled w=27.0 h=25.0 p=0.0 @ghost_action -> open_thread_for(message.seq)
                box w=fill h=fill align-x=center align-y=center
                  Icon name="nav-chat" tone="muted" px=15.0
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button label="Inspect event" disabled=disabled w=27.0 h=25.0 p=0.0 @ghost_action -> open_message_actions(message.seq, message.body, message.rev)
                box w=fill h=fill align-x=center align-y=center
                  Icon name="shield" tone="muted" px=15.0
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "⋯" label="More message actions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> open_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg

component ThreadMessageBody(message:ChatMessage)
  row w=fill gap=10.0 align=start
    MessageAvatar initials=message.initial kind=message.avatar_kind
    col w=fill gap=3.0
      row w=fill gap=6.0 align=center
        text message.author size=14.0 wrap=none font=display @text-fg
        text message.meta size=11.0 wrap=none font=code_medium @text-muted
        space w=fill
      MessageBody message=message
      if !empty(message.reactions)
        row w=fill gap=5.0 align=center
          for reaction in message.reactions
            if reaction.reacted_by_me
              button label="Remove reaction" description=reaction.emoji p=0.0 @ghost_action -> remove_reaction_at(message.seq, reaction.emoji)
                box pl=6.0 pr=8.0 pt=1.0 pb=1.0
                  row gap=4.0 align=center
                    text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
                    text reaction.count size=12.0 wrap=none font=code_medium @text-brand
                active bg=brand_bg text=brand border=brand_line border-w=1.0 r=11.0
                hovered bg=brand_bg text=brand border=brand
                pressed bg=elevated text=brand border=brand
            if !reaction.reacted_by_me
              button label="Add reaction" description=reaction.emoji p=0.0 @ghost_action -> add_reaction_at(message.seq, reaction.emoji)
                box pl=6.0 pr=8.0 pt=1.0 pb=1.0
                  row gap=4.0 align=center
                    text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
                    text reaction.count size=12.0 wrap=none font=code_medium @text-muted
                active bg=surface text=muted border=border border-w=1.0 r=11.0
                hovered bg=muted_bg text=fg border=control_line
                pressed bg=elevated text=fg

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
