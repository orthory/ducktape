//! macOS app menu. Tauri's default menu binds Cmd+W to Close Window, which
//! swallows the key before the webview can close a doc tab (the window
//! itself only hides to the tray anyway — see `tray.rs`). Rebuild the menu
//! without that item: app + Edit (system clipboard bindings) + Window,
//! no Close Window accelerator. Other platforms have no default menu.

#[cfg(target_os = "macos")]
pub fn install(app: &crate::rt::App) -> tauri::Result<()> {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
    let handle = app.handle();
    let name = &app.package_info().name;
    let app_menu = Submenu::with_items(
        handle,
        name,
        true,
        &[
            &PredefinedMenuItem::about(handle, None, Some(AboutMetadata::default()))?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::services(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::hide(handle, None)?,
            &PredefinedMenuItem::hide_others(handle, None)?,
            &PredefinedMenuItem::show_all(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::quit(handle, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
            &PredefinedMenuItem::select_all(handle, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        handle,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::fullscreen(handle, None)?,
        ],
    )?;
    app.set_menu(Menu::with_items(handle, &[&app_menu, &edit, &window])?)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn install(_app: &crate::rt::App) -> tauri::Result<()> {
    Ok(())
}
