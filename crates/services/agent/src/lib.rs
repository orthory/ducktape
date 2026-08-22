//! `agent-service` — the execution half of the interactive terminal plane.
//!
//! This crate is what `ducktape service run agent` is made of. It owns the part
//! of a pty session that owns a PROCESS: resolving a provider tag, spawning the
//! sandboxed TUI, pumping its output, and tearing it down. It owns nothing
//! else — no scrollback ring, no ws topic, no mesh peer, no consensus query, no
//! admission decision. Those belong to the node, which drives this over
//! [`wire`].
//!
//! ## the boundary, and why it is here
//!
//! The pty plane splits at the pty, not at the session. Everything above the
//! pty is the node's by construction:
//!
//! - the scrollback and command rings are owned by the node's stream hub and
//!   read by its ws catch-up path;
//! - a pty client attaches to the NODE's `/v1/ws`, so `term:<id>` must be
//!   served there;
//! - cross-node sessions ride the mesh term plane, which authenticates peers by
//!   their mesh `PeerId` and answers admission from committed state on the
//!   node's actor lane — a daemon holds no keypair and no mesh identity.
//!
//! What is left below the pty is exactly this crate: a provider set, a bounded
//! map of live `InteractiveSession`s, one pump task each, and a wall-clock
//! reaper. That is also precisely the sandbox-touching part, which is the
//! point: after the carve the node process constructs no provider set and no
//! pty.
//!
//! ## the failure domain
//!
//! A session cannot outlive the connection that created it. When the link to
//! the node drops, [`Sessions::close_all`] ends every live session: the node
//! has already forgotten them (it cannot serve a topic it can no longer feed),
//! so a surviving pty would be an orphaned container nobody can reach or close.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use provider_host::{
    AirlockConfig, CredentialKind, InteractiveSession, Provider, ProviderSet, ResolvedCredential,
    RunContext, WorkRef,
};
use tokio::sync::{mpsc, oneshot};

pub mod wire;

/// the per-daemon concurrent-session cap. a terminal is arbitrary code
/// execution on the operator's host burning the operator's subscription, so the
/// ceiling is deliberately small; over it, create refuses rather than spawning.
pub const MAX_TERM_SESSIONS: usize = 4;

/// the hard wall-clock ceiling on any single session. A session is a human
/// driving a CLI TUI, so 4h is a generous single working session; past it the
/// session is force-closed no matter what. This is the backstop that makes a
/// session non-immortal: the primary teardown is still an explicit close, but
/// if the client is killed (its close never runs) or an idle TUI is just left
/// open, this timer guarantees the container + its slot are reclaimed instead of
/// pinned forever. There is deliberately NO idle-timeout — a terminal is
/// legitimately idle while a human reads; silence is not death.
const MAX_SESSION_LIFETIME: Duration = Duration::from_secs(4 * 60 * 60);

/// one pty read chunk. human typing + TUI redraws are modest; a chunk this size
/// coalesces a redraw burst into few frames without a large per-session buffer.
const TERM_READ_BUF: usize = 32 * 1024;

/// the interactive-session execution plane. Arc-backed so a clone rides into
/// each session's pump and reaper task.
#[derive(Clone)]
pub struct Sessions(Arc<Inner>);

struct Inner {
    /// the sandbox-backed provider set. Unlike the in-node manager this is not
    /// optional: a daemon with no runnable sandbox refuses to start, so "no
    /// sandbox" is a state only the node can be in (no daemon attached).
    providers: ProviderSet,
    /// this host's canonical execution id — a run's directories are named from
    /// it, so two nodes sharing one user never collide.
    executing_node: String,
    /// per-session workdirs are created under here (the provider mounts one rw
    /// into the container; the fresh mount namespace fences the rest off), and
    /// removed with the session that owns them — see [`SessionHome`].
    workdir_root: PathBuf,
    /// live sessions. `std::sync::Mutex`: every critical section clones an `Arc`
    /// out and drops the guard before any `.await`, so it never crosses an await
    /// point.
    sessions: Mutex<HashMap<String, Live>>,
    /// reserved-or-live session count, the atomic backing the concurrency cap.
    /// reserved at create (before the spawn await), released exactly once when
    /// the session leaves the map.
    active: AtomicUsize,
    /// which link generation is current. Bumped by [`Sessions::close_all`] on
    /// every disconnect, and captured by a create BEFORE its spawn await: a pty
    /// that finishes starting under a stale epoch is one the node has already
    /// forgotten, so it is torn down instead of registered. Without this the
    /// sweep cannot see a session that is not in the map yet, and the container
    /// survives, unreachable, until the wall-clock ceiling.
    epoch: AtomicU64,
    /// the one way anything here reaches the node. Every writer goes through
    /// [`Inner::emit`], so frame ordering is owned by a single place.
    events: mpsc::Sender<wire::Event>,
}

