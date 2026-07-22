component ChannelButton(channel:ChatChannel, selected:bool)
  col width=fill
    if selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill spacing=9.0 align=center
          if channel.members_only
            text "◇" width=18.0 size=12.0 align-x=center @text-fg font-bold
          if !channel.members_only
            text "#" width=18.0 size=13.0 align-x=center @text-fg font-bold
          text channel.name width=fill size=12.0 wrapping=none @text-fg font-bold
          if channel.huddle_count > 0
            text channel.huddle_count size=10.0 @text-muted
        active bg=linear(2.3, white/78@0.0, surface/58@1.0) text=fg border=white/78 border-w=1.0 r=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed bg=selection
    if !selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill spacing=9.0 align=center
          if channel.members_only
            text "◇" width=18.0 size=12.0 align-x=center @text-muted
          if !channel.members_only
            text "#" width=18.0 size=13.0 align-x=center @text-muted
          text channel.name width=fill size=12.0 wrapping=none @text-muted
          if channel.archived
            text "archived" size=10.0 @text-muted
          if !channel.archived && channel.huddle_count > 0
            text channel.huddle_count size=10.0 @text-muted
        active bg=transparent text=muted r=10.0
        hovered bg=white/34 text=fg
        pressed bg=selection text=fg

component MessageContents(message:ChatMessage)
  col width=fill spacing=4.0
    row width=fill spacing=8.0 align=center
      text message.author width=fill size=12.0 @font-bold text-fg
      text message.meta size=10.0 @text-muted
    text message.body width=fill size=13.0 wrapping=word @text-fg
    if message.reply_count > 0 || !empty(message.reactions)
      row width=fill spacing=5.0 align=center
        if message.reply_count > 0
          container padding=4.0 padding-left=7.0 padding-right=7.0 bg=white/38 border=white/55 border-w=1.0 r=8.0
            row spacing=4.0 align=center
              text "Thread" size=10.0 @font-bold text-muted
              text message.reply_count size=10.0 @text-muted
        for reaction in message.reactions
          container padding=4.0 padding-left=7.0 padding-right=7.0 bg=white/38 border=white/55 border-w=1.0 r=8.0
            row spacing=4.0 align=center
              text reaction.emoji size=10.0 @text-fg
              text reaction.count size=10.0 @text-muted

component MessageCard(message:ChatMessage, selected:bool)
  col width=fill
    if message.deleted
      container width=fill padding=8.0 bg=transparent border=transparent border-w=1.0 r=10.0
        MessageContents message=message
    if !message.deleted && selected
      button label=message.body width=fill padding=8.0 -> select_message(message.seq, message.body, message.rev)
        MessageContents message=message
        active bg=linear(2.3, white/70@0.0, surface/52@1.0) text=fg border=white/72 border-w=1.0 r=10.0
        hovered bg=white/74 text=fg
        pressed bg=selection text=fg
    if !message.deleted && !selected
      button label=message.body width=fill padding=8.0 -> select_message(message.seq, message.body, message.rev)
        MessageContents message=message
        active bg=transparent text=fg border=transparent border-w=1.0 r=10.0
        hovered bg=white/34 text=fg border=white/42
        pressed bg=selection text=fg

component ThreadMessageCard(message:ChatMessage)
  container width=fill padding=8.0 bg=transparent r=8.0
    col width=fill spacing=3.0
      row width=fill spacing=7.0 align=center
        text message.author width=fill size=11.0 @font-bold text-fg
        text message.meta size=10.0 @text-muted
      text message.body width=fill size=12.0 wrapping=word @text-fg

component ChatSearchResult(hit:ChatSearchHit)
  button label=hit.text width=fill padding=7.0 -> open_chat_search_hit(hit.channel_id)
    col width=fill spacing=2.0
      row width=fill spacing=7.0 align=center
        text hit.author width=fill size=10.0 @font-bold text-fg
        text hit.meta size=10.0 @text-muted
      text hit.text width=fill size=11.0 wrapping=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=white/48 text=fg border=white/52
    pressed bg=selection text=fg

