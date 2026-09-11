//! THE CANARY RUNNER — headless evidence that the module-owned views follow
//! a node's deployments while the app runs, from a box with no window.
//!
//! It seats the five module-owned views from the node's active artifacts the
//! way the app does (`backend::connect` → `module_view::connected`), then
//! polls the deployments once a second (`deployments_checked`, what a block
//! tip spawns). On every `view_source` transition that names a hash —
//! Ready, Swapped, Missing, Failed — it writes the line to
//! `<out>/view_source.log` and a PNG of that module's tab to
//! `<out>/<n>-<module>-<state>-<hash8>.png`. The steps themselves (activate
//! A, then B, then an asset-only B′, then a removal) are somebody else's to
//! drive; this runs until it is killed, or `DUCKTAPE_CANARY_STEPS` captures.
//! Beside each PNG, `<n>-….txt` holds every text in the transitioned view's
//! tree, and the live run asserts what the canary views promise: the B
//! views carry a visible marker (`CANARY B`, `· B`) the A views do not, and
//! B′ changes the governance seal's colour and nothing else.
//!
//! Against a real node, from a fresh home, with the six desktop views staged
//! where `DUCKTAPE_VIEWS_DIR` points:
//!
//! ```text
//! DUCKTAPE_HOME=$(mktemp -d) DUCKTAPE_VIEWS_DIR=target/views \
//! DUCKTAPE_NODE=http://127.0.0.1:<http port> DUCKTAPE_CANARY_OUT=target/canary \
//! DUCKTAPE_CANARY_STEPS=8 DUCKTAPE_CANARY_MARKERS=1 \
//! cargo test -p ducktape-app -- --ignored --nocapture canary_follows_a_live_node
//! ```
//!
//! The in-suite test below drives the same runner through the fake node the
//! view-source tests use, A → B → B′ → removal, and reads the four PNGs back:
//! the seal the deployment ships is what changes between B and B′, so their
//! pixels must differ.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use iced::advanced::renderer::Headless as _;
use iced::advanced::{clipboard, mouse, renderer};
use iced::{Color, Event, Size, Theme};
use iced_test::runtime::user_interface::{self, UserInterface};

use crate::module_view::canary::{drawn, seated_hash, tap, texts};

/// A tab as captured: console-sized, one pixel per point.
const TAB: Size = Size::new(900.0, 600.0);
/// How often the deployments are checked — a block interval, near enough.
const POLL: Duration = Duration::from_secs(1);
/// How long a logged Ready waits for its seat before the capture is taken
/// anyway (the line is logged before the seat, the seat is microseconds).
const SEAT: Duration = Duration::from_secs(5);
/// The corner of the Approvals tab the seal is drawn in.
const SEAL: (u32, u32) = (64, 64);

/// The text a module's canary B view shows that its A view does not.
fn marker(module: &str) -> &'static str {
    match module {
        "files" => "+ Folder · B",
        "pages" => "Pages · B",
        "chat" => "CHANNELS · B",
        _ => "CANARY B",
    }
}

/// What a capture left behind, for the next one of the same module to be
/// judged against.
struct Seen {
    state: String,
    hash: String,
    seal: Vec<u8>,
}

struct Transition {
    module: String,
    state: String,
    hash: String,
}

/// `module=… hash=… state=… gen=… reason=…`, as `view_source` logs it.
fn parse(line: &str) -> Option<Transition> {
    let field = |name: &str| {
        line.split_whitespace()
            .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
            .map(str::to_owned)
    };
    Some(Transition {
        module: field("module")?,
        state: field("state")?,
        hash: field("hash")?,
    })
}

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set — see this module's doc"))
}

