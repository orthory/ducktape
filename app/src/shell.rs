//! Native window effects are executed on GPUI's application thread. The app's
//! async actions await the result, so opening a window never reports success
//! before the platform has actually opened it.

use ducktape_view_guest::Task;
use futures::{
    StreamExt as _,
    channel::{mpsc, oneshot},
};
use gpui_kit::prelude::FluentBuilder;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

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

#[derive(Clone)]
pub(crate) struct KeyPress {
    pub(crate) key: String,
    pub(crate) modifiers: gpui_kit::Modifiers,
}

pub(crate) enum Command {
    Open {
        key: WindowKey,
        kind: WindowKind,
        reply: oneshot::Sender<WindowKey>,
    },
    Close(WindowKey),
    Raise(WindowKey),
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
    let sent = sender()
        .lock()
        .expect("native shell commands")
        .as_ref()
        .is_some_and(|sender| sender.unbounded_send(pending).is_ok());
    if !sent {
        tracing::error!(target: "ducktape::app", reason = "native_shell_closed", "native window command could not be delivered");
        return;
    }
    let _ = received.await;
}

pub(crate) fn open(kind: WindowKind) -> (WindowKey, Task<WindowKey>) {
    let key = WindowKey::unique();
    let task = Task::stream(
        futures::stream::once(async move {
            let (reply, receive) = oneshot::channel();
            send(Command::Open { key, kind, reply }).await;
            receive.await.ok()
        })
        .filter_map(std::future::ready),
    );
    (key, task)
}

fn effect<Message: 'static>(command: Command) -> Task<Message> {
    Task::future(async move {
        send(command).await;
    })
    .discard()
}

pub(crate) fn close<Message: 'static>(key: WindowKey) -> Task<Message> {
    effect(Command::Close(key))
}

pub(crate) fn raise<Message: 'static>(key: WindowKey) -> Task<Message> {
    effect(Command::Raise(key))
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

use crate::{AppMessage as Message, Ducktape, ShellTab};
use gpui_kit::{
    AppContext as _, AsyncApp, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window,
};
use std::collections::{BTreeMap, HashMap};

struct Desktop {
    state: Ducktape,
    tray: crate::tray::Tray,
    windows: BTreeMap<WindowKey, gpui_kit::AnyWindowHandle>,
    views: BTreeMap<WindowKey, gpui_kit::WeakEntity<DesktopWindow>>,
    streams: HashMap<u64, gpui_kit::Task<()>>,
    pending_focus: Option<String>,
    pending_urls: Vec<String>,
}

impl Desktop {
    fn dispatch(&mut self, message: Message, cx: &mut Context<Self>) {
        // Native callbacks run on GPUI's thread, not a Tokio worker. Reducers
        // may construct effects which spawn immediately, before their first poll.
        let runtime = crate::module_view::runtime();
        let _runtime = runtime.enter();
        let appearance = self.state.appearance;
        let task = self.state.update(message);
        if appearance != self.state.appearance {
            self.sync_appearance(cx);
        }
        self.tray.sync(&self.state);
        self.start(task, cx).detach();
        self.subscriptions(cx);
        cx.notify();
        self.open_pending_urls(cx);
    }

    fn open_pending_urls(&mut self, cx: &mut Context<Self>) {
        let ready = self.state.connected
            && self.state.console_win.is_some()
            && !self.state.network_chain_id.is_empty();
        if !ready {
            return;
        }
        for url in std::mem::take(&mut self.pending_urls) {
            self.dispatch(Message::OpenMessageLink(url), cx);
        }
    }

    fn sync_appearance(&self, cx: &mut Context<Self>) {
        use gpui_kit::component::{Theme, ThemeMode};
        match self.state.appearance {
            crate::Appearance::Light => Theme::change(ThemeMode::Light, None, cx),
            crate::Appearance::Dark => Theme::change(ThemeMode::Dark, None, cx),
            crate::Appearance::System => Theme::sync_system_appearance(None, cx),
        }
    }

    fn start(&self, task: Task<Message>, cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        let mut stream = task.into_stream();
        let runtime = crate::module_view::runtime();
        cx.spawn(async move |desktop, cx| {
            loop {
                let message = futures::future::poll_fn(|context| {
                    let _runtime = runtime.enter();
                    stream.poll_next_unpin(context)
                })
                .await;
                let Some(message) = message else {
                    break;
                };
                if desktop
                    .update(cx, |this, cx| this.dispatch(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn subscriptions(&mut self, cx: &mut Context<Self>) {
        // Boot also enters here without dispatch; stream constructors may spawn.
        let runtime = crate::module_view::runtime();
        let _runtime = runtime.enter();
        let recipes = self.state.subscriptions().into_recipes();
        self.streams
            .retain(|key, _| recipes.iter().any(|recipe| recipe.key == *key));
        for recipe in recipes {
            if self.streams.contains_key(&recipe.key) {
                continue;
            }
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
            Command::Clipboard(text) => self.write_clipboard(text, cx),
            Command::Focus(key) => self.focus_control(key, cx),
            Command::Quit => self.quit(cx),
        }
    }

    fn open_window(
        &mut self,
        key: WindowKey,
        kind: WindowKind,
        reply: oneshot::Sender<WindowKey>,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::*;
        let size = match kind {
            crate::shell::WindowKind::Onboarding => size(px(480.0), px(680.0)),
            crate::shell::WindowKind::Console => size(px(1280.0), px(800.0)),
            crate::shell::WindowKind::Huddle => size(px(320.0), px(460.0)),
        };
        let model = cx.entity();
        let titlebar = match kind {
            crate::shell::WindowKind::Onboarding => None,
            crate::shell::WindowKind::Console => Some(TitlebarOptions {
                title: (!cfg!(target_os = "macos")).then(|| "Ducktape".into()),
                appears_transparent: cfg!(target_os = "macos"),
                ..Default::default()
            }),
            crate::shell::WindowKind::Huddle => Some(TitlebarOptions {
                title: Some("Ducktape · Huddle".into()),
                ..Default::default()
            }),
        };
        let minimum = match kind {
            crate::shell::WindowKind::Onboarding => size,
            crate::shell::WindowKind::Console => gpui_kit::size(px(1040.), px(540.)),
            crate::shell::WindowKind::Huddle => gpui_kit::size(px(320.), px(340.)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
            titlebar,
            window_min_size: Some(minimum),
            is_resizable: kind != crate::shell::WindowKind::Onboarding,
            app_id: Some("dev.ducktape.app".into()),
            kind: if kind == crate::shell::WindowKind::Huddle {
                gpui_kit::WindowKind::PopUp
            } else {
                gpui_kit::WindowKind::Normal
            },
            icon: image::RgbaImage::from_raw(
                128,
                128,
                include_bytes!("../assets/icon.rgba").to_vec(),
            )
            .map(std::sync::Arc::new),
            ..Default::default()
        };
        // Native window creation can synchronously render its root. Release
        // the model borrow before GPUI enters that renderer.
        cx.defer(move |cx| {
        let mut opened_view = None;
        let window_model = model.clone();
        match cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| {
                cx.on_release(DesktopWindow::released).detach();
                let observer = cx.observe(&window_model, |_, _, cx| cx.notify());
                let activation = cx.observe_window_activation(
                    window,
                    move |this: &mut DesktopWindow, window, cx| {
                        let message = if window.is_window_active() {
                            Message::WindowFocused(key)
                        } else {
                            Message::WindowUnfocused(key)
                        };
                        let model = this.model.clone();
                        cx.defer(move |cx| model.update(cx, |model, cx| model.dispatch(message, cx)));
                    },
                );
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                let keystrokes = DesktopWindow::intercept_global_keys(window, cx);
                DesktopWindow {
                    model: window_model,
                    kind,
                    module: None,
                    module_route: None,
                    route: None,
                    inputs: HashMap::new(),
                    input_step: None,
                    qr: None,
                    video: None,
                    focus,
                    _activation: activation,
                    _observer: observer,
                    _keystrokes: keystrokes,
                }
            });
            opened_view = Some(view.downgrade());
            let closing = view.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                let _ = closing.update(cx, |this, cx| {
                    this.observe_module_window(ui_lang_wire::events::Window::CloseRequested, cx)
                });
                release_window_input(window, cx);
                true
            });
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        }) {
            Ok(handle) => {
                model.update(cx, |model, _| {
                    model.windows.insert(key, handle.into());
                    if let Some(view) = opened_view {
                        model.views.insert(key, view);
                    }
                });
                let _ = reply.send(key);
            }
            Err(error) => {
                tracing::error!(target: "ducktape::app", reason = "native_window_open_failed", %error, "window could not be opened");
                model.update(cx, |model, cx| {
                    model.state.onboarding_error = format!("The window could not be opened: {error}");
                    cx.notify();
                });
            }
        }
        });
    }

    fn close_window(&mut self, key: WindowKey, cx: &mut Context<Self>) {
        let Some(handle) = self.windows.remove(&key) else {
            return;
        };
        if let Some(view) = self.views.remove(&key) {
            let _ = view.update(cx, |this, cx| {
                this.observe_module_window(ui_lang_wire::events::Window::CloseRequested, cx)
            });
        }
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                release_window_input(window, cx);
                window.remove_window();
            });
        });
        self.dispatch(Message::WindowWasClosed(key), cx);
    }

    fn raise_window(&mut self, key: WindowKey, cx: &mut Context<Self>) {
        let Some(handle) = self.windows.get(&key) else {
            return;
        };
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }

    fn write_clipboard(&self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
    }

    fn focus_control(&mut self, key: String, cx: &mut Context<Self>) {
        self.pending_focus = Some(key);
        cx.notify();
    }

    fn quit(&mut self, cx: &mut Context<Self>) {
        let windows = self.windows.values().copied().collect::<Vec<_>>();
        cx.defer(move |cx| {
            for handle in windows {
                let _ = handle.update(cx, |_, window, cx| release_window_input(window, cx));
            }
            cx.quit();
        });
    }
}

