font ui family=sans weight=normal stretch=normal style=normal default=true
font mono family="monospace" weight=normal stretch=normal style=normal

app Ducktape
  title "Ducktape"
  theme app_theme
  background app_background
  text-color app_text
  id "dev.ducktape.app"
  default-text-size 14
  antialiasing true
  window
    size 1120 720
    min-size 820 540
    position centered
    transparent true
    blur true
    platform macos
      title-hidden true
      titlebar-transparent true
      fullsize-content-view true

extern crate::backend
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64)
  ChatMessage(id:str, seq:i64, author:str, meta:str, body:str, pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, reactions:[ChatReaction])
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str)
  ThreadData(root_seq:i64, messages:[ChatMessage])
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  PageBlock(id:str, parent:str, kind:str, text:str, pending:bool, checked:bool, prefix:str, child_count:i64, mark_count:i64)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  LiveUpdate(kind:str, status:str, height:i64)
  AppError(message:str)
  HydrationError(generation:i64, message:str)
  connect(rpc:str) -> WorkspaceData ! AppError
  stream live_events(rpc:str) -> LiveUpdate
  refresh(rpc:str, channel_id:str, page_id:str, generation:i64) -> WorkspaceData ! HydrationError
  retry_refresh(rpc:str, channel_id:str, page_id:str, generation:i64, attempt:i64) -> WorkspaceData ! HydrationError
  sync optimistic_message(messages:[ChatMessage], body:str) -> [ChatMessage]
  sync rollback_messages(messages:[ChatMessage]) -> [ChatMessage]
  sync optimistic_block(blocks:[PageBlock], kind:str, text:str) -> [PageBlock]
  sync rollback_blocks(blocks:[PageBlock]) -> [PageBlock]
  sync restore_draft(current:str, pending:str) -> str
  load_chat(rpc:str, channel_id:str) -> ChatData ! AppError
  create_channel(rpc:str, password:str, name:str) -> ChatData ! AppError
  send_message(rpc:str, password:str, channel_id:str, body:str) -> ChatData ! AppError
  load_thread(rpc:str, channel_id:str, root_seq:i64) -> ThreadData ! AppError
  send_reply(rpc:str, password:str, channel_id:str, root_seq:i64, body:str) -> ThreadData ! AppError
  edit_message(rpc:str, password:str, channel_id:str, seq:i64, base_rev:i64, body:str) -> ChatData ! AppError
  delete_message(rpc:str, password:str, channel_id:str, seq:i64) -> ChatData ! AppError
  add_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> ChatData ! AppError
  load_page(rpc:str, page_id:str) -> PagesData ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  create_child_page(rpc:str, password:str, parent:str, title:str) -> PagesData ! AppError
  rename_page(rpc:str, password:str, page_id:str, title:str) -> PagesData ! AppError
  move_page_top(rpc:str, password:str, page_id:str) -> PagesData ! AppError
  delete_page(rpc:str, password:str, page_id:str) -> PagesData ! AppError
  add_block(rpc:str, password:str, page_id:str, after_id:str, kind:str, text:str) -> PagesData ! AppError
  save_block(rpc:str, password:str, page_id:str, block_id:str, kind:str, text:str) -> PagesData ! AppError
  set_block_checked(rpc:str, password:str, page_id:str, block_id:str, checked:bool) -> PagesData ! AppError
  move_block(rpc:str, password:str, page_id:str, block_id:str, direction:str) -> PagesData ! AppError
  remove_block(rpc:str, password:str, page_id:str, block_id:str) -> PagesData ! AppError

theme
  background #d8d8d880
  surface    #f2f2f2a6
  sidebar    #e8e8e896
  elevated   #ffffffb8
  foreground #202020
  muted      #686868
  primary    #383838
  danger     #4a4a4a
  success    #5c5c5c
  border     #ffffffb3
  subtle     #ffffff66
  selection  #ffffffa8
  separator  #68686833
  shadow     #00000026

state
  app_theme = "app"
  app_background = "#d8d8d880"
  app_text = "#202020"
  rpc = ""
  connected_rpc = ""
  password = ""
  status = "Connecting…"
  connected = false
  loading = false
  block_height:i64 = -1
  sync_phase = "idle"
  hydration_generation:i64 = 0
  hydration_retry_attempt:i64 = 0
  mutation_phase = "idle"
  live_dirty = false
  error = ""
  channels:[ChatChannel] = []
  messages:[ChatMessage] = []
  active_channel = ""
  active_channel_name = ""
  selected_message_seq:i64 = 0
  selected_message_rev:i64 = 0
  message_edit_draft = ""
  active_thread_seq:i64 = 0
  thread_messages:[ChatMessage] = []
  thread_loading = false
  reply_draft = ""
  pending_reply = ""
  channel_draft = ""
  pending_channel = ""
  message_draft = ""
  pending_message = ""
  pages:[PageItem] = []
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_draft = ""
  pending_page = ""
  subpage_draft = ""
  pending_subpage = ""
  block_kinds = ["Text", "Heading 1", "Heading 2", "Heading 3", "Bullet", "Number", "Todo", "Toggle", "Quote", "Code", "Callout", "Divider"]
  new_block_kind = "Text"
  block_draft = ""
  pending_block = ""
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  page_delete_armed = false
  block_delete_armed = false

