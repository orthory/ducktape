//! Runtime and capabilities for dynamically deployed views. No GPUI types or
//! module-specific operations belong here: rendering consumes validated
//! frames, and capability implementations are registered by the application.

use std::{collections::{BTreeMap, BTreeSet}, path::Path, sync::{Arc, OnceLock}, time::Duration};
use futures::{StreamExt, stream::BoxStream};
use tokio::{runtime::Handle, sync::mpsc, task::JoinHandle};
use view_protocol as wire;
use wasmtime::{Config, Engine, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder};
use wasmtime::component::{Component, Linker, TypedFunc};

const MAX_COMPONENT_BYTES: usize = 64 << 20;
const FUEL: u64 = 100_000_000;
const EPOCH: Duration = Duration::from_millis(10);
const DEADLINE_EPOCHS: u64 = 10;
const REPLY_QUEUE: usize = 8;

pub type Answer = Result<Vec<u8>, String>;
type Open = Arc<dyn Fn(Vec<u8>) -> BoxStream<'static, Answer> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Subscribe,
}

#[derive(Clone)]
struct Capability {
    operation: Operation,
    open: Open,
}

#[derive(Clone, Default)]
pub struct Capabilities(BTreeMap<String, Capability>);

impl Capabilities {
    pub fn register(
        &mut self,
        name: &str,
        operation: Operation,
        open: impl Fn(Vec<u8>) -> BoxStream<'static, Answer> + Send + Sync + 'static,
    ) -> Result<(), String> {
        let valid = wire::name(name) && !self.0.contains_key(name);
        if !valid {
            return Err("invalid or duplicate view capability".into());
        }
        self.0.insert(name.into(), Capability { operation, open: Arc::new(open) });
        Ok(())
    }
}

struct Running {
    operation: Operation,
    task: JoinHandle<()>,
}

struct Reply {
    id: u64,
    result: Answer,
    done: bool,
}

