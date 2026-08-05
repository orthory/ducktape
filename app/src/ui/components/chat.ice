// A sidebar channel row: 7px radius, 8px side inset, the hash in `label` ink
// and the name in `list` weight. Selected is the same tint capsule the rail uses.
component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)
  emits
    choose_channel(str)
  box w=fill pl=8.0 pr=8.0
    col w=fill
      if selected
        button label=channel.name w=fill p=0.0 @icon_action -> emit(choose_channel, channel.id)
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
        button label=channel.name w=fill p=0.0 @icon_action -> emit(choose_channel, channel.id)
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

// One roster row of the details drawer: the key label in machine ink and a
// quiet ×; the danger reading arrives on hover, not as a red button per row.
component ChatMemberRow(member:ChatMember, disabled:bool)
  emits
    remove_channel_member_submit(str)
  row w=fill gap=6.0 align=center
    text member.label w=fill size=11.5 wrap=none font=code @text-accent_fg
    button label="Remove member" description=member.label disabled=disabled w=24.0 h=24.0 p=0.0 @icon_action -> emit(remove_channel_member_submit, member.key)
      box w=fill h=fill align-x=center align-y=center
        text "×" size=13.0 wrap=none @text-muted
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=danger_bg text=fg
      pressed bg=danger_line text=fg

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
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
  col #root
    if reaction.reacted_by_me
      button label="Remove reaction" description=reaction.emoji p=0.0 @icon_action -> emit(remove_reaction_at, seq, reaction.emoji)
        box pl=6.0 pr=8.0 pt=1.0 pb=1.0
          row gap=4.0 align=center
            text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
            text reaction.count size=11.0 wrap=none font=code_medium @text-brand
        active bg=tree_selected text=brand border=brand_line border-w=1.0 r=11.0
        hovered bg=tree_selected text=brand border=brand
        pressed bg=warning_plate text=brand border=brand
    if !reaction.reacted_by_me
      button label="Add reaction" description=reaction.emoji p=0.0 @icon_action -> emit(add_reaction_at, seq, reaction.emoji)
        box pl=6.0 pr=8.0 pt=1.0 pb=1.0
          row gap=4.0 align=center
            text reaction.emoji size=11.0 wrap=none font=code_medium @text-fg
            text reaction.count size=11.0 wrap=none font=code_medium @text-muted
        active bg=muted_bg text=muted border=card_line border-w=1.0 r=11.0
        hovered bg=elevated text=fg border=control_line
        pressed bg=subtle text=fg

component MessageContents(message:ChatMessage, flash:f64)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
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
          space w=fill
      MessageBody message=message
      // Reactions and the replies button STACK — the artifact gives each its
      // own line under the body, never one shared row.
      if !empty(message.reactions)
        flex w=fill wrap=wrap gap-x=5.0 gap-y=5.0 items=start pt=6.0
          for reaction in message.reactions
            ReactionChip reaction=reaction seq=message.seq
              forward
                add_reaction_at
                remove_reaction_at
      if message.reply_count > 0
        row w=fill gap=6.0 align=center pt=6.0
          button label="Open thread" p=0.0 @icon_action -> emit(open_thread_for, message.seq)
            box pl=7.0 pr=9.0 pt=3.0 pb=3.0
              row gap=6.0 align=center
                Icon name="nav-chat" tone="accent" px=12.0
                text message.reply_count size=11.0 wrap=none font=code_medium @text-brand
                text "replies" size=11.0 wrap=none font=code_medium @text-brand
            active bg=surface text=brand border=border border-w=1.0 r=8.0
            hovered bg=muted_bg text=brand border=control_line
            pressed bg=elevated text=brand
    // The quiet send-state lane at the row's right edge: an in-flight row
    // carries a small dot, the settle swaps it for a ✓ that fades out (the
    // `flash` prop is the animation's opacity — nonzero only on the row the
    // stream's flash arm anchors). Replaces the old "finalizing…" chip line
    // that restyled the whole message.
    if message.pending
      svg icon("dot") memory w=7.0 h=7.0 style=icon_tint("hint") opacity=0.55
    if !message.pending && flash > 0.0
      svg icon("check") memory w=13.0 h=13.0 style=icon_tint("success-tick") opacity=flash