component Brand()
  row width=fill spacing=9.0 align=center
    container width=28.0 height=28.0 align-x=center align-y=center background=linear(2.3, white/85@0.0, surface/55@1.0) border=white/90 border-width=1.0 radius=8.0 shadow=black/12 shadow-y=2.0 shadow-blur=8.0
      text "D" size=13.0 @font-bold text-foreground
    col width=fill spacing=0.0
      text "Ducktape" size=13.0 @font-bold text-foreground
      text "Workspace" size=10.0 @text-muted

component ChannelButton(channel:ChatChannel, selected:bool)
  col width=fill
    if selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill spacing=9.0 align=center
          if channel.members_only
            text "◇" width=18.0 size=12.0 align-x=center @text-foreground font-bold
          if !channel.members_only
            text "#" width=18.0 size=13.0 align-x=center @text-foreground font-bold
          text channel.name width=fill size=12.0 wrapping=none @text-foreground font-bold
          if channel.huddle_count > 0
            text channel.huddle_count size=10.0 @text-muted
        active background=linear(2.3, white/78@0.0, surface/58@1.0) text=foreground border=white/78 border-width=1.0 radius=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed background=selection
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
        active background=transparent text=muted radius=10.0
        hovered background=white/34 text=foreground
        pressed background=selection text=foreground

component PageButton(page:PageItem, selected:bool)
  col width=fill
    if selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-foreground
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=12.0 wrapping=none @text-foreground font-bold
          if page.child_count > 0
            text page.child_count size=10.0 @text-muted
        active background=linear(2.3, white/78@0.0, surface/58@1.0) text=foreground border=white/78 border-width=1.0 radius=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed background=selection
    if !selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-muted
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=12.0 wrapping=none @text-muted
          if page.child_count > 0
            text page.child_count size=10.0 @text-muted
        active background=transparent text=muted radius=10.0
        hovered background=white/34 text=foreground
        pressed background=selection text=foreground

component MessageContents(message:ChatMessage)
  col width=fill spacing=4.0
    row width=fill spacing=8.0 align=center
      text message.author width=fill size=12.0 @font-bold text-foreground
      text message.meta size=10.0 @text-muted
    text message.body width=fill size=13.0 wrapping=word @text-foreground
    if message.reply_count > 0 || !empty(message.reactions)
      row width=fill spacing=5.0 align=center
        if message.reply_count > 0
          container padding=4.0 padding-left=7.0 padding-right=7.0 background=white/38 border=white/55 border-width=1.0 radius=8.0
            row spacing=4.0 align=center
              text "Thread" size=10.0 @font-bold text-muted
              text message.reply_count size=10.0 @text-muted
        for reaction in message.reactions
          container padding=4.0 padding-left=7.0 padding-right=7.0 background=white/38 border=white/55 border-width=1.0 radius=8.0
            row spacing=4.0 align=center
              text reaction.emoji size=10.0 @text-foreground
              text reaction.count size=10.0 @text-muted

component MessageCard(message:ChatMessage, selected:bool)
  col width=fill
    if message.deleted
      container width=fill padding=8.0 background=transparent border=transparent border-width=1.0 radius=10.0
        MessageContents message=message
    if !message.deleted && selected
      button label=message.body width=fill padding=8.0 -> select_message(message.seq, message.body, message.rev)
        MessageContents message=message
        active background=linear(2.3, white/70@0.0, surface/52@1.0) text=foreground border=white/72 border-width=1.0 radius=10.0
        hovered background=white/74 text=foreground
        pressed background=selection text=foreground
    if !message.deleted && !selected
      button label=message.body width=fill padding=8.0 -> select_message(message.seq, message.body, message.rev)
        MessageContents message=message
        active background=transparent text=foreground border=transparent border-width=1.0 radius=10.0
        hovered background=white/34 text=foreground border=white/42
        pressed background=selection text=foreground

component ThreadMessageCard(message:ChatMessage)
  container width=fill padding=8.0 background=transparent radius=8.0
    col width=fill spacing=3.0
      row width=fill spacing=7.0 align=center
        text message.author width=fill size=11.0 @font-bold text-foreground
        text message.meta size=10.0 @text-muted
      text message.body width=fill size=12.0 wrapping=word @text-foreground