fn release_window_input(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    // A platform window can outlive its GPUI window during asynchronous native
    // teardown. Complete the blur frame first so it no longer holds an input
    // handler (and therefore a strong entity reference) when teardown begins.
    window.blur(cx);
    window.draw(cx).clear(cx);
}

pub(crate) struct DesktopWindow {
    video: Option<(
        String,
        Entity<crate::video::VideoView>,
        Entity<crate::video::VideoView>,
    )>,
    model: Entity<Desktop>,
    kind: WindowKind,
    module: Option<(&'static str, Entity<crate::module_view::NativeModuleView>)>,
    module_route: Option<fn(crate::module_view::ModuleViewEvent) -> Message>,
    route: Option<gpui_kit::Subscription>,
    inputs: HashMap<&'static str, NativeInput>,
    input_step: Option<crate::HubStep>,
    qr: Option<(String, Entity<crate::view_tree::ViewTree>)>,
    focus: gpui_kit::FocusHandle,
    _activation: gpui_kit::Subscription,
    _observer: gpui_kit::Subscription,
    _keystrokes: gpui_kit::Subscription,
}

struct NativeInput {
    state: Entity<gpui_kit::component::input::InputState>,
    subscription: gpui_kit::Subscription,
}

impl DesktopWindow {
    fn intercept_global_keys(
        window: &gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Subscription {
        let window_id = window.window_handle().window_id();
        let view = cx.entity().downgrade();
        // Native input actions resolve before element key listeners. Only the
        // owning window's shell commands precede them; ordinary keys stay native.
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != window_id {
                return;
            }
            let _ = view.update(cx, |view, cx| {
                view.global_key(
                    KeyPress {
                        key: event.keystroke.key.clone(),
                        modifiers: event.keystroke.modifiers,
                    },
                    cx,
                );
            });
        })
    }

    fn global_key(&mut self, key: KeyPress, cx: &mut Context<Self>) {
        let state = &self.model.read(cx).state;
        let chord = crate::backend::command_chord(key.key.clone(), key.modifiers);
        let palette =
            crate::backend::palette_key_action(key.key.clone(), key.modifiers, state.palette_open);
        let escape = crate::backend::escape_target(
            key.key.clone(),
            state.palette_open,
            state.bell_open,
            state.channel_create_open,
        );
        let global = palette != "none" || !escape.is_empty();
        let message = match chord {
            crate::CommandChord::Quit | crate::CommandChord::CloseWindow => {
                Message::CommandChordPressed(key)
            }
            crate::CommandChord::Ignored => {
                if !global {
                    return;
                }
                Message::GlobalKeyPressed(key)
            }
        };
        self.model
            .update(cx, |model, cx| model.dispatch(message, cx));
        cx.stop_propagation();
    }

