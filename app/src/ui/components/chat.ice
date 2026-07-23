component ChannelButton(channel:ChatChannel, selected:bool)
  col width=fill
    if selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill height=fill spacing=9.0 align=center
          if channel.members_only
            text "◆" width=16.0 size=11.0 align-x=center @text-primary
          if !channel.members_only
            text "#" width=16.0 size=15.0 align-x=center font=medium @text-primary
          text channel.name width=fill size=14.0 wrapping=none font=medium @text-fg
          if channel.huddle_count > 0
            text channel.huddle_count size=11.0 font=medium @text-primary
        active bg=primary/16 text=fg border=primary/26 border-w=1.0 r=9.0
        hovered bg=primary/22 text=fg border=primary/34
        pressed bg=primary/30 text=fg border=primary/40
    if !selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill height=fill spacing=9.0 align=center
          if channel.members_only
            text "◆" width=16.0 size=11.0 align-x=center @text-muted
          if !channel.members_only
            text "#" width=16.0 size=15.0 align-x=center @text-muted
          text channel.name width=fill size=14.0 wrapping=none @text-muted
          if channel.archived
            container padding=2.0 padding-left=6.0 padding-right=6.0 bg=white/6 border=white/12 border-w=1.0 r=6.0
              text "Archived" size=11.0 font=medium @text-muted
          if !channel.archived && channel.huddle_count > 0
            text channel.huddle_count size=11.0 @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
        hovered bg=white/6 text=fg border=white/9
        pressed bg=white/10 text=fg border=white/12

component ChatMemberRow(member:ChatMember, disabled:bool)
  row width=fill spacing=6.0 align=center
    text member.label width=fill size=11.0 font=mono @text-muted
    button "Remove" description=member.label disabled=disabled height=28.0 padding=5.0 -> remove_channel_member_submit(member.key)
      active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
      hovered bg=danger/18 text=fg border=danger/28
      pressed bg=danger/26 text=fg border=danger/34

component RichLine(block:ChatBlock)
  flex width=fill wrap column-gap=0.0 row-gap=4.0 align-items=start
    for span in block.spans
      if span.highlight
        text span.text size=14.0 line-height=1.45 font=medium @text-primary
      if !span.highlight && span.bold && span.italic
        text span.text size=14.0 line-height=1.45 font=strongitalic @text-fg
      if !span.highlight && span.bold && !span.italic
        text span.text size=14.0 line-height=1.45 font=display @text-fg
      if !span.highlight && !span.bold && span.italic
        text span.text size=14.0 line-height=1.45 font=italic @text-fg
      if !span.highlight && !span.bold && !span.italic
        text span.text size=14.0 line-height=1.45 @text-fg

component MessageBody(message:ChatMessage)
  col width=fill spacing=5.0
    for block in message.blocks
      if block.kind == "divider"
        container width=fill height=1.0 bg=separator
          text ""
      if block.kind == "code"
        container width=fill padding=11.0 bg=black/26 border=white/11 border-w=1.0 r=9.0
          col width=fill spacing=5.0
            if !empty(block.lang)
              text block.lang size=11.0 wrapping=none font=medium @text-muted
            text block.text width=fill size=13.0 line-height=1.5 font=mono wrapping=word @text-fg
      if block.kind == "quote"
        container width=fill padding=9.0 padding-left=13.0 bg=primary/9 border=primary/20 border-w=1.0 r=8.0
          col width=fill
            if block.rich
              RichLine block=block
            if !block.rich
              text block.text width=fill size=14.0 line-height=1.45 wrapping=word @text-fg
      if block.kind == "paragraph"
        if block.rich
          RichLine block=block
        if !block.rich
          text block.text width=fill size=14.0 line-height=1.45 wrapping=word @text-fg