component MessageCard(message:ChatMessage, selected:bool, disabled:bool, flash:f64)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_reactions(i64, str, i64)
    open_message_actions(i64, str, i64)
  // DRAW-TIME HOVER: the tint and the toolbar reveal ride the `hover`
  // widget's cursor check — no enter/exit routes, no hovered state, no view
  // rebuild per row crossing. A cached lazy row keeps its toolbar at native
  // latency; that is the whole point (the state-driven version left the
  // highlight trailing the cursor by the queued rebuilds).
  hover tint=row_hover r=9.0
    stack w=fill
      if message.deleted
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=transparent border=transparent border-w=1.0 r=9.0
          MessageContents message=message flash=flash
            forward
              add_reaction_at
              remove_reaction_at
              open_thread_for
      // Selection is a tint, not a ring — see the QA note in the stream.
      if !message.deleted && selected
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=brand_bg border=transparent border-w=1.0 r=9.0
          MessageContents message=message flash=flash
            forward
              add_reaction_at
              remove_reaction_at
              open_thread_for
      if !message.deleted && !selected
        box w=fill pl=7.0 pr=7.0 pt=6.0 pb=6.0 bg=transparent border=transparent border-w=1.0 r=9.0
          MessageContents message=message flash=flash
            forward
              add_reaction_at
              remove_reaction_at
              open_thread_for
    col w=fill
      if !message.deleted && !message.pending
        box w=fill align-x=end align-y=start pr=8.0
          // The designer's own opaque answer for this bar: white card, 1px
          // border, r9 over the 3/12 popover shadow.
          box p=2.0 bg=surface border=border border-w=1.0 r=9.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
            row gap=1.0 align=center
              button "👍" label="React with 👍" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> emit(add_reaction_at, message.seq, "👍")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "✅" label="React with ✅" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> emit(add_reaction_at, message.seq, "✅")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "👀" label="React with 👀" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> emit(add_reaction_at, message.seq, "👀")
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              box w=1.0 h=16.0 bg=subtle
                space w=1.0 h=1.0
              button "♡" label="Manage reactions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> emit(open_message_reactions, message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button label="Open thread" disabled=disabled p=5.0 @icon_action -> emit(open_thread_for, message.seq)
                Icon name="nav-chat" tone="muted" px=15.0
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
              button "⋯" label="More message actions" disabled=disabled w=27.0 h=25.0 p=4.0 @ghost_action -> emit(open_message_actions, message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
      if message.deleted || message.pending
        space w=1.0 h=1.0

// A reply reads one notch SMALLER than the same message in the timeline: 26px
// plate, 12px author, and the stamp a step down. The body keeps the shared
// block renderer — `MessageBody` is also mounted by the forge discussion, so
// its scale is not this component's to change.
component ThreadMessageBody(message:ChatMessage)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
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
              forward
                add_reaction_at
                remove_reaction_at

component ThreadMessageCard(message:ChatMessage, selected:bool, disabled:bool)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_message_actions(i64, str, i64)
    open_thread_message_reactions(i64, str, i64)
  // Same draw-time hover as MessageCard — see the note there.
  hover tint=row_hover r=9.0
    stack w=fill
      if message.deleted
        box w=fill p=8.0 bg=transparent border=transparent border-w=1.0 r=9.0
          ThreadMessageBody message=message
            forward
              add_reaction_at
              remove_reaction_at
      if !message.deleted && selected
        box w=fill p=8.0 bg=brand_bg border=transparent border-w=1.0 r=9.0
          ThreadMessageBody message=message
            forward
              add_reaction_at
              remove_reaction_at
      if !message.deleted && !selected
        box w=fill p=8.0 bg=transparent border=transparent border-w=1.0 r=9.0
          ThreadMessageBody message=message
            forward
              add_reaction_at
              remove_reaction_at
    col w=fill
      if !message.deleted && !message.pending
        box w=fill align-x=end align-y=start pt=3.0 pr=9.0
          box p=2.0 style=raised_style()
            row gap=1.0 align=center
              button "♡" label="Manage reactions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> emit(open_thread_message_reactions, message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg
              button "…" label="More message actions" disabled=disabled w=26.0 h=26.0 p=4.0 @ghost_action -> emit(open_thread_message_actions, message.seq, message.body, message.rev)
                active bg=transparent text=muted r=6.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14 text=fg
      if message.deleted || message.pending
        space w=1.0 h=1.0

component ChatSearchResult(hit:ChatSearchHit)
  emits
    open_chat_search_hit(str, i64, i64)
  button label=hit.text w=fill p=8.0 @ghost_action -> emit(open_chat_search_hit, hit.channel_id, hit.root_seq, hit.seq)
    col w=fill gap=3.0
      row w=fill gap=7.0 align=center
        text hit.author w=fill size=13.0 font=medium @text-fg
        text hit.meta size=11.0 font=code_medium @text-muted
      text hit.text w=fill size=13.5 wrap=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
    hovered bg=fg/6 text=fg border=fg/9
    pressed bg=fg/10 text=fg border=fg/13

// THE REFUSED COMPOSER. `post_gate` names the refusal; this turns the token
// into the sentence, and every sentence names the move that clears it. An
// empty reason renders nothing at all — the composer itself is the else arm.
//
// MOUNTED above the composer plate (view.ice), as the VIEW half of frozen
// contract item `post_gate(archived, members_only, members, me) -> String`. The
// composer used to grey the editor on `active_channel_archived` alone and say
// NOTHING at all when a members-only channel refused the viewer's key.
//
// `post_gate` is CALLED at the mount, not mirrored into a state field: it is
// pure over facts the view already holds, and `channel_members` lands in seven
// handlers — a mirrored field would be seven assignments and six chances to
// drift. Its reason is now the whole gate, so `active_channel_archived` no
// longer appears in the editor's or Send's `disabled=`: the archived case IS
// the `channel_archived` arm, and one discriminant beats two.
//
// `me` is `settings_user_key` — full-hex, from `SettingsFacts`, which is what a
// `ChatMember.key` carries. There was no such fact when this component landed:
// `account_id` is a short_label of the identity module's ACCOUNT id and cannot
// be compared with a membership row, so the key was added to SettingsFacts
// rather than guessed at from the account card.
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
// THREAD RAIL — the artifact opens the rail body with the root message in its
// own divided block, at a type scale one notch under a reply.
//
// The rail's 50px HEADER BAR is not a component: it is drawn inline by view.ice
// (view.ice:600), it is a pane header — which main.rs pins as a view.ice-only
// shape, never a second header inside a component file — and closing its ledger
// gap also moves the 300px plate to 330px `bg=sidebar` with a 16px body inset,
// all attributes of the box in view.ice. The artifact's values for whoever owns
// that edit: 50px plate, `pl=16 pr=16`, bottom 1px `separator` hairline,
// `Thread` at 13.0 `font=display`, then `#<channel>` at 11.0 `@text-caption`,
// then a right-aligned 24×24 r6 `×`. Note the artifact has NO reply count in
// the header and no "Thread result" title — view.ice ships both today, and the
// second one is the only signpost a chat-search hit has, so the port must keep
// it rather than follow the artifact literally.
// ============================================================================

// MOUNTED — the view half of ledger gap "Thread rail gives the parent message
// its own divided block", closed by splitting the rail's existing loop in
// view.ice on `thread_message.seq == active_thread_seq`. No state field and no
// backend fn: `active_thread_seq` IS the root's seq and `thread_messages`
// carries the root.
//
// THE TRADE, TAKEN DELIBERATELY: the artifact's parent block is READ-ONLY, so
// the root loses the hover bar, reactions and edit/delete that ThreadMessageCard
// gave it in the rail. It does NOT lose them from the product — the rail is a
// 300px pane BESIDE the stream, never over it, so the same message is on screen
// in the stream at the same moment with its full MessageCard toolbar. The cost
// is one extra glance, not a capability.
//
// NO `N replies` RULE between the block and the replies. It was built, deleted,
// and stays deleted: its count wants `thread_messages` minus the root, which is
// the loaded page and not the reply count whenever `thread_has_more` holds.
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

// The artifact's `N replies` rule between the parent block and the replies was
// built here and is deleted: two `text` widgets are markup, not a component, and
// it could not have told the truth anyway. Its count wanted `thread_messages`
// MINUS the root, and no fn returns that — `len(thread_messages)` (the count
// view.ice already shows in the rail header) is one too many for a reply count.
// Whoever mounts ThreadParentBlock gets the rule for free in the same view.ice
// edit, from the loop's own arms; the artifact draws it at 10.5 `font=code_medium`
// `@text-label` with `pt=13.0 pb=4.0`.

// The EVENT INSPECTOR was built here and mounted nowhere, so it is deleted
// rather than left as a panel no user can reach. What it needed was a lookup
// from `inspector_seq` back to a `ChatMessage`, which no backend fn provides —
// inventing one during an integration pass is how a surface ends up drawing a
// record nobody fetched. `OwnMessageRow` went with it: its finality chip was
// the panel's only remaining route and the row itself was never mounted either.
// The stale `MY OWN POSTS` banner that survived that deletion is gone too — it
// described a right-aligned own-message bubble no arm of MessageCard draws.

// `MessageMenuItem` (a 14px icon + its label) was built here for the message ⋯
// menu and is deleted. The menu it was shaped for is the ARTIFACT's — Inspect
// event / Reply in thread / Copy link / Copy proof / Pin to channel — and this
// app can honestly serve exactly one of those five: there is no inspector rail
// (see the empty shield seat in MessageCard), no `ducktape://` link builder, no
// op hash to copy as a proof, and no pin. A shared row component earns its keep
// by repetition, and the repetition here was four surfaces that do not exist.
// The menu the app actually ships is React / Edit / Delete / Close, inline in
// view.ice:659, and its real gap is that those four rows carry no icon. That is
// one `Icon name=… tone="muted" px=14.0` added to four buttons in view.ice, in
// the file that owns them. The artifact's row metrics for that edit: `gap=9.0`,
// label 12.5 `@text-accent_fg`, `p=8.0`/`pl=9.0`, `r=7.0`, on a 184px plate.
