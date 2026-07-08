//! The joiner's first-contact race over the invite's unified path set.
//!
//! A unified all-paths invite offers the joiner MORE than one way to bring its
//! WireGuard tunnel up: the inviter itself, plus every reachable member the
//! inviter meshes with (the invite's `fronts`). This module turns that set into
//! ONE candidate list (`{inviter} ∪ {fronts}`), races first contact across the
//! whole union, and stops at the first candidate whose intro is installed —
//! cancelling the rest. If every path is exhausted it returns an HONEST
//! terminal (a distinct, mode-naming failure the caller surfaces loudly and
//! exits on), never a silent success.
//!
//! Two mechanics, reused verbatim from PR #260:
//! - a candidate with a routable underlay `endpoint` is DIRECT: install the
//!   join-window tunnel peer at that endpoint, then announce the token-signed
//!   intro over plain UDP to its intro listener (`wg_port + 1`) until acked;
//! - a candidate with no endpoint is COORDINATED (by identity): drive
//!   `BootstrapCoordinatedInvitePeer` through the joiner's AMBIENT coordinator
//!   until the ack rides back over the punched underlay.
//!
//! The decision logic (union building, TUN-mode filtering, first-ack-wins,
//! honest terminal) is pure and unit-tested; the two real mechanics live below
//! it and are exercised end-to-end against a live plane.

use std::future::Future;
use std::net::{SocketAddr, ToSocketAddrs as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use futures::StreamExt as _;

use crate::config::Front;
use crate::lobby;

/// how long each attempt paces a retry / waits for an ack.
const RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// one first-contact path in the race: the inviter or one front, in a single
/// shape. `endpoint Some` ⇒ a DIRECT underlay endpoint (`host:wg_port`);
/// `None` ⇒ reachable only BY IDENTITY through the ambient coordinator.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// the member's real ed25519 node identity the joiner authenticates.
    pub key: ed25519::PublicKey,
    /// the member's X25519 WireGuard public key.
    pub wg: [u8; 32],
    /// the member's overlay control port (unused by the race itself; carried
    /// so callers can log/inject the winner's overlay Direct hint).
    pub mesh_port: u16,
    /// the member's routable WireGuard underlay endpoint (`host:wg_port`), or
    /// `None` for a punchable member reached through the coordinator.
    pub endpoint: Option<String>,
}

impl Candidate {
    fn is_coordinated(&self) -> bool {
        self.endpoint.is_none()
    }

    /// how this candidate was reached — for the winner log.
    pub fn via(&self) -> ContactVia {
        match &self.endpoint {
            Some(endpoint) => ContactVia::Direct(endpoint.clone()),
            None => ContactVia::Coordinated,
        }
    }
}

/// which mechanic brought a candidate up (names the mode in logs).
#[derive(Clone, Debug, PartialEq)]
pub enum ContactVia {
    Direct(String),
    Coordinated,
}

impl std::fmt::Display for ContactVia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactVia::Direct(endpoint) => write!(f, "direct ({endpoint})"),
            ContactVia::Coordinated => write!(f, "coordinated rendezvous"),
        }
    }
}

/// the outcome of a single candidate's attempt.
#[derive(Debug, PartialEq)]
pub enum AttemptResult {
    /// the inviter/member acked our intro: the tunnel is up.
    Installed,
    /// the attempt exhausted its window (or the plane went away).
    Failed(String),
}

/// the race's terminal state.
#[derive(Debug, PartialEq)]
pub enum FirstContactOutcome {
    /// a candidate installed our intro first — its tunnel carries the join.
    Installed {
        key: ed25519::PublicKey,
        via: ContactVia,
    },
    /// every offered path was exhausted. HONEST: the caller must surface this
    /// and exit non-zero rather than proceed as if joined.
    Terminal { tried: usize, reason: String },
}

/// the inviter as a first-contact candidate (its own WireGuard bootstrap).
pub struct InviterContact {
    pub key: ed25519::PublicKey,
    pub wg: [u8; 32],
    pub mesh_port: u16,
    /// the inviter's underlay endpoint (`host:wg_port`), or `None` for a
    /// coordinated-only inviter.
    pub endpoint: Option<String>,
}