/// a live session plus the drop-guards that end what the map entry owns. When
/// the entry leaves the map, dropping `_reaper_cancel` resolves the reaper's
/// cancel receiver, so its timer exits WITHOUT firing — an early end can never
/// leave a stale timer around to reap a later session that reused this id — and
/// dropping `_home` removes the session's workdir.
struct Live {
    session: Arc<InteractiveSession>,
    /// this session's ordered input lane. Its only long-lived sender, so
    /// dropping the map entry ends the driver task — the same drop-driven
    /// teardown the pump and reaper take.
    drive: mpsc::UnboundedSender<Drive>,
    _reaper_cancel: oneshot::Sender<()>,
    /// declared LAST so it drops last: the container that mounts this directory
    /// is torn down by [`Sessions::finish`] before the entry is dropped at all.
    _home: SessionHome,
}

/// this session's workdir under [`Inner::workdir_root`] — created when the
/// session starts and REMOVED when it ends, on every exit path (an explicit
/// close, the pty's EOF, the wall-clock ceiling, a lost link, a spawn that never
/// got off the ground), because a Drop runs on all of them.
///
/// The session's whole host footprint is in there: the provider mounts it rw
/// into the container, the executor writes its state into it, and the run's
/// fresh config home sits inside it. Nothing else ever reaped it, so a node
/// accumulated one such tree for every pty session it had EVER hosted.
///
/// Removing it at session end loses nothing. A pty session is marked
/// `portable` — host-local, never resumed or captured — and its container is
/// destroyed by the same teardown, so once the session ends there is no longer
/// anything that can reach this directory.
///
/// A SIGKILLed daemon is the one death that leaves one standing, and that is
/// deliberately not swept: the node draws a session id at random, so no later
/// session can name a leftover and inherit it, and what survives is inert bytes
/// bounded by [`MAX_TERM_SESSIONS`] per kill. The part that is NOT inert — the
/// containers — is what the daemon's boot sweep already reaps.
struct SessionHome {
    dir: PathBuf,
}

impl SessionHome {
    /// materialize the session's workdir. The provider would `create_dir_all`
    /// it too, but doing it HERE puts both ends of the directory's life in the
    /// one place that owns [`Inner::workdir_root`] — and makes a spawn that
    /// fails before the provider ever runs still leave nothing behind.
    fn create(root: &Path, session: &str) -> Result<Self, String> {
        let dir = root.join(session);
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("create session workdir: {error}"))?;
        Ok(Self { dir })
    }

    fn path(&self) -> PathBuf {
        self.dir.clone()
    }
}

impl Drop for SessionHome {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            // the path is deliberately absent: it names a directory that held
            // the session's credential material. the session id is the handle.
            tracing::warn!(
                target: "ducktape::term",
                reason = "session_workdir_not_removed",
                %error,
                "the session workdir outlived its session"
            );
        }
    }
}

/// one thing to do to a live pty, in the order it arrived.
///
/// These ride a PER-SESSION lane rather than being performed on the link task.
/// A pty master write blocks whenever the child stops draining stdin — a TUI
/// mid-render, a paste larger than the tty buffer — and doing it inline would
/// make the link the serialization point for every session at once: one
/// blocked pty would stop all four sessions' output. Per-session ordering is
/// the requirement; a shared queue was never part of it.
enum Drive {
    Input(String),
    Resize { cols: u16, rows: u16 },
}