component BlockContents(block:PageBlock)
  row width=fill spacing=7.0 align=start
    text block.prefix size=11.0 wrapping=none @text-muted
    match block.kind
      "Bullet"
        text "•" width=16.0 size=13.0 align-x=center @text-muted
      "Number"
        text "1." width=16.0 size=11.0 align-x=center @text-muted
      "Todo"
        if block.checked
          text "✓" width=16.0 size=11.0 align-x=center @font-bold text-foreground
        if !block.checked
          text "○" width=16.0 size=12.0 align-x=center @text-muted
      "Toggle"
        text "›" width=16.0 size=15.0 align-x=center @text-muted
      "Quote"
        text "│" width=16.0 size=15.0 align-x=center @text-muted
      "Code"
        text "{}" width=16.0 size=10.0 align-x=center font=mono @text-muted
      "Callout"
        text "!" width=16.0 size=10.0 align-x=center @font-bold text-muted
      _
        space width=0.0
    col width=fill spacing=2.0
      match block.kind
        "Heading 1"
          text block.text width=fill size=20.0 wrapping=word @font-bold text-foreground
        "Heading 2"
          text block.text width=fill size=17.0 wrapping=word @font-bold text-foreground
        "Heading 3"
          text block.text width=fill size=15.0 wrapping=word @font-bold text-foreground
        "Code"
          container width=fill padding=7.0 background=foreground/7 border=white/48 border-width=1.0 radius=7.0
            text block.text width=fill size=11.0 wrapping=word font=mono @text-foreground
        "Divider"
          container width=fill height=1.0 background=separator
            text ""
        _
          text block.text width=fill size=13.0 wrapping=word @text-foreground
      if block.child_count > 0 || block.mark_count > 0
        row width=fill spacing=7.0 align=center
          if block.child_count > 0
            text block.child_count size=10.0 @text-muted
          if block.mark_count > 0
            text "Formatted" size=10.0 @text-muted

component BlockCard(block:PageBlock, selected:bool)
  col width=fill
    if block.pending
      container width=fill padding=8.0 background=white/24 border=transparent border-width=1.0 radius=9.0
        BlockContents block=block
    if !block.pending && selected
      button label=block.kind width=fill padding=8.0 -> select_block(block.id, block.kind, block.text, block.checked)
        BlockContents block=block
        active background=linear(2.3, white/68@0.0, surface/48@1.0) text=foreground border=white/70 border-width=1.0 radius=9.0
        hovered background=white/72 text=foreground
        pressed background=selection text=foreground
    if !block.pending && !selected
      button label=block.kind width=fill padding=8.0 -> select_block(block.id, block.kind, block.text, block.checked)
        BlockContents block=block
        active background=transparent text=foreground border=transparent border-width=1.0 radius=9.0
        hovered background=white/30 text=foreground border=white/38
        pressed background=selection text=foreground

component EmptyState(title:str, detail:str)
  container width=fill height=fill align-x=center align-y=center
    col spacing=6.0 align=center
      container width=34.0 height=34.0 align-x=center align-y=center background=subtle radius=8.0
        text "·" size=22.0 @text-foreground
      text title size=15.0 @font-bold text-foreground
      text detail size=12.0 @text-muted

component WorkspaceTabs(status:str, loading:bool)
  state
    tab = "chat"
  on select_tab(next)
    tab = next
  container width=fill height=fill clip=true background=linear(2.35, white/48@0.0, background/78@0.55, surface/58@1.0) border=white/65 border-width=1.0 radius=20.0 shadow=black/18 shadow-y=8.0 shadow-blur=28.0 pixel-snap=true
    row width=fill height=fill
      container width=242.0 height=fill padding=12.0 padding-top=38.0 background=linear(2.25, white/62@0.0, sidebar/80@0.48, background/66@1.0) border=white/58 border-width=1.0 radius-tr=18.0 radius-br=18.0 shadow=black/10 shadow-x=4.0 shadow-blur=18.0 clip=true
        col width=fill height=fill spacing=8.0
          Brand
          space height=6.0
          container width=fill padding-left=8.0
            text "APPS" size=10.0 @font-bold text-muted
          match tab
            "chat"
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill spacing=9.0 align=center
                    text "#" width=18.0 size=14.0 align-x=center @font-bold text-foreground
                    text "Chat" width=fill size=12.0 @font-bold text-foreground
                  active background=linear(2.3, white/85@0.0, surface/66@1.0) text=foreground border=white/85 border-width=1.0 radius=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=8.0
                  pressed background=selection
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill spacing=9.0 align=center
                    text "□" width=18.0 size=14.0 align-x=center @text-muted
                    text "Pages" width=fill size=12.0 @text-muted
                  active background=transparent text=muted radius=10.0
                  hovered background=white/38 text=foreground
                  pressed background=selection text=foreground
            _
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill spacing=9.0 align=center
                    text "#" width=18.0 size=14.0 align-x=center @text-muted
                    text "Chat" width=fill size=12.0 @text-muted
                  active background=transparent text=muted radius=10.0
                  hovered background=white/38 text=foreground
                  pressed background=selection text=foreground
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill spacing=9.0 align=center
                    text "□" width=18.0 size=14.0 align-x=center @font-bold text-foreground
                    text "Pages" width=fill size=12.0 @font-bold text-foreground
                  active background=linear(2.3, white/85@0.0, surface/66@1.0) text=foreground border=white/85 border-width=1.0 radius=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=8.0
                  pressed background=selection
          container width=fill height=1.0 background=separator
            text ""
          match tab
            "chat"
              slot chat_sidebar
            _
              slot pages_sidebar
          slot connection
          row width=fill spacing=7.0 padding=7.0 align=center
            container width=7.0 height=7.0 background=foreground/55 radius=3.5
              text ""
            if loading
              text "Working…" width=fill size=10.0 wrapping=none @text-muted
            if !loading
              text status width=fill size=10.0 wrapping=none @text-muted
      col width=fill height=fill
        container width=fill padding=12.0 padding-left=16.0
          row width=fill height=38.0 spacing=12.0 align=center
            match tab
              "chat"
                col spacing=0.0
                  text "Chat" size=17.0 @font-bold text-foreground
                  text "Workspace conversations" size=10.0 @text-muted
              _
                col spacing=0.0
                  text "Pages" size=17.0 @font-bold text-foreground
                  text "Shared documents" size=10.0 @text-muted
        slot notice
        col width=fill height=fill padding=12.0 padding-top=4.0
          match tab
            "chat"
              slot chat
            _
              slot pages