    fn released(&mut self, cx: &mut gpui_kit::App) {
        self.observe_module_window(ui_lang_wire::events::Window::Closed, cx);
    }
    fn observe_module_window(
        &mut self,
        event: ui_lang_wire::events::Window,
        cx: &mut gpui_kit::App,
    ) {
        let (Some((_, module)), Some(route)) = (&self.module, self.module_route) else {
            return;
        };
        let intents = module.update(cx, |module, cx| {
            module.observe_final_window_event(event, cx)
        });
        // Closing can run inside a model update. Keep the model and frozen
        // route alive until that borrow ends, independent of the presenter.
        let model = self.model.clone();
        cx.defer(move |cx| {
            for intent in intents {
                model.update(cx, |model, cx| model.dispatch(route(intent), cx));
            }
        });
    }
    #[cfg(test)]
    pub(crate) fn test_state<'a>(&self, cx: &'a gpui_kit::App) -> &'a Ducktape {
        &self.model.read(cx).state
    }

    #[cfg(test)]
    pub(crate) fn test_dispatch(&mut self, message: Message, cx: &mut Context<Self>) {
        self.model
            .update(cx, |model, cx| model.dispatch(message, cx));
    }
    fn value(&self, key: &'static str, cx: &gpui_kit::App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.state.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn input(
        &mut self,
        key: &'static str,
        placeholder: &'static str,
        masked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        use gpui_kit::component::input::{Input, InputEvent, InputState};
        if !self.inputs.contains_key(key) {
            let state = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .masked(masked)
            });
            let model = self.model.clone();
            let subscription = cx.subscribe(&state, move |_, input, event, cx| {
                let InputEvent::Change = event else {
                    return;
                };
                let secret_slot = matches!(key, "restore_words" | "join_invite");
                if secret_slot {
                    let text = input.read(cx).value().to_string();
                    model.update(cx, |model, cx| {
                        model.dispatch(Message::SecretTyped(key.into(), text), cx)
                    });
                }
                match key {
                    "palette-input" => {
                        let text = input.read(cx).value().to_string();
                        model.update(cx, |model, cx| {
                            model.dispatch(Message::PaletteChanged(text), cx)
                        });
                    }
                    "channel-draft" => {
                        let text = input.read(cx).value().to_string();
                        model.update(cx, |model, cx| {
                            model.dispatch(Message::ChannelDraftChanged(text), cx)
                        });
                    }
                    _ => {}
                }
                cx.notify();
            });
            self.inputs.insert(
                key,
                NativeInput {
                    state,
                    subscription,
                },
            );
        }
        Input::new(&self.inputs[key].state)
            .aria_label(placeholder)
            .rounded_none()
            .into_any_element()
    }

    fn action(
        &self,
        key: impl Into<gpui_kit::ElementId>,
        label: impl Into<gpui_kit::SharedString>,
        message: Message,
        disabled: bool,
    ) -> gpui_kit::component::button::Button {
        use gpui_kit::component::Disableable as _;
        let model = self.model.clone();
        gpui_kit::component::button::Button::new(key)
            .label(label)
            .rounded_none()
            .disabled(disabled)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                model.update(cx, |model, cx| model.dispatch(message.clone(), cx))
            })
    }

    fn submit(
        &self,
        key: &'static str,
        label: &'static str,
        disabled: bool,
        message: impl Fn(&Self, &gpui_kit::App) -> Message + 'static,
        cx: &mut Context<Self>,
    ) -> gpui_kit::component::button::Button {
        use gpui_kit::component::Disableable as _;
        gpui_kit::component::button::Button::new(key)
            .label(label)
            .rounded_none()
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let message = message(this, cx);
                this.model
                    .update(cx, |model, cx| model.dispatch(message, cx));
            }))
    }

    fn onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use crate::HubStep;
        use gpui_kit::component::button::ButtonVariants as _;
        use gpui_kit::*;
        let colors = gpui_kit::component::Theme::global(cx).color_tokens();
        let state = &self.model.read(cx).state;
        let step = state.hub_step;
        let busy = state.mutation_phase != crate::MutationPhase::Idle;
        let error = state.onboarding_error.clone();
        let step_changed = self.input_step != Some(step);
        if step_changed {
            for (_, input) in std::mem::take(&mut self.inputs) {
                drop(input.subscription);
                input
                    .state
                    .update(cx, |state, cx| state.set_value("", window, cx));
            }
            self.input_step = Some(step);
        }
        let mut body = div().flex().flex_col().gap_3().w_full();
        body = match step {
            HubStep::Loading => body.child("Opening your workspace…"),
            HubStep::Wallets => {
                let state = &self.model.read(cx).state;
                let selected = state.hub_wallet_selected.clone();
                let wallets = state.hub_wallets.clone();
                body = body.child("Choose a wallet");
                for wallet in wallets {
                    body = body.child(self.action(
                        format!("wallet/{}", wallet.name),
                        format!("{} · {}", wallet.name, wallet.state),
                        Message::PickWallet(wallet.name),
                        busy,
                    ));
                }
                if !selected.is_empty() {
                    body = body
                        .child(self.input("unlock", "Wallet password", true, window, cx))
                        .child(self.submit(
                            "unlock-submit",
                            "Unlock",
                            busy,
                            |this, cx| Message::UnlockSubmit(this.value("unlock", cx)),
                            cx,
                        ));
                }
                body.child(self.action(
                    "wallet-restore",
                    "Restore a wallet",
                    Message::GoRestore,
                    busy,
                ))
                .child(self.action("wallet-create", "Create a wallet", Message::LoginSkip, busy))
                .child(self.action(
                    "wallet-networks",
                    "Networks",
                    Message::GoNetworks,
                    busy,
                ))
            }
            HubStep::Password => {
                body = body
                    .child("Protect your wallet")
                    .child(self.input("password", "Password", true, window, cx))
                    .child(self.input("password-confirm", "Confirm password", true, window, cx));
                let problem = crate::backend::password_problem(
                    &self.value("password", cx),
                    &self.value("password-confirm", cx),
                );
                let invalid = busy || !problem.is_empty();
                body.child(problem)
                    .child(self.submit(
                        "password-submit",
                        "Create wallet",
                        invalid,
                        |this, cx| Message::PasswordSubmit(this.value("password", cx)),
                        cx,
                    ))
                    .child(self.action("password-back", "Back", Message::GoLogin, busy))
            }
            HubStep::Phrase => {
                body = body
                    .child("Write down your recovery phrase")
                    .child("Keep it private. This phrase can restore your wallet.");
                for row in crate::backend::phrase_rows() {
                    body = body.child(
                        div()
                            .flex()
                            .justify_between()
                            .child(format!("{} {}", row.left_number, row.left_word))
                            .child(format!("{} {}", row.right_number, row.right_word)),
                    );
                }
                body.child(self.action(
                    "phrase-saved",
                    "I wrote it down",
                    Message::PhraseWrittenDown,
                    busy,
                ))
            }
            HubStep::Confirm => body
                .child(crate::backend::recovery_prompt())
                .child(self.input(
                    "phrase-answer",
                    "Requested words, separated by spaces",
                    true,
                    window,
                    cx,
                ))
                .child(self.submit(
                    "phrase-confirm",
                    "Confirm recovery phrase",
                    busy,
                    |this, cx| Message::ConfirmPhraseSubmit(this.value("phrase-answer", cx)),
                    cx,
                ))
                .child(self.action(
                    "phrase-again",
                    "Show phrase again",
                    Message::ShowPhraseAgain,
                    busy,
                )),
            HubStep::Restore => body
                .child("Restore your wallet")
                .child(self.input("restore-name", "Wallet name", false, window, cx))
                .child(self.input("restore_words", "Recovery phrase", true, window, cx))
                .child(self.input("restore-password", "New password", true, window, cx))
                .child(self.submit(
                    "restore-submit",
                    "Restore",
                    busy,
                    |this, cx| {
                        Message::RestoreSubmit(
                            this.value("restore-name", cx),
                            this.value("restore-password", cx),
                        )
                    },
                    cx,
                ))
                .child(self.action("restore-back", "Back", Message::GoLogin, busy)),
            HubStep::Networks => {
                let state = &self.model.read(cx).state;
                let networks = state.hub_networks.clone();
                let selected = state.hub_selected.clone();
                body = body
                    .gap_5()
                    .child(div().flex().flex_col().gap_2()
                        .child(div().text_size(px(11.)).font_family(design::fonts::FAMILY_MONO).text_color(colors.muted_foreground).child("WORKSPACE / CONNECT"))
                        .child(div().text_size(px(28.)).font_weight(FontWeight::SEMIBOLD).child("Your networks"))
                        .child(div().text_color(colors.muted_foreground).child("Choose where your team works.")));
                let empty = networks.is_empty();
                let mut recent = div().border_1().border_color(colors.border).bg(colors.surface)
                    .child(div().px_4().py_3().border_b_1().border_color(colors.border).flex().justify_between()
                        .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).child("SAVED NETWORKS"))
                        .child(div().text_size(px(11.)).font_family(design::fonts::FAMILY_MONO).text_color(colors.muted_foreground).child(format!("{:02}", networks.len()))));
                if empty {
                    recent = recent.child(div().px_4().py_6().flex().flex_col().gap_2()
                        .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).child("No networks yet"))
                        .child(div().text_size(px(12.)).text_color(colors.muted_foreground).child("Join a network or connect to a node below.")));
                }
                for network in networks {
                    let label = match (network.probed, network.live) {
                        (false, _) => format!("{} · checking", network.name),
                        (true, true) => format!("{} · block {}", network.name, network.height),
                        (true, false) => format!("{} · offline", network.name),
                    };
                    let picked = network.id == selected;
                    recent = recent.child(
                        div()
                            .flex()
                            .gap_2()
                            .p_2()
                            .child(self.action(
                                format!("network/{}", network.id),
                                label,
                                Message::PickNetwork(network.id.clone()),
                                busy,
                            ).flex_1().when(picked, |button| button.primary()))
                            .child(self.action(
                                format!("forget/{}", network.id),
                                "Forget",
                                Message::ForgetNetworkSubmit(network.id),
                                busy,
                            ).ghost()),
                    );
                }
                let no_selection = busy || selected.is_empty();
                if !empty {
                    recent = recent.child(div().p_3().border_t_1().border_color(colors.border).child(self.action(
                    "network-open",
                    "Open network",
                    Message::OpenNetworkSubmit,
                    no_selection,
                    ).primary().w_full().h_10()));
                }
                body.child(recent)
                .child(div().flex().flex_col().gap_3()
                    .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).child("CONNECT DIRECTLY"))
                    .child(self.input("remote", "Remote node address", false, window, cx))
                    .child(self.submit(
                    "remote-connect",
                    "Connect to node",
                    busy || self.value("remote", cx).trim().is_empty(),
                    |this, cx| Message::ConnectRemoteSubmit(this.value("remote", cx)),
                    cx,
                ).primary().w_full().h_10()))
                .child(div().border_t_1().border_color(colors.border).pt_4().flex().flex_col().gap_2()
                    .child(div().text_size(px(12.)).text_color(colors.muted_foreground).child("Already have an invitation?"))
                    .child(self.action(
                    "network-join",
                    "Join with invitation",
                    Message::GoJoin,
                    busy,
                ).outline().w_full().h_10()))
            }
            HubStep::Join => body
                .gap_5()
                .child(div().flex().flex_col().gap_2()
                    .child(div().text_size(px(11.)).font_family(design::fonts::FAMILY_MONO).text_color(colors.muted_foreground).child("WORKSPACE / INVITATION"))
                    .child(div().text_size(px(28.)).font_weight(FontWeight::SEMIBOLD).child("Join your team"))
                    .child(div().text_color(colors.muted_foreground).child("One invitation. A shared place to work.")))
                .child(div().border_1().border_color(colors.border).bg(colors.surface).p_4().flex().flex_col().gap_3()
                    .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).child("NETWORK INVITATION"))
                    .child(self.input("join_invite", "Invitation", true, window, cx))
                    .child(div().text_size(px(12.)).text_color(colors.muted_foreground).child("Paste the invitation shared by a network member. It stays hidden on this screen.")))
                .child(self.action("join-submit", "Join network", Message::JoinNetworkSubmit, busy || self.value("join_invite", cx).trim().is_empty()).primary().w_full().h_10())
                .child(self.action("join-back", "Back to networks", Message::GoNetworks, busy).ghost().w_full())
                .child(div().border_t_1().border_color(colors.border).pt_4().text_size(px(12.)).text_color(colors.muted_foreground).child("Your wallet identifies you. The invitation connects you to the right workspace.")),
            HubStep::Provisioning => {
                for step in &self.model.read(cx).state.provision_steps {
                    body = body.child(format!("{} · {}", step.label, step.state));
                }
                body
            }
            HubStep::Live => body
                .child("Your network is ready")
                .child(self.action(
                    "copy-invite",
                    "Copy invitation",
                    Message::CopyOnboardingInvite,
                    busy,
                ))
                .child(self.action(
                    "enter-console",
                    "Open Ducktape",
                    Message::EnterConsole,
                    busy,
                )),
            HubStep::Account => {
                let state = &self.model.read(cx).state;
                let detail = state.ceremony_detail.clone();
                let left = state.ceremony_left.clone();
                let payload = state.ceremony_qr.clone();
                body = body.child("Your account").child(detail).child(left);
                if !payload.is_empty() {
                    let changed = self
                        .qr
                        .as_ref()
                        .is_none_or(|(current, _)| current != &payload);
                    if changed {
                        let node = ui_lang_wire::Node::Qr {
                            key: "account-qr".into(),
                            code: ui_lang_wire::Qr {
                                payload: Some(payload.as_bytes().to_vec()),
                                size: Some(ui_lang_wire::QrSize::Total(220.0)),
                                ..Default::default()
                            },
                        };
                        self.qr =
                            Some((payload, cx.new(|_| crate::view_tree::ViewTree::new(node))));
                    }
                    body = body.child(self.qr.as_ref().expect("account QR").1.clone());
                }
                body.child(self.input("account-name", "Account name", false, window, cx))
                    .child(self.submit(
                        "account-create",
                        "Create account",
                        busy,
                        |this, cx| Message::WelcomeCreateSubmit(this.value("account-name", cx)),
                        cx,
                    ))
                    .child(self.action(
                        "account-login",
                        "Sign in",
                        Message::WelcomeLoginSubmit,
                        busy,
                    ))
                    .child(self.action(
                        "account-desktop",
                        "Use this device",
                        Message::WelcomeDesktop,
                        busy,
                    ))
                    .child(self.action(
                        "account-skip",
                        "Continue without account",
                        Message::WelcomeSkip,
                        busy,
                    ))
                    .child(self.action("account-cancel", "Cancel", Message::WelcomeCancel, false))
            }
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .h(px(64.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(28.))
                                    .bg(colors.foreground)
                                    .text_color(colors.background)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_weight(FontWeight::BOLD)
                                    .child("D"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("DUCKTAPE"),
                            )
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    )
                    .child(
                        self.action("launch-close", "×", Message::CloseLaunchWindow, false)
                            .ghost()
                            .accessibility_label("Close window")
                            .w_8()
                            .h_8(),
                    ),
            )
            .child(
                div()
                    .id("onboarding-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_5()
                    .child(body)
                    .when(!error.is_empty(), |element| {
                        element.child(
                            div()
                                .mt_4()
                                .p_3()
                                .border_1()
                                .border_color(colors.destructive)
                                .text_color(colors.destructive)
                                .text_size(px(12.))
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(40.))
                    .px_5()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(design::fonts::FAMILY_MONO)
                            .text_color(colors.muted_foreground)
                            .child("BUILT TO WORK TOGETHER"),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(colors.muted_foreground)
                            .child("DESKTOP"),
                    ),
            )
            .into_any_element()
    }

    fn huddle(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::*;
        let state = &self.model.read(cx).state;
        let mute = if state.call_muted { "Unmute" } else { "Mute" };
        let camera = if state.call_camera {
            "Stop camera"
        } else {
            "Camera"
        };
        let screen = if state.call_sharing {
            "Stop sharing"
        } else {
            "Share screen"
        };
        let stage = state.huddle_stage.clone();
        let video_live = state.call_video_live;
        let row_count = state.huddle_rows.len();
        let columns = ((f32::from(window.viewport_size().width) - 24.) / 128.)
            .floor()
            .max(1.) as usize;
        let title = state.huddle_channel_name.clone();
        let status = state.call_status.clone();
        let elapsed = if state.huddle_joined_at > 0 {
            crate::backend::mmss(state.huddle_now - state.huddle_joined_at)
        } else {
            String::new()
        };
        match &mut self.video {
            Some((previous, tiles, picture)) => {
                if previous != &stage {
                    tiles.update(cx, |view, cx| view.replace_tiles(stage.clone(), cx));
                    picture.update(cx, |view, cx| view.replace_stage(stage.clone(), cx));
                    *previous = stage.clone();
                }
            }
            None => {
                self.video = Some((
                    stage.clone(),
                    cx.new(|_| crate::video::call_video_tiles(&stage)),
                    cx.new(|_| crate::video::call_video_stage(&stage)),
                ))
            }
        }
        let (_, tiles, picture) = self.video.as_ref().expect("retained video surfaces");
        let mut body = div()
            .id("huddle-stage")
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .p_3()
            .flex()
            .flex_col()
            .gap_3();
        if !stage.is_empty() {
            body = body.child(picture.clone());
        }
        if video_live {
            body = body.child(tiles.clone());
        }
        body = body.child(
            uniform_list("huddle-roster", row_count.div_ceil(columns), {
                let model = self.model.clone();
                move |range, _, cx| {
                    let state = &model.read(cx).state;
                    range
                        .map(|index| {
                            div().h(px(112.)).pb_2().flex().gap_2().children(
                                state
                                    .huddle_rows
                                    .iter()
                                    .skip(index * columns)
                                    .take(columns)
                                    .map(|row| {
                                        let caption = match (row.person.is_you, row.muted) {
                                            (true, true) => "you · muted",
                                            (true, false) => "you",
                                            (false, true) => "muted",
                                            (false, false) => "",
                                        };
                                        div()
                                            .w_0()
                                            .min_w_0()
                                            .flex_1()
                                            .p_3()
                                            .border_1()
                                            .rounded_lg()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap_2()
                                            .child(row.person.initials.clone())
                                            .child(div().truncate().child(row.person.label.clone()))
                                            .child(caption)
                                    }),
                            )
                        })
                        .collect()
                }
            })
            .flex_1()
            .min_h_0(),
        );
        let controls = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .p_2()
            .flex_shrink_0()
            .child(self.action("huddle-mute", mute, Message::ToggleCallMute, false))
            .child(self.action("huddle-camera", camera, Message::ToggleCallCamera, false))
            .child(self.action("huddle-screen", screen, Message::ToggleCallScreen, false))
            .child(self.action(
                "huddle-channel",
                "Go to channel",
                Message::HuddleGoChannel,
                false,
            ))
            .child(self.action(
                "huddle-leave",
                "Leave huddle",
                Message::LeaveHuddleHere,
                false,
            ));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_3()
                    .flex_shrink_0()
                    .child(format!("LIVE {elapsed} · {title}"))
                    .child(div().text_xs().child(status)),
            )
            .child(body)
            .child(controls)
            .into_any_element()
    }

    fn console(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::component::{Selectable as _, button::ButtonVariants as _};
        use gpui_kit::*;
        let colors = gpui_kit::component::Theme::global(cx).color_tokens();
        let (spec, route) = self.model.read(cx).state.native_view();
        let module_changed = self
            .module
            .as_ref()
            .is_none_or(|(module, _)| *module != spec.module);
        if module_changed {
            let view = cx.new(|_| crate::module_view::NativeModuleView::new(spec.module));
            let model = self.model.clone();
            self.route = Some(cx.subscribe(&view, move |_, _, event, cx| {
                model.update(cx, |model, cx| model.dispatch(route(event.clone()), cx));
            }));
            self.module = Some((spec.module, view));
            self.module_route = Some(route);
        }
        let view = self.module.as_ref().expect("module seated").1.clone();
        view.update(cx, |view, cx| view.set_props(spec.props, cx));
        let selected_tab = self.model.read(cx).state.shell_tab;
        let navigation = [
            (ShellTab::Chat, "Chat"),
            (ShellTab::Pages, "Pages"),
            (ShellTab::Forge, "Forge"),
            (ShellTab::Agents, "Agents"),
            (ShellTab::Files, "Files"),
            (ShellTab::Explorer, "Explorer"),
            (ShellTab::Node, "Node"),
            (ShellTab::Members, "Members"),
            (ShellTab::Governance, "Governance"),
            (ShellTab::Settings, "Settings"),
        ];
        let title = navigation
            .iter()
            .find(|(tab, _)| *tab == selected_tab)
            .map(|(_, label)| *label)
            .expect("native navigation covers every shell tab");
        let mut tabs = div()
            .id("workspace-rail")
            .flex()
            .flex_col()
            .gap_1()
            .w(px(184.))
            .h_full()
            .flex_shrink_0()
            .px_3()
            .py_4()
            .bg(colors.muted)
            .border_r_1()
            .border_color(colors.border)
            .child(
                div()
                    .h_10()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .mb_4()
                    .child(
                        div()
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(colors.foreground)
                            .text_color(colors.background)
                            .font_weight(FontWeight::BOLD)
                            .child("D"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("DUCKTAPE"),
                    ),
            );
        for (tab, label) in navigation {
            let section = match tab {
                ShellTab::Chat => Some("WORKSPACE"),
                ShellTab::Explorer => Some("NETWORK"),
                ShellTab::Settings => Some("PREFERENCES"),
                _ => None,
            };
            if tab == ShellTab::Settings {
                tabs = tabs.child(div().flex_1().min_h_4());
            }
            if let Some(section) = section {
                tabs = tabs.child(
                    div()
                        .px_3()
                        .pt_3()
                        .pb_2()
                        .flex_shrink_0()
                        .text_size(px(10.))
                        .font_family(design::fonts::FAMILY_MONO)
                        .text_color(colors.muted_foreground)
                        .child(section),
                );
            }
            let selected = tab == selected_tab;
            tabs = tabs.child(
                self.action(label, label, Message::SelectShellTab(tab), false)
                    .ghost()
                    .selected(selected)
                    .w_full()
                    .h_9()
                    .px_3()
                    .justify_start()
                    .when(selected, |button| button.primary()),
            );
        }
        let state = &self.model.read(cx).state;
        let mut modifiers = Modifiers::default();
        if cfg!(target_os = "macos") {
            modifiers.platform = true;
        } else {
            modifiers.control = true;
        }
        let header = div()
            .id("workspace-header")
            .flex()
            .gap_3()
            .items_center()
            .h(px(72.))
            .flex_shrink_0()
            .px_5()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.background)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(colors.muted_foreground)
                            .child(state.network_name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .font_family(design::fonts::FAMILY_MONO)
                    .text_color(colors.muted_foreground)
                    .child(state.status.clone()),
            )
            .child(
                self.action(
                    "search",
                    "Search",
                    Message::GlobalKeyPressed(KeyPress {
                        key: "k".into(),
                        modifiers,
                    }),
                    !state.connected,
                )
                .outline()
                .h_8()
                .px_3(),
            )
            .child(
                self.action(
                    "bell",
                    format!("Notifications ({})", state.bell_unread),
                    Message::ToggleBell,
                    !state.connected,
                )
                .outline()
                .h_8()
                .px_3(),
            )
            .child(
                self.action(
                    "switch-network",
                    "Switch network",
                    Message::SwitchNetwork,
                    false,
                )
                .ghost()
                .h_8()
                .px_3(),
            );
        let error = state.error.clone();
        let toast = state.toast.clone();
        let needs_account =
            state.connected && !state.account_exists && !state.account_banner_dismissed;
        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .bg(colors.background)
            .child(header);
        if needs_account {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_5()
                    .py_2()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(colors.muted_foreground)
                            .child("Sign in to use your account on this network"),
                    )
                    .child(
                        self.action(
                            "account-open",
                            "Sign in",
                            Message::OpenAccountWelcome,
                            false,
                        )
                        .primary()
                        .h_8(),
                    )
                    .child(
                        self.action(
                            "account-dismiss",
                            "Dismiss",
                            Message::DismissAccountBanner,
                            false,
                        )
                        .ghost()
                        .h_8(),
                    ),
            );
        }
        if !error.is_empty() {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_5()
                    .py_2()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(colors.border)
                    .text_color(colors.destructive)
                    .child(div().flex_1().text_size(px(12.)).child(error))
                    .child(
                        self.action("error-dismiss", "Dismiss", Message::DismissError, false)
                            .ghost()
                            .h_8(),
                    ),
            );
        }
        content = content.child(
            div()
                .id("workspace-content")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .w_full()
                .overflow_hidden()
                .child(view),
        );
        if !toast.is_empty() {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_5()
                    .py_2()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .child(div().flex_1().text_size(px(12.)).child(toast))
                    .child(
                        self.action("toast-dismiss", "Dismiss", Message::DismissToast, false)
                            .ghost()
                            .h_8(),
                    ),
            );
        }
        let mut root = div()
            .relative()
            .flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(tabs)
            .child(content);
        if let Some(overlay) = self.overlay(window, cx) {
            root = root.child(overlay);
        }
        root.into_any_element()
    }

    fn overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        use gpui_kit::*;
        let state = &self.model.read(cx).state;
        let topmost = crate::backend::topmost_overlay(
            state.palette_open,
            state.bell_open,
            state.channel_create_open,
        );
        let mut panel = div().flex().flex_col().gap_2().w(px(600.0)).p_4();
        let dismiss = match topmost.as_str() {
            "palette" => {
                let chats = state.palette_chat_hits.clone();
                let pages = state.palette_page_hits.clone();
                let phase = state.palette_search_phase;
                let query = state.palette_draft.clone();
                let empty = chats.is_empty() && pages.is_empty();
                panel = panel.child("Search this workspace").child(self.input(
                    "palette-input",
                    "Search messages and pages",
                    false,
                    window,
                    cx,
                ));
                match phase {
                    crate::SearchPhase::Searching => panel = panel.child("Searching…"),
                    crate::SearchPhase::Done => {
                        if empty {
                            panel = panel.child("No messages or pages matched.");
                        }
                    }
                    crate::SearchPhase::Idle => {
                        if !query.trim().is_empty() {
                            panel = panel.child("Search failed.");
                        }
                    }
                }
                for hit in chats {
                    panel = panel.child(self.action(
                        format!("search-chat/{}/{}", hit.channel_id, hit.seq),
                        format!("{} · {}", hit.author, hit.text),
                        Message::OpenChatSearchHit(hit.channel_id, hit.seq),
                        false,
                    ));
                }
                for hit in pages {
                    panel = panel.child(self.action(
                        format!("search-page/{}/{}", hit.page_id, hit.block_id),
                        format!("{} · {}", hit.page_title, hit.text),
                        Message::OpenPageSearchHit(hit.page_id, hit.block_id),
                        false,
                    ));
                }
                panel =
                    panel.child(self.action("search-close", "Close", Message::ClosePalette, false));
                Message::ClosePalette
            }
            "bell" => {
                let generation = state.connect_generation;
                let account = state.account_number.clone();
                let items = crate::backend::bell_visible_items(
                    &state.bell_items,
                    &account,
                    &state.settings_user_key,
                );
                let presentations = state.bell_presentations.clone();
                panel = panel
                    .child("Notifications")
                    .child(state.bell_error.clone())
                    .child(self.action(
                        "bell-mark-read",
                        "Mark all read",
                        Message::MarkBellReadSubmit,
                        state.bell_marking,
                    ));
                if !state.bell_error.is_empty() {
                    panel =
                        panel.child(self.action("bell-retry", "Retry", Message::ReloadBell, false));
                }
                for item in items {
                    let presentation = crate::backend::bell_presentation(&item, &presentations);
                    let unavailable = !crate::backend::bell_openable(&item, &presentations);
                    panel = panel.child(self.action(
                        format!("notification/{}", presentation.seq),
                        format!("{} · {}", presentation.title, presentation.detail),
                        Message::BellOpenItem(generation, account.clone(), presentation),
                        unavailable,
                    ));
                }
                panel = panel.child(self.action("bell-close", "Close", Message::CloseBell, false));
                Message::CloseBell
            }
            "channel_create" => {
                let busy = state.mutation_phase != crate::MutationPhase::Idle;
                let members_only = state.channel_create_members_only;
                panel = panel
                    .child("Create a channel")
                    .child(self.input("channel-draft", "Channel name", false, window, cx))
                    .child(self.action(
                        "channel-private",
                        if members_only {
                            "Members only: on"
                        } else {
                            "Members only: off"
                        },
                        Message::ToggleChannelCreateMembersOnly,
                        busy,
                    ))
                    .child(self.action(
                        "channel-submit",
                        "Create",
                        Message::CreateChannelSubmit,
                        busy,
                    ))
                    .child(self.action(
                        "channel-cancel",
                        "Cancel",
                        Message::ToggleChannelCreate,
                        busy,
                    ));
                Message::ToggleChannelCreate
            }
            _ => return None,
        };
        let model = self.model.clone();
        use gpui_kit::component::ActiveTheme as _;
        let panel = div()
            .id("shell-modal")
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .rounded_lg()
            .max_h(relative(0.85))
            .overflow_y_scroll()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(panel);
        Some(
            div()
                .id("shell-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x00000066))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    model.update(cx, |model, cx| model.dispatch(dismiss.clone(), cx))
                })
                .child(panel)
                .into_any_element(),
        )
    }
}