/// A kernel belongs to exactly one guest generation. Replacing a view creates
/// a new kernel, so late responses cannot reach the replacement instance.
pub struct Kernel {
    capabilities: Capabilities,
    granted: BTreeSet<String>,
    running: BTreeMap<u64, Running>,
    highest_id: Option<u64>,
    send: mpsc::Sender<Reply>,
    receive: mpsc::Receiver<Reply>,
    runtime: Handle,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Kernel {
    pub fn new(
        capabilities: Capabilities,
        declared: &[String],
        allowed: &BTreeSet<String>,
        runtime: Handle,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let granted = declared.iter().filter(|name| allowed.contains(*name)).cloned().collect();
        let (send, receive) = mpsc::channel(REPLY_QUEUE);
        Self { capabilities, granted, running: BTreeMap::new(), highest_id: None,
            send, receive, runtime, wake: Arc::new(wake) }
    }

    pub fn requests(&mut self, frame: &wire::Frame) -> Vec<wire::Event> {
        // Cancellation precedes admission: a request born and cancelled in
        // the same frame must never execute a side effect.
        let cancelled: BTreeSet<_> = frame.cancels.iter().copied().collect();
        for id in &cancelled { self.cancel(*id); }
        let mut refused = Vec::new();
        for request in &frame.requests {
            if cancelled.contains(&request.id) { continue; }
            if let Err(reason) = self.admit(request) {
                refused.push(wire::Event::Response { id: request.id, result: Err(reason), done: true });
            }
        }
        refused
    }

    fn admit(&mut self, request: &wire::Request) -> Result<(), String> {
        request.validate()?;
        let fresh = self.highest_id.is_none_or(|highest| request.id > highest);
        if !fresh {
            return Err("view request id is stale or duplicated".into());
        }
        self.highest_id = Some(request.id);
        if !self.granted.contains(&request.kind) {
            return Err("view capability is not granted".into());
        }
        if self.running.len() >= wire::MAX_REQUESTS {
            return Err("view has too many active requests".into());
        }
        let capability = self.capabilities.0.get(&request.kind).ok_or("unknown view capability")?.clone();
        let id = request.id;
        let payload = request.payload.clone();
        let send = self.send.clone();
        let wake = self.wake.clone();
        let operation = capability.operation;
        let task = self.runtime.spawn(async move {
            let mut stream = (capability.open)(payload);
            match operation {
                Operation::Read | Operation::Write => {
                    let result = stream.next().await.unwrap_or_else(|| Err("capability returned no result".into()));
                    deliver(&send, &wake, Reply { id, result, done: true }).await;
                }
                Operation::Subscribe => {
                    while let Some(result) = stream.next().await {
                        if !deliver(&send, &wake, Reply { id, result, done: false }).await {
                            return;
                        }
                    }
                    deliver(&send, &wake, Reply { id, result: Ok(Vec::new()), done: true }).await;
                }
            }
        });
        self.running.insert(id, Running { operation, task });
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<wire::Event> {
        let mut events = Vec::new();
        while let Ok(reply) = self.receive.try_recv() {
            if !self.running.contains_key(&reply.id) { continue; }
            if reply.done { self.running.remove(&reply.id); }
            events.push(wire::Event::Response { id: reply.id, result: reply.result, done: reply.done });
        }
        events
    }

    fn cancel(&mut self, id: u64) {
        let Some(running) = self.running.get(&id) else { return; };
        // Cancelling a subscription is safe. A write may already have reached
        // the node: let it settle and report its result before permitting a
        // reload, rather than pretending cancellation rolled it back.
        if running.operation == Operation::Write { return; }
        if let Some(running) = self.running.remove(&id) { running.task.abort(); }
    }

    pub fn settled(&self) -> bool {
        self.running.values().all(|running| running.operation == Operation::Subscribe)
    }
}

impl Drop for Kernel {
    fn drop(&mut self) {
        for running in self.running.values() {
            match running.operation {
                Operation::Write => {},
                Operation::Read | Operation::Subscribe => running.task.abort(),
            }
        }
    }
}

async fn deliver(send: &mpsc::Sender<Reply>, wake: &Arc<dyn Fn() + Send + Sync>, mut reply: Reply) -> bool {
    let size = match &reply.result { Ok(bytes) => bytes.len(), Err(reason) => reason.len() };
    if size > wire::MAX_PAYLOAD_BYTES {
        reply.result = Err("capability response exceeds byte budget".into());
        reply.done = true;
    }
    if send.send(reply).await.is_err() { return false; }
    wake();
    true
}

struct State {
    limits: StoreLimits,
    panic: Option<String>,
}

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("view engine");
        let clock = engine.clone();
        std::thread::Builder::new().name("view-deadlines".into()).spawn(move || loop {
            std::thread::sleep(EPOCH);
            clock.increment_epoch();
        }).expect("view deadline clock");
        engine
    })
}

pub struct Guest {
    pub manifest: wire::Manifest,
    store: Store<State>,
    tick: TypedFunc<(Vec<u8>,), (Result<Vec<u8>, String>,)>,
    snapshot: TypedFunc<(), (Result<Vec<u8>, String>,)>,
    restore: TypedFunc<(Vec<u8>,), (Result<(), String>,)>,
    pub root: Option<wire::Node>,
    pub revision: u64,
    fault: Option<String>,
}

impl Guest {
    pub fn from_file(path: &Path, state: Option<&[u8]>) -> Result<Self, String> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        file.take(MAX_COMPONENT_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
        Self::from_bytes(&bytes, state)
    }

