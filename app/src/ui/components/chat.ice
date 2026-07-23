component ChannelButton(channel:ChatChannel, selected:bool)
  col width=fill
    if selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill height=fill spacing=9.0 align=center
          if channel.members_only
            text "◆" width=18.0 size=11.0 align-x=center @text-fg
          if !channel.members_only
            text "#" width=18.0 size=13.0 align-x=center font=medium @text-fg
          text channel.name width=fill size=13.0 wrapping=none font=medium @text-fg
          if channel.huddle_count > 0
            text channel.huddle_count size=11.0 @text-muted
        active bg=white/8 text=fg border=white/11 border-w=1.0 r=8.0
        hovered bg=white/11 text=fg border=white/14
        pressed bg=white/14 text=fg border=white/16
    if !selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill height=fill spacing=9.0 align=center
          if channel.members_only
            text "◆" width=18.0 size=11.0 align-x=center @text-muted
          if !channel.members_only
            text "#" width=18.0 size=13.0 align-x=center @text-muted
          text channel.name width=fill size=13.0 wrapping=none @text-muted
          if channel.archived
            text "archived" size=11.0 @text-muted
          if !channel.archived && channel.huddle_count > 0
            text channel.huddle_count size=11.0 @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
        hovered bg=white/6 text=fg border=white/8
        pressed bg=white/10 text=fg border=white/12

component ChatMemberRow(member:ChatMember, disabled:bool)
  row width=fill spacing=6.0 align=center
    text member.label width=fill size=11.0 @text-muted
    button "Remove" description=member.label disabled=disabled height=28.0 padding=5.0 -> remove_channel_member_submit(member.key)
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=white/8 text=fg border=white/10
      pressed bg=white/12 text=fg border=white/13

component MessageContents(message:ChatMessage)
  row width=fill spacing=9.0 align=start
    container width=30.0 height=30.0 align-x=center align-y=center bg=white/7 border=white/8 border-w=1.0 r=8.0
      text "•" size=13.0 @text-muted
    col width=fill spacing=3.0
      row width=fill spacing=6.0 align=center
        text message.author size=13.0 wrapping=none font=medium @text-fg
        text message.meta size=11.0 wrapping=none @text-muted
        space width=fill
      text message.body width=fill size=13.0 wrapping=word @text-fg
      if message.reply_count > 0 || !empty(message.reactions)
        row width=fill spacing=5.0 align=center
          if message.reply_count > 0
            button label="Open thread" padding=4.0 -> open_thread_for(message.seq)
              row spacing=4.0 align=center
                text "Thread" size=11.0 font=medium
                text message.reply_count size=11.0
              active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
              hovered bg=white/6 text=fg border=white/8
              pressed bg=white/10 text=fg border=white/12
          for reaction in message.reactions
            container padding=4.0 padding-left=7.0 padding-right=7.0 bg=transparent border=transparent border-w=1.0 r=7.0
              row spacing=4.0 align=center
                text reaction.emoji size=11.0 @text-fg
                text reaction.count size=11.0 @text-muted

component MessageCard(message:ChatMessage, selected:bool, hovered:bool, disabled:bool)
  mouse enter=message_entered(message.seq) exit=message_exited(message.seq)
    stack width=fill
      if message.deleted
        container width=fill padding=9.0 bg=transparent border=transparent border-w=1.0 r=8.0
          MessageContents message=message
      if !message.deleted && selected
        container width=fill padding=9.0 bg=white/7 border=white/9 border-w=1.0 r=8.0
          MessageContents message=message
      if !message.deleted && !selected && hovered
        container width=fill padding=9.0 bg=white/4 border=white/6 border-w=1.0 r=8.0
          MessageContents message=message
      if !message.deleted && !selected && !hovered
        container width=fill padding=9.0 bg=transparent border=transparent border-w=1.0 r=8.0
          MessageContents message=message
      if !message.deleted && !message.pending && !hovered
        container width=fill align-x=end align-y=start padding-top=2.0 padding-right=8.0
          button "…" label="More message actions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_actions_accessibly(message.seq, message.body, message.rev)
            active bg=transparent text=muted r=6.0
            hovered bg=white/9 text=fg
            pressed bg=white/13 text=fg
      if !message.deleted && !message.pending && hovered
        container width=fill align-x=end align-y=start padding-top=2.0 padding-right=8.0
          container padding=2.0 bg=popover border=white/13 border-w=1.0 r=7.0 shadow=black/16 shadow-y=2.0 shadow-blur=7.0
            row spacing=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_reactions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=5.0
                hovered bg=white/9 text=fg
                pressed bg=white/13 text=fg
              button "↳" label="Open thread" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_thread_for(message.seq)
                active bg=transparent text=muted r=5.0
                hovered bg=white/9 text=fg
                pressed bg=white/13 text=fg
              button "…" label="More message actions" disabled=disabled width=26.0 height=26.0 padding=4.0 -> open_message_actions(message.seq, message.body, message.rev)
                active bg=transparent text=muted r=5.0
                hovered bg=white/9 text=fg
                pressed bg=white/13 text=fg

component ThreadMessageCard(message:ChatMessage, selected:bool)
  stack width=fill
    if selected
      container width=fill padding=8.0 bg=white/6 border=white/8 border-w=1.0 r=8.0
        col width=fill spacing=3.0
          row width=fill spacing=7.0 align=center
            text message.author width=fill size=13.0 font=medium @text-fg
            text message.meta size=11.0 @text-muted
          text message.body width=fill size=13.0 wrapping=word @text-fg
    if !selected
      container width=fill padding=8.0 bg=transparent border=transparent border-w=1.0 r=8.0
        col width=fill spacing=3.0
          row width=fill spacing=7.0 align=center
            text message.author width=fill size=13.0 font=medium @text-fg
            text message.meta size=11.0 @text-muted
          text message.body width=fill size=13.0 wrapping=word @text-fg

component ChatSearchResult(hit:ChatSearchHit)
  button label=hit.text width=fill padding=7.0 -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
    col width=fill spacing=2.0
      row width=fill spacing=7.0 align=center
        text hit.author width=fill size=11.0 font=medium @text-fg
        text hit.meta size=11.0 @text-muted
      text hit.text width=fill size=13.0 wrapping=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=white/6 text=fg border=white/8
    pressed bg=white/10 text=fg border=white/12