impl Render for DesktopWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::InteractiveElement as _;
        use gpui_kit::component::ActiveTheme as _;
        let content = match self.kind {
            WindowKind::Console => self.console(window, cx),
            WindowKind::Onboarding => self.onboarding(window, cx),
            WindowKind::Huddle => self.huddle(window, cx),
        };
        let pending_focus = self.model.read(cx).pending_focus.clone();
        if let Some(key) = pending_focus {
            let local_key = key.rsplit('/').next().unwrap_or(&key);
            if let Some(input) = self.inputs.get(local_key) {
                input.state.update(cx, |state, cx| state.focus(window, cx));
                self.model.update(cx, |model, _| model.pending_focus = None);
            }
        }
        gpui_kit::div()
            .id("desktop-root")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus)
            .on_drop(cx.listener(|this, paths: &gpui_kit::ExternalPaths, _, cx| {
                if this.kind != WindowKind::Console {
                    return;
                }
                // The existing Files reducer owns the write gate and permits
                // one upload at a time. Native paths never reach the WASM view.
                for path in paths.paths() {
                    let Some(path) = path.to_str() else {
                        this.model.update(cx, |model, cx| {
                            model.dispatch(
                                Message::FsDropFailed(crate::backend::AppError {
                                    message: "This file path cannot be represented as UTF-8."
                                        .into(),
                                    committed: false,
                                }),
                                cx,
                            )
                        });
                        continue;
                    };
                    this.model.update(cx, |model, cx| {
                        model.dispatch(Message::FsFileDropped(path.to_owned()), cx)
                    });
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                let key = KeyPress {
                    key: event.keystroke.key.clone(),
                    modifiers: event.keystroke.modifiers,
                };
                let copy = this.model.read(cx).state.shell_tab == ShellTab::Chat
                    && crate::backend::is_copy_chord(key.key.clone(), key.modifiers);
                if !copy {
                    return;
                }
                this.model.update(cx, |model, cx| {
                    model.dispatch(Message::CopyChordPressed(key), cx)
                });
            }))
            .on_modifiers_changed(cx.listener(
                |this, event: &gpui_kit::ModifiersChangedEvent, _, cx| {
                    this.model.update(cx, |model, cx| {
                        model.dispatch(Message::ModifierStateChanged(event.modifiers), cx)
                    });
                },
            ))
            .child(content)
    }
}

