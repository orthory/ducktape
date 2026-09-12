//! The native menu bar mirrors session facts; choosing a row reuses the
//! desktop's ordinary messages. macOS owns the status item and menu loop.

use crate::{__DucktapeMessage as Message, Appearance, Ducktape, backend};
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use gpui_kit::App;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    icon: usize,
    badge: String,
    tooltip: String,
    labels: [String; 22],
    visible: [bool; 22],
}

impl Snapshot {
    fn of(state: &Ducktape) -> Self {
        let icon = icon_index(state.connected, state.bell_unread);
        let mut labels = std::array::from_fn(|_| String::new());
        for (row, label) in [
            (3, "Open Ducktape"),
            (5, "Go to"),
            (6, "Chat"),
            (7, "Pages"),
            (8, "Node"),
            (9, "Settings"),
            (13, "Leave huddle"),
            (14, "Appearance"),
            (18, "Copy node key"),
            (19, "Reconnect"),
            (21, "Quit Ducktape"),
        ] {
            labels[row] = label.into();
        }
        labels[0] = match state.network_name.is_empty() {
            true => "No network".into(),
            false => state.network_name.clone(),
        };
        labels[1] = state.status.clone();
        labels[4] = backend::tray_bell_row(state.bell_unread);
        labels[11] =
            backend::tray_huddle_row(state.huddle_joined, state.huddle_channel_name.clone());
        labels[12] = match state.call_muted {
            true => "Unmute".into(),
            false => "Mute".into(),
        };
        labels[15] =
            backend::tray_choice_row("Light".into(), state.appearance == Appearance::Light);
        labels[16] = backend::tray_choice_row("Dark".into(), state.appearance == Appearance::Dark);
        let mut visible = [true; 22];
        for row in [4, 5, 18, 19] {
            visible[row] = state.console_win.is_some();
        }
        visible[11] = state.huddle_joined;
        Self {
            icon,
            badge: backend::tray_badge(state.bell_unread),
            tooltip: backend::tray_tooltip(state.network_name.clone(), state.status.clone()),
            labels,
            visible,
        }
    }
}

fn icon_index(connected: bool, unread: i64) -> usize {
    [!connected, unread > 0]
        .iter()
        .position(|selected| *selected)
        .unwrap_or(2)
}

pub fn message(row: usize) -> Option<Message> {
    match row {
        3 => Some(Message::TrayOpen),
        4 => Some(Message::TrayOpenBell),
        6 => Some(Message::TrayGoChat),
        7 => Some(Message::TrayGoPages),
        8 => Some(Message::TrayGoNode),
        9 => Some(Message::TrayGoSettings),
        12 => Some(Message::ToggleCallMute),
        13 => Some(Message::LeaveHuddleHere),
        15 => Some(Message::SetAppearanceLight),
        16 => Some(Message::SetAppearanceDark),
        18 => Some(Message::TrayCopyNodeKey),
        19 => Some(Message::TrayReconnect),
        21 => Some(Message::TrayQuit),
        _ => None,
    }
}

pub struct Tray {
    snapshot: Option<Snapshot>,
    #[cfg(target_os = "macos")]
    native: Option<native::StatusItem>,
}

/// Call once on GPUI's foreground thread after its native event loop starts.
pub fn init(_: &mut App) -> (Tray, UnboundedReceiver<usize>) {
    let (send, receive) = unbounded();
    #[cfg(target_os = "macos")]
    let native = match native::StatusItem::new(send) {
        Ok(tray) => Some(tray),
        Err(error) => {
            tracing::warn!(target:"ducktape::app",reason="tray_init_failed",%error,"native status item unavailable");
            None
        }
    };
    #[cfg(not(target_os = "macos"))]
    drop(send);
    (
        Tray {
            snapshot: None,
            #[cfg(target_os = "macos")]
            native,
        },
        receive,
    )
}

impl Tray {
    pub fn sync(&mut self, state: &Ducktape) {
        let next = Snapshot::of(state);
        if self.snapshot.as_ref() == Some(&next) {
            return;
        }
        #[cfg(target_os = "macos")]
        if let Some(native) = &mut self.native {
            if let Err(error) = native.sync(self.snapshot.as_ref(), &next) {
                tracing::warn!(target:"ducktape::app",reason="tray_update_failed",%error,"native status item update refused");
                return;
            }
        }
        self.snapshot = Some(next);
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::Snapshot;
    use futures::channel::mpsc::UnboundedSender;
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder,
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    };