/// build the joiner's candidate set: the inviter (if it offered a bootstrap)
/// UNION every front. One `Candidate` shape; a front whose `member_key` is not
/// a valid ed25519 key is dropped rather than aborting the whole join.
pub fn build_candidates(inviter: Option<InviterContact>, fronts: &[Front]) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(inv) = inviter {
        out.push(Candidate {
            key: inv.key,
            wg: inv.wg,
            mesh_port: inv.mesh_port,
            endpoint: inv.endpoint,
        });
    }
    for front in fronts {
        match ed25519::PublicKey::decode(&front.member_key[..]) {
            Ok(key) => out.push(Candidate {
                key,
                wg: front.wireguard_public_key,
                mesh_port: front.mesh_port,
                endpoint: front.endpoint.clone(),
            }),
            Err(_) => continue,
        }
    }
    out
}

/// filter the race for the effect mode. Under a real kernel TUN interface the
/// userspace rendezvous/hole-punch resolver the coordinated path needs is not
/// active, so a coordinated (by-identity) candidate can only hang — drop it.
/// Direct candidates are always kept.
pub fn plan_race(candidates: Vec<Candidate>, tun_mode: bool) -> Vec<Candidate> {
    if !tun_mode {
        return candidates;
    }
    candidates
        .into_iter()
        .filter(|c| !c.is_coordinated())
        .collect()
}

/// Race `attempt` across every candidate concurrently; the FIRST to install
/// wins and the rest are cancelled (their futures are dropped). Exhaustion ⇒
/// an honest [`FirstContactOutcome::Terminal`]. Pure over the attempt function
/// so the selection logic is unit-testable without a live plane.
pub async fn race_first_contact<F, Fut>(candidates: Vec<Candidate>, attempt: F) -> FirstContactOutcome
where
    F: Fn(Candidate) -> Fut,
    Fut: Future<Output = AttemptResult>,
{
    let tried = candidates.len();
    if tried == 0 {
        return FirstContactOutcome::Terminal {
            tried: 0,
            reason: "no reachable first-contact paths in the invite (inviter + fronts all \
                     filtered out for this effect mode)"
                .into(),
        };
    }
    let mut inflight = futures::stream::FuturesUnordered::new();
    for candidate in candidates {
        let key = candidate.key.clone();
        let via = candidate.via();
        // build the future eagerly (futures are lazy — nothing runs until
        // polled), so the attempt closure is only borrowed, never moved.
        let fut = attempt(candidate);
        inflight.push(async move { (key, via, fut.await) });
    }
    let mut last_reason = String::from("every offered first-contact path failed");
    while let Some((key, via, result)) = inflight.next().await {
        match result {
            AttemptResult::Installed => return FirstContactOutcome::Installed { key, via },
            AttemptResult::Failed(reason) => last_reason = reason,
        }
    }
    FirstContactOutcome::Terminal {
        tried,
        reason: last_reason,
    }
}

/// the real driver: race the candidate union using #260's two mechanics. The
/// intro datagram is built ONCE by the caller (this joiner's own token-signed
/// intro) and reused across every candidate. `window` bounds the whole race;
/// each attempt paces itself at [`RETRY_INTERVAL`].
pub async fn drive_first_contact(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    candidates: Vec<Candidate>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    label: String,
    window: Duration,
) -> FirstContactOutcome {
    let iters = (window.as_secs() / RETRY_INTERVAL.as_secs()).max(1) as u32;
    let attempt = |candidate: Candidate| {
        let reach = reach.clone();
        let intro = intro.clone();
        let token_nonce = token_nonce.clone();
        let label = label.clone();
        async move {
            match candidate.endpoint.clone() {
                Some(endpoint) => {
                    direct_attempt(reach, intro, token_nonce, candidate, endpoint, label, iters).await
                }
                None => coordinated_attempt(reach, intro, token_nonce, candidate, label, iters).await,
            }
        }
    };
    race_first_contact(candidates, attempt).await
}

/// trips a stop flag when the owning attempt future is dropped (the attempt
/// lost the race), so its blocking announcer thread exits promptly.
struct StopGuard(Arc<AtomicBool>);