#[cfg(test)]
pub(crate) fn test_window(
    state: Ducktape,
    kind: WindowKind,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) -> Entity<DesktopWindow> {
    let model = cx.new(|_| Desktop {
        state,
        tray: crate::tray::Tray::without_status_item(),
        windows: BTreeMap::new(),
        views: BTreeMap::new(),
        streams: HashMap::new(),
        pending_focus: None,
        pending_urls: Vec::new(),
    });
    cx.new(|cx| {
        cx.on_release(DesktopWindow::released).detach();
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        let activation = cx.observe_window_activation(window, |_, _, _| {});
        let keystrokes = DesktopWindow::intercept_global_keys(window, cx);
        DesktopWindow {
            model,
            kind,
            module: None,
            module_route: None,
            route: None,
            inputs: HashMap::new(),
            input_step: None,
            qr: None,
            video: None,
            focus: cx.focus_handle(),
            _activation: activation,
            _observer: observer,
            _keystrokes: keystrokes,
        }
    })
}

#[cfg(test)]
mod close_tests {
    use super::*;

    #[test]
    fn shell_commands_precede_focused_input_actions_in_only_their_window() {
        use gpui_kit::test::TestWindowExt as _;
        let _turn = crate::module_view::tests::blocking_connection_turn();
        let mut native = crate::frame_probe::headless_context();
        let cx = &mut native;
        let mut views = Vec::new();
        let mut windows = Vec::new();
        for _ in 0..2 {
            let handle = cx
                .open_window(
                    gpui_kit::size(gpui_kit::px(600.), gpui_kit::px(700.)),
                    |window, cx| {
                        let mut state = Ducktape::initial_state();
                        state.hub_step = crate::HubStep::Networks;
                        state.connected = true;
                        let view = test_window(state, WindowKind::Onboarding, window, cx);
                        views.push(view.clone());
                        cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
                    },
                )
                .unwrap();
            windows.push(gpui_kit::AnyWindowHandle::from(handle));
        }
        let view = &views[0];
        windows[0]
            .update(cx, |_, window, cx| {
                window.render_frame(cx);
                let input = view.read(cx).inputs["remote"].state.clone();
                input.update(cx, |input, cx| input.focus(window, cx));
                window.render_frame(cx);
                window.input("keep these words", cx);
                let command = if cfg!(target_os = "macos") {
                    "cmd"
                } else {
                    "ctrl"
                };
                window.press(&format!("{command}-k"), cx);
                assert!(view.read(cx).model.read(cx).state.palette_open);
                view.read(cx)
                    .model
                    .clone()
                    .update(cx, |model, _| model.state.bell_open = true);
                window.press("escape", cx);
                assert!(!view.read(cx).model.read(cx).state.palette_open);
                assert!(
                    view.read(cx).model.read(cx).state.bell_open,
                    "only the top shell overlay closes"
                );
                window.press("escape", cx);
                assert!(!view.read(cx).model.read(cx).state.bell_open);
                window.press(&format!("{command}-w"), cx);
                assert_eq!(
                    input.read(cx).value(),
                    "keep these words",
                    "Close is not native delete-word"
                );
                window.press(&format!("{command}-a"), cx);
                assert_eq!(
                    input.read(cx).selected_range(),
                    0.."keep these words".len(),
                    "ordinary native shortcuts remain available"
                );
                let before = view
                    .read(cx)
                    .model
                    .read(cx)
                    .state
                    .account_qr_auth_generation;
                let other_before = views[1]
                    .read(cx)
                    .model
                    .read(cx)
                    .state
                    .account_qr_auth_generation;
                window.press(&format!("{command}-q"), cx);
                assert_eq!(
                    view.read(cx)
                        .model
                        .read(cx)
                        .state
                        .account_qr_auth_generation,
                    before + 1
                );
                assert_eq!(
                    views[1]
                        .read(cx)
                        .model
                        .read(cx)
                        .state
                        .account_qr_auth_generation,
                    other_before,
                    "global interceptor must not route another native window's command"
                );
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn closing_a_focused_native_input_releases_its_handler(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::Focusable as _;
        use gpui_kit::test::TestWindowExt as _;
        cx.update(gpui_kit::init);
        let mut presenter = None;
        let handle = cx.open_window(
            gpui_kit::size(gpui_kit::px(600.), gpui_kit::px(700.)),
            |window, cx| {
                let mut state = Ducktape::initial_state();
                state.hub_step = crate::HubStep::Networks;
                let view = test_window(state, WindowKind::Onboarding, window, cx);
                presenter = Some(view.downgrade());
                gpui_kit::component::Root::new(view, window, cx)
            },
        );
        let handle: gpui_kit::AnyWindowHandle = handle.into();
        handle
            .update(cx, |_, window, cx| {
                window.render_frame(cx);
            })
            .unwrap();
        let presenter = presenter.unwrap();
        let input = presenter
            .update(cx, |view, _| view.inputs["remote"].state.downgrade())
            .unwrap();
        handle
            .update(cx, |_, window, cx| {
                input
                    .upgrade()
                    .unwrap()
                    .update(cx, |input, cx| input.focus(window, cx));
                window.render_frame(cx);
                window.input("typing-before-close", cx);
                assert!(
                    input
                        .upgrade()
                        .unwrap()
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                );
                release_window_input(window, cx);
                assert!(window.focused(cx).is_none());
                window.remove_window();
            })
            .unwrap();
        cx.run_until_parked();
        assert!(presenter.upgrade().is_none());
        assert!(
            input.upgrade().is_none(),
            "native input handler cannot retain the closed window's input"
        );
    }

    #[test]
    fn native_drop_error_dismiss_and_bell_retry_reach_domain_handlers() {
        use gpui_kit::test::TestWindowExt as _;
        use gpui_kit::{ExternalPaths, FileDropEvent, InputEvent, point, px, size};
        assert!(tokio::runtime::Handle::try_current().is_err());
        let _turn = crate::module_view::tests::blocking_connection_turn();
        let mut cx = crate::frame_probe::headless_context();
        let mut state = Ducktape::initial_state();
        state.connected = true;
        state.connected_rpc = "http://127.0.0.1:0".into();
        state.shell_tab = ShellTab::Files;
        state.settings_user_key = "invalid signing key".into();
        state.fs_drop_dir = "/shared".into();
        let expected = crate::backend::files_write_gate(
            state.fs_drop_dir.clone(),
            state.settings_user_key.clone(),
        );
        assert!(!expected.is_empty());
        let mut view = None;
        let handle = cx
            .open_window(size(px(1120.), px(720.)), |window, cx| {
                let presenter = test_window(state, WindowKind::Console, window, cx);
                view = Some(presenter.clone());
                cx.new(|cx| gpui_kit::component::Root::new(presenter, window, cx))
            })
            .unwrap();
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        let position = point(px(400.), px(350.));
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(
                FileDropEvent::Entered {
                    position,
                    paths: ExternalPaths(
                        [std::path::PathBuf::from("/local/report.txt")]
                            .into_iter()
                            .collect(),
                    ),
                }
                .to_platform_input(),
                cx,
            )
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(FileDropEvent::Submit { position }.to_platform_input(), cx)
        })
        .unwrap();
        view.read_with(&cx, |view, cx| {
            assert_eq!(view.test_state(cx).error, expected);
            assert!(
                !view.test_state(cx).fs_dropping,
                "write gate precedes local file I/O"
            );
        });
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("error-dismiss", cx)
        })
        .unwrap();
        view.read_with(&cx, |view, cx| {
            assert!(view.test_state(cx).error.is_empty())
        });
        view.update(&mut cx, |view, cx| {
            view.model.update(cx, |model, cx| {
                model.state.bell_open = true;
                model.state.bell_error = "Could not load notifications".into();
                cx.notify();
            });
        });
        let generation = view.read_with(&cx, |view, cx| view.test_state(cx).bell_load_generation);
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("bell-retry", cx)
        })
        .unwrap();
        view.read_with(&cx, |view, cx| {
            assert_eq!(
                view.test_state(cx).bell_load_generation,
                generation.wrapping_add(1)
            );
        });
    }

    fn frozen_route(event: crate::module_view::ModuleViewEvent) -> Message {
        Message::ExternalUrlFailed(crate::backend::AppError {
            message: event.detail,
            committed: false,
        })
    }

    #[gpui_kit::test]
    async fn final_observations_tick_and_route_after_the_presenter_is_released(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::module_view::tests::{
            close_observer_fixture, close_observer_reading, queue_close_intent,
        };
        let _turn = crate::module_view::tests::blocking_connection_turn();
        cx.update(gpui_kit::init);
        let mut presenter = None;
        let handle = cx.open_window(
            gpui_kit::size(gpui_kit::px(320.), gpui_kit::px(460.)),
            |window, cx| {
                let (state, _) = Ducktape::boot();
                let view = test_window(state, WindowKind::Huddle, window, cx);
                presenter = Some(view.clone());
                gpui_kit::component::Root::new(view, window, cx)
            },
        );
        let presenter = presenter.unwrap();
        let model = presenter.update(cx, |view, cx| {
            view.module = Some(("governance", cx.new(|_| close_observer_fixture())));
            view.module_route = Some(frozen_route);
            view.model.clone()
        });
        let baseline = close_observer_reading().0;
        queue_close_intent("requested");
        // The command executor owns this model borrow while requesting close.
        // Routing inline here would re-enter it and panic.
        model.update(cx, |_, cx| {
            presenter.update(cx, |view, cx| {
                view.observe_module_window(ui_lang_wire::events::Window::CloseRequested, cx);
            });
        });
        assert_eq!(close_observer_reading(), (baseline + 1, 0));
        cx.condition(&model, |model, _| model.state.error == "requested")
            .await;

        // A real guest frame replaces interest; explicitly rearm this host-only
        // fixture before exercising the actual native presenter's release hook.
        queue_close_intent("closed");
        model.update(cx, |model, _| model.state.shell_tab = ShellTab::Files);
        let weak = presenter.downgrade();
        drop(presenter);
        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.condition(&model, |model, _| model.state.error == "closed")
            .await;
        assert!(
            weak.upgrade().is_none(),
            "the route does not retain the presenter"
        );
        assert_eq!(close_observer_reading(), (baseline + 2, 0));
    }
}

