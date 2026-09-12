//! Native window effects are executed on GPUI's application thread. The app's
//! async actions await the result, so opening a window never reports success
//! before the platform has actually opened it.

use std::sync::{Mutex, OnceLock, atomic::{AtomicU64, Ordering}};
use ducktape_view_guest::Task;
use futures::{StreamExt as _, channel::{mpsc, oneshot}};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct WindowKey(u64);

impl WindowKey {
    pub(crate) fn unique() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowKind {
    Onboarding,
    Console,
    Huddle,
}

pub(crate) enum Command {
    Open { key: WindowKey, kind: WindowKind, reply: oneshot::Sender<WindowKey> },
    Close(WindowKey),
    Raise(WindowKey),
    Drag(WindowKey),
    Oldest(oneshot::Sender<Option<WindowKey>>),
    Clipboard(String),
    Focus(String),
    Quit,
}

pub(crate) struct PendingCommand {
    pub command: Command,
    pub completed: oneshot::Sender<()>,
}

fn sender() -> &'static Mutex<Option<mpsc::UnboundedSender<PendingCommand>>> {
    static SENDER: OnceLock<Mutex<Option<mpsc::UnboundedSender<PendingCommand>>>> = OnceLock::new();
    SENDER.get_or_init(Mutex::default)
}

pub(crate) fn commands() -> mpsc::UnboundedReceiver<PendingCommand> {
    let (send, receive) = mpsc::unbounded();
    let mut current = sender().lock().expect("native shell commands");
    assert!(current.is_none(), "one native shell per process");
    *current = Some(send);
    receive
}

async fn send(command: Command) {
    let (completed, received) = oneshot::channel();
    let pending = PendingCommand { command, completed };
    let sent = sender().lock().expect("native shell commands")
        .as_ref().is_some_and(|sender| sender.unbounded_send(pending).is_ok());
    if !sent {
        tracing::error!(target: "ducktape::app", reason = "native_shell_closed", "native window command could not be delivered");
        return;
    }
    let _ = received.await;
}

pub(crate) fn open(kind: WindowKind) -> (WindowKey, Task<WindowKey>) {
    let key = WindowKey::unique();
    let task = Task::stream(futures::stream::once(async move {
        let (reply, receive) = oneshot::channel();
        send(Command::Open { key, kind, reply }).await;
        receive.await.ok()
    }).filter_map(std::future::ready));
    (key, task)
}

fn effect<Message: 'static>(command: Command) -> Task<Message> {
    Task::future(async move { send(command).await; }).discard()
}

pub(crate) fn close<Message: 'static>(key: WindowKey) -> Task<Message> {
    effect(Command::Close(key))
}

pub(crate) fn raise<Message: 'static>(key: WindowKey) -> Task<Message> {
    effect(Command::Raise(key))
}

pub(crate) fn drag<Message: 'static>(key: WindowKey) -> Task<Message> {
    effect(Command::Drag(key))
}

pub(crate) fn oldest() -> Task<Option<WindowKey>> {
    Task::future(async {
        let (reply, receive) = oneshot::channel();
        send(Command::Oldest(reply)).await;
        receive.await.unwrap_or_default()
    })
}

pub(crate) fn clipboard<Message: 'static>(text: String) -> Task<Message> {
    effect(Command::Clipboard(text))
}

pub(crate) fn focus<Message: 'static>(key: String) -> Task<Message> {
    effect(Command::Focus(key))
}

pub(crate) fn quit<Message: 'static>() -> Task<Message> {
    effect(Command::Quit)
}

use std::collections::{BTreeMap, HashMap};
use gpui_kit::{AppContext as _, AsyncApp, Context, Entity, Render, Window, IntoElement, ParentElement as _, Styled as _};
use crate::{Ducktape, __DucktapeMessage as Message, ShellTab};

struct Desktop {
    state: Ducktape,
    windows: BTreeMap<WindowKey, gpui_kit::AnyWindowHandle>,
    streams: HashMap<u64, gpui_kit::Task<()>>,
}

impl Desktop {
    fn dispatch(&mut self, message: Message, cx: &mut Context<Self>) {
        let task = self.state.__update(message);
        self.start(task, cx).detach();
        self.subscriptions(cx);
        cx.notify();
    }

    fn start(&self, task: Task<Message>, cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        let mut stream = task.into_stream();
        let runtime = crate::module_view::runtime();
        cx.spawn(async move |desktop, cx| {
            loop {
                let message = futures::future::poll_fn(|context| {
                    let _runtime = runtime.enter();
                    stream.poll_next_unpin(context)
                }).await;
                let Some(message) = message else { break; };
                if desktop.update(cx, |this, cx| this.dispatch(message, cx)).is_err() { break; }
            }
        })
    }

    fn subscriptions(&mut self, cx: &mut Context<Self>) {
        let recipes = self.state.__subscription().into_recipes();
        self.streams.retain(|key, _| recipes.iter().any(|recipe| recipe.key == *key));
        for recipe in recipes {
            if self.streams.contains_key(&recipe.key) { continue; }
            let stream = (recipe.start)();
            let task = self.start(Task::stream(stream), cx);
            self.streams.insert(recipe.key, task);
        }
    }

