// SETTINGS, as a module-owned view: this device's preferences, the account
// this key speaks for and the workspace's lifecycle, drawn from the facts the
// desktop app pushes. The screen body is the app's own (screens/settings.ice
// before the port). The drafts are the view's: the app hears a name, a key,
// a ticket or a password only when the reader submits it, and tells the view
// which drafts an op consumed through `drafts_cleared`.
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
  SettingsProps(dark:bool, connected:bool, loading:bool, status:str, busy:bool, recovering:bool, appearance:str, desktop_notifications:bool, unlocked:bool, account_name:str, network_name:str, connected_rpc:str, account_ceremony_phase:str, account_ceremony_qr:str, account_ceremony_detail:str, account_ceremony_left:str, settings_key_state:str, settings_key_path:str, settings_open_tabs:i64, tier:str, admin:bool, members_line:str, members_answered:bool, account_number:str, account_renaming:bool, account_exists:bool, account_keys:i64, account_key_rows:[AccountKeyRow], account_busy:bool, account_ticket:str, drafts_cleared:i64, drafts_scope:str)
  stream props() -> SettingsProps ! HostError
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
  pure clear_tabs() -> bool
  pure set_light() -> bool
  pure set_dark() -> bool
  pure set_notifications(enabled:bool) -> bool
  pure connection_degraded(status:&str) -> bool
  pure initial_of(name:&str) -> str
  pure drafts_cleared_by(scope:&str, draft:&str) -> bool
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
  account_name = ""
  network_name = ""
  connected_rpc = ""
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  settings_key_state = ""
  settings_key_path = ""
  settings_open_tabs:i64 = 0
  tier = ""
  admin = false
  members_line = ""
  members_answered = false
  account_number = ""
  account_renaming = false
  account_exists = false
  account_keys:i64 = 0
  account_key_rows:[AccountKeyRow] = []
  account_busy = false
  account_ticket = ""
  // the last consumption the app reported: a count that moves once per
  // committed op, and which drafts it took (`name`, `keys`, `label`, `account`)
  drafts_cleared:i64 = 0
  // the reader's own: the five drafts the identity card edits
  account_name_draft = ""
  account_create_draft = ""
  account_key_draft = ""
  account_key_label_draft = ""
  account_join_draft = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  connected = next.connected
  loading = next.loading
  status = next.status
  busy = next.busy
  recovering = next.recovering
  appearance = next.appearance
  desktop_notifications = next.desktop_notifications
  unlocked = next.unlocked
  account_name = next.account_name
  network_name = next.network_name
  connected_rpc = next.connected_rpc
  account_ceremony_phase = next.account_ceremony_phase
  account_ceremony_qr = next.account_ceremony_qr
  account_ceremony_detail = next.account_ceremony_detail
  account_ceremony_left = next.account_ceremony_left
  settings_key_state = next.settings_key_state
  settings_key_path = next.settings_key_path
  settings_open_tabs = next.settings_open_tabs
  tier = next.tier
  admin = next.admin
  members_line = next.members_line
  members_answered = next.members_answered
  account_number = next.account_number
  account_renaming = next.account_renaming
  account_exists = next.account_exists
  account_keys = next.account_keys
  account_key_rows = next.account_key_rows
  account_busy = next.account_busy
  account_ticket = next.account_ticket
  // A COMMITTED OP CONSUMES THE DRAFTS IT READ, and only those: a rename
  // leaves a half-pasted key alone. The count says an op landed since the
  // last props; the scope says which drafts it took.
  let consumed = next.drafts_cleared != drafts_cleared
  drafts_cleared = next.drafts_cleared
  account_name_draft = keep_draft(consumed && drafts_cleared_by(next.drafts_scope, "name"), account_name_draft)
  account_key_draft = keep_draft(consumed && drafts_cleared_by(next.drafts_scope, "keys"), account_key_draft)
  account_key_label_draft = keep_draft(consumed && drafts_cleared_by(next.drafts_scope, "label"), account_key_label_draft)
  account_create_draft = keep_draft(consumed && drafts_cleared_by(next.drafts_scope, "account"), account_create_draft)
  account_join_draft = keep_draft(consumed && drafts_cleared_by(next.drafts_scope, "account"), account_join_draft)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

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

on account_rename_submit
  return if account_renaming || empty(trim(account_name_draft))
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

on account_key_remove(pubkey)
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

on settings_clear_tabs
  sent = clear_tabs()

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
        settings_open_tabs
        tier
        admin
        members_line
        members_answered
        account_number
        account_renaming
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
        settings_clear_tabs -> settings_clear_tabs
        set_appearance_light -> set_appearance_light
        set_appearance_dark -> set_appearance_dark
        set_desktop_notifications -> set_desktop_notifications _
