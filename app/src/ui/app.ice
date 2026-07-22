app Ducktape
  title "Ducktape"
  theme app_theme
  background app_background
  text-color app_text
  id "dev.ducktape.app"
  default-text-size 15
  antialiasing true
  window
    size 1180 760
    min-size 900 600
    position centered

extern crate::backend
  ChatChannel(id:str, name:str)
  ChatMessage(author:str, meta:str, body:str)
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str)
  PageItem(id:str, title:str)
  PageBlock(id:str, kind:str, text:str)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str)
  WorkspaceData(rpc:str, status:str, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str)
  AppError(message:str)
  connect(rpc:str) -> WorkspaceData ! AppError
  load_chat(rpc:str, channel_id:str) -> ChatData ! AppError
  create_channel(rpc:str, password:str, name:str) -> ChatData ! AppError
  send_message(rpc:str, password:str, channel_id:str, body:str) -> ChatData ! AppError
  load_page(rpc:str, page_id:str) -> PagesData ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  rename_page(rpc:str, password:str, page_id:str, title:str) -> PagesData ! AppError
  add_paragraph(rpc:str, password:str, page_id:str, text:str) -> PagesData ! AppError

theme
  background #0a1020
  surface    #111a2d
  elevated   #17233b
  foreground #f5f7ff
  muted      #8f9bb3
  primary    #747bff
  danger     #ef5b67
  success    #36c98f
  border     #263451
  subtle     #1d2942

state
  app_theme = "app"
  app_background = "#0a1020"
  app_text = "#f5f7ff"
  tab = "chat"
  rpc = ""
  password = ""
  status = "Connecting…"
  connected = false
  loading = false
  error = ""
  channels:[ChatChannel] = []
  messages:[ChatMessage] = []
  active_channel = ""
  channel_draft = ""
  message_draft = ""
  pages:[PageItem] = []
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  page_draft = ""
  paragraph_draft = ""

component Brand()
  row spacing=11.0 align=center
    container width=34.0 height=34.0 align-x=center align-y=center background=primary radius=10.0
      text "D" size=17.0 @font-bold text-white
    col spacing=0.0
      text "Ducktape" size=17.0 @font-bold text-foreground
      text "shared work, on-chain" size=10.0 @text-muted

component TabButton(label:str, value:str, selected:bool)
  col
    if selected
      button label=label padding=10.0 -> select_tab(trim(value))
        row spacing=8.0 align=center
          container width=7.0 height=7.0 background=primary radius=4.0
            text ""
          text label @font-bold text-foreground
        active background=elevated text=foreground radius=9.0
        hovered background=elevated text=foreground radius=9.0
        pressed background=subtle text=foreground radius=9.0
    if !selected
      button label=label padding=10.0 -> select_tab(trim(value))
        row spacing=8.0 align=center
          container width=7.0 height=7.0 background=muted radius=4.0
            text ""
          text label @text-muted
        active background=transparent text=muted radius=9.0
        hovered background=subtle text=foreground radius=9.0
        pressed background=elevated text=foreground radius=9.0

component ChannelButton(channel:ChatChannel, selected:bool)
  col width=fill
    if selected
      button label=channel.name width=fill padding=11.0 -> choose_channel(channel.id)
        row spacing=9.0 align=center
          text "#" size=15.0 @text-primary font-bold
          text channel.name width=fill wrapping=none @text-foreground font-bold
        active background=elevated text=foreground radius=9.0
        hovered background=elevated text=foreground radius=9.0
        pressed background=subtle text=foreground radius=9.0
    if !selected
      button label=channel.name width=fill padding=11.0 -> choose_channel(channel.id)
        row spacing=9.0 align=center
          text "#" size=15.0 @text-muted
          text channel.name width=fill wrapping=none @text-muted
        active background=transparent text=muted radius=9.0
        hovered background=subtle text=foreground radius=9.0
        pressed background=elevated text=foreground radius=9.0

component PageButton(page:PageItem, selected:bool)
  col width=fill
    if selected
      button label=page.title width=fill padding=11.0 -> choose_page(page.id)
        row spacing=9.0 align=center
          text "□" size=15.0 @text-primary
          text page.title width=fill wrapping=none @text-foreground font-bold
        active background=elevated text=foreground radius=9.0
        hovered background=elevated text=foreground radius=9.0
        pressed background=subtle text=foreground radius=9.0
    if !selected
      button label=page.title width=fill padding=11.0 -> choose_page(page.id)
        row spacing=9.0 align=center
          text "□" size=15.0 @text-muted
          text page.title width=fill wrapping=none @text-muted
        active background=transparent text=muted radius=9.0
        hovered background=subtle text=foreground radius=9.0
        pressed background=elevated text=foreground radius=9.0

