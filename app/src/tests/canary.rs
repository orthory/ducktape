//! THE CANARY RUNNER — headless evidence that the module-owned views follow
//! a node's deployments while the app runs, from a box with no window.
//!
//! It seats the five module-owned views from the node's active artifacts the
//! way the app does (`backend::connect` → `module_view::connected`), then
//! polls the deployments once a second (`deployments_checked`, what a block
//! tip spawns). On every `view_source` transition that names a hash —
//! Ready, Swapped, Missing, Failed — it writes the line to
//! `<out>/view_source.log` and a PNG of the Approvals tab to
//! `<out>/<n>-<module>-<state>-<hash8>.png`. The steps themselves (activate
//! A, then B, then an asset-only B′, then a removal) are somebody else's to
//! drive; this runs until it is killed, or `DUCKTAPE_CANARY_STEPS` captures.
//!
//! Against a real node, from a fresh home, with the six desktop views staged
//! where `DUCKTAPE_VIEWS_DIR` points:
//!
//! ```text
//! DUCKTAPE_HOME=$(mktemp -d) DUCKTAPE_VIEWS_DIR=target/views \
//! DUCKTAPE_NODE=http://127.0.0.1:<http port> DUCKTAPE_CANARY_OUT=target/canary \
//! DUCKTAPE_CANARY_STEPS=4 \
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

use crate::module_view::canary::{mount_module_owned, seated_hash, tap};

/// The Approvals tab as captured: a console-sized tab, one pixel per point.
const TAB: Size = Size::new(900.0, 600.0);
/// How often the deployments are checked — a block interval, near enough.
const POLL: Duration = Duration::from_secs(1);
/// How long a logged Ready waits for its seat before the capture is taken
/// anyway (the line is logged before the seat, the seat is microseconds).
const SEAT: Duration = Duration::from_secs(5);

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
/// `out`; returns after `steps` captures (0: only when killed).
fn run(node: &str, out: &Path, steps: usize) -> usize {
    use std::io::Write as _;
    std::fs::create_dir_all(out).expect("the output directory");
    let mut log = std::fs::File::create(out.join("view_source.log")).expect("the log file");
    let lines = tap();
    mount_module_owned();
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
                capture(&mut renderer, &out.join(&name));
                eprintln!("canary: {name}");
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

/// The Approvals tab drawn to `path`: three frames, so the view's own
/// requests are routed and its tree rebuilt, then the pixels.
fn capture(renderer: &mut iced::Renderer, path: &Path) {
    let mut cache = user_interface::Cache::default();
    let mut clipboard = clipboard::Null;
    for _ in 0..3 {
        let mut ui = UserInterface::build(
            crate::module_view::governance_view(false, true, false, true, "", &[]),
            TAB,
            cache,
            renderer,
        );
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
    image::RgbaImage::from_raw(width, height, rgba)
        .expect("a full RGBA buffer")
        .save(path)
        .expect("the PNG");
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
    let captures = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || run(&node, &out, steps))
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
            .spawn(move || run(&origin, &out, 4))
            .unwrap()
    };
    // each step once the runner has captured the one before
    for (count, artifact) in [&b, &b_prime, &removed].into_iter().enumerate() {
        let until = Instant::now() + Duration::from_secs(60);
        // the log, and one PNG per capture so far
        while std::fs::read_dir(&out).map_or(0, |dir| dir.count()) < count + 2 {
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
