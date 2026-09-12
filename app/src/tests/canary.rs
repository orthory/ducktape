//! Deployment canary: real native scenes, hydrated WASM trees, and optional GPU pixels.
//! DUCKTAPE_CANARY_PIXELS=1 requires an actual screenshot backend; unsupported
//! platforms fail explicitly. No polling or synthetic screenshot substitutes.
use crate::module_view::canary::{frame, seated_hash, tap, texts};
use gpui_kit::{AppContext, px, size};
use std::path::{Path, PathBuf};
fn component(module: &str) -> Vec<u8> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../target/views/{module}_view.wasm"));
    std::fs::read(&path)
        .unwrap_or_else(|error| panic!("{}: {error}; run make views", path.display()))
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn capture(module: &'static str, path: &Path) -> Vec<u8> {
    let mut cx = crate::frame_probe::headless_context();
    let mut app = super::Ducktape::__state();
    app.connected = true;
    app.shell_tab = match module {
        "chat" => super::ShellTab::Chat,
        "pages" => super::ShellTab::Pages,
        "files" => super::ShellTab::Files,
        "members" => super::ShellTab::Members,
        "governance" => super::ShellTab::Governance,
        _ => panic!("unknown canary module {module}"),
    };
    let (spec, _) = app.native_view();
    let window = cx
        .open_window(size(px(900.), px(600.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = crate::module_view::NativeModuleView::new(module);
                view.set_props(spec.props, cx);
                view
            });
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let bytes = ui_lang_wire::encode(&frame(module));
    std::fs::write(path.with_extension("wire"), &bytes).unwrap();
    std::fs::write(path.with_extension("txt"), texts(module).join("\n")).unwrap();
    if std::env::var_os("DUCKTAPE_CANARY_PIXELS").is_some() {
        cx.capture_screenshot(window.into())
            .expect("requested real GPU screenshot unavailable")
            .save(path.with_extension("png"))
            .unwrap();
    }
    bytes
}
fn seal(colour: &str) -> Vec<u8> {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="11" fill="{colour}"/></svg>"#
    )
    .into_bytes()
}

fn artifact(component: &[u8], code: Vec<u8>, seal: Vec<u8>) -> module_artifact::ModuleArtifact {
    module_artifact::ModuleArtifact {
        component: code,
        index: None,
        view: Some(module_artifact::ViewArtifact {
            component: component.to_vec(),
            assets: [("icons/seal.svg".to_owned(), seal)].into(),
        }),
    }
}

