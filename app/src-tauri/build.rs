fn main() {
    // tauri_build validates bundle.externalBin at COMPILE time, but the node
    // sidecar is another workspace crate's artifact and cargo gives no build
    // ordering between them. materialize an empty placeholder so plain
    // `cargo build` succeeds; real bundles get the fresh binary because
    // beforeBuildCommand runs scripts/prepare-sidecar.sh, which overwrites it.
    let triple = std::env::var("TARGET").expect("cargo sets TARGET");
    let executable_suffix = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    {
        let binary = "ducktape-node";
        let path =
            std::path::PathBuf::from(format!("binaries/{binary}-{triple}{executable_suffix}"));
        if !path.exists() {
            std::fs::create_dir_all("binaries").expect("create binaries dir");
            std::fs::write(&path, []).expect("write sidecar placeholder");
        }
    }
    // Register every application command with Tauri's ACL. Without an app
    // manifest, custom invoke-handler commands are globally allowed; that is
    // unsafe once executable gateway content has its own capability-free
    // WebView. `permissions/trusted.toml` grants this exact list only to the
    // bundled main/tray/huddle capability (`trusted`).
    const COMMANDS: &[&str] = &[
        "workspace_list",
        "workspace_active",
        "gateway_route_bind",
        "gateway_route_unbind",
        "gateway_route_list",
        "gateway_open_window",
        "workspace_create",
        "workspace_join",
        "workspace_invite_blob",
        "workspace_join_requests",
        "workspace_admit",
        "workspace_promote",
        "workspace_resident_remove",
        "workspace_demote",
        "workspace_request_leave",
        "workspace_forget",
        "workspace_select",
        "workspace_phase",
        "workspace_log_tail",
        "workspace_runtime_facts",
        "user_identity_confirm_mnemonic",
        "user_identity_status",
        "user_identity_state",
        "user_identity_create",
        "user_identity_restore",
        "user_identity_unlock",
        "user_identity_reveal",
        "user_identity_encrypt",
        "user_identity_lock",
        "user_sign_bind",
        "user_sign_unbind",
        "user_sign_gateway_route",
        "user_sign_possession",
        "user_sign_add_member",
        "user_sign_remove_member",
        "enroll_start",
        "enroll_poll",
        "enroll_cancel",
        "forge_list_repos",
        "forge_list_branches",
        "forge_head",
        "forge_log",
        "forge_tree",
        "forge_read_file",
        "forge_read_file_page",
        "forge_diff",
        "forge_compare",
        "forge_build_merge",
        "tray_open_console",
        "tray_quit",
        "huddle_pop_out",
        "huddle_pop_in",
        "notify_configure",
        "notify_mark_seen",
    ];
    let main_source = std::fs::read_to_string("src/main.rs").expect("read Tauri command registry");
    let handler = main_source
        .split_once(".invoke_handler(tauri::generate_handler![")
        .and_then(|(_, rest)| rest.split_once("]);").map(|(body, _)| body))
        .expect("find Tauri generate_handler registry");
    let registered: std::collections::BTreeSet<&str> = handler
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_suffix(',')
                .and_then(|line| line.rsplit_once("::").map(|(_, command)| command))
        })
        .collect();
    let declared: std::collections::BTreeSet<&str> = COMMANDS.iter().copied().collect();
    assert_eq!(
        registered, declared,
        "Tauri invoke handler and ACL command manifest drifted"
    );
    let trusted = std::fs::read_to_string("permissions/trusted.toml")
        .expect("read trusted application permission");
    for command in COMMANDS {
        assert!(
            trusted.contains(&format!("\"{command}\"")),
            "trusted application permission omits {command}"
        );
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("build Tauri application manifest")
}
