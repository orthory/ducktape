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

#[derive(Clone)]
pub(crate) struct KeyPress {
    pub(crate) key: String,
    pub(crate) modifiers: gpui_kit::Modifiers,
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
    pending_focus: Option<String>,
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
                let activation = cx.observe_window_activation(window, move |this: &mut DesktopWindow, window, cx| {
                    let message = if window.is_window_active() { Message::WindowFocused(key) } else { Message::WindowUnfocused(key) };
                    this.model.update(cx, |model, cx| model.dispatch(message, cx));
                });
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                DesktopWindow { model, kind, module: None, route: None, inputs: HashMap::new(), input_step: None, qr: None, focus, _activation: activation, _observer: observer }
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

    fn focus_control(&mut self, key: String, cx: &mut Context<Self>) {
        self.pending_focus = Some(key);
        cx.notify();
    }

    fn quit(&mut self, cx: &mut Context<Self>) { cx.quit(); }
}

struct DesktopWindow {
    model: Entity<Desktop>,
    kind: WindowKind,
    module: Option<(&'static str, Entity<crate::module_view::NativeModuleView>)>,
    route: Option<gpui_kit::Subscription>,
    inputs: HashMap<&'static str, NativeInput>,
    input_step: Option<crate::HubStep>,
    qr: Option<(String, Entity<crate::view_tree::ViewTree>)>,
    focus: gpui_kit::FocusHandle,
    _activation: gpui_kit::Subscription,
    _observer: gpui_kit::Subscription,
}

struct NativeInput {
    state: Entity<gpui_kit::component::input::InputState>,
    subscription: gpui_kit::Subscription,
}

impl DesktopWindow {
    fn value(&self, key: &'static str, cx: &gpui_kit::App) -> String {
        self.inputs.get(key).map(|input| input.state.read(cx).value().to_string()).unwrap_or_default()
    }

