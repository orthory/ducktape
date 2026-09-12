//! Rust-authored, renderer-free WASM views. Each driver has an independent
//! request broker, task set and subscription set; native tests can drive
//! multiple instances on one thread without sharing application state.

pub mod host;
pub use view_protocol as wire;
pub use wit_bindgen;

use futures::{FutureExt, StreamExt, future::LocalBoxFuture, stream::LocalBoxStream};
use std::{collections::BTreeMap, task::{Context, Poll}};

pub trait App: Sized + 'static {
    type Message: 'static;
    fn boot() -> (Self, Task<Self::Message>);
    fn event(&self, event: wire::Event) -> Option<Self::Message>;
    fn update(&mut self, message: Self::Message) -> Task<Self::Message>;
    fn view(&self) -> wire::Node;
    fn subscriptions(&self) -> Vec<Subscription<Self::Message>>;
    fn snapshot(&self) -> Result<Vec<u8>, String>;
    /// Restore owned state only. The driver recreates subscriptions on the
    /// next tick; restoring must not replay boot or a submitted mutation.
    fn restore(bytes: &[u8]) -> Result<Self, String>;
}

pub struct Task<M> {
    futures: Vec<LocalBoxFuture<'static, M>>,
    commands: Vec<wire::Command>,
}

impl<M: 'static> Task<M> {
    pub fn none() -> Self {
        Self { futures: Vec::new(), commands: Vec::new() }
    }

    pub fn perform<T: 'static>(future: impl Future<Output = T> + 'static, map: impl FnOnce(T) -> M + 'static) -> Self {
        Self { futures: vec![future.map(map).boxed_local()], commands: Vec::new() }
    }

    pub fn command(command: wire::Command) -> Self {
        Self { futures: Vec::new(), commands: vec![command] }
    }

    pub fn batch(tasks: impl IntoIterator<Item = Self>) -> Self {
        let mut result = Self::none();
        for mut task in tasks {
            result.futures.append(&mut task.futures);
            result.commands.append(&mut task.commands);
        }
        result
    }
}

/// A key identifies the subscription's input, including connection and route
/// generation. A factory is called only when that key first becomes active.
pub struct Subscription<M> {
    key: String,
    open: Box<dyn FnOnce() -> LocalBoxStream<'static, M>>,
}

impl<M: 'static> Subscription<M> {
    pub fn new<S: futures::Stream<Item = M> + 'static>(key: impl Into<String>, open: impl FnOnce() -> S + 'static) -> Self {
        Self { key: key.into(), open: Box::new(move || open().boxed_local()) }
    }
}

pub struct Driver<A: App> {
    app: A,
    host: host::Host,
    tasks: futures::stream::FuturesUnordered<LocalBoxFuture<'static, A::Message>>,
    subscriptions: BTreeMap<String, LocalBoxStream<'static, A::Message>>,
    commands: Vec<wire::Command>,
    revision: u64,
    dirty: bool,
    busy: bool,
}

impl<A: App> Driver<A> {
    pub fn new() -> Self {
        let host = host::Host::default();
        let _scope = host.enter();
        let (app, task) = A::boot();
        let mut driver = Self::from_app(app, host.clone());
        driver.schedule(task);
        driver
    }

    fn from_app(app: A, host: host::Host) -> Self {
        Self { app, host, tasks: Default::default(), subscriptions: BTreeMap::new(),
            commands: Vec::new(), revision: 0, dirty: true, busy: false }
    }

    fn schedule(&mut self, mut task: Task<A::Message>) {
        self.tasks.extend(task.futures);
        self.commands.append(&mut task.commands);
    }

    fn update(&mut self, message: A::Message) {
        let task = self.app.update(message);
        self.schedule(task);
        self.dirty = true;
    }

    fn reconcile_subscriptions(&mut self) -> Result<(), String> {
        let wanted = self.app.subscriptions();
        if wanted.len() > wire::MAX_REQUESTS {
            return Err("too many view subscriptions".into());
        }
        let mut keys = std::collections::BTreeSet::new();
        for subscription in &wanted {
            if !keys.insert(subscription.key.clone()) {
                return Err("duplicate view subscription key".into());
            }
        }
        self.subscriptions.retain(|key, _| keys.contains(key));
        for subscription in wanted {
            self.subscriptions.entry(subscription.key).or_insert_with(subscription.open);
        }
        Ok(())
    }

