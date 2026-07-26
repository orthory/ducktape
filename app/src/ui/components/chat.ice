component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)
  col w=fill
    if selected
      button label=channel.name w=fill h=34.0 p=7.0 @ghost_action -> choose_channel(channel.id)
        row w=fill h=fill gap=9.0 align=center
          if channel.members_only
            text "◆" w=16.0 size=12.0 align-x=center @text-fg
          if !channel.members_only
            text "#" w=16.0 size=13.0 align-x=center font=medium @text-fg
          text channel.name w=fill size=13.0 wrap=none font=medium @text-fg
          if channel.huddle_count > 0
            text channel.huddle_count size=9.0 font=code_semibold @text-success
        active bg=subtle text=fg border=transparent border-w=1.0 r=9.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !selected
      button label=channel.name w=fill h=34.0 p=7.0 @ghost_action -> choose_channel(channel.id)
        row w=fill h=fill gap=9.0 align=center
          if channel.members_only && unread
            text "◆" w=16.0 size=12.0 align-x=center @text-brand
          if channel.members_only && !unread
            text "◆" w=16.0 size=12.0 align-x=center @text-muted
          if !channel.members_only && unread
            text "#" w=16.0 size=13.0 align-x=center font=medium @text-brand
          if !channel.members_only && !unread
            text "#" w=16.0 size=13.0 align-x=center @text-muted
          if unread
            text channel.name w=fill size=13.0 wrap=none font=medium @text-fg
          if !unread
            text channel.name w=fill size=13.0 wrap=none @text-muted
          if channel.archived
            box p=2.0 pl=6.0 pr=6.0 bg=fg/6 border=fg/12 border-w=1.0 r=6.0
              text "Archived" size=9.0 font=code_semibold @text-muted
          if !channel.archived && channel.huddle_count > 0
            text channel.huddle_count size=9.0 font=code_semibold @text-muted
          if unread
            box w=8.0 h=8.0 bg=brand r=4.0
              text ""
        active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
        hovered bg=fg/6 text=fg border=fg/9
        pressed bg=fg/10 text=fg border=fg/12

component ChatMemberRow(member:ChatMember, disabled:bool)
  row w=fill gap=6.0 align=center
    text member.label w=fill size=12.0 font=code @text-muted
    button "Remove" description=member.label disabled=disabled h=28.0 p=5.0 @danger_action -> remove_channel_member_submit(member.key)

component RichLine(block:ChatBlock)
  flex w=fill wrap=wrap gap-x=0.0 gap-y=4.0 items=start
    for span in block.spans
      if span.highlight
        text span.text size=13.5 line-h=1.45 font=medium @text-brand
      if !span.highlight && span.bold && span.italic
        text span.text size=13.5 line-h=1.45 font=strongitalic @text-fg
      if !span.highlight && span.bold && !span.italic
        text span.text size=13.5 line-h=1.45 font=strong @text-fg
      if !span.highlight && !span.bold && span.italic
        text span.text size=13.5 line-h=1.45 font=italic @text-fg
      if !span.highlight && !span.bold && !span.italic
        text span.text size=13.5 line-h=1.45 @text-fg

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
          text block.text w=fill size=13.5 line-h=1.45 wrap=word @text-fg

component MessageAvatar(initials:str, kind:str)
  stack #root w=30.0 h=30.0
    match kind
      "human"
        Avatar initials=initials
      "agent"
        Avatar.Agent initials=initials
      _
        Avatar.Agent initials=initials

component MessageContents(message:ChatMessage)
  row w=fill gap=11.0 align=start
    if message.show_author
      MessageAvatar initials=message.initial kind=message.avatar_kind
    if !message.show_author
      space w=30.0
    col w=fill gap=3.0
      if message.show_author
        row w=fill gap=7.0 align=center
          text message.author size=14.0 wrap=none font=display @text-fg
          text message.meta size=11.0 wrap=none font=code_medium @text-muted
          space w=fill
      MessageBody message=message
      if message.reply_count > 0 || !empty(message.reactions)
        row w=fill gap=6.0 align=center
          if message.reply_count > 0
            button label="Open thread" p=4.0 @ghost_action -> open_thread_for(message.seq)
              row gap=5.0 align=center
                text "Thread" size=13.0 font=medium
                text message.reply_count size=12.0 font=code
              active bg=brand/14 text=brand border=brand/24 border-w=1.0 r=8.0
              hovered bg=brand/22 text=fg border=brand/34
              pressed bg=brand/30 text=fg border=brand/40
          for reaction in message.reactions
            if reaction.reacted_by_me
              button label="Remove reaction" description=reaction.emoji p=0.0 @ghost_action -> remove_reaction_at(message.seq, reaction.emoji)
                box p=3.0 pl=8.0 pr=8.0
                  row gap=5.0 align=center
                    text reaction.emoji size=13.0 @text-fg
                    text reaction.count size=12.0 font=code @text-brand
                active bg=brand/18 text=fg border=brand/36 border-w=1.0 r=9.0
                hovered bg=brand/26 text=fg border=brand/46
                pressed bg=brand/32 text=fg
            if !reaction.reacted_by_me
              button label="Add reaction" description=reaction.emoji p=0.0 @ghost_action -> add_reaction_at(message.seq, reaction.emoji)
                box p=3.0 pl=8.0 pr=8.0
                  row gap=5.0 align=center
                    text reaction.emoji size=13.0 @text-fg
                    text reaction.count size=12.0 font=code @text-muted
                active bg=fg/6 text=fg border=fg/13 border-w=1.0 r=9.0
                hovered bg=fg/12 text=fg border=fg/18
                pressed bg=fg/16 text=fg

component MessageCard(message:ChatMessage, selected:bool, hovered:bool, disabled:bool)
  mouse enter=message_entered(message.seq) exit=message_exited(message.seq)
    stack w=fill
      if message.deleted
        box w=fill p=8.0 pl=10.0 pr=10.0 bg=transparent border=transparent border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && selected
        box w=fill p=8.0 pl=10.0 pr=10.0 bg=accent border=border border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !selected && hovered
        box w=fill p=8.0 pl=10.0 pr=10.0 bg=fg/4 border=fg/7 border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !selected && !hovered
        box w=fill p=8.0 pl=10.0 pr=10.0 bg=transparent border=transparent border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !message.pending && !hovered
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_message_actions_accessibly(message.seq, message.body, message.rev)
            active bg=transparent text=muted r=7.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/13 text=fg
      if !message.deleted && !message.pending && hovered
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          box p=2.0 style=raised_style()
            row gap=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg
              button "↳" label="Open thread" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_thread_for(message.seq)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg
              button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> open_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg

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
                box p=3.0 pl=8.0 pr=8.0
                  row gap=5.0 align=center
                    text reaction.emoji size=13.0 @text-fg
                    text reaction.count size=12.0 font=code @text-brand
                active bg=brand/18 text=fg border=brand/36 border-w=1.0 r=9.0
                hovered bg=brand/26 text=fg border=brand/46
                pressed bg=brand/32 text=fg
            if !reaction.reacted_by_me
              button label="Add reaction" description=reaction.emoji p=0.0 @ghost_action -> add_reaction_at(message.seq, reaction.emoji)
                box p=3.0 pl=8.0 pr=8.0
                  row gap=5.0 align=center
                    text reaction.emoji size=13.0 @text-fg
                    text reaction.count size=12.0 font=code @text-muted
                active bg=fg/6 text=fg border=fg/13 border-w=1.0 r=9.0
                hovered bg=fg/12 text=fg border=fg/18
                pressed bg=fg/16 text=fg

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