    fn execute(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::Open { key, kind, reply } => self.open_window(key, kind, reply, cx),
            Command::Close(key) => self.close_window(key, cx),
            Command::Raise(key) => self.raise_window(key, cx),
            Command::Drag(key) => self.drag_window(key, cx),
            Command::Oldest(reply) => self.oldest_window(reply),
            Command::Clipboard(text) => self.write_clipboard(text, cx),
            Command::Focus(key) => self.focus_control(key, cx),
            Command::Quit => self.quit(cx),
        }
    }

    fn open_window(&mut self, key: WindowKey, kind: WindowKind, reply: oneshot::Sender<WindowKey>, cx: &mut Context<Self>) {
        use gpui_kit::*;
        let size = match kind {
            WindowKind::Onboarding => size(px(480.0), px(680.0)),
            WindowKind::Console => size(px(1280.0), px(800.0)),
            WindowKind::Huddle => size(px(320.0), px(460.0)),
        };
        let model = cx.entity();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
            titlebar: Some(TitlebarOptions { title: Some("Ducktape".into()), ..Default::default() }),
            ..Default::default()
        };
        match cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| {
                let observer = cx.observe(&model, |_, _, cx| cx.notify());
                DesktopWindow { model, kind, module: None, route: None, _observer: observer }
            });
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        }) {
            Ok(handle) => { self.windows.insert(key, handle.into()); let _ = reply.send(key); }
            Err(error) => {
                self.state.onboarding_error = format!("The window could not be opened: {error}");
                tracing::error!(target: "ducktape::app", reason = "native_window_open_failed", %error, "window could not be opened");
                cx.notify();
            }
        }
    }

    fn close_window(&mut self, key: WindowKey, cx: &mut Context<Self>) {
        let Some(handle) = self.windows.remove(&key) else { return; };
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        self.dispatch(Message::WindowWasClosed(key), cx);
    }

    fn raise_window(&mut self, key: WindowKey, cx: &mut Context<Self>) {
        let Some(handle) = self.windows.get(&key) else { return; };
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }

    fn drag_window(&mut self, key: WindowKey, cx: &mut Context<Self>) {
        let Some(handle) = self.windows.get(&key) else { return; };
        let _ = handle.update(cx, |_, window, _| window.start_window_move());
    }

    fn oldest_window(&self, reply: oneshot::Sender<Option<WindowKey>>) {
        let _ = reply.send(self.windows.keys().next().copied());
    }

    fn write_clipboard(&self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
    }

    fn focus_control(&mut self, _key: String, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn quit(&mut self, cx: &mut Context<Self>) { cx.quit(); }
}

struct DesktopWindow {
    model: Entity<Desktop>,
    kind: WindowKind,
    module: Option<(&'static str, Entity<crate::module_view::NativeModuleView>)>,
    route: Option<gpui_kit::Subscription>,
    _observer: gpui_kit::Subscription,
}

impl DesktopWindow {
    fn console(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::*;
        let (spec, route) = self.model.read(cx).state.native_view();
        let module_changed = self.module.as_ref().is_none_or(|(module, _)| *module != spec.module);
        if module_changed {
            let view = cx.new(|_| crate::module_view::NativeModuleView::new(spec.module));
            let model = self.model.clone();
            self.route = Some(cx.subscribe(&view, move |_, _, event, cx| {
                model.update(cx, |model, cx| model.dispatch(route(event.clone()), cx));
            }));
            self.module = Some((spec.module, view));
        }
        let view = self.module.as_ref().expect("module seated").1.clone();
        view.update(cx, |view, cx| view.set_props(spec.props, cx));
        let mut tabs = div().flex().flex_col().gap_1().w(px(132.0)).p_2();
        for (tab, label) in [(ShellTab::Chat,"Chat"),(ShellTab::Pages,"Pages"),(ShellTab::Forge,"Forge"),(ShellTab::Agents,"Agents"),(ShellTab::Files,"Files"),(ShellTab::Explorer,"Explorer"),(ShellTab::Node,"Node"),(ShellTab::Members,"Members"),(ShellTab::Governance,"Governance"),(ShellTab::Settings,"Settings")] {
            let model = self.model.clone();
            tabs = tabs.child(gpui_kit::component::button::Button::new(label).label(label).on_click(move |_, _, cx| {
                model.update(cx, |model, cx| model.dispatch(Message::SelectShellTab(tab), cx));
            }));
        }
        div().flex().size_full().child(tabs).child(div().flex_1().h_full().child(view)).into_any_element()
    }
}

impl Render for DesktopWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.kind {
            WindowKind::Console => self.console(cx),
            WindowKind::Onboarding => gpui_kit::div().p_6().child(self.model.read(cx).state.onboarding_error.clone()).into_any_element(),
            WindowKind::Huddle => gpui_kit::div().p_6().child(self.model.read(cx).state.huddle_channel_name.clone()).into_any_element(),
        }
    }
}

pub(crate) fn run() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        let mut commands = commands();
        let (state, initial) = Ducktape::__boot();
        let desktop = cx.new(|_| Desktop { state, windows: BTreeMap::new(), streams: HashMap::new() });
        desktop.update(cx, |desktop, cx| { desktop.start(initial, cx).detach(); desktop.subscriptions(cx); });
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Some(pending) = commands.next().await {
                let _ = desktop.update(cx, |desktop, cx| desktop.execute(pending.command, cx));
                let _ = pending.completed.send(());
            }
        }).detach();
    });
}

pub(crate) fn seconds() -> impl futures::Stream<Item = ()> {
    futures::stream::unfold((), |()| async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Some(((), ()))
    })
}