pub(crate) fn run() {
    let application = gpui_kit::application();
    let (url_sender, mut urls) = mpsc::unbounded::<Vec<String>>();
    // Install before launching: macOS may deliver its initial URL before the
    // desktop actor exists. The channel keeps it until the actor can receive.
    application.on_open_urls(move |urls| {
        let _ = url_sender.unbounded_send(urls);
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
            std::borrow::Cow::Borrowed(include_bytes!("../../crates/views/support/design/assets/fonts/Geist[wght].ttf")),
            std::borrow::Cow::Borrowed(include_bytes!("../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf")),
            std::borrow::Cow::Borrowed(include_bytes!("../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf")),
        ];
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            tracing::error!(target: "ducktape::app", reason = "font_registration_failed", %error, "bundled desktop fonts could not be registered");
        }
        let theme = gpui_kit::component::Theme::global_mut(cx);
        theme.font_family = design::fonts::FAMILY_UI.into();
        theme.mono_font_family = design::fonts::FAMILY_MONO.into();
        theme.font_size = gpui_kit::px(design::type_scale::BODY as f32);
        gpui_kit::component::Theme::sync_base(cx);
        let mut commands = commands();
        let (state, initial) = Ducktape::boot();
        let (mut tray, mut tray_events) = crate::tray::init(cx);
        tray.sync(&state);
        let desktop = cx.new(|_| Desktop {
            state,
            tray,
            windows: BTreeMap::new(),
            views: BTreeMap::new(),
            streams: HashMap::new(),
            pending_focus: None,
            pending_urls: Vec::new(),
        });
        desktop.update(cx, |desktop, cx| desktop.sync_appearance(cx));
        let url_desktop = desktop.downgrade();
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Some(urls) = urls.next().await {
                let result = url_desktop.update(cx, |desktop, cx| {
                    desktop.pending_urls.extend(urls.into_iter().filter(|url| url.starts_with("duck://")));
                    desktop.open_pending_urls(cx);
                });
                if result.is_err() {
                    break;
                }
            }
        }).detach();
        let tray_desktop = desktop.downgrade();
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Some(row) = tray_events.next().await {
                let Some(message) = crate::tray::message(row) else {
                    continue;
                };
                if tray_desktop
                    .update(cx, |desktop, cx| desktop.dispatch(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let weak = desktop.downgrade();
        cx.on_window_closed(move |cx, id| {
            let weak = weak.clone();
            cx.defer(move |cx| {
            let _ = weak.update(cx, |desktop, cx| {
                let key = desktop
                    .windows
                    .iter()
                    .find_map(|(key, handle)| (handle.window_id() == id).then_some(*key));
                let Some(key) = key else {
                    return;
                };
                desktop.windows.remove(&key);
                desktop.views.remove(&key);
                desktop.dispatch(Message::WindowWasClosed(key), cx);
            });
            });
        })
        .detach();
        desktop.update(cx, |desktop, cx| {
            desktop.start(initial, cx).detach();
            desktop.subscriptions(cx);
        });
        let command_desktop = desktop.downgrade();
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Some(pending) = commands.next().await {
                let _ = command_desktop.update(cx, |desktop, cx| desktop.execute(pending.command, cx));
                let _ = pending.completed.send(());
            }
        }).detach();
        // Keep the windowless desktop/tray alive until quit, without putting
        // its strong handle in a detached future whose cancellation may lag.
        let mut desktop = Some(desktop);
        cx.on_app_quit(move |_| {
            drop(desktop.take());
            async {}
        }).detach();
    });
}

pub(crate) fn seconds() -> impl futures::Stream<Item = ()> {
    futures::stream::unfold((), |()| async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Some(((), ()))
    })
}

pub(crate) fn toast_ticks() -> impl futures::Stream<Item = ()> {
    futures::stream::unfold((), |()| async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Some(((), ()))
    })
}