    pub(super) struct StatusItem {
        tray: TrayIcon,
        icons: [Icon; 3],
    }

    impl StatusItem {
        pub(super) fn new(send: UnboundedSender<usize>) -> Result<Self, String> {
            let pixels: [&[u8]; 3] = [
                include_bytes!("../assets/tray-offline.rgba"),
                include_bytes!("../assets/tray-unread.rgba"),
                include_bytes!("../assets/tray.rgba"),
            ];
            let mut icons = pixels
                .into_iter()
                .map(|bytes| Icon::from_rgba(bytes.to_vec(), 128, 128));
            let icons = [
                icons.next().unwrap().map_err(|error| error.to_string())?,
                icons.next().unwrap().map_err(|error| error.to_string())?,
                icons.next().unwrap().map_err(|error| error.to_string())?,
            ];
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let Some(row) = event
                    .id
                    .as_ref()
                    .strip_prefix("ducktape-tray-")
                    .and_then(|row| row.parse::<usize>().ok())
                else {
                    return;
                };
                let _ = send.unbounded_send(row);
            }));
            let tray = TrayIconBuilder::new()
                .with_id("ducktape")
                .with_icon(icons[0].clone())
                .with_icon_as_template(false)
                .with_menu_on_left_click(true)
                .with_menu(Box::new(Menu::new()))
                .build()
                .map_err(|error| error.to_string())?;
            Ok(Self { tray, icons })
        }

        pub(super) fn sync(
            &mut self,
            previous: Option<&Snapshot>,
            next: &Snapshot,
        ) -> Result<(), String> {
            if previous.is_none_or(|old| old.icon != next.icon) {
                self.tray
                    .set_icon(Some(self.icons[next.icon].clone()))
                    .map_err(|error| error.to_string())?;
            }
            if previous.is_none_or(|old| old.badge != next.badge) {
                self.tray.set_title(Some(&next.badge));
            }
            if previous.is_none_or(|old| old.tooltip != next.tooltip) {
                self.tray
                    .set_tooltip(Some(&next.tooltip))
                    .map_err(|error| error.to_string())?;
            }
            let menu_changed =
                previous.is_none_or(|old| old.labels != next.labels || old.visible != next.visible);
            if menu_changed {
                self.tray.set_menu(Some(Box::new(
                    menu(next).map_err(|error| error.to_string())?,
                )));
            }
            Ok(())
        }
    }

    fn item(snapshot: &Snapshot, row: usize, enabled: bool) -> MenuItem {
        MenuItem::with_id(
            format!("ducktape-tray-{row}"),
            &snapshot.labels[row],
            enabled,
            None,
        )
    }

    fn submenu(
        snapshot: &Snapshot,
        row: usize,
        children: &[usize],
    ) -> Result<Submenu, tray_icon::menu::Error> {
        let menu = Submenu::new(&snapshot.labels[row], true);
        for child in children {
            menu.append(&item(snapshot, *child, true))?;
        }
        Ok(menu)
    }

    fn menu(snapshot: &Snapshot) -> Result<Menu, tray_icon::menu::Error> {
        let menu = Menu::new();
        menu.append(&item(snapshot, 0, false))?;
        menu.append(&item(snapshot, 1, false))?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&item(snapshot, 3, true))?;
        if snapshot.visible[4] {
            menu.append(&item(snapshot, 4, true))?;
        }
        if snapshot.visible[5] {
            menu.append(&submenu(snapshot, 5, &[6, 7, 8, 9])?)?;
        }
        menu.append(&PredefinedMenuItem::separator())?;
        if snapshot.visible[11] {
            menu.append(&submenu(snapshot, 11, &[12, 13])?)?;
        }
        menu.append(&submenu(snapshot, 14, &[15, 16])?)?;
        menu.append(&PredefinedMenuItem::separator())?;
        for row in [18, 19] {
            if snapshot.visible[row] {
                menu.append(&item(snapshot, row, true))?;
            }
        }
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&item(snapshot, 21, true))?;
        Ok(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_priority_and_menu_routes_match_the_desktop() {
        assert_eq!(icon_index(false, 3), 0);
        assert_eq!(icon_index(true, 3), 1);
        assert_eq!(icon_index(true, 0), 2);
        assert!(matches!(message(3), Some(Message::TrayOpen)));
        assert!(matches!(message(21), Some(Message::TrayQuit)));
        for row in [0, 1, 2, 5, 10, 11, 14, 17, 20, 22] {
            assert!(message(row).is_none());
        }
    }
}