    /// Bytes must come from a runtime file or a hash-verified deployment.
    pub fn from_bytes(bytes: &[u8], snapshot: Option<&[u8]>) -> Result<Self, String> {
        if bytes.len() > MAX_COMPONENT_BYTES { return Err("view component exceeds byte budget".into()); }
        let manifest = manifest(bytes)?;
        let component = Component::new(engine(), bytes).map_err(|error| error.to_string())?;
        let limits = StoreLimitsBuilder::new().memory_size(64 << 20).memories(1)
            .instances(8).tables(4).table_elements(1 << 20).trap_on_grow_failure(true).build();
        let mut store = Store::new(engine(), State { limits, panic: None });
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_trap();
        arm(&mut store)?;
        let mut linker = Linker::<State>::new(engine());
        linker.root().func_wrap("panicked", |mut store: StoreContextMut<'_, State>, (message,): (String,)| {
            store.data_mut().panic = Some(message.chars().take(1024).collect());
            Ok(())
        }).map_err(|error| error.to_string())?;
        // No unknown-import fallback: the only guest import is the panic
        // reporter. WASI, filesystem, network and native toolkit imports fail.
        let instance = linker.instantiate(&mut store, &component).map_err(|error| error.to_string())?;
        let init = instance.get_typed_func::<(), ()>(&mut store, "init").map_err(|error| error.to_string())?;
        let tick = instance.get_typed_func(&mut store, "tick").map_err(|error| error.to_string())?;
        let snapshot_fn = instance.get_typed_func(&mut store, "snapshot").map_err(|error| error.to_string())?;
        let restore = instance.get_typed_func(&mut store, "restore").map_err(|error| error.to_string())?;
        let mut guest = Self { manifest, store, tick, snapshot: snapshot_fn, restore,
            root: None, revision: 0, fault: None };
        arm(&mut guest.store)?;
        match snapshot {
            Some(bytes) => {
                if bytes.len() > wire::MAX_FRAME_BYTES { return Err("view snapshot exceeds budget".into()); }
                let (result,) = guest.restore.call(&mut guest.store, (bytes.to_vec(),)).map_err(|error| error.to_string())?;
                result?;
                guest.restore.post_return(&mut guest.store).map_err(|error| error.to_string())?;
            }
            None => {
                init.call(&mut guest.store, ()).map_err(|error| error.to_string())?;
                init.post_return(&mut guest.store).map_err(|error| error.to_string())?;
            }
        }
        Ok(guest)
    }

    pub fn tick(&mut self, events: Vec<wire::Event>) -> Result<wire::Frame, String> {
        if let Some(fault) = &self.fault { return Err(fault.clone()); }
        let result = self.tick_inner(events);
        if let Err(error) = &result {
            self.fault = Some(self.store.data_mut().panic.take().unwrap_or_else(|| error.clone()));
        }
        result
    }

    fn tick_inner(&mut self, events: Vec<wire::Event>) -> Result<wire::Frame, String> {
        let input = wire::encode(&events)?;
        arm(&mut self.store)?;
        let (result,) = self.tick.call(&mut self.store, (input,)).map_err(|error| error.to_string())?;
        self.tick.post_return(&mut self.store).map_err(|error| error.to_string())?;
        let mut frame: wire::Frame = wire::decode(&result?)?;
        frame.validate()?;
        let valid_revision = match &frame.root {
            Some(_) => frame.revision > self.revision,
            None => self.root.is_some() && frame.revision == self.revision,
        };
        if !valid_revision { return Err("view tree revision is invalid".into()); }
        if let Some(root) = frame.root.take() { self.root = Some(root); }
        self.revision = frame.revision;
        Ok(frame)
    }

    pub fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        if let Some(fault) = &self.fault { return Err(fault.clone()); }
        arm(&mut self.store)?;
        let (result,) = self.snapshot.call(&mut self.store, ()).map_err(|error| error.to_string())?;
        self.snapshot.post_return(&mut self.store).map_err(|error| error.to_string())?;
        let bytes = result?;
        if bytes.len() > wire::MAX_FRAME_BYTES { return Err("view snapshot exceeds budget".into()); }
        Ok(bytes)
    }
}

fn arm(store: &mut Store<State>) -> Result<(), String> {
    store.set_fuel(FUEL).map_err(|error| error.to_string())?;
    store.set_epoch_deadline(DEADLINE_EPOCHS);
    Ok(())
}

fn manifest(bytes: &[u8]) -> Result<wire::Manifest, String> {
    let mut payloads = wasmparser::Parser::new(0).parse_all(bytes);
    let component = matches!(payloads.next(), Some(Ok(wasmparser::Payload::Version { encoding: wasmparser::Encoding::Component, .. })));
    if !component { return Err("view must be a WASM component".into()); }
    let mut manifest = None;
    for payload in payloads {
        if let wasmparser::Payload::CustomSection(section) = payload.map_err(|error| error.to_string())?
            && section.name() == wire::MANIFEST_SECTION {
            if manifest.is_some() { return Err("duplicate view manifest".into()); }
            if section.data().len() > 16 << 10 { return Err("view manifest exceeds budget".into()); }
            let value: wire::Manifest = wire::decode(section.data())?;
            value.validate()?;
            manifest = Some(value);
        }
    }
    manifest.ok_or_else(|| "view manifest is missing".into())
}