component MessageCard(message:ChatMessage)
  container width=fill padding=14.0 background=elevated radius=12.0
    col width=fill spacing=7.0
      row width=fill align=center
        text message.author width=fill size=12.0 @font-bold text-primary
        text message.meta size=11.0 @text-muted
      text message.body width=fill size=15.0 wrapping=word @text-foreground

component BlockCard(block:PageBlock)
  container width=fill padding=15.0 background=elevated radius=12.0
    col width=fill spacing=7.0
      row width=fill align=center
        text block.kind width=fill size=11.0 @font-bold text-primary
        text block.id size=10.0 wrapping=none @text-muted
      text block.text width=fill size=15.0 wrapping=word @text-foreground

component EmptyState(title:str, detail:str)
  container width=fill height=fill align-x=center align-y=center
    col spacing=8.0 align=center
      container width=44.0 height=44.0 align-x=center align-y=center background=elevated radius=14.0
        text "·" size=28.0 @text-primary
      text title size=18.0 @font-bold text-foreground
      text detail size=13.0 @text-muted

on mount
  loading = true
  run connect(rpc) -> connected_ok _ | failed _

on reconnect
  return if loading
  loading = true
  connected = false
  error = ""
  status = "Connecting…"
  run connect(trim(rpc)) -> connected_ok _ | failed _

on connected_ok(next)
  rpc = next.rpc
  status = next.status
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  connected = true
  loading = false
  error = ""

on select_tab(next)
  tab = next

on choose_channel(id)
  return if loading
  loading = true
  error = ""
  run load_chat(rpc, id) -> chat_updated _ | failed _

on create_channel_submit
  return if loading || empty(trim(channel_draft))
  loading = true
  error = ""
  run create_channel(rpc, password, trim(channel_draft)) -> chat_updated _ | failed _

on send_message_submit
  return if loading || empty(active_channel) || empty(trim(message_draft))
  loading = true
  error = ""
  run send_message(rpc, password, active_channel, trim(message_draft)) -> chat_updated _ | failed _

on chat_updated(next)
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  channel_draft = ""
  message_draft = ""
  loading = false

on choose_page(id)
  return if loading
  loading = true
  error = ""
  run load_page(rpc, id) -> pages_updated _ | failed _

on create_page_submit
  return if loading || empty(trim(page_draft))
  loading = true
  error = ""
  run create_page(rpc, password, trim(page_draft)) -> pages_updated _ | failed _

on rename_page_submit
  return if loading || empty(active_page) || empty(trim(active_page_title))
  loading = true
  error = ""
  run rename_page(rpc, password, active_page, trim(active_page_title)) -> pages_updated _ | failed _

on add_paragraph_submit
  return if loading || empty(active_page) || empty(trim(paragraph_draft))
  loading = true
  error = ""
  run add_paragraph(rpc, password, active_page, trim(paragraph_draft)) -> pages_updated _ | failed _

on pages_updated(next)
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  page_draft = ""
  paragraph_draft = ""
  loading = false

on dismiss_error
  error = ""

on failed(cause)
  loading = false
  status = "Offline"
  error = cause.message