impl Sessions {
    /// build the plane. `executing_node` is this host's run-scoping id and
    /// `events` is the daemon's link to its node.
    pub fn new(
        providers: ProviderSet,
        executing_node: String,
        workdir_root: PathBuf,
        events: mpsc::Sender<wire::Event>,
    ) -> Self {
        Self(Arc::new(Inner {
            providers,
            executing_node,
            workdir_root,
            sessions: Mutex::new(HashMap::new()),
            active: AtomicUsize::new(0),
            epoch: AtomicU64::new(0),
            events,
        }))
    }

    /// THE dispatch. Every input the node can send is a named variant, and each
    /// arm is a single delegation to a handler named for it — so a new command
    /// fails the build until it is routed.
    pub async fn dispatch(&self, command: wire::Command) {
        match command {
            wire::Command::TermCreate(create) => self.create(create).await,
            wire::Command::TermInput { session, data_b64 } => {
                self.enqueue(&session, Drive::Input(data_b64))
            }
            wire::Command::TermResize {
                session,
                cols,
                rows,
            } => self.enqueue(&session, Drive::Resize { cols, rows }),
            wire::Command::TermClose { session } => self.finish(&session).await,
        }
    }

    /// how many sessions are live — what the daemon's status line reports.
    pub fn live(&self) -> usize {
        self.0.active.load(Ordering::SeqCst)
    }

    /// end every live session, because the link that owned them is gone.
    ///
    /// The node forgets its sessions when a daemon detaches (it cannot feed a
    /// topic it can no longer reach), so a pty that survived the disconnect
    /// would be an orphaned container nobody can attach to, close, or reap
    /// until the wall-clock ceiling fires hours later. Called by the link task
    /// on every disconnect, before it redials.
    pub async fn close_all(&self) {
        // BEFORE the snapshot: a create still inside `spawn_interactive` is not
        // in the map yet, so the sweep below cannot see it. Bumping first is
        // what makes that create notice, on its way out, that the node it was
        // starting for has already forgotten it.
        self.0.epoch.fetch_add(1, Ordering::SeqCst);
        let live: Vec<String> = self
            .0
            .sessions
            .lock()
            .expect("agent sessions lock poisoned")
            .keys()
            .cloned()
            .collect();
        if live.is_empty() {
            return;
        }
        tracing::info!(
            target: "ducktape::term",
            sessions = live.len(),
            reason = "link_lost",
            "ending every session: the node connection dropped"
        );
        for id in live {
            self.finish(&id).await;
        }
    }

    // ---- the handlers, one per command ------------------------------------

    /// spawn a session, then answer with exactly one terminal event. The
    /// decision and the write are separated: [`Self::try_create`] decides and
    /// touches no wire, this emits.
    async fn create(&self, spec: wire::Create) {
        let session = spec.session.clone();
        let provider = spec.provider.clone();
        let event = match self.try_create(spec).await {
            Ok(()) => {
                tracing::info!(target: "ducktape::term", session = %session, provider, "session_created");
                wire::Event::TermCreated { session }
            }
            Err((reason, detail)) => {
                tracing::warn!(
                    target: "ducktape::term",
                    session = %session,
                    provider,
                    reason = reason.token(),
                    "session create refused"
                );
                wire::Event::TermRefused {
                    session,
                    reason,
                    detail,
                }
            }
        };
        self.0.emit(event).await;
    }

    /// resolve, reserve, spawn. Returns the refusal rather than emitting it, so
    /// the whole admission ladder is testable without a wire.
    async fn try_create(&self, spec: wire::Create) -> Result<(), (wire::Refusal, String)> {
        let provider = self
            .0
            .providers
            .resolve(&spec.provider)
            .map_err(|detail| (wire::Refusal::UnknownProvider, detail))?;
        // reserve BEFORE the spawn await, so two concurrent creates cannot both
        // slip past a stale count. Released on any failure below.
        let over_cap = self.0.active.fetch_add(1, Ordering::SeqCst) + 1 > MAX_TERM_SESSIONS;
        if over_cap {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
            return Err((
                wire::Refusal::AtCapacity,
                format!("terminal session cap ({MAX_TERM_SESSIONS}) reached"),
            ));
        }
        match self.spawn(provider, spec).await {
            Ok(()) => Ok(()),
            Err(failure) => {
                self.0.active.fetch_sub(1, Ordering::SeqCst);
                Err(failure)
            }
        }
    }

