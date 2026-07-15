use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[(&str, &str)] = &[
    ("crate::adapters", "trusted adapters"),
    ("crate::backend", "desktop backend"),
    ("crate::transport", "node transport"),
    ("crate::browser", "CEF/browser host"),
    ("crate::desktop", "desktop host"),
    ("crate::external_url", "system browser host"),
    ("crate::huddle_media", "native media host"),
    ("crate::huddle_session", "native session host"),
    ("crate::mac_tray", "platform tray host"),
    ("crate::module_host", "host composition"),
    ("module_view_host::", "packaged-view runtime host"),
    ("crate::notifications", "notification host"),
    ("crate::page_presence", "presence host"),
    ("crate::search", "search host"),
    ("crate::shell", "application shell"),
    ("crate::account_service", "trusted service"),
    ("crate::community_service", "trusted service"),
    ("crate::forge_agents_service", "trusted service"),
    ("crate::operator_service", "trusted service"),
    ("crate::profile_service", "trusted service"),
    ("crate::screen_service", "trusted service"),
    ("crate::user_content_service", "trusted service"),
    ("crate::workspace_service", "trusted service"),
    ("std::fs", "filesystem"),
    ("tokio::fs", "filesystem"),
    ("std::net", "network"),
    ("tokio::net", "network"),
    ("std::process", "process control"),
    ("tokio::process", "process control"),
    ("cef::", "CEF"),
    ("git2::", "Git/filesystem"),
    ("libc::", "platform FFI"),
    ("objc2", "platform FFI"),
    ("reqwest::", "network"),
    ("rfd::", "native dialogs"),
    ("windows::", "platform FFI"),
    ("x11", "platform FFI"),
];

#[test]
fn views_do_not_import_host_capabilities() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = rust_files(&root.join("screens"));
    files.push(root.join("view_api.rs"));

    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        if imports_iced_clipboard(&source) {
            violations.push(format!(
                "{} imports native clipboard access",
                file.strip_prefix(&root).unwrap().display(),
            ));
        }
        for (index, line) in source.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for (needle, capability) in FORBIDDEN {
                if code.contains(needle) {
                    violations.push(format!(
                        "{}:{} imports {capability} through `{needle}`",
                        file.strip_prefix(&root).unwrap().display(),
                        index + 1
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "views must request host capabilities through typed effects:\n{}",
        violations.join("\n")
    );
}

fn imports_iced_clipboard(source: &str) -> bool {
    let mut statement = String::new();
    let mut collecting = false;
    for line in source.lines().map(str::trim) {
        if !collecting && line.starts_with("use ") {
            collecting = true;
        }
        if !collecting {
            continue;
        }
        statement.push_str(line);
        if line.ends_with(';') {
            if statement.contains("iced::advanced")
                && (statement.contains("clipboard") || statement.contains("Clipboard"))
            {
                return true;
            }
            statement.clear();
            collecting = false;
        }
    }
    false
}

#[test]
fn clipboard_imports_are_part_of_the_view_boundary() {
    assert!(imports_iced_clipboard("use iced::advanced::clipboard;"));
    assert!(imports_iced_clipboard(
        "use iced::advanced::{\n    Clipboard, Layout, Widget,\n};"
    ));
    assert!(!imports_iced_clipboard(
        "fn update(_clipboard: &mut dyn iced::advanced::Clipboard) {}"
    ));
}

#[test]
fn files_view_cannot_receive_or_mint_native_paths() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = fs::read_to_string(root.join("screens/file_browser.rs")).unwrap();

    assert!(
        !source.contains("PathBuf") && !source.contains("DropToken::from_host"),
        "Files must receive only host-minted opaque drop tokens, never native paths"
    );
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