impl Drop for StopGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// DIRECT: install the join-window peer at its underlay endpoint, then run the
/// blocking UDP intro announcer (its own OS thread, cancellable by the stop
/// guard) targeting the member's intro listener at `wg_port + 1`.
async fn direct_attempt(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    candidate: Candidate,
    endpoint: String,
    label: String,
    iters: u32,
) -> AttemptResult {
    let endpoint_addr = match endpoint.to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(addr) => addr,
            None => return AttemptResult::Failed(format!("front endpoint {endpoint:?} did not resolve")),
        },
        Err(e) => return AttemptResult::Failed(format!("front endpoint {endpoint:?} unusable ({e})")),
    };

    // (a) merge the peer onto the interface so this side can initiate.
    let (install_tx, install_rx) = tokio::sync::oneshot::channel();
    if reach
        .send(reachability::ReachabilityCommand::InstallInvitePeer {
            peer: candidate.key.clone(),
            wireguard_public_key: wireguard_upgrade::X25519PublicKey(candidate.wg),
            endpoint: endpoint_addr,
            reply: reachability::InstallReply(install_tx),
        })
        .await
        .is_err()
    {
        return AttemptResult::Failed("reachability plane is gone".into());
    }
    match install_rx.await {
        Ok(Ok(())) => {}
        // a failed install is not fatal to THIS attempt — WireGuard roaming can
        // still pin the tunnel once the intro reaches the peer; log and press on.
        Ok(Err(e)) => eprintln!(
            "[node {label}] first-contact: direct peer {endpoint_addr} not installed ({e}) — \
             announcing anyway"
        ),
        Err(_) => return AttemptResult::Failed("install reply dropped".into()),
    }

    // (b) the intro listener is the underlay port + 1 (the product-wide
    // `invite_listen` default).
    let intro_dest = SocketAddr::new(endpoint_addr.ip(), endpoint_addr.port().saturating_add(1));
    let stop = Arc::new(AtomicBool::new(false));
    let _guard = StopGuard(stop.clone());
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("first-contact-direct".into())
        .spawn(move || {
            let _ = done_tx.send(run_direct_announcer(
                &intro,
                &token_nonce,
                intro_dest,
                iters,
                &thread_stop,
            ));
        })
        .expect("spawn first-contact-direct thread");

    match done_rx.await {
        Ok(result) => result,
        // the future is being dropped (we lost the race) or the thread died;
        // either way this attempt did not win.
        Err(_) => AttemptResult::Failed("direct announcer ended without a result".into()),
    }
}

/// the blocking UDP announce loop (its own thread — nothing here touches the
/// async runtime): re-send the intro every [`RETRY_INTERVAL`] until the member
/// acks an install, the window is exhausted, or the stop flag trips.
fn run_direct_announcer(
    intro: &[u8],
    token_nonce: &[u8],
    dest: SocketAddr,
    iters: u32,
    stop: &AtomicBool,
) -> AttemptResult {
    let socket = match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => socket,
        Err(e) => return AttemptResult::Failed(format!("intro udp bind: {e}")),
    };
    let _ = socket.set_read_timeout(Some(RETRY_INTERVAL));
    let mut buf = [0u8; 2048];
    for _ in 0..iters {
        if stop.load(Ordering::Relaxed) {
            return AttemptResult::Failed("cancelled".into());
        }
        let _ = socket.send_to(intro, dest);
        // the read timeout paces the loop; an ack for THIS invite that reports
        // installed wins, any other reply is ignored and we retry.
        if let Ok((n, _)) = socket.recv_from(&mut buf)
            && let Ok(ack) = lobby::decode_intro_ack(&buf[..n])
            && ack.nonce == token_nonce
            && ack.installed
        {
            return AttemptResult::Installed;
        }
    }
    AttemptResult::Failed(format!(
        "direct intro to {dest} was not acked within the join window"
    ))
}