    /// build the run context, spawn the pty, and register it with its pump and
    /// reaper. The cap reservation is held by [`Self::try_create`].
    async fn spawn(
        &self,
        provider: &dyn Provider,
        spec: wire::Create,
    ) -> Result<(), (wire::Refusal, String)> {
        // the id becomes a directory name below, so it is checked HERE, at the
        // boundary where it arrives, and not trusted for having come from our
        // own node.
        if !wire::valid_session(&spec.session) {
            return Err((
                wire::Refusal::SpawnFailed,
                "session id must be 16 lowercase hex".to_string(),
            ));
        }
        // captured before the await: see `Inner::epoch`.
        let epoch = self.0.epoch.load(Ordering::SeqCst);
        // built BEFORE the spawn below, so a spawn that fails takes its
        // half-materialized workdir down with it on the `?`.
        let home = SessionHome::create(&self.0.workdir_root, &spec.session)
            .map_err(|detail| (wire::Refusal::SpawnFailed, detail))?;
        let ctx = RunContext {
            agent_id: Some(spec.provider.clone()),
            // the executing-node id scopes this run's directories.
            executing_node: Some(self.0.executing_node.clone()),
            // a fresh per-session workdir, carried into the sandbox rw and
            // removed when `home` drops.
            workdir_override: Some(home.path()),
            limits: spec.limits,
            airlock: spec.credential.map(airlock_config),
            ..Default::default()
        };
        let session = provider
            .spawn_interactive(&ctx, spec.restricted)
            .await
            .map_err(|detail| (wire::Refusal::SpawnFailed, detail))?;
        let session = Arc::new(session);
        // the link dropped while this pty was starting. The node forgot this
        // session when it detached, so nothing will ever reach it, close it, or
        // read its output — registering it now would leak a container running
        // the agent CLI until the wall-clock ceiling.
        let node_forgot_it = self.0.epoch.load(Ordering::SeqCst) != epoch;
        if node_forgot_it {
            session.close().await;
            return Err((
                wire::Refusal::SpawnFailed,
                "the node link dropped while the session was starting".to_string(),
            ));
        }
        // dropping `cancel_tx` (when the entry leaves the map) cancels the
        // reaper; holding it in the map keeps the ceiling armed for the session.
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (drive, drive_rx) = mpsc::unbounded_channel();
        self.0
            .sessions
            .lock()
            .expect("agent sessions lock poisoned")
            .insert(
                spec.session.clone(),
                Live {
                    session: session.clone(),
                    drive,
                    _reaper_cancel: cancel_tx,
                    _home: home,
                },
            );
        self.spawn_driver(spec.session.clone(), session.clone(), drive_rx);
        self.spawn_pump(spec.session.clone(), session);
        self.spawn_reaper(spec.session, cancel_rx);
        Ok(())
    }

    /// hand one input or resize to its session's own ordered lane.
    ///
    /// Non-blocking by construction — that is the whole point. An unknown id is
    /// a no-op + `warn` with a named reason, never a panic.
    fn enqueue(&self, id: &str, drive: Drive) {
        let lane = self
            .0
            .sessions
            .lock()
            .expect("agent sessions lock poisoned")
            .get(id)
            .map(|live| live.drive.clone());
        let Some(lane) = lane else {
            tracing::warn!(target: "ducktape::term", session = %id, reason = "unknown_session", "term drive dropped");
            return;
        };
        // a send failure means the driver already exited (a teardown race with
        // `finish`); the session is ending, so the drop is benign.
        let _ = lane.send(drive);
    }

