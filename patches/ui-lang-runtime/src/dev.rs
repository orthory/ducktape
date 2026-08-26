//! Runtime support for the `cargo ice dev` process handoff.

use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Environment variable containing the readiness marker path.
#[doc(hidden)]
pub const READY_PATH_ENV: &str = "ICE_DEV_READY_PATH";

/// Environment variable containing the exact readiness marker payload.
#[doc(hidden)]
pub const READY_TOKEN_ENV: &str = "ICE_DEV_READY_TOKEN";

/// Optional draw probe that must run before the readiness marker is published.
#[doc(hidden)]
pub const REQUIRED_DRAW_ENV: &str = "ICE_DEV_REQUIRED_DRAW";

static READY_CONFIG: OnceLock<Option<ReadyConfig>> = OnceLock::new();
static READY_PUBLISHED: AtomicBool = AtomicBool::new(false);
static READY_PUBLISH_LOCK: Mutex<()> = Mutex::new(());
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);
static DRAW_PROBES: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

#[derive(Debug)]
struct ReadyConfig {
    path: PathBuf,
    token: String,
    required_draw: Option<String>,
}

/// Records that a named widget completed its renderer-specific draw path.
#[doc(hidden)]
pub fn record_draw_probe(name: &'static str) {
    let mut probes = DRAW_PROBES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    probes.get_or_insert_with(HashSet::new).insert(name);
}

/// Wraps the generated root so a dev candidate signals readiness after its
/// first successful child draw.
#[doc(hidden)]
pub fn ready<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    let content = content.into();
    if READY_CONFIG.get_or_init(ReadyConfig::from_env).is_none() {
        content
    } else {
        Element::new(Ready {
            content,
            publish: publish_ready,
        })
    }
}

struct Ready<'a, Message, Theme, Renderer, Publish>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    publish: Publish,
}

impl<Message, Theme, Renderer, Publish> Widget<Message, Theme, Renderer>
    for Ready<'_, Message, Theme, Renderer, Publish>
where
    Renderer: iced::advanced::Renderer,
    Publish: Fn(),
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
        (self.publish)();
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

fn publish_ready() {
    let Some(config) = READY_CONFIG.get_or_init(ReadyConfig::from_env) else {
        return;
    };

    if !required_draw_completed(config.required_draw.as_deref()) {
        return;
    }

    let _ = try_publish_ready(config, &READY_PUBLISHED, &READY_PUBLISH_LOCK);
}

fn required_draw_completed(required: Option<&str>) -> bool {
    let Some(required) = required else {
        return true;
    };
    let probes = DRAW_PROBES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(probes) = probes.as_ref() else {
        return false;
    };
    required
        .split(',')
        .map(str::trim)
        .filter(|probe| !probe.is_empty())
        .all(|probe| probes.contains(probe))
}

impl ReadyConfig {
    fn from_env() -> Option<Self> {
        let path = std::env::var_os(READY_PATH_ENV)?;
        let token = std::env::var(READY_TOKEN_ENV).ok()?;

        if path.is_empty() || token.is_empty() {
            return None;
        }

        Some(Self {
            path: path.into(),
            token,
            required_draw: std::env::var(REQUIRED_DRAW_ENV)
                .ok()
                .filter(|probe| !probe.is_empty()),
        })
    }
}

fn try_publish_ready(
    config: &ReadyConfig,
    published: &AtomicBool,
    publish_lock: &Mutex<()>,
) -> bool {
    if published.load(Ordering::Acquire) {
        return true;
    }

    let _guard = publish_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if published.load(Ordering::Acquire) {
        return true;
    }

    if write_ready_marker(&config.path, config.token.as_bytes()).is_err() {
        return false;
    }

    published.store(true, Ordering::Release);
    true
}