    fn input(&mut self, key: &'static str, placeholder: &'static str, masked: bool, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::component::input::{Input, InputEvent, InputState};
        if !self.inputs.contains_key(key) {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).masked(masked));
            let model = self.model.clone();
            let subscription = cx.subscribe(&state, move |_, input, event, cx| {
                let InputEvent::Change = event else { return; };
                let secret_slot = matches!(key, "restore_words" | "join_invite");
                if secret_slot {
                    let text = input.read(cx).value().to_string();
                    model.update(cx, |model, cx| model.dispatch(Message::__SecretTyped(key.into(), text), cx));
                }
                match key {
                    "palette-input" => {
                        let text = input.read(cx).value().to_string();
                        model.update(cx, |model, cx| model.dispatch(Message::PaletteChanged(text), cx));
                    }
                    "channel-draft" => {
                        let text = input.read(cx).value().to_string();
                        model.update(cx, |model, cx| model.dispatch(Message::__BindChannelDraft(text), cx));
                    }
                    _ => {}
                }
                cx.notify();
            });
            self.inputs.insert(key, NativeInput { state, subscription });
        }
        Input::new(&self.inputs[key].state).into_any_element()
    }

    fn action(&self, key: impl Into<gpui_kit::ElementId>, label: impl Into<gpui_kit::SharedString>, message: Message, disabled: bool) -> gpui_kit::component::button::Button {
        use gpui_kit::component::Disableable as _;
        let model = self.model.clone();
        gpui_kit::component::button::Button::new(key).label(label).disabled(disabled)
            .on_click(move |_, _, cx| model.update(cx, |model, cx| model.dispatch(message.clone(), cx)))
    }

    fn submit(&self, key: &'static str, label: &'static str, disabled: bool, message: impl Fn(&Self, &gpui_kit::App) -> Message + 'static, cx: &mut Context<Self>) -> gpui_kit::component::button::Button {
        use gpui_kit::component::Disableable as _;
        gpui_kit::component::button::Button::new(key).label(label).disabled(disabled)
            .on_click(cx.listener(move |this, _, _, cx| {
                let message = message(this, cx);
                this.model.update(cx, |model, cx| model.dispatch(message, cx));
            }))
    }

    fn onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::*;
        use crate::HubStep;
        let state = &self.model.read(cx).state;
        let step = state.hub_step;
        let busy = state.mutation_phase != crate::MutationPhase::Idle;
        let error = state.onboarding_error.clone();
        let step_changed = self.input_step != Some(step);
        if step_changed {
            for (_, input) in std::mem::take(&mut self.inputs) {
                drop(input.subscription);
                input.state.update(cx, |state, cx| state.set_value("", window, cx));
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
                    body = body.child(self.action(format!("wallet/{}", wallet.name), format!("{} · {}", wallet.name, wallet.state), Message::PickWallet(wallet.name), busy));
                }
                if !selected.is_empty() {
                    body = body.child(self.input("unlock", "Wallet password", true, window, cx))
                        .child(self.submit("unlock-submit", "Unlock", busy, |this, cx| Message::UnlockSubmit(this.value("unlock", cx)), cx));
                }
                body.child(self.action("wallet-restore", "Restore a wallet", Message::GoRestore, busy))
                    .child(self.action("wallet-create", "Create a wallet", Message::LoginSkip, busy))
                    .child(self.action("wallet-networks", "Networks", Message::GoNetworks, busy))
            }
            HubStep::Password => {
                body = body.child("Protect your wallet")
                    .child(self.input("password", "Password", true, window, cx))
                    .child(self.input("password-confirm", "Confirm password", true, window, cx));
                let problem = crate::backend::password_problem(&self.value("password", cx), &self.value("password-confirm", cx));
                let invalid = busy || !problem.is_empty();
                body.child(problem).child(self.submit("password-submit", "Create wallet", invalid, |this, cx| Message::PasswordSubmit(this.value("password", cx)), cx))
                    .child(self.action("password-back", "Back", Message::GoLogin, busy))
            }
            HubStep::Phrase => {
                body = body.child("Write down your recovery phrase").child("Keep it private. This phrase can restore your wallet.");
                for row in crate::backend::phrase_rows() {
                    body = body.child(div().flex().justify_between()
                        .child(format!("{} {}", row.left_number, row.left_word))
                        .child(format!("{} {}", row.right_number, row.right_word)));
                }
                body.child(self.action("phrase-saved", "I wrote it down", Message::PhraseWrittenDown, busy))
            }
            HubStep::Confirm => body.child(crate::backend::recovery_prompt())
                .child(self.input("phrase-answer", "Requested words, separated by spaces", true, window, cx))
                .child(self.submit("phrase-confirm", "Confirm recovery phrase", busy, |this, cx| Message::ConfirmPhraseSubmit(this.value("phrase-answer", cx)), cx))
                .child(self.action("phrase-again", "Show phrase again", Message::ShowPhraseAgain, busy)),
            HubStep::Restore => body.child("Restore your wallet")
                .child(self.input("restore-name", "Wallet name", false, window, cx))
                .child(self.input("restore_words", "Recovery phrase", true, window, cx))
                .child(self.input("restore-password", "New password", true, window, cx))
                .child(self.submit("restore-submit", "Restore", busy, |this, cx| Message::RestoreSubmit(this.value("restore-name", cx), this.value("restore-password", cx)), cx))
                .child(self.action("restore-back", "Back", Message::GoLogin, busy)),
            HubStep::Networks => {
                let state = &self.model.read(cx).state;
                let networks = state.hub_networks.clone();
                let selected = state.hub_selected.clone();
                body = body.child("Choose a network");
                for network in networks {
                    let label = match (network.probed, network.live) {
                        (false, _) => format!("{} · checking", network.name),
                        (true, true) => format!("{} · block {}", network.name, network.height),
                        (true, false) => format!("{} · offline", network.name),
                    };
                    body = body.child(div().flex().gap_2()
                        .child(self.action(format!("network/{}", network.id), label, Message::PickNetwork(network.id.clone()), busy))
                        .child(self.action(format!("forget/{}", network.id), "Forget", Message::ForgetNetworkSubmit(network.id), busy)));
                }
                let no_selection = busy || selected.is_empty();
                body.child(self.action("network-open", "Open network", Message::OpenNetworkSubmit, no_selection))
                    .child(self.input("remote", "Remote node address", false, window, cx))
                    .child(self.submit("remote-connect", "Connect", busy, |this, cx| Message::ConnectRemoteSubmit(this.value("remote", cx)), cx))
                    .child(self.action("network-join", "Join with invitation", Message::GoJoin, busy))
            }
            HubStep::Join => body.child("Join a network")
                .child(self.input("join_invite", "Invitation", true, window, cx))
                .child(self.action("join-submit", "Join", Message::JoinNetworkSubmit, busy))
                .child(self.action("join-back", "Back", Message::GoNetworks, busy)),
            HubStep::Provisioning => {
                for step in &self.model.read(cx).state.provision_steps {
                    body = body.child(format!("{} · {}", step.label, step.state));
                }
                body
            }
            HubStep::Live => body.child("Your network is ready")
                .child(self.action("copy-invite", "Copy invitation", Message::CopyOnboardingInvite, busy))
                .child(self.action("enter-console", "Open Ducktape", Message::EnterConsole, busy)),
            HubStep::Account => {
                let state = &self.model.read(cx).state;
                let detail = state.ceremony_detail.clone();
                let left = state.ceremony_left.clone();
                let payload = state.ceremony_qr.clone();
                body = body.child("Your account").child(detail).child(left);
                if !payload.is_empty() {
                    let changed = self.qr.as_ref().is_none_or(|(current, _)| current != &payload);
                    if changed {
                        let node = ui_lang_wire::Node::Qr { key: "account-qr".into(), code: ui_lang_wire::Qr {
                            payload: Some(payload.as_bytes().to_vec()), size: Some(ui_lang_wire::QrSize::Total(220.0)), ..Default::default()
                        }};
                        self.qr = Some((payload, cx.new(|_| crate::view_tree::ViewTree::new(node))));
                    }
                    body = body.child(self.qr.as_ref().expect("account QR").1.clone());
                }
                body.child(self.input("account-name", "Account name", false, window, cx))
                    .child(self.submit("account-create", "Create account", busy, |this, cx| Message::WelcomeCreateSubmit(this.value("account-name", cx)), cx))
                    .child(self.action("account-login", "Sign in", Message::WelcomeLoginSubmit, busy))
                    .child(self.action("account-desktop", "Use this device", Message::WelcomeDesktop, busy))
                    .child(self.action("account-skip", "Continue without account", Message::WelcomeSkip, busy))
                    .child(self.action("account-cancel", "Cancel", Message::WelcomeCancel, false))
            }
        };
        div().size_full().flex().flex_col().p_6().gap_4()
            .child(div().text_xl().child("Ducktape"))
            .child(body).child(div().text_color(rgb(0xb42318)).child(error)).into_any_element()
    }

    fn huddle(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        use gpui_kit::*;
        let state = &self.model.read(cx).state;
        let mute = if state.call_muted { "Unmute" } else { "Mute" };
        let camera = if state.call_camera { "Stop camera" } else { "Camera" };
        let screen = if state.call_sharing { "Stop sharing" } else { "Share screen" };
        div().size_full().flex().flex_col().gap_3().p_3()
            .child(state.huddle_channel_name.clone()).child(state.call_status.clone())
            .child(self.action("huddle-mute", mute, Message::ToggleCallMute, false))
            .child(self.action("huddle-camera", camera, Message::ToggleCallCamera, false))
            .child(self.action("huddle-screen", screen, Message::ToggleCallScreen, false))
            .child(self.action("huddle-channel", "Go to channel", Message::HuddleGoChannel, false))
            .child(self.action("huddle-leave", "Leave huddle", Message::LeaveHuddleHere, false)).into_any_element()
    }

    fn console(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
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
        let state = &self.model.read(cx).state;
        let mut modifiers = Modifiers::default();
        if cfg!(target_os = "macos") { modifiers.platform = true; } else { modifiers.control = true; }
        let header = div().flex().gap_2().items_center().p_2()
            .child(state.network_name.clone()).child(state.status.clone())
            .child(self.action("search", "Search", Message::GlobalKeyPressed(KeyPress { key: "k".into(), modifiers }), !state.connected))
            .child(self.action("bell", format!("Notifications ({})", state.bell_unread), Message::ToggleBell, !state.connected))
            .child(self.action("switch-network", "Switch network", Message::SwitchNetwork, false));
        let error = state.error.clone();
        let toast = state.toast.clone();
        let needs_account = state.connected && !state.account_exists && !state.account_banner_dismissed;
        let mut content = div().flex().flex_col().flex_1().h_full().child(header);
        if needs_account {
            content = content.child(div().flex().gap_2().p_2().child("Sign in to use your account on this network")
                .child(self.action("account-open", "Sign in", Message::OpenAccountWelcome, false))
                .child(self.action("account-dismiss", "Dismiss", Message::DismissAccountBanner, false)));
        }
        if !error.is_empty() { content = content.child(div().text_color(rgb(0xb42318)).p_2().child(error)); }
        content = content.child(div().flex_1().min_h_0().child(view));
        if !toast.is_empty() {
            content = content.child(div().flex().gap_2().p_2().child(toast)
                .child(self.action("toast-dismiss", "Dismiss", Message::DismissToast, false)));
        }
        let mut root = div().relative().flex().size_full().child(tabs).child(content);
        if let Some(overlay) = self.overlay(window, cx) { root = root.child(overlay); }
        root.into_any_element()
    }

    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        use gpui_kit::*;
        let state = &self.model.read(cx).state;
        let topmost = crate::backend::topmost_overlay(state.palette_open, state.bell_open, state.channel_create_open);
        let mut panel = div().flex().flex_col().gap_2().w(px(600.0)).p_4();
        let dismiss = match topmost.as_str() {
            "palette" => {
                let chats = state.palette_chat_hits.clone();
                let pages = state.palette_page_hits.clone();
                let phase = state.palette_search_phase;
                panel = panel.child("Search this workspace")
                    .child(self.input("palette-input", "Search messages and pages", false, window, cx));
                if phase == crate::SearchPhase::Searching { panel = panel.child("Searching…"); }
                for hit in chats {
                    panel = panel.child(self.action(format!("search-chat/{}/{}", hit.channel_id, hit.seq), format!("{} · {}", hit.author, hit.text), Message::OpenChatSearchHit(hit.channel_id, hit.seq), false));
                }
                for hit in pages {
                    panel = panel.child(self.action(format!("search-page/{}/{}", hit.page_id, hit.block_id), format!("{} · {}", hit.page_title, hit.text), Message::OpenPageSearchHit(hit.page_id, hit.block_id), false));
                }
                panel = panel.child(self.action("search-close", "Close", Message::ClosePalette, false));
                Message::ClosePalette
            }
            "bell" => {
                let generation = state.connect_generation;
                let account = state.account_number.clone();
                let items = crate::backend::bell_visible_items(state.bell_items.clone(), &account, &state.settings_user_key);
                let presentations = state.bell_presentations.clone();
                panel = panel.child("Notifications").child(state.bell_error.clone())
                    .child(self.action("bell-mark-read", "Mark all read", Message::MarkBellReadSubmit, state.bell_marking));
                for item in items {
                    let presentation = crate::backend::bell_presentation(&item, &presentations);
                    let unavailable = !crate::backend::bell_openable(&item, &presentations);
                    panel = panel.child(self.action(format!("notification/{}", presentation.seq), format!("{} · {}", presentation.title, presentation.detail), Message::BellOpenItem(generation, account.clone(), presentation), unavailable));
                }
                panel = panel.child(self.action("bell-close", "Close", Message::CloseBell, false));
                Message::CloseBell
            }
            "channel_create" => {
                let busy = state.mutation_phase != crate::MutationPhase::Idle;
                let members_only = state.channel_create_members_only;
                panel = panel.child("Create a channel")
                    .child(self.input("channel-draft", "Channel name", false, window, cx))
                    .child(self.action("channel-private", if members_only { "Members only: on" } else { "Members only: off" }, Message::ToggleChannelCreateMembersOnly, busy))
                    .child(self.action("channel-submit", "Create", Message::CreateChannelSubmit, busy))
                    .child(self.action("channel-cancel", "Cancel", Message::ToggleChannelCreate, busy));
                Message::ToggleChannelCreate
            }
            _ => return None,
        };
        let model = self.model.clone();
        let panel = div().id("shell-modal").bg(rgb(0xfdfdfb)).text_color(rgb(0x2c2b27)).rounded_lg()
            .max_h(relative(0.85)).overflow_y_scroll()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(panel);
        Some(div().id("shell-scrim").absolute().inset_0().flex().items_center().justify_center().bg(rgba(0x00000066))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| model.update(cx, |model, cx| model.dispatch(dismiss.clone(), cx)))
            .child(panel).into_any_element())
    }
}