    /// the driver: one task per session, performing that session's inputs and
    /// resizes in the order they arrived.
    ///
    /// Serial per session IS the ordering guarantee. Doing this on the link task
    /// instead would have made a single blocked pty — a TUI mid-render, a paste
    /// bigger than the tty buffer — stop every other session's output too, since
    /// the link would not be reading while it waited.
    fn spawn_driver(
        &self,
        id: String,
        session: Arc<InteractiveSession>,
        mut lane: mpsc::UnboundedReceiver<Drive>,
    ) {
        tokio::spawn(async move {
            while let Some(drive) = lane.recv().await {
                match drive {
                    Drive::Input(data_b64) => write_input(&id, &session, &data_b64).await,
                    Drive::Resize { cols, rows } => resize(&id, &session, cols, rows),
                }
            }
        });
    }

    // ---- the session lifetime ---------------------------------------------

    /// the pump: copy pty output to the node until EOF, then end the session.
    /// One task per session.
    fn spawn_pump(&self, id: String, session: Arc<InteractiveSession>) {
        let plane = self.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; TERM_READ_BUF];
            loop {
                match session.read(&mut buf).await {
                    // EOF: the child (and its container) is gone.
                    Ok(0) => break,
                    Ok(read) => {
                        // never log the bytes — only their count.
                        tracing::trace!(target: "ducktape::term", session = %id, bytes = read, "term_output");
                        plane
                            .0
                            .emit(wire::Event::TermOutput {
                                session: id.clone(),
                                chunk_b64: STANDARD.encode(&buf[..read]),
                            })
                            .await;
                    }
                    Err(error) => {
                        tracing::warn!(target: "ducktape::term", session = %id, reason = "read_failed", error = %error, "term pump stopped");
                        break;
                    }
                }
            }
            plane.finish(&id).await;
        });
    }

    /// arm the hard wall-clock ceiling. Cancelled cleanly the moment the session
    /// ends earlier: [`Self::finish`] drops the entry (and with it the cancel
    /// sender), the select takes the cancel arm, and the timer never fires — so
    /// it cannot reap a later session that reused this id.
    fn spawn_reaper(&self, id: String, cancel: oneshot::Receiver<()>) {
        let plane = self.clone();
        tokio::spawn(async move {
            if !reaper_fires(MAX_SESSION_LIFETIME, cancel).await {
                return; // the session ended before the ceiling — nothing to reap.
            }
            tracing::info!(target: "ducktape::term", session = %id, reason = "lifetime_ceiling", "session reaped");
            plane.finish(&id).await;
        });
    }

    /// end a session exactly once: remove it, release its slot, tell the node,
    /// tear the container down, and take its workdir with it.
    ///
    /// Whoever removes the entry owns the teardown, so an explicit close racing
    /// the pump's EOF can never double-terminate. The `TermEnded` frame is
    /// emitted BEFORE `close()` because the terminator is what unblocks an
    /// attached client and the container teardown can take seconds — the frame
    /// ordering on the link is what makes this safe, not the timing.
    ///
    /// The workdir goes LAST, and by construction rather than by a statement:
    /// `live` — and the [`SessionHome`] inside it — is dropped at the end of
    /// this function, which the awaited `close()` above already precedes. The
    /// directory the container mounts is never removed while it is mounted.
    async fn finish(&self, id: &str) {
        let removed = self
            .0
            .sessions
            .lock()
            .expect("agent sessions lock poisoned")
            .remove(id);
        let Some(live) = removed else {
            return; // already ended: idempotent by construction.
        };
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0
            .emit(wire::Event::TermEnded {
                session: id.to_string(),
            })
            .await;
        live.session.close().await;
        tracing::info!(target: "ducktape::term", session = %id, "session_ended");
    }
}

impl Inner {
    /// the ONE writer. A closed lane means the link task is gone and the process
    /// is shutting down; the pty dies with it, so the drop is benign and worth a
    /// debug line, never a warn per frame.
    async fn emit(&self, event: wire::Event) {
        if self.events.send(event).await.is_err() {
            tracing::debug!(
                target: "ducktape::term",
                reason = "link_closed",
                "agent event dropped"
            );
        }
    }
}

