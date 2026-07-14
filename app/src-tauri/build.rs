fn main() {
    // ── tauri.conf.json `security.csp` rationale (JSON carries no comments) ──
    // `connect-src` is scheme-wide (`http: https: ws: wss:`) so a user-entered
    // remote node origin is dialable (#599). Accepted ONLY because `script-src`
    // stays `'self'`: no inline or remote script can run in the webview, so
    // nothing untrusted exists to exfiltrate over the widened connect-src.
    // Widening `script-src` re-opens this decision. The post-#599 follow-up is
    // a runtime per-endpoint allowlist (admit exactly the node origins the user
    // connected to). Full rationale: docs/superpowers/specs/
    // 2026-07-14-w2-owner-control-design.md.

    // Pin the CEF runtime lookup to the binary's own directory. cef-dll-sys
    // stages libcef.so and its support files next to the binary on every
    // build, and the Linux `make install-app` stages the same set beside the
    // installed binary. --disable-new-dtags emits DT_RPATH, which ld.so
    // consults BEFORE LD_LIBRARY_PATH — deliberate: the app is ABI-pinned to
    // its CEF distribution, and an ambient path to a system CEF (e.g.
    // /usr/lib/cef) boots via API-versioning but silently drops features
    // (cef-vaapi-bin 150 has no GTK input-method integration, killing IME).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
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
    const TRUSTED_COMMANDS: &[&str] = &[
        "workspace_list",
        "workspace_active",
        "gateway_route_bind",
        "gateway_route_unbind",
        "gateway_route_list",
        "gateway_open_inline",
        "gateway_inline_place",
        "gateway_inline_close",
        "gateway_inline_hide_all",
        "duck_set_gateway_base",
        "workspace_create",
        "workspace_join",
        "workspace_join_code",
        "workspace_invite_blob",
        "workspace_join_requests",
        "workspace_forget",
        "workspace_select",
        "workspace_sandbox_apply",
        "workspace_phase",
        "workspace_log_tail",
        "workspace_runtime_facts",
        "user_identity_confirm_mnemonic",
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
        "user_sign_files_frame",
        "user_sign_frame",
        "user_sign_admin",
        "touchid_available",
        "touchid_enroll",
        "touchid_enrolled",
        "touchid_unlock",
        "touchid_disable",
        "enroll_start",
        "enroll_poll",
        "enroll_cancel",
        "link_relay_start",
        "link_relay_poll",
        "link_relay_cancel",
        "link_fetch_challenge",
        "link_send_response",
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
        "forge_sync_remote",
        "tray_open_console",
        "tray_quit",
        "huddle_pop_out",
        "huddle_pop_in",
        "notify_configure",
        "notify_mark_seen",
        "sandbox_preflight",
        "notify_recent",
    ];
    // The native permission consent window's own commands. They answer requests
    // made by OTHER webviews, so they live in their own capability
    // (`permissions/prompt.toml`, window label `permission-prompt`) and are
    // deliberately absent from the console's `trusted` surface.
    const PROMPT_COMMANDS: &[&str] = &["permission_prompt_state", "permission_prompt_decide"];

    // Leaked: the manifest borrows the list for the rest of the build script.
    let commands: &'static [&str] = Vec::leak(
        TRUSTED_COMMANDS
            .iter()
            .chain(PROMPT_COMMANDS)
            .copied()
            .collect::<Vec<_>>(),
    );
    let main_source = std::fs::read_to_string("src/main.rs").expect("read Tauri command registry");
    let handler = main_source
        .split_once(".invoke_handler(tauri::generate_handler![")
        .and_then(|(_, rest)| rest.split_once("]);").map(|(body, _)| body))
        .expect("find Tauri generate_handler registry");
    let registered: std::collections::BTreeSet<&str> = handler
        .lines()
        .filter_map(|line| {
            // Commands register as `module::command` or as a bare `command`;
            // both forms must land in the ACL or the drift assert is vacuous.
            let line = line.trim().strip_suffix(',')?;
            Some(line.rsplit_once("::").map_or(line, |(_, command)| command))
        })
        .collect();
    let declared: std::collections::BTreeSet<&str> = commands.iter().copied().collect();
    assert_eq!(
        registered, declared,
        "Tauri invoke handler and ACL command manifest drifted"
    );
    let trusted = std::fs::read_to_string("permissions/trusted.toml")
        .expect("read trusted application permission");
    for command in TRUSTED_COMMANDS {
        assert!(
            trusted.contains(&format!("\"{command}\"")),
            "trusted application permission omits {command}"
        );
    }
    let prompt =
        std::fs::read_to_string("permissions/prompt.toml").expect("read consent window permission");
    for command in PROMPT_COMMANDS {
        assert!(
            prompt.contains(&format!("\"{command}\"")),
            "permission-prompt permission omits {command}"
        );
        assert!(
            !trusted.contains(&format!("\"{command}\"")),
            "{command} answers another webview's permission request — it must stay out of the console's trusted surface"
        );
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("build Tauri application manifest")
}