/// COORDINATED: drive `BootstrapCoordinatedInvitePeer` through the ambient
/// coordinator until the ack rides back over the punched underlay socket.
async fn coordinated_attempt(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    candidate: Candidate,
    label: String,
    iters: u32,
) -> AttemptResult {
    for _ in 0..iters {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if reach
            .send(reachability::ReachabilityCommand::BootstrapCoordinatedInvitePeer {
                peer: candidate.key.clone(),
                wireguard_public_key: wireguard_upgrade::X25519PublicKey(candidate.wg),
                intro: intro.clone(),
                reply: reachability::CoordinatedInviteReply(reply_tx),
            })
            .await
            .is_err()
        {
            return AttemptResult::Failed("reachability plane is gone".into());
        }
        match reply_rx.await {
            Ok(Ok(bytes)) => {
                if let Ok(ack) = lobby::decode_intro_ack(&bytes)
                    && ack.nonce == token_nonce
                    && ack.installed
                {
                    return AttemptResult::Installed;
                }
            }
            // the rendezvous underlay is not ready yet — retry within the window.
            Ok(Err(_)) => {}
            Err(_) => return AttemptResult::Failed("coordinated reply dropped".into()),
        }
        let _ = &label; // reserved for future per-attempt tracing
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
    AttemptResult::Failed("coordinated intro was not acked within the join window".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u64) -> ed25519::PublicKey {
        use commonware_cryptography::Signer as _;
        ed25519::PrivateKey::from_seed(seed).public_key()
    }

    fn inviter(endpoint: Option<&str>) -> InviterContact {
        InviterContact {
            key: key(1),
            wg: [1u8; 32],
            mesh_port: 52200,
            endpoint: endpoint.map(str::to_string),
        }
    }

    fn front(seed: u8, endpoint: Option<&str>) -> Front {
        use commonware_cryptography::Signer as _;
        let member = ed25519::PrivateKey::from_seed(seed as u64 + 100).public_key();
        Front {
            member_key: member.as_ref().try_into().unwrap(),
            wireguard_public_key: [seed; 32],
            mesh_port: 52200 + seed as u16,
            endpoint: endpoint.map(str::to_string),
        }
    }

    #[test]
    fn build_candidates_inviter_only_yields_one() {
        let candidates = build_candidates(Some(inviter(Some("198.51.100.1:51820"))), &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].key, key(1));
    }

    #[test]
    fn build_candidates_unions_inviter_and_fronts_preserving_mode() {
        let fronts = vec![front(2, Some("198.51.100.2:51820")), front(3, None)];
        let candidates = build_candidates(Some(inviter(Some("198.51.100.1:51820"))), &fronts);
        assert_eq!(candidates.len(), 3, "inviter ∪ 2 fronts");
        // a front with an endpoint is DIRECT, without is COORDINATED.
        let direct = &candidates[1];
        assert!(matches!(direct.via(), ContactVia::Direct(_)));
        let coordinated = &candidates[2];
        assert_eq!(coordinated.via(), ContactVia::Coordinated);
    }

    #[test]
    fn plan_race_drops_coordinated_under_tun() {
        let candidates = build_candidates(
            Some(inviter(None)), // coordinated inviter
            &[front(2, Some("198.51.100.2:51820")), front(3, None)],
        );
        assert_eq!(candidates.len(), 3);
        let planned = plan_race(candidates, true);
        assert_eq!(planned.len(), 1, "only the direct front survives TUN mode");
        assert!(matches!(planned[0].via(), ContactVia::Direct(_)));
    }

    #[tokio::test]
    async fn race_installs_the_first_to_ack_without_waiting_on_the_rest() {
        let winner = key(1);
        let candidates = vec![
            Candidate { key: winner.clone(), wg: [1; 32], mesh_port: 1, endpoint: Some("win".into()) },
            Candidate { key: key(2), wg: [2; 32], mesh_port: 2, endpoint: Some("slow".into()) },
        ];
        let outcome = race_first_contact(candidates, |c| async move {
            match c.endpoint.as_deref() {
                Some("win") => AttemptResult::Installed,
                // the loser never resolves; the race must not wait on it.
                _ => std::future::pending::<AttemptResult>().await,
            }
        })
        .await;
        match outcome {
            FirstContactOutcome::Installed { key, via } => {
                assert_eq!(key, winner);
                assert_eq!(via, ContactVia::Direct("win".into()));
            }
            other => panic!("expected Installed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn race_returns_honest_terminal_when_all_fail() {
        let candidates = vec![
            Candidate { key: key(1), wg: [1; 32], mesh_port: 1, endpoint: Some("a".into()) },
            Candidate { key: key(2), wg: [2; 32], mesh_port: 2, endpoint: None },
        ];
        let outcome = race_first_contact(candidates, |_c| async move {
            AttemptResult::Failed("nope".into())
        })
        .await;
        match outcome {
            FirstContactOutcome::Terminal { tried, reason } => {
                assert_eq!(tried, 2);
                assert!(reason.contains("nope"), "reason names the failure: {reason}");
            }
            other => panic!("expected Terminal, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_candidate_set_is_terminal_not_a_hang() {
        // a lone coordinated candidate filtered out by TUN leaves nothing to
        // race — an immediate honest terminal, never a hang.
        let planned = plan_race(build_candidates(Some(inviter(None)), &[]), true);
        assert!(planned.is_empty());
        let outcome =
            race_first_contact(planned, |_c| async move { AttemptResult::Installed }).await;
        assert!(matches!(
            outcome,
            FirstContactOutcome::Terminal { tried: 0, .. }
        ));
    }
}