/// write raw bytes to a pty. Bad base64 or a failed write is a no-op + `warn`
/// with a named reason — never a panic, and never the bytes in a log line.
async fn write_input(id: &str, session: &InteractiveSession, data_b64: &str) {
    let Ok(bytes) = STANDARD.decode(data_b64) else {
        tracing::warn!(target: "ducktape::term", session = %id, reason = "bad_base64", "term input dropped");
        return;
    };
    if let Err(error) = session.write_all(&bytes).await {
        tracing::warn!(target: "ducktape::term", session = %id, reason = "write_failed", error = %error, "term input dropped");
    }
}

/// set a pty's window size so the child's TUI reflows.
fn resize(id: &str, session: &InteractiveSession, cols: u16, rows: u16) {
    if let Err(error) = session.resize(cols, rows) {
        tracing::warn!(target: "ducktape::term", session = %id, reason = "resize_failed", error = %error, "term resize dropped");
    }
}

/// resolve to `true` iff `lifetime` elapses before the session is cancelled (its
/// reaper-cancel sender dropped). Split out so the ceiling-vs-cancel race is
/// unit-testable under paused time without a live pty.
async fn reaper_fires(lifetime: Duration, cancel: oneshot::Receiver<()>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(lifetime) => true,
        _ = cancel => false,
    }
}

/// put a consensus-resolved credential record on the wire. The node resolves it
/// (only the node can — that takes committed state and its own actor lane) and
/// the daemon rebuilds it with [`airlock_config`]. Nothing secret crosses; see
/// [`wire::Credential`].
pub fn credential_wire(resolved: &ResolvedCredential) -> wire::Credential {
    wire::Credential {
        name: resolved.name.clone(),
        kind: match resolved.kind {
            CredentialKind::Claude => wire::CredentialKind::Claude,
            CredentialKind::Codex => wire::CredentialKind::Codex,
        },
        authority: resolved.authority.clone(),
        via: resolved.via.clone(),
        seal_pk: resolved.seal_pk,
    }
}

/// rebuild the broker's self-host airlock config from the record the node
/// resolved out of committed state. Nothing secret crosses — see [`wire`].
///
/// [`WorkRef::Direct`], and that is a statement rather than a default: an
/// interactive session has NO committed record of who asked for it, so there is
/// no pointer this side could offer and nothing a lender could resolve. The
/// subject stays the account the lender's node vouches for on the hop — the
/// node running this pty. Delegation is a saga-lane property because a saga is
/// the thing consensus wrote down.
fn airlock_config(credential: wire::Credential) -> AirlockConfig {
    AirlockConfig::self_host(
        &ResolvedCredential {
            name: credential.name,
            kind: match credential.kind {
                wire::CredentialKind::Claude => CredentialKind::Claude,
                wire::CredentialKind::Codex => CredentialKind::Codex,
            },
            authority: credential.authority,
            via: credential.via,
            seal_pk: credential.seal_pk,
        },
        WorkRef::Direct,
    )
}

