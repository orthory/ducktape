use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[(&str, &str)] = &[
    ("crate::adapters", "trusted adapters"),
    ("crate::backend", "desktop backend"),
    ("crate::transport", "node transport"),
    ("crate::browser", "CEF/browser host"),
    ("crate::desktop", "desktop host"),
    ("crate::huddle_media", "native media host"),
    ("crate::huddle_session", "native session host"),
    ("crate::mac_tray", "platform tray host"),
    ("crate::module_host", "host composition"),
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
