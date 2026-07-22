font ui family=sans weight=normal stretch=normal style=normal default=true

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
  ChatChannel(id:str, name:str)
  ChatMessage(author:str, meta:str, body:str, pending:bool)
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str)
  PageItem(id:str, title:str)
  PageBlock(id:str, kind:str, text:str, pending:bool)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str)
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str)
  LiveUpdate(kind:str, status:str, height:i64)
  AppError(message:str)
  HydrationError(generation:i64, message:str)
  connect(rpc:str) -> WorkspaceData ! AppError
  stream live_events(rpc:str) -> LiveUpdate
  refresh(rpc:str, channel_id:str, page_id:str, generation:i64) -> WorkspaceData ! HydrationError
  retry_refresh(rpc:str, channel_id:str, page_id:str, generation:i64, attempt:i64) -> WorkspaceData ! HydrationError
  sync optimistic_message(messages:[ChatMessage], body:str) -> [ChatMessage]
  sync rollback_messages(messages:[ChatMessage]) -> [ChatMessage]
  sync optimistic_paragraph(blocks:[PageBlock], text:str) -> [PageBlock]
  sync rollback_blocks(blocks:[PageBlock]) -> [PageBlock]
  sync restore_draft(current:str, pending:str) -> str
  load_chat(rpc:str, channel_id:str) -> ChatData ! AppError
  create_channel(rpc:str, password:str, name:str) -> ChatData ! AppError
  send_message(rpc:str, password:str, channel_id:str, body:str) -> ChatData ! AppError
  load_page(rpc:str, page_id:str) -> PagesData ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  rename_page(rpc:str, password:str, page_id:str, title:str) -> PagesData ! AppError
  add_paragraph(rpc:str, password:str, page_id:str, text:str) -> PagesData ! AppError

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
  channel_draft = ""
  pending_channel = ""
  message_draft = ""
  pending_message = ""
  pages:[PageItem] = []
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  page_draft = ""
  pending_page = ""
  paragraph_draft = ""
  pending_paragraph = ""

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
          text "#" width=18.0 size=13.0 align-x=center @text-foreground font-bold
          text channel.name width=fill size=12.0 wrapping=none @text-foreground font-bold
        active background=linear(2.3, white/78@0.0, surface/58@1.0) text=foreground border=white/78 border-width=1.0 radius=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed background=selection
    if !selected
      button label=channel.name width=fill height=34.0 padding=7.0 -> choose_channel(channel.id)
        row width=fill spacing=9.0 align=center
          text "#" width=18.0 size=13.0 align-x=center @text-muted
          text channel.name width=fill size=12.0 wrapping=none @text-muted
        active background=transparent text=muted radius=10.0
        hovered background=white/34 text=foreground
        pressed background=selection text=foreground

component PageButton(page:PageItem, selected:bool)
  col width=fill
    if selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-foreground
          text page.title width=fill size=12.0 wrapping=none @text-foreground font-bold
        active background=linear(2.3, white/78@0.0, surface/58@1.0) text=foreground border=white/78 border-width=1.0 radius=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed background=selection
    if !selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-muted
          text page.title width=fill size=12.0 wrapping=none @text-muted
        active background=transparent text=muted radius=10.0
        hovered background=white/34 text=foreground
        pressed background=selection text=foreground

component MessageCard(message:ChatMessage)
  container width=fill padding=8.0 background=transparent radius=6.0
    col width=fill spacing=3.0
      row width=fill align=center
        text message.author width=fill size=12.0 @font-bold text-foreground
        text message.meta size=11.0 @text-muted
      text message.body width=fill size=14.0 wrapping=word @text-foreground

component BlockCard(block:PageBlock)
  container width=fill padding=8.0 background=transparent radius=6.0
    col width=fill spacing=3.0
      row width=fill align=center
        text block.kind width=fill size=11.0 @font-bold text-muted
        text block.id size=11.0 wrapping=none @text-muted
      text block.text width=fill size=14.0 wrapping=word @text-foreground

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
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
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
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
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
  pending_channel = ""
  pending_message = ""
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
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

on rename_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(trim(active_page_title))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page-title"
  error = ""
  run rename_page(connected_rpc, password, active_page, trim(active_page_title)) -> pages_mutated _ | mutation_failed _

on add_paragraph_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(trim(paragraph_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "paragraph"
  pending_paragraph = trim(paragraph_draft)
  paragraph_draft = ""
  blocks = optimistic_paragraph(blocks, pending_paragraph)
  error = ""
  run add_paragraph(connected_rpc, password, active_page, pending_paragraph) -> pages_mutated _ | mutation_failed _

on pages_updated(next)
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
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
  pending_page = ""
  pending_paragraph = ""
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
  paragraph_draft = restore_draft(paragraph_draft, pending_paragraph)
  messages = rollback_messages(messages)
  blocks = rollback_blocks(blocks)
  pending_channel = ""
  pending_message = ""
  pending_page = ""
  pending_paragraph = ""
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
        col width=fill height=fill spacing=9.0
          if !empty(active_channel)
            row width=fill height=26.0 spacing=7.0 align=center
              container width=22.0 height=22.0 align-x=center align-y=center background=white/52 border=white/72 border-width=1.0 radius=7.0
                text "#" size=11.0 @font-bold text-foreground
              text active_channel width=fill size=12.0 @font-bold text-foreground
          if empty(messages)
            EmptyState title="No messages yet" detail="Create a channel or start the conversation."
          if !empty(messages)
            scroll direction=vertical width=fill height=fill bar=hidden
              col width=fill spacing=1.0
                for message in messages
                  MessageCard message=message
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
              container width=fill height=1.0 background=separator
                text ""
              if empty(blocks)
                EmptyState title="An empty page" detail="Add the first paragraph below."
              if !empty(blocks)
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for block in blocks
                      BlockCard block=block
              container width=fill padding=6.0 background=linear(2.3, white/64@0.0, surface/42@1.0) border=white/72 border-width=1.0 radius=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
                flex width=fill gap=6.0 align-items=center
                  input "" #paragraph label="New paragraph" <-> paragraph_draft hint="Add a paragraph…" disabled=(loading || !connected) submit=add_paragraph_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                    active background=transparent border=transparent value=foreground placeholder=muted selection=foreground/18 border-width=0.0 radius=9.0
                    focused background=white/38 border=white/66 border-width=1.0
                    disabled value=muted
                  button "Add" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(paragraph_draft))) height=30.0 padding=7.0 -> add_paragraph_submit
                    active background=foreground/90 text=white border=white/28 border-width=1.0 radius=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                    hovered background=foreground/80 text=white
                    pressed background=foreground text=white
                    disabled background=foreground/28 text=white/60