view
  col width=fill height=fill @bg-background
    container width=fill padding=16.0 background=surface border=border border-width=1.0
      row width=fill spacing=16.0 align=center
        Brand
        space width=fill
        input "" #rpc label="RPC endpoint" description="Ducktape node HTTP origin" <-> rpc hint="http://127.0.0.1:8844" disabled=loading submit=reconnect width=310.0 padding=10.0
          active background=background border=border value=foreground placeholder=muted selection=primary border-width=1.0 radius=9.0
          hovered background=background border=primary value=foreground placeholder=muted selection=primary border-width=1.0 radius=9.0
          focused background=background border=primary value=foreground placeholder=muted selection=primary border-width=2.0 radius=9.0
          focused-hovered background=background border=primary value=foreground placeholder=muted selection=primary border-width=2.0 radius=9.0
          disabled background=subtle border=border value=muted placeholder=muted selection=primary border-width=1.0 radius=9.0
        input "" #password label="Local key password" description="Password for the encrypted local user key" secure=true <-> password hint="Key password" disabled=loading width=180.0 padding=10.0
          active background=background border=border value=foreground placeholder=muted selection=primary border-width=1.0 radius=9.0
          hovered background=background border=primary value=foreground placeholder=muted selection=primary border-width=1.0 radius=9.0
          focused background=background border=primary value=foreground placeholder=muted selection=primary border-width=2.0 radius=9.0
          focused-hovered background=background border=primary value=foreground placeholder=muted selection=primary border-width=2.0 radius=9.0
          disabled background=subtle border=border value=muted placeholder=muted selection=primary border-width=1.0 radius=9.0
        button "Connect" disabled=loading padding=10.0 style=primary -> reconnect

    container width=fill padding-top=10.0 padding-left=18.0 padding-right=18.0
      row width=fill align=center
        row spacing=4.0
          TabButton label="Chat" value="chat" selected=(tab == "chat")
          TabButton label="Pages" value="pages" selected=(tab == "pages")
        space width=fill
        if loading
          text "Working…" size=12.0 @text-primary
        if !loading
          text status size=12.0 @text-muted

    if error != ""
      container width=fill padding-left=18.0 padding-right=18.0 padding-top=10.0
        container width=fill padding=12.0 background=danger radius=10.0
          row width=fill spacing=12.0 align=center
            text error width=fill size=13.0 @text-white
            button "Dismiss" padding=7.0 style=text -> dismiss_error

    col width=fill height=fill padding=18.0
      if tab == "chat"
        row width=fill height=fill spacing=14.0
          container width=270.0 height=fill padding=12.0 background=surface border=border border-width=1.0 radius=14.0
            col width=fill height=fill spacing=10.0
              row width=fill align=center
                text "CHANNELS" width=fill size=11.0 @font-bold text-muted
                text len(channels) size=11.0 @text-muted
              scroll direction=vertical width=fill height=fill bar=hidden
                col width=fill spacing=3.0
                  for channel in channels
                    ChannelButton channel=channel selected=(channel.id == active_channel)
              row width=fill spacing=7.0
                input "Channel name" label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || !connected) submit=create_channel_submit width=fill padding=9.0 @bg-background border border-border rounded-lg
                button "+" label="Create channel" disabled=(loading || !connected || empty(trim(channel_draft))) padding=9.0 style=primary -> create_channel_submit

          col width=fill height=fill padding=16.0 @bg-surface border border-border rounded-lg
            col width=fill height=fill spacing=13.0
              row width=fill align=center
                col width=fill spacing=2.0
                  text "Chat" size=21.0 @font-bold text-foreground
                  text "Local-key signed messages" size=12.0 @text-muted
                button "Refresh" disabled=(loading || !connected) padding=8.0 style=secondary -> reconnect
              if empty(messages)
                EmptyState title="No messages yet" detail="Create a channel or start the conversation."
              if !empty(messages)
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=9.0
                    for message in messages
                      MessageCard message=message
              row width=fill spacing=9.0
                input "Message" #message label="Message" <-> message_draft hint="Write a message…" disabled=(loading || !connected || empty(active_channel)) submit=send_message_submit width=fill padding=12.0 @bg-background border border-border rounded-lg
                button "Send" disabled=(loading || !connected || empty(active_channel) || empty(trim(message_draft))) padding=12.0 style=primary -> send_message_submit

      if tab == "pages"
        row width=fill height=fill spacing=14.0
          container width=270.0 height=fill padding=12.0 background=surface border=border border-width=1.0 radius=14.0
            col width=fill height=fill spacing=10.0
              row width=fill align=center
                text "PAGES" width=fill size=11.0 @font-bold text-muted
                text len(pages) size=11.0 @text-muted
              scroll direction=vertical width=fill height=fill bar=hidden
                col width=fill spacing=3.0
                  for page in pages
                    PageButton page=page selected=(page.id == active_page)
              row width=fill spacing=7.0
                input "Page title" label="New page title" <-> page_draft hint="New page" disabled=(loading || !connected) submit=create_page_submit width=fill padding=9.0 @bg-background border border-border rounded-lg
                button "+" label="Create page" disabled=(loading || !connected || empty(trim(page_draft))) padding=9.0 style=primary -> create_page_submit

          col width=fill height=fill padding=16.0 @bg-surface border border-border rounded-lg
            if empty(active_page)
              EmptyState title="No page selected" detail="Create a page to begin writing."
            if !empty(active_page)
              col width=fill height=fill spacing=13.0
                row width=fill spacing=9.0 align=center
                  input "Page title" #page-title label="Page title" <-> active_page_title disabled=(loading || !connected) submit=rename_page_submit width=fill padding=11.0 text-size=20.0 @bg-background border border-border rounded-lg
                  button "Save title" disabled=(loading || !connected || empty(trim(active_page_title))) padding=11.0 style=secondary -> rename_page_submit
                text "Every edit is a signed Pages transaction." size=12.0 @text-muted
                if empty(blocks)
                  EmptyState title="An empty page" detail="Add the first paragraph below."
                if !empty(blocks)
                  scroll direction=vertical width=fill height=fill bar=hidden
                    col width=fill spacing=9.0
                      for block in blocks
                        BlockCard block=block
                row width=fill spacing=9.0
                  input "Paragraph" #paragraph label="New paragraph" <-> paragraph_draft hint="Add a paragraph…" disabled=(loading || !connected) submit=add_paragraph_submit width=fill padding=12.0 @bg-background border border-border rounded-lg
                  button "Add" disabled=(loading || !connected || empty(trim(paragraph_draft))) padding=12.0 style=primary -> add_paragraph_submit
