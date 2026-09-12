//! Native layout and authored WASM presentation contracts.
use super::*;
use gpui_kit::{AppContext, px, size};

#[test]
fn full_view_fits_the_default_test_stack() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            for (kind, width, height) in [
                (crate::shell::WindowKind::Console, 1280., 800.),
                (crate::shell::WindowKind::Onboarding, 480., 640.),
                (crate::shell::WindowKind::Huddle, 320., 460.),
            ] {
                let mut cx = crate::frame_probe::headless_context();
                let mut app = Ducktape::initial_state();
                app.hub_step = HubStep::Networks;
                let window = cx
                    .open_window(size(px(width), px(height)), |window, cx| {
                        let view = crate::shell::test_window(app, kind, window, cx);
                        cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
                    })
                    .expect("native screen opens");
                cx.update_window(window.into(), |_, window, cx| {
                    window.draw(cx).clear(cx);
                    assert_eq!(window.viewport_size(), size(px(width), px(height)));
                    assert!(
                        !window.painted_quads().is_empty(),
                        "native screen paints its surface"
                    );
                })
                .unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn the_page_surface_is_one_editor_with_no_click_to_edit_left() {
    let root = rust_tokens(include_str!(
        "../../../crates/views/pages/src/ui/app_view.rs"
    ));
    let pages = rust_tokens(include_str!("../../../crates/views/pages/src/ui/pages.rs"));
    assert_eq!(pages.matches("Node::Editor").count(), 1);
    assert!(pages.contains("EditorOptions"));
    assert!(!root.contains("Clicktoedit"));
    let host = rust_tokens(include_str!("../editor/blocks.rs"));
    assert!(host.contains("line_projections("));
    assert!(host.contains("format.size"));
    assert!(host.contains("source_at("));
}
#[test]
fn shell_keeps_opaque_window_and_alpha_authored_content() {
    let source = rust_tokens(include_str!("../shell.rs"));
    assert!(source.contains("desktop-root"));
    assert!(!source.contains("WindowBackgroundAppearance::Blurred"));
    assert!(source.contains("appears_transparent:cfg!(target_os=\"macos\")"));
    let renderer = rust_tokens(include_str!("../view_tree.rs"));
    assert!(renderer.contains("wire::Background::Color"));
    assert!(renderer.contains("let[r,g,b,a]=color.0"));
}
#[test]
fn persistent_split_panes_have_native_resize_handles_and_cursor_feedback() {
    let renderer = rust_tokens(include_str!("../view_tree.rs"));
    assert!(renderer.contains("Node::ResizeHandle"));
    assert!(renderer.contains("on_mouse_down"));
    assert!(renderer.contains("on_mouse_move"));
    assert!(
        renderer.contains("cursor_col_resize")
            || renderer.contains("ResizeLeftRight")
            || renderer.contains("ResizeColumn")
    );
}
#[test]
fn message_action_toolbar_retains_named_actions_and_admission_guards() {
    let chat = rust_tokens(super::connection::CHAT);
    assert!(chat.contains("wire::Node::Hover"));
    // Native kit geometry is authoritative; the actionable label and state
    // guards remain the behavioral contract, not an old toolbar pixel size.
    assert!(chat.contains("Reactwith👍"));
    assert!(chat.contains("OpenMessageReactions("));
    assert!(chat.contains("message.deleted"));
    assert!(chat.contains("message.pending"));
    assert!(chat.contains("description:"));
}
#[test]
fn compact_controls_share_a_single_geometry_and_type_scale() {
    let design = include_str!("../../../crates/views/support/design/src/lib.rs");
    assert!(design.contains("Geist"));
    assert!(design.contains("13.5"));
    let shell = rust_tokens(include_str!("../shell.rs"));
    assert!(shell.contains("fn action") || shell.contains("fnaction("));
    assert!(shell.contains("Button::new"));
}
#[test]
fn semantic_recipes_own_action_focus_and_status_colors() {
    let renderer = rust_tokens(include_str!("../view_tree.rs"));
    for rule in ["hovered", "pressed", "disabled", "focused"] {
        assert!(renderer.contains(rule), "{rule} face");
    }
    assert!(renderer.contains("wire::ButtonPreset::Danger"));
}
#[test]
fn control_focus_ring_survives_the_active_base() {
    let renderer = rust_tokens(include_str!("../view_tree.rs"));
    assert!(renderer.contains("focused"));
    assert!(renderer.contains("border"));
    assert!(renderer.contains("focus_handle"));
}
#[test]
fn native_sources_hold_to_the_design_system() {
    for source in [
        include_str!("../shell.rs"),
        include_str!("../view_tree.rs"),
        include_str!("../editor/blocks.rs"),
    ] {
        let source = rust_tokens(source);
        assert!(!source.contains("ui_lang_runtime"));
        assert!(!source.contains("iced::"));
    }
    for tab in [
        ShellTab::Chat,
        ShellTab::Pages,
        ShellTab::Forge,
        ShellTab::Agents,
        ShellTab::Files,
        ShellTab::Explorer,
        ShellTab::Node,
        ShellTab::Members,
        ShellTab::Governance,
        ShellTab::Settings,
    ] {
        let mut state = Ducktape::initial_state();
        state.shell_tab = tab;
        let (spec, _) = state.native_view();
        assert!(!spec.module.is_empty());
        assert!(!spec.props.is_empty());
    }
}
#[test]
fn app_and_wasm_guests_do_not_resolve_the_ice_toolchain_or_iced_runtime() {
    for lockfile in [
        include_str!("../../../Cargo.lock"),
        include_str!("../../../crates/views/Cargo.lock"),
    ] {
        for line in lockfile.lines() {
            let Some(name) = line
                .strip_prefix("name = \"")
                .and_then(|name| name.strip_suffix('"'))
            else {
                continue;
            };
            let old_renderer = name == "iced" || name.starts_with("iced_");
            let old_toolchain = matches!(
                name,
                "ui-lang-runtime"
                    | "ui-lang-components"
                    | "ui-lang-compiler"
                    | "ui-lang-core"
                    | "ui-lang-guest"
            );
            assert!(
                !old_renderer && !old_toolchain,
                "removed dependency: {name}"
            );
        }
    }
}
#[test]
fn view_theme_follows_the_current_appearance_without_cache_invalidation() {
    let mut state = Ducktape::initial_state();
    for (appearance, dark) in [
        (Appearance::Dark, true),
        (Appearance::Light, false),
        (Appearance::Dark, true),
        (Appearance::System, false),
    ] {
        state.appearance = appearance;
        let (spec, _) = state.native_view();
        let props: serde_json::Value = serde_json::from_slice(&spec.props).unwrap();
        assert_eq!(props["dark"], dark);
    }
}
#[test]
fn every_current_row_marker_rests_on_one_selection_token() {
    let tree = rust_tokens(include_str!("../view_tree.rs"));
    assert!(tree.contains("wire::Face"));
    assert!(tree.contains("background"));
    let chat = rust_tokens(super::connection::CHAT);
    assert!(chat.contains("selected"));
    assert!(chat.contains("row_hover") || chat.contains("palette"));
}
#[test]
fn every_repeated_component_mount_is_culled_or_argued() {
    let renderer = rust_tokens(include_str!("../view_tree.rs"));
    assert!(renderer.contains("list("));
    assert!(renderer.contains("Node::KeyedColumn"));
    assert!(renderer.contains("virtual_row:Some("));
    assert!(renderer.contains("start") && renderer.contains("end"));
}
#[test]
fn no_view_expression_hands_an_extern_an_owned_list() {
    let source = rust_tokens(include_str!("../ui/native_view.rs"));
    for field in [
        "self.rooms.clone()",
        "self.dm_rows.clone()",
        "self.members_rows.clone()",
        "self.live_agents.clone()",
    ] {
        assert!(
            !source.contains(field),
            "borrow the app-owned collection: {field}"
        );
    }
}
#[test]
fn no_button_wears_an_icon_glyph_as_its_string_label() {
    assert!(::design::icons::svg("search").contains("<svg"));
    let shell = rust_tokens(include_str!("../shell.rs"));
    assert!(!shell.contains("label(\"⚙\")"));
    assert!(!shell.contains("label(\"✕\")"));
}