impl Render for DesktopWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _};
        let content = match self.kind {
            WindowKind::Console => self.console(window, cx),
            WindowKind::Onboarding => self.onboarding(window, cx),
            WindowKind::Huddle => self.huddle(cx),
        };
        let pending_focus = self.model.read(cx).pending_focus.clone();
        if let Some(key) = pending_focus {
            let local_key = key.rsplit('/').next().unwrap_or(&key);
            if let Some(input) = self.inputs.get(local_key) {
                input.state.update(cx, |state, cx| state.focus(window, cx));
                self.model.update(cx, |model, _| model.pending_focus = None);
            }
        }
        gpui_kit::div().id("desktop-root").size_full().track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                let key = KeyPress { key: event.keystroke.key.clone(), modifiers: event.keystroke.modifiers };
                let state = &this.model.read(cx).state;
                let chord = crate::backend::command_chord(key.key.clone(), key.modifiers);
                let palette = crate::backend::palette_key_action(key.key.clone(), key.modifiers, state.palette_open);
                let escape = crate::backend::escape_target(key.key.clone(), state.palette_open, state.bell_open, state.channel_create_open);
                let global = palette != "none" || !escape.is_empty();
                this.model.update(cx, |model, cx| model.dispatch(Message::ModifierStateChanged(key.modifiers), cx));
                match chord {
                    crate::CommandChord::Quit | crate::CommandChord::CloseWindow => {
                        this.model.update(cx, |model, cx| model.dispatch(Message::CommandChordPressed(key), cx));
                        cx.stop_propagation();
                    }
                    crate::CommandChord::Ignored => {
                        if global {
                            this.model.update(cx, |model, cx| model.dispatch(Message::GlobalKeyPressed(key), cx));
                            cx.stop_propagation();
                        }
                    }
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                let key = KeyPress { key: event.keystroke.key.clone(), modifiers: event.keystroke.modifiers };
                this.model.update(cx, |model, cx| model.dispatch(Message::CopyChordPressed(key), cx));
            }))
            .on_modifiers_changed(cx.listener(|this, event: &gpui_kit::ModifiersChangedEvent, _, cx| {
                this.model.update(cx, |model, cx| model.dispatch(Message::ModifierStateChanged(event.modifiers), cx));
            }))
            .child(content)
    }
}

pub(crate) fn run() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        let mut commands = commands();
        let (state, initial) = Ducktape::__boot();
        let desktop = cx.new(|_| Desktop { state, windows: BTreeMap::new(), streams: HashMap::new(), pending_focus: None });
        let weak = desktop.downgrade();
        cx.on_window_closed(move |cx, id| {
            let _ = weak.update(cx, |desktop, cx| {
                let key = desktop.windows.iter().find_map(|(key, handle)| (handle.window_id() == id).then_some(*key));
                let Some(key) = key else { return; };
                desktop.windows.remove(&key);
                desktop.dispatch(Message::WindowWasClosed(key), cx);
            });
        }).detach();
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