    pub fn tick(&mut self, events: Vec<wire::Event>) -> Result<wire::Frame, String> {
        let host = self.host.clone();
        let _scope = host.enter();
        if events.len() > wire::MAX_REQUESTS * 2 {
            return Err("too many view events".into());
        }
        for event in events {
            match event {
                wire::Event::Response { id, result, done } => host.deliver(id, result, done)?,
                wire::Event::Resync => self.dirty = true,
                wire::Event::Action { revision, .. } if revision != self.revision => {},
                event => {
                    if let Some(message) = self.app.event(event) {
                        self.update(message);
                    }
                }
            }
        }
        self.reconcile_subscriptions()?;
        // A ready stream cannot monopolize the window thread. Fuel still
        // bounds work inside one guest poll; this bounds ready messages.
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        self.busy = false;
        for round in 0..wire::MAX_REQUESTS {
            let mut messages = Vec::new();
            if let Poll::Ready(Some(message)) = self.tasks.poll_next_unpin(&mut cx) {
                messages.push(message);
            }
            for stream in self.subscriptions.values_mut() {
                if let Poll::Ready(Some(message)) = stream.poll_next_unpin(&mut cx) {
                    messages.push(message);
                }
            }
            if messages.is_empty() {
                break;
            }
            for message in messages {
                self.update(message);
            }
            self.reconcile_subscriptions()?;
            self.busy = round + 1 == wire::MAX_REQUESTS;
        }
        let root = match self.dirty {
            true => {
                self.revision = self.revision.checked_add(1).ok_or("view revision exhausted")?;
                self.dirty = false;
                Some(self.app.view())
            }
            false => None,
        };
        let (requests, cancels) = host.drain();
        let frame = wire::Frame { revision: self.revision, root, requests, cancels,
            commands: std::mem::take(&mut self.commands), busy: self.busy };
        frame.validate()?;
        Ok(frame)
    }

    pub fn snapshot(&self) -> Result<Vec<u8>, String> {
        let settled = self.tasks.is_empty() && self.commands.is_empty() && !self.busy;
        if !settled {
            return Err("view has pending work".into());
        }
        let bytes = self.app.snapshot()?;
        if bytes.len() > wire::MAX_FRAME_BYTES {
            return Err("view snapshot exceeds byte budget".into());
        }
        Ok(bytes)
    }

    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > wire::MAX_FRAME_BYTES {
            return Err("view snapshot exceeds byte budget".into());
        }
        let host = host::Host::default();
        let _scope = host.enter();
        Ok(Self::from_app(A::restore(bytes)?, host.clone()))
    }
}

impl<A: App> Default for Driver<A> {
    fn default() -> Self { Self::new() }
}

/// Every guest exports the same component ABI. The native driver is also
/// available to tests; WASM bytes are always loaded by the host from a file.
#[macro_export]
macro_rules! export_view {
    ($app:ty, $manifest:literal) => {
        #[cfg(target_arch = "wasm32")]
        mod component {
            $crate::wit_bindgen::generate!({
                inline: "package ducktape:view@1.0.0; world view { import panicked: func(message: string); export init: func(); export tick: func(events: list<u8>) -> result<list<u8>, string>; export snapshot: func() -> result<list<u8>, string>; export restore: func(state: list<u8>) -> result<_, string>; }",
                runtime_path: "::view_guest::wit_bindgen::rt",
            });
            #[used]
            #[unsafe(link_section = "ducktape.view")]
            static MANIFEST: [u8; $manifest.len()] = *$manifest;
            thread_local! {
                static DRIVER: std::cell::RefCell<Option<$crate::Driver<$app>>> = const { std::cell::RefCell::new(None) };
            }
            struct Component;
            impl Guest for Component {
                fn init() {
                    std::panic::set_hook(Box::new(|info| {
                        let mut message = info.to_string();
                        let mut end = message.len().min(1024);
                        while !message.is_char_boundary(end) { end -= 1; }
                        message.truncate(end);
                        panicked(&message);
                    }));
                    DRIVER.with_borrow_mut(|driver| *driver = Some($crate::Driver::new()));
                }
                fn tick(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
                    let events = $crate::wire::decode(&bytes)?;
                    DRIVER.with_borrow_mut(|driver| {
                        let frame = driver.as_mut().ok_or("view is not initialized")?.tick(events)?;
                        $crate::wire::encode(&frame)
                    })
                }
                fn snapshot() -> Result<Vec<u8>, String> {
                    DRIVER.with_borrow(|driver| driver.as_ref().ok_or("view is not initialized")?.snapshot())
                }
                fn restore(bytes: Vec<u8>) -> Result<(), String> {
                    let candidate = $crate::Driver::restore(&bytes)?;
                    DRIVER.with_borrow_mut(|driver| *driver = Some(candidate));
                    Ok(())
                }
            }
            export!(Component);
        }
    };
}