on mount
  loading = true
  run connect(rpc) -> workspace_connected _ | failed _

on reconnect
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  connected = false
  error = ""
  status = "Connecting…"
  run connect(trim(rpc)) -> workspace_connected _ | failed _

on workspace_connected(next)
  rpc = next.rpc
  connected_rpc = next.rpc
  status = next.status
  block_height = next.height
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  connected = true
  loading = false
  mutation_phase = "idle"
  sync_phase = "idle"
  hydration_retry_attempt = 0
  error = ""

on workspace_refreshed(next)
  return if next.generation != hydration_generation
  return if sync_phase != "refreshing"
  sync_phase = "idle"
  hydration_retry_attempt = 0
  status = next.status
  block_height = next.height
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_parent = next.active_page_parent
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on live_updated(next)
  status = next.status
  return if next.kind == "retrying"
  live_dirty = true
  return if loading || mutation_phase != "idle" || sync_phase == "refreshing"
  live_dirty = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on refresh_failed(cause)
  return if cause.generation != hydration_generation
  return if sync_phase != "refreshing"
  status = "Sync delayed"
  error = cause.message
  hydration_retry_attempt = hydration_retry_attempt + 1
  run retry_refresh(connected_rpc, active_channel, active_page, hydration_generation, hydration_retry_attempt) -> workspace_refreshed _ | refresh_failed _

subscribe
  run live_events(connected_rpc) when connected -> live_updated _