/// Seats the views from `node` and follows its deployments, writing into
/// `out`; returns after `steps` captures (0: only when killed). With
/// `markers`, every transition is also judged against what the canary
/// views promise (see the module doc) — and a broken promise is a panic.
fn run(node: &str, out: &Path, steps: usize, markers: bool) -> usize {
    use std::io::Write as _;
    std::fs::create_dir_all(out).expect("the output directory");
    let mut log = std::fs::File::create(out.join("view_source.log")).expect("the log file");
    let lines = tap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime for the node");
    // the app's own connect: the views are seated from the node before the
    // workspace is read, and a workspace this box cannot read is not the
    // evidence — the views are
    match runtime.block_on(crate::backend::connect(node.to_owned(), 0, 0)) {
        Ok(_) => eprintln!("canary: connected to {node}"),
        Err(error) => eprintln!(
            "canary: connected to {node}; workspace not read: {}",
            error.message
        ),
    }
    let mut renderer = crate::frame_probe::headless_renderer();
    let mut captures = 0;
    let mut polled = Instant::now();
    let mut seen: std::collections::HashMap<String, Seen> = std::collections::HashMap::new();
    loop {
        match lines.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                writeln!(log, "{line}").expect("the log line");
                let Some(step) = parse(&line).filter(|step| {
                    step.hash != "-"
                        && ["Ready", "Swapped", "Missing", "Failed"].contains(&step.state.as_str())
                }) else {
                    continue;
                };
                seated(&step);
                captures += 1;
                let name = format!(
                    "{captures}-{}-{}-{}.png",
                    step.module,
                    step.state,
                    &step.hash[..8]
                );
                let seal = capture(&mut renderer, &step.module, &out.join(&name));
                let shown = shown(&step.module);
                std::fs::write(out.join(name.replace(".png", ".txt")), shown.join("\n"))
                    .expect("the texts file");
                let marked = shown.iter().any(|text| text.contains(marker(&step.module)));
                eprintln!("canary: {name} marker={marked} texts={}", shown.len());
                if markers {
                    judge(&step, marked, &shown, &seal, seen.get(&step.module));
                }
                seen.insert(
                    step.module.clone(),
                    Seen {
                        state: step.state.clone(),
                        hash: step.hash.clone(),
                        seal,
                    },
                );
                if steps > 0 && captures >= steps {
                    return captures;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return captures,
        }
        if polled.elapsed() >= POLL {
            drop(runtime.block_on(crate::module_view::deployments_checked()));
            polled = Instant::now();
        }
    }
}

/// The texts of `module`'s tree, or none for a module this app does not own.
fn shown(module: &str) -> Vec<String> {
    crate::backend::view_source::MODULE_OWNED
        .into_iter()
        .find(|owned| *owned == module)
        .map(texts)
        .unwrap_or_default()
}

/// The canary views' promise, held against this transition: an A (Ready)
/// shows no marker, a B (Swapped) moves the hash and shows its module's
/// marker, a B′ (a Swapped after a Swapped) changes the governance seal, a
/// removal (Missing) shows nothing. A module's marker is read only where
/// its tab has a tree and is past "Not connected" — this read-only client never connects the
/// Files tab to a workspace, and its marker sits behind that.
fn judge(step: &Transition, marked: bool, shown: &[String], seal: &[u8], before: Option<&Seen>) {
    let module = &step.module;
    let readable = module == "governance"
        || (!shown.is_empty() && !shown.iter().any(|text| text.contains("Not connected")));
    match step.state.as_str() {
        "Ready" => assert!(
            !marked,
            "{module} shows the B marker before any swap: {shown:?}"
        ),
        "Swapped" => {
            if let Some(before) = before {
                assert_ne!(before.hash, step.hash, "{module} swapped to the same hash");
            }
            assert!(
                marked || !readable,
                "{module} swapped without its marker {:?}: {shown:?}",
                marker(module)
            );
            if module == "governance" && before.is_some_and(|before| before.state == "Swapped") {
                assert!(
                    before.is_some_and(|before| before.seal != seal),
                    "the governance seal did not change between B and B′"
                );
            }
        }
        "Missing" => assert!(
            shown.is_empty(),
            "{module} removed but still drawn: {shown:?}"
        ),
        _ => {}
    }
}

/// A Ready is logged before its seat: wait for the slot to answer for the
/// hash the line names, so the capture shows the view the line is about.
fn seated(step: &Transition) {
    let module = crate::backend::view_source::MODULE_OWNED
        .into_iter()
        .find(|module| *module == step.module);
    let (Some(module), false) = (module, step.state == "Failed") else {
        std::thread::sleep(Duration::from_millis(300));
        return;
    };
    let until = Instant::now() + SEAT;
    while Instant::now() < until {
        let seated = seated_hash(module).map(|hash| crate::backend::hex_encode(&hash));
        if seated.as_deref() == Some(step.hash.as_str()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!(
        "canary: {} did not seat {} within {SEAT:?}",
        step.module, step.hash
    );
}

/// `module`'s tab drawn to `path`: three frames, so the view's own
/// requests are routed and its tree rebuilt, then the pixels — of which the
/// seal's corner comes back, for the next capture to be held against.
fn capture(renderer: &mut iced::Renderer, module: &str, path: &Path) -> Vec<u8> {
    let mut cache = user_interface::Cache::default();
    let mut clipboard = clipboard::Null;
    let owned = crate::backend::view_source::MODULE_OWNED
        .into_iter()
        .find(|owned| *owned == module)
        .expect("a module this app owns");
    for _ in 0..3 {
        // the Approvals tab under the props the seal is judged on; every
        // other module's tree as it stands, a read-only client's
        let element = if module == "governance" {
            crate::module_view::governance_view(false, true, false)
        } else {
            drawn(owned)
        };
        let mut ui = UserInterface::build(element, TAB, cache, renderer);
        let mut messages = Vec::new();
        ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            ))],
            mouse::Cursor::Unavailable,
            renderer,
            &mut clipboard,
            &mut messages,
        );
        ui.draw(
            renderer,
            &Theme::Light,
            &renderer::Style {
                text_color: Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
    }
    let (width, height) = (TAB.width as u32, TAB.height as u32);
    let rgba = renderer.screenshot(Size::new(width, height), 1.0, Color::WHITE);
    let tab = image::RgbaImage::from_raw(width, height, rgba).expect("a full RGBA buffer");
    tab.save(path).expect("the PNG");
    image::imageops::crop_imm(&tab, 0, 0, SEAL.0, SEAL.1)
        .to_image()
        .into_raw()
}