/// build the sandbox-backed interactive provider set. Unlike the in-node
/// predecessor this returns the error instead of logging and degrading: a daemon
/// whose whole job is spawning ptys must not start without a provider set — it
/// would signal an interactive plane it cannot serve.
pub fn discover(
    node_identity: &[u8],
    backend: provider_host::SandboxBackend,
    owner: &str,
) -> Result<ProviderSet, String> {
    provider_host::discover(node_identity, None, backend, owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a provider that spawns a pty on a plain host `cat` — no VM, no broker,
    /// no guest image — so the session LIFECYCLE (which is what owns the
    /// workdir) is exercisable without a sandbox. `spawns = false` is the
    /// spawn-failure arm.
    struct StubProvider {
        spawns: bool,
    }

    #[async_trait::async_trait]
    impl Provider for StubProvider {
        fn capability(&self) -> &str {
            "stub"
        }

        async fn run(&self, _prompt: &str, _ctx: &RunContext) -> Result<String, String> {
            Err("the stub provider is interactive-only".into())
        }

        async fn spawn_interactive(
            &self,
            _ctx: &RunContext,
            _restricted: bool,
        ) -> Result<InteractiveSession, String> {
            if !self.spawns {
                return Err("stub: no sandbox".into());
            }
            // `cat` sits on its pty until it is terminated — a live child with a
            // real master fd, which is all the session lifecycle needs.
            InteractiveSession::spawn_local(tokio::process::Command::new("cat"))
        }
    }

    const STUB_SESSION: &str = "00000000deadbeef";

    fn stub_create() -> wire::Create {
        wire::Create {
            session: STUB_SESSION.to_string(),
            provider: "stub".to_string(),
            restricted: false,
            limits: std::collections::BTreeMap::new(),
            credential: None,
        }
    }

    /// a `Sessions` over a scratch workdir root, plus the root and the event
    /// lane's receiver (held so `emit` never sees a closed channel).
    fn plane(name: &str) -> (Sessions, PathBuf, mpsc::Receiver<wire::Event>) {
        let root =
            std::env::temp_dir().join(format!("ducktape-term-home-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (events, rx) = mpsc::channel(8);
        let plane = Sessions::new(
            ProviderSet::empty(),
            "test-node".to_string(),
            root.clone(),
            events,
        );
        (plane, root, rx)
    }

    #[tokio::test]
    async fn a_session_workdir_dies_with_its_session() {
        // the leak this closes: every pty session a node hosted left
        // `<storage>/term-sessions/<id>` — its config home and whatever the
        // executor wrote — standing forever.
        let (plane, root, _rx) = plane("ends");
        let provider = StubProvider { spawns: true };
        plane
            .spawn(&provider, stub_create())
            .await
            .expect("the stub session spawns");
        let dir = root.join(STUB_SESSION);
        assert!(dir.is_dir(), "the session's workdir is materialized");

        // `finish` IS the teardown seam — the explicit close, the pump's EOF,
        // the wall-clock ceiling and a lost link all route through it.
        plane.finish(STUB_SESSION).await;
        assert!(
            !dir.exists(),
            "the session's workdir outlived the session: {}",
            dir.display()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_spawn_that_fails_leaves_no_workdir_behind() {
        // the guard is built BEFORE the spawn, so the failure path has to take
        // the half-materialized directory with it — this session never reaches
        // the map, so `finish` will never run for it.
        let (plane, root, _rx) = plane("refused");
        let provider = StubProvider { spawns: false };
        let (refusal, _detail) = plane
            .spawn(&provider, stub_create())
            .await
            .expect_err("the stub refuses to spawn");
        assert_eq!(refusal, wire::Refusal::SpawnFailed);
        let dir = root.join(STUB_SESSION);
        assert!(
            !dir.exists(),
            "a refused create left a workdir behind: {}",
            dir.display()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test(start_paused = true)]
    async fn the_reaper_fires_when_nothing_cancels_it() {
        let (_tx, rx) = oneshot::channel();
        assert!(reaper_fires(Duration::from_secs(1), rx).await);
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_cancel_sender_disarms_the_reaper() {
        // this is the "session ended early" path: `finish` drops the map entry,
        // which drops the sender. The ceiling must NOT fire afterwards, or it
        // would reap a later session that reused the id.
        let (tx, rx) = oneshot::channel::<()>();
        drop(tx);
        assert!(!reaper_fires(Duration::from_secs(1), rx).await);
    }

    #[test]
    fn the_credential_mirror_keeps_both_vendor_arms() {
        // a silent mis-map here would send a Claude session to a Codex gateway.
        for (wire_kind, expected) in [
            (wire::CredentialKind::Claude, CredentialKind::Claude),
            (wire::CredentialKind::Codex, CredentialKind::Codex),
        ] {
            let resolved = ResolvedCredential {
                name: "c".into(),
                kind: expected,
                authority: "a".into(),
                via: "http://v".into(),
                seal_pk: [7u8; 32],
            };
            let expected_config = AirlockConfig::self_host(&resolved, WorkRef::Direct);
            let built = airlock_config(wire::Credential {
                name: "c".into(),
                kind: wire_kind,
                authority: "a".into(),
                via: "http://v".into(),
                seal_pk: [7u8; 32],
            });
            assert_eq!(built, expected_config);
        }
    }
}