on choose_channel(id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""
  active_thread_seq = 0
  thread_messages = []
  reply_draft = ""
  pending_reply = ""
  error = ""
  run load_chat(connected_rpc, id) -> chat_updated _ | failed _

on create_channel_submit
  return if loading || mutation_phase != "idle" || empty(trim(channel_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel"
  pending_channel = trim(channel_draft)
  channel_draft = ""
  error = ""
  run create_channel(connected_rpc, password, pending_channel) -> chat_mutated _ | mutation_failed _

on send_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(message_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message"
  pending_message = trim(message_draft)
  message_draft = ""
  messages = optimistic_message(messages, pending_message)
  error = ""
  run send_message(connected_rpc, password, active_channel, pending_message) -> chat_mutated _ | mutation_failed _

on chat_updated(next)
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on chat_mutated(next)
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""
  active_thread_seq = 0
  thread_messages = []
  reply_draft = ""
  pending_reply = ""
  pending_channel = ""
  pending_message = ""
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on select_message(seq, body, rev)
  return if seq <= 0 || mutation_phase != "idle"
  selected_message_seq = seq
  selected_message_rev = rev
  message_edit_draft = body

on clear_message_selection
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""

on open_thread
  return if thread_loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0
  thread_loading = true
  error = ""
  run load_thread(connected_rpc, active_channel, selected_message_seq) -> thread_loaded _ | thread_failed _

on thread_loaded(next)
  active_thread_seq = next.root_seq
  thread_messages = next.messages
  thread_loading = false
  reply_draft = ""
  pending_reply = ""
  error = ""

on thread_failed(cause)
  thread_loading = false
  error = cause.message

on close_thread
  active_thread_seq = 0
  thread_messages = []
  thread_loading = false
  reply_draft = ""
  pending_reply = ""

on edit_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || empty(trim(message_edit_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-edit"
  error = ""
  run edit_message(connected_rpc, password, active_channel, selected_message_seq, selected_message_rev, trim(message_edit_draft)) -> chat_mutated _ | mutation_failed _

on delete_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-delete"
  error = ""
  run delete_message(connected_rpc, password, active_channel, selected_message_seq) -> chat_mutated _ | mutation_failed _

on add_reaction_submit(emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reaction"
  error = ""
  run add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> chat_mutated _ | mutation_failed _

on send_reply_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_thread_seq <= 0 || empty(trim(reply_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reply"
  pending_reply = trim(reply_draft)
  reply_draft = ""
  thread_messages = optimistic_message(thread_messages, pending_reply)
  error = ""
  run send_reply(connected_rpc, password, active_channel, active_thread_seq, pending_reply) -> thread_mutated _ | mutation_failed _

on thread_mutated(next)
  active_thread_seq = next.root_seq
  thread_messages = next.messages
  pending_reply = ""
  mutation_phase = "idle"
  live_dirty = false
  error = ""
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, id) -> pages_updated _ | failed _

on create_page_submit
  return if loading || mutation_phase != "idle" || empty(trim(page_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page"
  pending_page = trim(page_draft)
  page_draft = ""
  error = ""
  run create_page(connected_rpc, password, pending_page) -> pages_mutated _ | mutation_failed _

on create_child_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(trim(subpage_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "subpage"
  pending_subpage = trim(subpage_draft)
  subpage_draft = ""
  error = ""
  run create_child_page(connected_rpc, password, active_page, pending_subpage) -> pages_mutated _ | mutation_failed _

on rename_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(trim(active_page_title))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page-title"
  error = ""
  run rename_page(connected_rpc, password, active_page, trim(active_page_title)) -> pages_mutated _ | mutation_failed _

on move_page_top_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(active_page_parent)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page-parent"
  error = ""
  run move_page_top(connected_rpc, password, active_page) -> pages_mutated _ | mutation_failed _

on arm_page_delete
  return if loading || mutation_phase != "idle" || empty(active_page)
  page_delete_armed = true

on delete_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || !page_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page-delete"
  page_delete_armed = false
  error = ""
  run delete_page(connected_rpc, password, active_page) -> pages_mutated _ | mutation_failed _

on new_block_kind_changed(next)
  new_block_kind = next

on add_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || (new_block_kind != "Divider" && empty(trim(block_draft)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block"
  pending_block = trim(block_draft)
  block_draft = ""
  blocks = optimistic_block(blocks, new_block_kind, pending_block)
  error = ""
  run add_block(connected_rpc, password, active_page, selected_block_id, new_block_kind, pending_block) -> pages_mutated _ | mutation_failed _

on select_block(id, kind, text, checked)
  return if mutation_phase != "idle"
  selected_block_id = id
  selected_block_kind = kind
  selected_block_checked = checked
  block_edit_draft = text
  block_delete_armed = false

on selected_block_kind_changed(next)
  selected_block_kind = next

on clear_block_selection
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  block_delete_armed = false

on save_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id) || (selected_block_kind != "Divider" && empty(trim(block_edit_draft)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-save"
  error = ""
  run save_block(connected_rpc, password, active_page, selected_block_id, selected_block_kind, trim(block_edit_draft)) -> pages_mutated _ | mutation_failed _

on toggle_block_checked
  return if loading || mutation_phase != "idle" || selected_block_kind != "Todo" || empty(selected_block_id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-check"
  error = ""
  run set_block_checked(connected_rpc, password, active_page, selected_block_id, !selected_block_checked) -> pages_mutated _ | mutation_failed _

on move_block_submit(direction)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-move"
  error = ""
  run move_block(connected_rpc, password, active_page, selected_block_id, direction) -> pages_mutated _ | mutation_failed _

on arm_block_delete
  return if loading || mutation_phase != "idle" || empty(selected_block_id)
  block_delete_armed = true

on remove_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id) || !block_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-delete"
  block_delete_armed = false
  error = ""
  run remove_block(connected_rpc, password, active_page, selected_block_id) -> pages_mutated _ | mutation_failed _

on pages_updated(next)
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on pages_mutated(next)
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  pending_subpage = ""
  pending_block = ""
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  page_delete_armed = false
  block_delete_armed = false
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on mutation_failed(cause)
  mutation_phase = "idle"
  channel_draft = restore_draft(channel_draft, pending_channel)
  message_draft = restore_draft(message_draft, pending_message)
  page_draft = restore_draft(page_draft, pending_page)
  subpage_draft = restore_draft(subpage_draft, pending_subpage)
  block_draft = restore_draft(block_draft, pending_block)
  reply_draft = restore_draft(reply_draft, pending_reply)
  messages = rollback_messages(messages)
  thread_messages = rollback_messages(thread_messages)
  blocks = rollback_blocks(blocks)
  pending_channel = ""
  pending_message = ""
  pending_page = ""
  pending_subpage = ""
  pending_block = ""
  pending_reply = ""
  error = cause.message
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on dismiss_error
  error = ""

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  sync_phase = "idle"
  status = "Offline"
  error = cause.message

view
  WorkspaceTabs status=status loading=(loading || mutation_phase != "idle") #workspace-tabs
    connection:
      col width=fill spacing=5.0
        text "CONNECTION" size=10.0 @font-bold text-muted
        input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || mutation_phase != "idle") submit=reconnect width=fill padding=8.0 text-size=12.0 line-height=1.2
          active background=white/52 border=white/72 value=foreground placeholder=muted selection=foreground/18 border-width=1.0 radius=9.0
          hovered background=white/62 border=white/88
          focused background=white/72 border=foreground/45 border-width=1.0
          disabled background=white/28 value=muted
        input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") width=fill padding=8.0 text-size=12.0 line-height=1.2
          active background=white/52 border=white/72 value=foreground placeholder=muted selection=foreground/18 border-width=1.0 radius=9.0
          hovered background=white/62 border=white/88
          focused background=white/72 border=foreground/45 border-width=1.0
          disabled background=white/28 value=muted
        button "Connect" disabled=(loading || mutation_phase != "idle") width=fill height=30.0 padding=7.0 -> reconnect
          active background=linear(2.3, foreground/92@0.0, primary/96@1.0) text=white border=white/32 border-width=1.0 radius=10.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
          hovered background=foreground/82 text=white
          pressed background=foreground text=white
          disabled background=foreground/36 text=white/65
    chat_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "CHANNELS" width=fill size=10.0 @font-bold text-muted
          text len(channels) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for channel in channels
              ChannelButton channel=channel selected=(channel.id == active_channel)
        container width=fill padding=5.0 background=white/34 border=white/55 border-width=1.0 radius=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || !connected) submit=create_channel_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=8.0
              focused background=white/45 border=white/72 border-width=1.0
              disabled value=muted
            button "+" label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) width=28.0 height=28.0 padding=5.0 -> create_channel_submit
              active background=white/62 text=foreground border=white/78 border-width=1.0 radius=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered background=white/82
              pressed background=selection
              disabled background=white/24 text=muted
    pages_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "PAGES" width=fill size=10.0 @font-bold text-muted
          text len(pages) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
        container width=fill padding=5.0 background=white/34 border=white/55 border-width=1.0 radius=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New page title" <-> page_draft hint="New page" disabled=(loading || !connected) submit=create_page_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=8.0
              focused background=white/45 border=white/72 border-width=1.0
              disabled value=muted
            button "+" label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) width=28.0 height=28.0 padding=5.0 -> create_page_submit
              active background=white/62 text=foreground border=white/78 border-width=1.0 radius=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered background=white/82
              pressed background=selection
              disabled background=white/24 text=muted
    notice:
      col width=fill
        if error != ""
          container width=fill padding-left=12.0 padding-right=12.0 padding-bottom=8.0
            container width=fill padding=8.0 background=linear(2.3, white/78@0.0, surface/60@1.0) border=white/82 border-width=1.0 radius=12.0 shadow=black/10 shadow-y=2.0 shadow-blur=10.0
              row width=fill spacing=8.0 align=center
                container width=20.0 height=20.0 align-x=center align-y=center background=foreground/82 radius=10.0
                  text "!" size=10.0 @font-bold text-white
                text error width=fill size=10.0 @text-foreground
                button "Dismiss" padding=5.0 style=text -> dismiss_error
    chat:
      container width=fill height=fill padding=14.0 background=linear(2.35, white/76@0.0, elevated/64@0.5, surface/54@1.0) border=white/78 border-width=1.0 radius=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true pixel-snap=true
        row width=fill height=fill spacing=10.0
          col width=fill height=fill spacing=9.0
            if !empty(active_channel)
              row width=fill height=26.0 spacing=7.0 align=center
                container width=22.0 height=22.0 align-x=center align-y=center background=white/52 border=white/72 border-width=1.0 radius=7.0
                  text "#" size=11.0 @font-bold text-foreground
                text active_channel_name width=fill size=12.0 @font-bold text-foreground
                text len(messages) size=10.0 @text-muted
            if empty(messages)
              EmptyState title="No messages yet" detail="Create a channel or start the conversation."
            if !empty(messages)
              scroll direction=vertical width=fill height=fill bar=hidden
                col width=fill spacing=1.0
                  for message in messages
                    MessageCard message=message selected=(message.seq == selected_message_seq)
            if selected_message_seq > 0
              container width=fill padding=7.0 background=linear(2.3, white/58@0.0, surface/38@1.0) border=white/62 border-width=1.0 radius=12.0
                col width=fill spacing=6.0
                  row width=fill spacing=5.0 align=center
                    text "Message actions" width=fill size=10.0 @font-bold text-muted
                    button "Thread" disabled=(thread_loading || mutation_phase != "idle") height=26.0 padding=6.0 -> open_thread
                      active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                      hovered background=white/72
                      pressed background=selection
                      disabled background=white/22 text=muted
                    button "👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle") width=28.0 height=26.0 padding=5.0 -> add_reaction_submit("👍")
                      active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                      hovered background=white/72
                      pressed background=selection
                      disabled background=white/22 text=muted
                    button "♥" label="Add heart reaction" disabled=(mutation_phase != "idle") width=28.0 height=26.0 padding=5.0 -> add_reaction_submit("❤️")
                      active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                      hovered background=white/72
                      pressed background=selection
                      disabled background=white/22 text=muted
                    button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=6.0 -> delete_message_submit
                      active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                      hovered background=white/72
                      pressed background=selection
                      disabled background=white/22 text=muted
                    button "×" label="Close message actions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=5.0 -> clear_message_selection
                      active background=transparent text=muted radius=8.0
                      hovered background=white/56 text=foreground
                      pressed background=selection
                  row width=fill spacing=6.0 align=center
                    input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active background=white/38 border=white/55 value=foreground placeholder=muted selection=foreground/18 border-width=1.0 radius=8.0
                      focused background=white/62 border=foreground/42
                      disabled value=muted
                    button "Save" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) height=28.0 padding=6.0 -> edit_message_submit
                      active background=foreground/88 text=white border=white/26 border-width=1.0 radius=9.0
                      hovered background=foreground/78
                      pressed background=foreground
                      disabled background=foreground/24 text=white/58
            container width=fill padding=6.0 background=linear(2.3, white/64@0.0, surface/42@1.0) border=white/72 border-width=1.0 radius=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
              flex width=fill gap=6.0 align-items=center
                input "" #message label="Message" <-> message_draft hint="Write a message…" disabled=(loading || !connected || empty(active_channel)) submit=send_message_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                  active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=9.0
                  focused background=white/38 border=white/66 border-width=1.0
                  disabled value=muted
                button "Send" disabled=(loading || mutation_phase != "idle" || !connected || empty(active_channel) || empty(trim(message_draft))) height=30.0 padding=7.0 -> send_message_submit
                  active background=foreground/90 text=white border=white/28 border-width=1.0 radius=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                  hovered background=foreground/80 text=white
                  pressed background=foreground text=white
                  disabled background=foreground/28 text=white/60
          if active_thread_seq > 0
            container width=286.0 height=fill padding=10.0 background=linear(2.35, white/62@0.0, surface/44@1.0) border=white/68 border-width=1.0 radius=13.0 shadow=black/8 shadow-y=2.0 shadow-blur=12.0
              col width=fill height=fill spacing=8.0
                row width=fill height=26.0 spacing=6.0 align=center
                  text "Thread" width=fill size=12.0 @font-bold text-foreground
                  text len(thread_messages) size=10.0 @text-muted
                  button "×" label="Close thread" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=5.0 -> close_thread
                    active background=transparent text=muted radius=8.0
                    hovered background=white/56 text=foreground
                    pressed background=selection
                container width=fill height=1.0 background=separator
                  text ""
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for thread_message in thread_messages
                      ThreadMessageCard message=thread_message
                container width=fill padding=5.0 background=white/36 border=white/58 border-width=1.0 radius=11.0
                  row width=fill spacing=5.0 align=center
                    input "" #reply label="Thread reply" <-> reply_draft hint="Reply…" disabled=(mutation_phase != "idle") submit=send_reply_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=8.0
                      focused background=white/42 border=white/64 border-width=1.0
                      disabled value=muted
                    button "Send" label="Send reply" disabled=(mutation_phase != "idle" || empty(trim(reply_draft))) height=28.0 padding=6.0 -> send_reply_submit
                      active background=foreground/88 text=white border=white/26 border-width=1.0 radius=9.0
                      hovered background=foreground/78
                      pressed background=foreground
                      disabled background=foreground/24 text=white/58
    pages:
      container width=fill height=fill padding=16.0 background=linear(2.35, white/78@0.0, elevated/64@0.48, surface/52@1.0) border=white/80 border-width=1.0 radius=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true pixel-snap=true
        col width=fill height=fill
          if empty(active_page)
            EmptyState title="No page selected" detail="Create a page to begin writing."
          if !empty(active_page)
            col width=fill height=fill spacing=9.0
              row width=fill spacing=7.0 align=center
                input "" #page-title label="Page title" <-> active_page_title hint="Untitled" disabled=(loading || !connected) submit=rename_page_submit width=fill padding=7.0 text-size=17.0 line-height=1.2
                  active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=9.0
                  hovered background=white/24
                  focused background=white/42 border=white/68 border-width=1.0
                  disabled value=muted
                button "Save" label="Save title" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(active_page_title))) height=34.0 padding=7.0 -> rename_page_submit
                  active background=white/58 text=foreground border=white/76 border-width=1.0 radius=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=7.0
                  hovered background=white/78
                  pressed background=selection
                  disabled background=white/22 text=muted
              container width=fill padding=6.0 background=white/28 border=white/52 border-width=1.0 radius=11.0
                row width=fill spacing=6.0 align=center
                  input "" #subpage label="New subpage title" <-> subpage_draft hint="New subpage" disabled=(loading || !connected) submit=create_child_page_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                    active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=8.0
                    focused background=white/42 border=white/64 border-width=1.0
                    disabled value=muted
                  button "Add subpage" disabled=(mutation_phase != "idle" || empty(trim(subpage_draft))) height=28.0 padding=6.0 -> create_child_page_submit
                    active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                    hovered background=white/70
                    pressed background=selection
                    disabled background=white/20 text=muted
                  if !empty(active_page_parent)
                    button "Move top" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> move_page_top_submit
                      active background=white/48 text=foreground border=white/62 border-width=1.0 radius=8.0
                      hovered background=white/70
                      pressed background=selection
                      disabled background=white/20 text=muted
                  if !page_delete_armed
                    button "Delete" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> arm_page_delete
                      active background=transparent text=muted border=white/48 border-width=1.0 radius=8.0
                      hovered background=white/58 text=foreground
                      pressed background=selection
                  if page_delete_armed
                    button "Confirm delete" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> delete_page_submit
                      active background=foreground/86 text=white border=white/24 border-width=1.0 radius=8.0
                      hovered background=foreground/76
                      pressed background=foreground
              container width=fill height=1.0 background=separator
                text ""
              if empty(blocks)
                EmptyState title="An empty page" detail="Add the first block below."
              if !empty(blocks)
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for block in blocks
                      BlockCard block=block selected=(block.id == selected_block_id)
              if !empty(selected_block_id)
                container width=fill padding=7.0 background=linear(2.3, white/58@0.0, surface/38@1.0) border=white/62 border-width=1.0 radius=12.0
                  col width=fill spacing=6.0
                    row width=fill spacing=5.0 align=center
                      pick block_kinds some(selected_block_kind) placeholder="Block type" width=124.0 menu-height=210.0 padding=6.0 text-size=11.0 line-height=1.2 -> selected_block_kind_changed _
                        active text=foreground placeholder=muted handle=muted background=white/42 border=white/58 border-width=1.0 radius=8.0
                        hovered text=foreground placeholder=muted handle=foreground background=white/58 border=white/72 border-width=1.0 radius=8.0
                        opened text=foreground placeholder=muted handle=foreground background=white/66 border=white/76 border-width=1.0 radius=8.0
                        menu text=foreground selected-text=foreground selected-background=white/78 background=surface border=white/72 border-width=1.0 radius=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                      button "↑" label="Move block up" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("up")
                        active background=white/44 text=foreground border=white/58 border-width=1.0 radius=8.0
                        hovered background=white/68
                        pressed background=selection
                      button "↓" label="Move block down" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("down")
                        active background=white/44 text=foreground border=white/58 border-width=1.0 radius=8.0
                        hovered background=white/68
                        pressed background=selection
                      button "→" label="Indent block" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("indent")
                        active background=white/44 text=foreground border=white/58 border-width=1.0 radius=8.0
                        hovered background=white/68
                        pressed background=selection
                      button "←" label="Outdent block" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("outdent")
                        active background=white/44 text=foreground border=white/58 border-width=1.0 radius=8.0
                        hovered background=white/68
                        pressed background=selection
                      if selected_block_kind == "Todo"
                        button "Check" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> toggle_block_checked
                          active background=white/44 text=foreground border=white/58 border-width=1.0 radius=8.0
                          hovered background=white/68
                          pressed background=selection
                      space width=fill
                      if !block_delete_armed
                        button "Delete" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> arm_block_delete
                          active background=transparent text=muted border=white/46 border-width=1.0 radius=8.0
                          hovered background=white/58 text=foreground
                          pressed background=selection
                      if block_delete_armed
                        button "Confirm" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> remove_block_submit
                          active background=foreground/86 text=white border=white/24 border-width=1.0 radius=8.0
                          hovered background=foreground/76
                          pressed background=foreground
                      button "×" label="Close block editor" disabled=(mutation_phase != "idle") width=27.0 height=27.0 padding=5.0 -> clear_block_selection
                        active background=transparent text=muted radius=8.0
                        hovered background=white/56 text=foreground
                        pressed background=selection
                    row width=fill spacing=6.0 align=center
                      input "" #block-edit label="Edit block" <-> block_edit_draft hint="Block text" disabled=(mutation_phase != "idle" || selected_block_kind == "Divider") submit=save_block_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                        active background=white/38 border=white/55 value=foreground placeholder=muted selection=foreground/18 border-width=1.0 radius=8.0
                        focused background=white/62 border=foreground/42
                        disabled background=white/20 value=muted
                      button "Save" disabled=(mutation_phase != "idle" || (selected_block_kind != "Divider" && empty(trim(block_edit_draft)))) height=28.0 padding=6.0 -> save_block_submit
                        active background=foreground/88 text=white border=white/26 border-width=1.0 radius=9.0
                        hovered background=foreground/78
                        pressed background=foreground
                        disabled background=foreground/24 text=white/58
              container width=fill padding=6.0 background=linear(2.3, white/64@0.0, surface/42@1.0) border=white/72 border-width=1.0 radius=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
                row width=fill spacing=6.0 align=center
                  pick block_kinds some(new_block_kind) placeholder="Block type" width=124.0 menu-height=210.0 padding=7.0 text-size=11.0 line-height=1.2 -> new_block_kind_changed _
                    active text=foreground placeholder=muted handle=muted background=transparent border=transparent border-width=0.0 radius=8.0
                    hovered text=foreground placeholder=muted handle=foreground background=white/38 border=white/55 border-width=1.0 radius=8.0
                    opened text=foreground placeholder=muted handle=foreground background=white/52 border=white/68 border-width=1.0 radius=8.0
                    menu text=foreground selected-text=foreground selected-background=white/78 background=surface border=white/72 border-width=1.0 radius=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                  input "" #block label="New block" <-> block_draft hint="Add a block…" disabled=(loading || !connected || new_block_kind == "Divider") submit=add_block_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                    active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=9.0
                    focused background=white/38 border=white/66 border-width=1.0
                    disabled background=white/16 value=muted
                  button "Add" disabled=(loading || mutation_phase != "idle" || !connected || (new_block_kind != "Divider" && empty(trim(block_draft)))) height=30.0 padding=7.0 -> add_block_submit
                    active background=foreground/90 text=white border=white/28 border-width=1.0 radius=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                    hovered background=foreground/80 text=white
                    pressed background=foreground text=white
                    disabled background=foreground/28 text=white/60
