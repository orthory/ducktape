// SETTINGS, as a module-owned view on the KERNEL CONTRACT: this device's
// preferences, the account this key speaks for and the workspace's lifecycle.
//
// The kernel pushes session facts only (`session()`): the colour mode, the
// connection and what the titlebar calls it, whether the seat is held, and
// the wallet/account machinery that is the kernel's alone. This node's
// STANDING and the account's KEY ASSOCIATIONS are read here, through
// `rpc.status` / `rpc.query`, and re-read on every valset and identity block
// (`rpc.live`).
//
// Every act still leaves as an intent: unlocking the seat, founding an
// account, minting a ticket, registering a passkey and linking a wallet are
// the kernel's operations — this view presses, the host signs. The drafts are
// the view's, and a draft is spent when the read it asked for moved.
app SettingsView
  title "Settings"
  palette active_palette
  id "dev.ducktape.view.settings"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "settings.ice"
use "kit.ice"

extern crate::host
  HostError(message:str)
  AccountKeyRow(scheme:str, pubkey:str, label:str)
  Session(dark:bool, connected:bool, loading:bool, status:str, busy:bool, recovering:bool, appearance:str, desktop_notifications:bool, unlocked:bool, seat_key:str, account_name:str, account_number:str, account_exists:bool, network_name:str, connected_rpc:str, account_ceremony_phase:str, account_ceremony_qr:str, account_ceremony_detail:str, account_ceremony_left:str, settings_key_state:str, settings_key_path:str, account_busy:bool, account_ticket:str)
  SessionItem(next:Session, error:str)
  Standing(tier:str, admin:bool, members_line:str)
  StandingItem(next:Standing, answered:bool, error:str)
  KeysItem(rows:[AccountKeyRow], answered:bool, error:str)
  subscription session() -> SessionItem
  // this node's standing and the workspace headcount: read once per
  // connection, then again on every valset block
  subscription standing(connection:i64) -> StandingItem
  // the seat's account keys, re-read on every identity block
  subscription account_keys(connection:i64, seat:str) -> KeysItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure renamed_to(account_name:&str, sent:&str) -> bool
  pure open_tab(tab:&str) -> bool
  pure reconnect_network() -> bool
  pure switch_workspace() -> bool
  pure unlock(password:&str) -> bool
  pure lock() -> bool
  pure rename_account(name:&str) -> bool
  pure create_account(name:&str) -> bool
  pure mint_ticket(pubkey:&str, label:&str) -> bool
  pure join_account(ticket:&str) -> bool
  pure remove_key(pubkey:&str) -> bool
  pure add_passkey(label:&str) -> bool
  pure add_passkey_here(label:&str) -> bool
  pure cancel_ceremony() -> bool
  pure link_wallet(label:&str) -> bool
  pure login() -> bool
  pure copy(text:&str, label:&str) -> bool
  pure set_light() -> bool
  pure set_dark() -> bool
  pure set_notifications(enabled:bool) -> bool
  pure connection_degraded(status:&str) -> bool
  pure initial_of(name:&str) -> str
  pure keep_draft(consumed:bool, draft:&str) -> str

state
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  loading = false
  status = ""
  busy = false
  recovering = false
  appearance = "system"
  desktop_notifications = true
  unlocked = false
  // the seated key's PUBLIC half — what the account read resolves by
  seat_key = ""
  account_name = ""
  network_name = ""
  connected_rpc = ""
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  settings_key_state = ""
  settings_key_path = ""
  account_number = ""
  account_exists = false
  account_busy = false
  account_ticket = ""
  // moves when the session comes up: the two reads restart
  connection_serial:i64 = 0
  // this view's own reads
  tier = ""
  admin = false
  members_line = ""
  members_answered = false
  account_key_rows:[AccountKeyRow] = []
  // the name a rename was sent for — the draft is spent when it comes back
  renaming_to = ""
  // the reader's own: the five drafts the identity card edits
  account_name_draft = ""
  account_create_draft = ""
  account_key_draft = ""
  account_key_label_draft = ""
  account_join_draft = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

derived
  account_keys = len(account_key_rows)

// Subscriptions, not mount tasks, so a replacement restored from this view's
// state asks for the session and its reads again on its own.
subscribe
  session() -> session_arrived _
  standing(connection_serial) when connected -> standing_arrived _
  account_keys(connection_serial, seat_key) when connected && !empty(seat_key) -> keys_arrived _