/// A capture names its module: the chat tab and the Approvals tab, seated
/// from one node, are not the same picture.
#[test]
fn a_capture_draws_the_module_it_names() {
    use crate::backend::view_source::tests::{FakeDeployment, fake_node};
    let views = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/views");
    let (governance, chat) = (
        views.join("governance_view.wasm"),
        views.join("chat_view.wasm"),
    );
    if !governance.is_file() || !chat.is_file() {
        eprintln!(
            "skipped: no staged views under {} — run `make views`",
            views.display()
        );
        return;
    }
    let governance = artifact(
        &std::fs::read(governance).unwrap(),
        vec![7],
        seal("#d00000"),
    );
    // codes of their own: a seat left at the transition test's A would
    // swallow that test's first transition
    let chat = artifact(&std::fs::read(chat).unwrap(), vec![8], seal("#d00000"));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _turn = runtime.block_on(crate::module_view::canary::connection_turn());
    let node = FakeDeployment::serving("governance", &governance);
    node.artifacts.lock().unwrap().push(chat.clone());
    *node.status.lock().unwrap() = serde_json::json!({"module_status": {"modules": [
        {"module_id": "governance", "active_code_hash": governance.hash(), "pending": null,
         "history": [{"height": 7, "code_hash": governance.hash()}]},
        {"module_id": "chat", "active_code_hash": chat.hash(), "pending": null,
         "history": [{"height": 7, "code_hash": chat.hash()}]},
    ]}});
    let client = runtime.block_on(fake_node(node));
    // the loads the connection starts, joined: the seats are in
    crate::module_view::connected(&client).joined();
    for (module, artifact) in [("governance", &governance), ("chat", &chat)] {
        assert_eq!(
            seated_hash(module),
            Some(artifact.hash()),
            "{module} seated"
        );
    }

    let out = std::env::temp_dir().join(format!("ducktape-canary-tabs-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let mut renderer = crate::frame_probe::headless_renderer();
    capture(&mut renderer, "governance", &out.join("governance.png"));
    capture(&mut renderer, "chat", &out.join("chat.png"));
    assert_ne!(
        std::fs::read(out.join("governance.png")).unwrap(),
        std::fs::read(out.join("chat.png")).unwrap(),
        "the chat capture is the Approvals tab"
    );
    assert!(
        texts("chat").iter().any(|text| text.contains("CHANNELS")),
        "{:?}",
        texts("chat")
    );
}

#[test]
#[ignore = "needs a live node; see the module doc"]
fn canary_follows_a_live_node() {
    let node = required("DUCKTAPE_NODE");
    let out = std::env::var("DUCKTAPE_CANARY_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/canary"));
    let steps = std::env::var("DUCKTAPE_CANARY_STEPS")
        .ok()
        .and_then(|steps| steps.parse().ok())
        .unwrap_or(0);
    // the canary views' promise is held only when asked: a rehearsal
    // network may seat anything
    let markers = std::env::var_os("DUCKTAPE_CANARY_MARKERS").is_some();
    let captures = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || run(&node, &out, steps, markers))
        .expect("the canary thread")
        .join()
        .expect("the canary finishes");
    println!("canary: {captures} captures");
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

fn pngs(out: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(out)
        .expect("the output directory")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".png"))
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The runner through the fake node, A → B → B′ (the same code, a seal of
/// another colour) → removal: four PNGs named for the transitions, the
/// lines that named them, and pixels that changed with the seal.
#[test]
fn the_canary_captures_every_transition_of_a_deployment() {
    use crate::backend::view_source::tests::{FakeDeployment, fake_node};
    let staged = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/views/governance_view.wasm");
    if !staged.is_file() {
        eprintln!("skipped: no {} — run `make views`", staged.display());
        return;
    }
    let component = std::fs::read(staged).expect("the staged view");
    let a = artifact(&component, vec![1], seal("#d00000"));
    let b = artifact(&component, vec![2], seal("#d00000"));
    let b_prime = artifact(&component, vec![2], seal("#0000d0"));
    let removed = module_artifact::ModuleArtifact::component(vec![3]);
    let hash8 = |artifact: &module_artifact::ModuleArtifact| {
        crate::backend::hex_encode(&artifact.hash())[..8].to_owned()
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _turn = runtime.block_on(crate::module_view::canary::connection_turn());
    let node = FakeDeployment::serving("governance", &a);
    let origin = runtime
        .block_on(fake_node(node.clone()))
        .origin()
        .to_owned();
    let out = std::env::temp_dir().join(format!("ducktape-canary-{}", std::process::id()));
    let canary = {
        let out = out.clone();
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || run(&origin, &out, 4, false))
            .unwrap()
    };
    // each step once the runner has captured the one before
    for (count, artifact) in [&b, &b_prime, &removed].into_iter().enumerate() {
        let until = Instant::now() + Duration::from_secs(60);
        // one PNG per capture so far (a .txt sits beside each, and the log)
        while !out.is_dir() || pngs(&out).len() < count + 1 {
            assert!(Instant::now() < until, "capture {} never came", count + 1);
            std::thread::sleep(Duration::from_millis(100));
        }
        node.deploy("governance", artifact);
    }
    assert_eq!(canary.join().unwrap(), 4);

    // A is Ready in a fresh process; a view an earlier test left seated
    // makes it a Swapped — the same seat either way
    let expected = pngs(&out);
    let first = expected.first().cloned().unwrap_or_default();
    let (a_state, first_named) = match first.strip_prefix("1-governance-") {
        Some(rest) if rest == format!("Ready-{}.png", hash8(&a)) => ("Ready", true),
        Some(rest) if rest == format!("Swapped-{}.png", hash8(&a)) => ("Swapped", true),
        _ => ("Ready", false),
    };
    assert!(first_named, "the first capture names A: {expected:?}");
    assert_eq!(
        expected[1..],
        [
            format!("2-governance-Swapped-{}.png", hash8(&b)),
            format!("3-governance-Swapped-{}.png", hash8(&b_prime)),
            format!("4-governance-Missing-{}.png", hash8(&removed)),
        ]
    );
    // the texts beside each PNG: the tab's, then nothing once removed
    let shown =
        |name: &str| std::fs::read_to_string(out.join(name.replace(".png", ".txt"))).unwrap();
    assert!(shown(&expected[1]).contains("Approvals"));
    assert!(shown(&expected[3]).is_empty(), "{}", shown(&expected[3]));
    let log = std::fs::read_to_string(out.join("view_source.log")).unwrap();
    for (state, artifact) in [
        (a_state, &a),
        ("Swapped", &b),
        ("Swapped", &b_prime),
        ("Missing", &removed),
    ] {
        let hash = crate::backend::hex_encode(&artifact.hash());
        assert!(
            log.contains(&format!("module=governance hash={hash} state={state} ")),
            "no {state} line for {hash} in:\n{log}"
        );
    }
    // the seal, by its pixels: a 24px disc is hundreds of them, of one hue
    let seal_pixels = |name: &str, hue: fn(&[u8; 4]) -> bool| {
        image::open(out.join(name))
            .unwrap()
            .into_rgba8()
            .pixels()
            .filter(|pixel| hue(&pixel.0))
            .count()
    };
    let red = |p: &[u8; 4]| p[0] > 150 && p[1] < 80 && p[2] < 80;
    let blue = |p: &[u8; 4]| p[2] > 150 && p[0] < 80 && p[1] < 80;
    assert!(seal_pixels(&expected[1], red) > 200, "B shows the red seal");
    assert_eq!(seal_pixels(&expected[1], blue), 0);
    assert!(
        seal_pixels(&expected[2], blue) > 200,
        "B′ shows the blue seal"
    );
    assert_eq!(seal_pixels(&expected[2], red), 0);
    assert_eq!(
        seal_pixels(&expected[3], blue),
        0,
        "the removal empties the tab"
    );
}