component MessageContents(message:ChatMessage)
  row width=fill spacing=11.0 align=start
    if message.show_author
      container width=36.0 height=36.0 align-x=center align-y=center style=avatar_style(message.avatar_r, message.avatar_g, message.avatar_b)
        text message.initial size=15.0 font=display @text-fg
    if !message.show_author
      space width=36.0
    col width=fill spacing=3.0
      if message.show_author
        row width=fill spacing=7.0 align=center
          text message.author size=14.0 wrapping=none font=display @text-fg
          text message.meta size=11.0 wrapping=none @text-muted
          space width=fill
      MessageBody message=message
      if message.reply_count > 0 || !empty(message.reactions)
        row width=fill spacing=6.0 align=center
          if message.reply_count > 0
            button label="Open thread" padding=4.0 -> open_thread_for(message.seq)
              row spacing=5.0 align=center
                text "Thread" size=11.0 font=medium
                text message.reply_count size=11.0
              active bg=primary/14 text=primaryhi border=primary/24 border-w=1.0 r=8.0
              hovered bg=primary/22 text=fg border=primary/34
              pressed bg=primary/30 text=fg border=primary/40
          for reaction in message.reactions
            container padding=3.0 padding-left=8.0 padding-right=8.0 bg=white/6 border=white/13 border-w=1.0 r=9.0
              row spacing=5.0 align=center
                text reaction.emoji size=13.0 @text-fg
                text reaction.count size=11.0 font=medium @text-muted

component MessageCard(message:ChatMessage, selected:bool, hovered:bool, disabled:bool)
  mouse enter=message_entered(message.seq) exit=message_exited(message.seq)
    stack width=fill
      if message.deleted
        container width=fill padding=8.0 padding-left=10.0 padding-right=10.0 bg=transparent border=transparent border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && selected
        container width=fill padding=8.0 padding-left=10.0 padding-right=10.0 bg=primary/10 border=primary/22 border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !selected && hovered
        container width=fill padding=8.0 padding-left=10.0 padding-right=10.0 bg=white/4 border=white/7 border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !selected && !hovered
        container width=fill padding=8.0 padding-left=10.0 padding-right=10.0 bg=transparent border=transparent border-w=1.0 r=10.0
          MessageContents message=message
      if !message.deleted && !message.pending && !hovered
        container width=fill align-x=end align-y=start padding-top=3.0 padding-right=9.0
          button "…" label="More message actions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_actions_accessibly(message.seq, message.body, message.rev)
            active bg=transparent text=muted r=7.0
            hovered bg=white/9 text=fg
            pressed bg=white/13 text=fg
      if !message.deleted && !message.pending && hovered
        container width=fill align-x=end align-y=start padding-top=3.0 padding-right=9.0
          container padding=2.0 bg=popover border=white/14 border-w=1.0 r=9.0 shadow=black/24 shadow-y=2.0 shadow-blur=9.0
            row spacing=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=white/10 text=fg
                pressed bg=white/14 text=fg
              button "↳" label="Open thread" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_thread_for(message.seq)
                active bg=transparent text=muted r=6.0
                hovered bg=white/10 text=fg
                pressed bg=white/14 text=fg
              button "…" label="More message actions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=white/10 text=fg
                pressed bg=white/14 text=fg

component ThreadMessageBody(message:ChatMessage)
  row width=fill spacing=10.0 align=start
    container width=32.0 height=32.0 align-x=center align-y=center style=avatar_style(message.avatar_r, message.avatar_g, message.avatar_b)
      text message.initial size=14.0 font=display @text-fg
    col width=fill spacing=3.0
      row width=fill spacing=6.0 align=center
        text message.author size=13.0 wrapping=none font=display @text-fg
        text message.meta size=11.0 wrapping=none @text-muted
        space width=fill
      MessageBody message=message
      if !empty(message.reactions)
        row width=fill spacing=5.0 align=center
          for reaction in message.reactions
            container padding=3.0 padding-left=8.0 padding-right=8.0 bg=white/6 border=white/13 border-w=1.0 r=9.0
              row spacing=5.0 align=center
                text reaction.emoji size=13.0 @text-fg
                text reaction.count size=11.0 font=medium @text-muted

component ThreadMessageCard(message:ChatMessage, selected:bool)
  stack width=fill
    if selected
      container width=fill padding=8.0 bg=primary/10 border=primary/22 border-w=1.0 r=9.0
        ThreadMessageBody message=message
    if !selected
      container width=fill padding=8.0 bg=transparent border=transparent border-w=1.0 r=9.0
        ThreadMessageBody message=message

component ChatSearchResult(hit:ChatSearchHit)
  button label=hit.text width=fill padding=8.0 -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
    col width=fill spacing=3.0
      row width=fill spacing=7.0 align=center
        text hit.author width=fill size=11.0 font=medium @text-fg
        text hit.meta size=11.0 @text-muted
      text hit.text width=fill size=13.0 wrapping=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
    hovered bg=white/6 text=fg border=white/9
    pressed bg=white/10 text=fg border=white/13