// THE SESSION: what the kernel knows and this view cannot — whether there is
// a connection, what the titlebar calls it, whether the seat is held, and
// where the wallet/account machinery stands.
on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connection_serial = connection_serial_after(connected, next.connected, connection_serial)
  connected = next.connected
  loading = next.loading
  status = next.status
  busy = next.busy
  recovering = next.recovering
  appearance = next.appearance
  desktop_notifications = next.desktop_notifications
  unlocked = next.unlocked
  seat_key = next.seat_key
  account_name = next.account_name
  network_name = next.network_name
  connected_rpc = next.connected_rpc
  account_ceremony_phase = next.account_ceremony_phase
  account_ceremony_qr = next.account_ceremony_qr
  account_ceremony_detail = next.account_ceremony_detail
  account_ceremony_left = next.account_ceremony_left
  settings_key_state = next.settings_key_state
  settings_key_path = next.settings_key_path
  account_busy = next.account_busy
  account_ticket = next.account_ticket
  // A RENAME IS SPENT WHEN THE CARD SHOWS THE NAME IT ASKED FOR. The op is
  // the kernel's, so the account it reports IS the acknowledgement.
  let renamed = renamed_to(next.account_name, renaming_to)
  renaming_to = keep_draft(renamed, renaming_to)
  account_name_draft = keep_draft(renamed, account_name_draft)
  // FOUNDING OR JOINING IS SPENT WHEN THERE IS AN ACCOUNT. Both drafts ask
  // the same question and one answer settles both.
  let founded = next.account_exists && !account_exists
  account_exists = next.account_exists
  account_number = next.account_number
  account_create_draft = keep_draft(founded, account_create_draft)
  account_join_draft = keep_draft(founded, account_join_draft)
  // A MINTED TICKET IS THE ANSWER TO THE KEY THAT WAS PASTED.
  let minted = !empty(next.account_ticket)
  account_key_draft = keep_draft(minted, account_key_draft)
  account_key_label_draft = keep_draft(minted, account_key_label_draft)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

// THIS NODE'S STANDING, read here: the roster answers or it does not, and an
// unanswered roster is not a guest (`fold_standing`).
on standing_arrived(item)
  host_error = item.error
  members_answered = item.answered
  return if !empty(item.error)
  tier = item.next.tier
  admin = item.next.admin
  members_line = item.next.members_line

on keys_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  account_key_rows = item.rows

on show_tab(tab)
  sent = open_tab(tab)

on reconnect
  sent = reconnect_network()

on switch_network
  sent = switch_workspace()

on settings_unlock_submit(pw)
  return if busy || empty(pw)
  sent = unlock(pw)

on lock_session
  sent = lock()

// A rename is an account op like the other four, so it waits on the same
// in-flight fact; what is remembered here is the NAME it asked for, which is
// how the draft learns it was spent.
on account_rename_submit
  return if account_busy || empty(trim(account_name_draft))
  renaming_to = trim(account_name_draft)
  sent = rename_account(trim(account_name_draft))

on account_create_submit
  return if account_busy || !unlocked || empty(trim(account_create_draft))
  sent = create_account(trim(account_create_draft))

on account_key_add_submit
  return if account_busy || !unlocked || empty(trim(account_key_draft))
  sent = mint_ticket(trim(account_key_draft), trim(account_key_label_draft))

on account_key_join_submit
  return if account_busy || !unlocked || empty(trim(account_join_draft))
  sent = join_account(trim(account_join_draft))

// THE LAST KEY CANNOT GO. An account with no association is one nothing can
// sign for, and the rows this view reads are what say how many are left.
on account_key_remove(pubkey)
  return if account_busy || !unlocked || account_keys <= 1
  sent = remove_key(pubkey)

on account_passkey_submit
  sent = add_passkey(trim(account_key_label_draft))

on account_passkey_desktop
  sent = add_passkey_here(trim(account_key_label_draft))

on account_ceremony_cancel
  sent = cancel_ceremony()

on account_wallet_submit
  sent = link_wallet(trim(account_key_label_draft))

on account_login_submit
  sent = login()

on copy_to_clipboard(text, label)
  sent = copy(text, label)

on set_appearance_light
  sent = set_light()

on set_appearance_dark
  sent = set_dark()

on set_desktop_notifications(enabled)
  sent = set_notifications(enabled)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    SettingsScreen account_name_draft<->account_name_draft account_create_draft<->account_create_draft account_key_draft<->account_key_draft account_key_label_draft<->account_key_label_draft account_join_draft<->account_join_draft #settings
      with
        account_name
        network_name
        connected_rpc
        account_ceremony_phase
        account_ceremony_qr
        account_ceremony_detail
        account_ceremony_left
        settings_key_state
        settings_key_path
        tier
        admin
        members_line
        members_answered
        account_number
        account_exists
        account_keys
        account_key_rows
        account_busy
        account_ticket
        appearance
        desktop_notifications
        unlocked
        status
        loading
        connected
        busy
        recovering
      events
        show_tab -> show_tab _
        reconnect -> reconnect
        switch_network -> switch_network
        settings_unlock_submit -> settings_unlock_submit _
        lock_session -> lock_session
        account_rename_submit -> account_rename_submit
        account_create_submit -> account_create_submit
        account_key_add_submit -> account_key_add_submit
        account_key_join_submit -> account_key_join_submit
        account_key_remove -> account_key_remove _
        account_passkey_submit -> account_passkey_submit
        account_passkey_desktop -> account_passkey_desktop
        account_ceremony_cancel -> account_ceremony_cancel
        account_wallet_submit -> account_wallet_submit
        account_login_submit -> account_login_submit
        copy_to_clipboard -> copy_to_clipboard _ _
        set_appearance_light -> set_appearance_light
        set_appearance_dark -> set_appearance_dark
        set_desktop_notifications -> set_desktop_notifications _