fn write_ready_marker(path: &Path, token: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "readiness marker must name a file",
        )
    })?;
    let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let mut temporary_name = OsString::from(file_name);
    temporary_name.push(format!(".ice-dev-{}-{sequence}.tmp", std::process::id()));
    let temporary_path = parent.join(temporary_name);

    let result = (|| {
        {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary_path)?;
            file.write_all(token)?;
            file.sync_all()?;
        }
        fs::rename(&temporary_path, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU8;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "ui-lang-runtime-dev-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn marker_contains_the_exact_token_and_is_published_once() {
        let directory = TestDirectory::new();
        let marker = directory.0.join("ready");
        let published = AtomicBool::new(false);
        let lock = Mutex::new(());

        assert!(try_publish_ready(
            &ReadyConfig {
                path: marker.clone(),
                token: "candidate token with spaces".into(),
                required_draw: None,
            },
            &published,
            &lock,
        ));
        assert_eq!(fs::read(&marker).unwrap(), b"candidate token with spaces");

        assert!(try_publish_ready(
            &ReadyConfig {
                path: marker.clone(),
                token: "replacement".into(),
                required_draw: None,
            },
            &published,
            &lock,
        ));
        assert_eq!(fs::read(&marker).unwrap(), b"candidate token with spaces");
    }

    #[test]
    fn failed_marker_write_is_retried() {
        let directory = TestDirectory::new();
        let parent = directory.0.join("created-later");
        let marker = parent.join("ready");
        let config = ReadyConfig {
            path: marker.clone(),
            token: "retry-token".into(),
            required_draw: None,
        };
        let published = AtomicBool::new(false);
        let lock = Mutex::new(());

        assert!(!try_publish_ready(&config, &published, &lock));
        assert!(!published.load(Ordering::Acquire));

        fs::create_dir(&parent).unwrap();
        assert!(try_publish_ready(&config, &published, &lock));
        assert_eq!(fs::read(marker).unwrap(), b"retry-token");
    }

    #[test]
    fn required_draw_probe_blocks_readiness_until_the_widget_draws() {
        const PROBE: &str = "virtual-list-test-probe";
        const TREE_PROBE: &str = "tree-view-test-probe";
        assert!(!required_draw_completed(Some(PROBE)));
        record_draw_probe(PROBE);
        assert!(required_draw_completed(Some(PROBE)));
        assert!(!required_draw_completed(Some(
            "virtual-list-test-probe, tree-view-test-probe"
        )));
        record_draw_probe(TREE_PROBE);
        assert!(required_draw_completed(Some(
            "virtual-list-test-probe, tree-view-test-probe"
        )));
        assert!(required_draw_completed(None));
    }

    // iced's null renderer — the `()` this stands a widget up against — is
    // `#[cfg(debug_assertions)]` in `iced_core`. Without the same gate the
    // whole lib test target stops compiling under `--release`.
    #[cfg(debug_assertions)]
    struct DrawRecorder(Arc<AtomicU8>);

    #[cfg(debug_assertions)]
    impl Widget<(), (), ()> for DrawRecorder {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Shrink, Length::Shrink)
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &(),
            _limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(Size::ZERO)
        }

        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut (),
            _theme: &(),
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
            self.0
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .unwrap();
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn wrapper_publishes_only_after_the_child_draw_returns() {
        let phase = Arc::new(AtomicU8::new(0));
        let content: Element<'_, (), (), ()> = Element::new(DrawRecorder(Arc::clone(&phase)));
        let publish_phase = Arc::clone(&phase);
        let wrapped: Element<'_, (), (), ()> = Element::new(Ready {
            content,
            publish: move || {
                publish_phase
                    .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
                    .unwrap();
            },
        });
        let tree = Tree::new(&wrapped);
        let node = layout::Node::new(Size::ZERO);
        let viewport = Rectangle::with_size(Size::ZERO);

        wrapped.as_widget().draw(
            &tree,
            &mut (),
            &(),
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );

        assert_eq!(phase.load(Ordering::Acquire), 2);
    }
}