#[test]
fn a_capture_draws_the_module_it_names() {
    use crate::backend::view_source::tests::{FakeDeployment, fake_node};
    let runtime = runtime();
    let _turn = runtime.block_on(crate::module_view::canary::connection_turn());
    let governance = artifact(&component("governance"), vec![7], seal("#d00000"));
    let chat = artifact(&component("chat"), vec![8], seal("#d00000"));
    let node = FakeDeployment::serving("governance", &governance);
    node.artifacts.lock().unwrap().push(chat.clone());
    *node.status.lock().unwrap() = serde_json::json!({"module_status":{"modules":[
        {"module_id":"governance","active_code_hash":governance.hash(),"pending":null,"history":[{"height":7,"code_hash":governance.hash()}]},
        {"module_id":"chat","active_code_hash":chat.hash(),"pending":null,"history":[{"height":7,"code_hash":chat.hash()}]}
    ]}});
    let client = runtime.block_on(fake_node(node));
    crate::module_view::connected(&client).joined();
    let out = tempfile::tempdir().unwrap();
    let a = capture("governance", &out.path().join("governance"));
    let b = capture("chat", &out.path().join("chat"));
    assert_eq!(seated_hash("governance"), Some(governance.hash()));
    assert_eq!(seated_hash("chat"), Some(chat.hash()));
    assert_ne!(a, b);
    assert!(texts("chat").iter().any(|text| text.contains("CHANNELS")));
}
#[test]
fn the_canary_captures_every_transition_of_a_deployment() {
    use crate::backend::view_source::tests::{FakeDeployment, fake_node};
    let runtime = runtime();
    let _turn = runtime.block_on(crate::module_view::canary::connection_turn());
    let component = component("governance");
    let a = artifact(&component, vec![1], seal("#d00000"));
    let b = artifact(&component, vec![2], seal("#d00000"));
    let bp = artifact(&component, vec![2], seal("#0000d0"));
    let removed = module_artifact::ModuleArtifact::component(vec![3]);
    let node = FakeDeployment::serving("governance", &a);
    let client = runtime.block_on(fake_node(node.clone()));
    let transitions = tap();
    let out = tempfile::tempdir().unwrap();
    let mut captures = Vec::new();
    crate::module_view::connected(&client).joined();
    for (index, artifact) in [&a, &b, &bp, &removed].into_iter().enumerate() {
        if index > 0 {
            node.deploy("governance", artifact);
            runtime
                .block_on(crate::module_view::deployments_checked())
                .joined();
        }
        let hash = crate::backend::hex_encode(&artifact.hash());
        let lines = transitions.try_iter().collect::<Vec<_>>().join("\n");
        assert!(
            lines.contains(&format!("module=governance hash={hash}")),
            "{lines}"
        );
        let state = match index {
            0 => "",
            1 | 2 => "state=Swapped",
            _ => "state=Missing",
        };
        assert!(lines.contains(state), "{lines}");
        assert_eq!(seated_hash("governance"), Some(artifact.hash()));
        captures.push(capture(
            "governance",
            &out.path().join(format!("{index}-{hash}")),
        ));
    }
    let has =
        |bytes: &[u8], needle: &[u8]| bytes.windows(needle.len()).any(|slice| slice == needle);
    assert!(
        has(&captures[1], b"#d00000"),
        "B delivers red SVG to native renderer"
    );
    assert!(
        has(&captures[2], b"#0000d0"),
        "asset-only B-prime delivers blue SVG"
    );
    assert!(!has(&captures[2], b"#d00000"));
    assert!(texts("governance").is_empty());
    assert!(frame("governance").is_none());
    assert!(!has(&captures[3], b"#0000d0"));
    if std::env::var_os("DUCKTAPE_CANARY_PIXELS").is_some() {
        let pixels = |index, artifact: &module_artifact::ModuleArtifact, blue| {
            let hash = crate::backend::hex_encode(&artifact.hash());
            image::open(out.path().join(format!("{index}-{hash}.png")))
                .unwrap()
                .into_rgba8()
                .pixels()
                .filter(|p| {
                    if blue {
                        p[2] > 150 && p[0] < 80 && p[1] < 80
                    } else {
                        p[0] > 150 && p[1] < 80 && p[2] < 80
                    }
                })
                .count()
        };
        assert!(pixels(1, &b, false) > 200);
        assert_eq!(pixels(1, &b, true), 0);
        assert!(pixels(2, &bp, true) > 200);
        assert_eq!(pixels(2, &bp, false), 0);
        assert_eq!(pixels(3, &removed, true), 0);
    }
}
#[test]
#[ignore = "needs a live node and staged views"]
fn canary_follows_a_live_node() {
    use futures::StreamExt;
    let node = std::env::var("DUCKTAPE_NODE").expect("DUCKTAPE_NODE");
    let out = std::env::var("DUCKTAPE_CANARY_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| "target/canary".into());
    std::fs::create_dir_all(&out).unwrap();
    let steps = std::env::var("DUCKTAPE_CANARY_STEPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0usize);
    let runtime = runtime();
    let _turn = runtime.block_on(crate::module_view::canary::connection_turn());
    let transitions = tap();
    let _ = runtime.block_on(crate::backend::connect(node.clone(), 0, 0));
    let mut live = crate::backend::live_events(node);
    let mut count = 0;
    let mut log = String::new();
    loop {
        for line in transitions.try_iter() {
            log.push_str(&line);
            log.push('\n');
            let fields = line
                .split_whitespace()
                .filter_map(|s| s.split_once('='))
                .collect::<std::collections::HashMap<_, _>>();
            let state = fields.get("state").copied().unwrap_or("");
            if !["Ready", "Swapped", "Missing", "Failed"].contains(&state) {
                continue;
            }
            let Some(module) = crate::backend::view_source::MODULE_OWNED
                .into_iter()
                .find(|module| Some(module) == fields.get("module"))
            else {
                continue;
            };
            count += 1;
            capture(module, &out.join(format!("{count}-{module}-{state}")));
            std::fs::write(out.join("view_source.log"), &log).unwrap();
            if steps > 0 && count >= steps {
                return;
            }
        }
        runtime
            .block_on(live.next())
            .expect("live stream ended before captures");
        runtime
            .block_on(crate::module_view::deployments_checked())
            .joined();
    }
}
