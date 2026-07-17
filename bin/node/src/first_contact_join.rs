//! The joiner's first-contact race over the invite's unified path set.
//!
//! A unified all-paths invite offers the joiner MORE than one way to bring its
//! WireGuard tunnel up: the inviter itself, plus every reachable member the
//! inviter meshes with (the invite's `fronts`). This module turns that set into
//! ONE candidate list (`{inviter} ∪ {fronts}`), races first contact across the
//! whole union, and stops at the first candidate whose doorbell SETTLES THE
//! GATE (Join v2 §4: the sealed intro is the gate request, and the acked
//! `Admitted`/terminal `Rejected` is the authoritative outcome) — cancelling
//! the rest. If every path is exhausted it returns an HONEST terminal (a
//! distinct, mode-naming failure the caller surfaces loudly and exits on),
//! never a silent success.
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
    /// the member's explicitly-advertised UDP intro listener (`host:port`) the
    /// joiner announces its token-signed intro to on the DIRECT path. Carried
    /// verbatim from the invite (the inviter advertises this as `wg.intro`,
    /// which honors an operator-overridden `invite_listen`); `None` ⇒ derive
    /// the intro port as `endpoint`'s `wg_port + 1` (the product-wide default,
    /// and the only shape fronts advertise).
    pub intro: Option<String>,
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

/// the outcome of a single candidate's attempt. the sealed intro IS the gate
/// request (Join v2 §4), so an attempt no longer succeeds at "tunnel up" — it
/// resolves when the member's doorbell answers the GATE.
#[derive(Debug, PartialEq)]
pub enum AttemptResult {
    /// the AUTHORITATIVE admission: the member settled `Redeem` and it
    /// COMMITTED at `height`; `cap` is the opaque coordinator capability
    /// (private coordination) or `None`.
    Admitted { height: u64, cap: Option<Vec<u8>> },
    /// the gate refused TERMINALLY — this invite can never redeem; the whole
    /// race stops and the joiner exits (ADR R2).
    Rejected {
        code: lobby::RejectCode,
        detail: String,
    },
    /// the attempt exhausted its window, was refused non-terminally, or the
    /// plane went away — the race fails over to the next candidate.
    Failed(String),
}

/// the race's terminal state.
// one value per race ROUND (a join makes a handful, ever) and consumed by a
// single match — boxing `Admitted`'s fields would buy nothing but noise at
// that match, hence the `large_enum_variant` allowance (the interface.rs
// precedent).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
pub enum FirstContactOutcome {
    /// a member answered the AUTHORITATIVE `Admitted` — standing is COMMITTED
    /// and its tunnel carries everything after.
    Admitted {
        key: ed25519::PublicKey,
        via: ContactVia,
        height: u64,
        cap: Option<Vec<u8>>,
    },
    /// a member answered a TERMINAL `Rejected` — this invite can never
    /// redeem. the caller exits loudly instead of failing over (ADR R2).
    Rejected {
        code: lobby::RejectCode,
        detail: String,
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
    /// the inviter's explicitly-advertised UDP intro listener (`host:port`,
    /// from the invite's `wg.intro`), honored verbatim on the DIRECT path so an
    /// operator-overridden `invite_listen` (a host/port other than
    /// `wg_port + 1`) is reached correctly. `None` ⇒ fall back to
    /// `endpoint`'s `wg_port + 1`.
    pub intro: Option<String>,
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
            // the inviter advertises its intro listener explicitly; honor it.
            intro: inv.intro,
        });
    }
    for front in fronts {
        match ed25519::PublicKey::decode(&front.member_key[..]) {
            Ok(key) => out.push(Candidate {
                key,
                wg: front.wireguard_public_key,
                mesh_port: front.mesh_port,
                endpoint: front.endpoint.clone(),
                // fronts advertise no separate intro listener — the direct
                // path derives `wg_port + 1`.
                intro: None,
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

/// Race `attempt` across every candidate concurrently; the FIRST to settle
/// the gate (`Admitted`, or a terminal `Rejected`) wins and the rest are
/// cancelled (their futures are dropped). Exhaustion ⇒ an honest
/// [`FirstContactOutcome::Terminal`]. Pure over the attempt function so the
/// selection logic is unit-testable without a live plane.
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
            AttemptResult::Admitted { height, cap } => {
                return FirstContactOutcome::Admitted {
                    key,
                    via,
                    height,
                    cap,
                };
            }
            // a terminal refusal is a NETWORK-WIDE truth (spent nonce, bad
            // token) — failing over would just collect the same answer.
            AttemptResult::Rejected { code, detail } => {
                return FirstContactOutcome::Rejected { code, detail };
            }
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
/// intro) and reused across every candidate. `keypair` is this joiner's OWN
/// WireGuard keypair — post-verify acks arrive SEALED to it (the coordinator
/// cap must never cross in the clear). `window` bounds the whole race; each
/// attempt paces itself at [`RETRY_INTERVAL`].
pub async fn drive_first_contact(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    candidates: Vec<Candidate>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    keypair: Arc<reachability::WireGuardKeypair>,
    label: String,
    window: Duration,
) -> FirstContactOutcome {
    let tried = candidates.len();
    let iters = (window.as_secs() / RETRY_INTERVAL.as_secs()).max(1) as u32;
    let attempt = |candidate: Candidate| {
        let reach = reach.clone();
        let intro = intro.clone();
        let token_nonce = token_nonce.clone();
        let keypair = keypair.clone();
        let label = label.clone();
        async move {
            match candidate.endpoint.clone() {
                Some(endpoint) => {
                    direct_attempt(
                        reach,
                        intro,
                        token_nonce,
                        keypair,
                        candidate,
                        endpoint,
                        label,
                        iters,
                    )
                    .await
                }
                None => {
                    coordinated_attempt(reach, intro, token_nonce, keypair, candidate, label, iters)
                        .await
                }
            }
        }
    };
    // The window is a HARD bound, not just loop pacing: an attempt parked on
    // a reply the plane never sends (its command loop stalled) would
    // otherwise hang the race forever — no Terminal, no exit, no log line.
    match tokio::time::timeout(window, race_first_contact(candidates, attempt)).await {
        Ok(outcome) => outcome,
        Err(_elapsed) => FirstContactOutcome::Terminal {
            tried,
            reason: format!(
                "join window ({}s) elapsed with no candidate acked — every path stayed dark \
                 (reachability plane unresponsive or peers unreachable)",
                window.as_secs()
            ),
        },
    }
}

/// Open one ack datagram for this attempt and decode the member's reply.
/// Post-verify acks arrive SEALED to this joiner's WG key; a pre-verify
/// `Refused` is cleartext — try the seal first, fall back to the raw bytes.
/// `None` ⇒ junk, or another attempt's ack (nonce mismatch): the announcer
/// ignores it and keeps sending.
fn open_ack(
    keypair: &reachability::WireGuardKeypair,
    token_nonce: &[u8],
    datagram: &[u8],
) -> Option<lobby::IntroReply> {
    let opened = keypair
        .open_sealed(datagram)
        .unwrap_or_else(|_| datagram.to_vec());
    let ack = lobby::decode_intro_ack(&opened).ok()?;
    if ack.nonce != token_nonce {
        return None;
    }
    Some(ack.reply)
}

/// What a decoded gate reply does to the attempt: `None` ⇒ the gate is still
/// settling (`Installed`) — keep announcing, a later retransmit carries the
/// outcome home; `Some` ⇒ the attempt resolved.
fn ack_resolution(reply: lobby::IntroReply) -> Option<AttemptResult> {
    match reply {
        lobby::IntroReply::Installed => None,
        lobby::IntroReply::Admitted { height, cap } => {
            Some(AttemptResult::Admitted { height, cap })
        }
        lobby::IntroReply::Rejected {
            code,
            detail,
            terminal: true,
        } => Some(AttemptResult::Rejected { code, detail }),
        // a non-terminal refusal (issuer view lag, member busy) fails THIS
        // candidate over — the race tries the next one.
        lobby::IntroReply::Rejected {
            code,
            detail,
            terminal: false,
        } => Some(AttemptResult::Failed(format!("{code:?}: {detail}"))),
        lobby::IntroReply::Refused { detail } => Some(AttemptResult::Failed(detail)),
    }
}

/// trips a stop flag when the owning attempt future is dropped (the attempt
/// lost the race), so its blocking announcer thread exits promptly.
struct StopGuard(Arc<AtomicBool>);

impl Drop for StopGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Where the DIRECT path announces this joiner's token-signed intro. Honor the
/// candidate's explicitly-advertised `intro` endpoint VERBATIM when present —
/// the inviter advertises this as `wg.intro`, which honors an operator-set
/// `invite_listen` that need not be `wg_port + 1` (nor even the same host).
/// Only when no intro is advertised (fronts never advertise one) does the
/// underlay `wg_port + 1` default apply. Pure, so it is unit-testable without a
/// live plane.
fn resolve_intro_dest(candidate: &Candidate, endpoint_addr: SocketAddr) -> Result<SocketAddr, String> {
    match &candidate.intro {
        Some(advertised) => match advertised.to_socket_addrs() {
            Ok(mut addrs) => addrs
                .next()
                .ok_or_else(|| format!("advertised intro endpoint {advertised:?} did not resolve")),
            Err(e) => Err(format!("advertised intro endpoint {advertised:?} unusable ({e})")),
        },
        None => Ok(SocketAddr::new(
            endpoint_addr.ip(),
            endpoint_addr.port().saturating_add(1),
        )),
    }
}

/// DIRECT: install the join-window peer at its underlay endpoint, then run the
/// blocking UDP intro announcer (its own OS thread, cancellable by the stop
/// guard) targeting the member's intro listener — its explicitly-advertised
/// `intro` endpoint when present (honoring an operator-overridden
/// `invite_listen`), otherwise the underlay `wg_port + 1` default.
#[allow(clippy::too_many_arguments)]
async fn direct_attempt(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    keypair: Arc<reachability::WireGuardKeypair>,
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
            wireguard_public_key: wireguard::X25519PublicKey(candidate.wg),
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

    // (b) resolve the intro listener: the candidate's advertised `intro`
    // endpoint verbatim when present, else the underlay `wg_port + 1` default.
    let intro_dest = match resolve_intro_dest(&candidate, endpoint_addr) {
        Ok(dest) => dest,
        Err(e) => return AttemptResult::Failed(e),
    };
    // SEAL the intro to THIS candidate's WireGuard X25519 key (item 5): the
    // plaintext bundle carries the bearer token, which must never cross the
    // wire in the clear. Sealed once per candidate (one ephemeral key), reused
    // across the attempt's retransmits — only this member can open it.
    let sealed_intro = reachability::seal(&candidate.wg, &intro);
    let stop = Arc::new(AtomicBool::new(false));
    let _guard = StopGuard(stop.clone());
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("first-contact-direct".into())
        .spawn(move || {
            let _ = done_tx.send(run_direct_announcer(
                &sealed_intro,
                &token_nonce,
                &keypair,
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
/// async runtime): re-send the intro every [`RETRY_INTERVAL`] until the gate
/// resolves, the window is exhausted, or the stop flag trips. an `Installed`
/// ack means "the gate is settling in consensus" — keep sending; a later
/// retransmit's ack carries the settled outcome (Join v2 §4).
fn run_direct_announcer(
    intro: &[u8],
    token_nonce: &[u8],
    keypair: &reachability::WireGuardKeypair,
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
        // the read timeout paces the loop; a resolving ack for THIS invite
        // settles the attempt, anything else is ignored and we retry.
        if let Ok((n, _)) = socket.recv_from(&mut buf)
            && let Some(reply) = open_ack(keypair, token_nonce, &buf[..n])
            && let Some(result) = ack_resolution(reply)
        {
            return result;
        }
    }
    AttemptResult::Failed(format!(
        "direct intro to {dest} was not answered within the join window"
    ))
}

/// COORDINATED: drive `BootstrapCoordinatedInvitePeer` through the ambient
/// coordinator until a gate-resolving ack rides back over the punched
/// underlay socket (`Installed` = still settling — keep driving).
async fn coordinated_attempt(
    reach: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    intro: Vec<u8>,
    token_nonce: Vec<u8>,
    keypair: Arc<reachability::WireGuardKeypair>,
    candidate: Candidate,
    label: String,
    iters: u32,
) -> AttemptResult {
    // SEAL the intro to THIS candidate's WireGuard X25519 key (item 5) once,
    // reused across the attempt's rendezvous retries — the bearer token never
    // crosses the wire in the clear, and only this member can open it.
    let sealed_intro = reachability::seal(&candidate.wg, &intro);
    for attempt in 1..=iters {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if reach
            .send(reachability::ReachabilityCommand::BootstrapCoordinatedInvitePeer {
                peer: candidate.key.clone(),
                wireguard_public_key: wireguard::X25519PublicKey(candidate.wg),
                intro: sealed_intro.clone(),
                reply: reachability::CoordinatedInviteReply(reply_tx),
            })
            .await
            .is_err()
        {
            return AttemptResult::Failed("reachability plane is gone".into());
        }
        match reply_rx.await {
            Ok(Ok(bytes)) => {
                if let Some(reply) = open_ack(&keypair, &token_nonce, &bytes)
                    && let Some(result) = ack_resolution(reply)
                {
                    return result;
                }
            }
            // the rendezvous underlay is not ready yet — retry within the window.
            Ok(Err(_)) => {}
            Err(_) => return AttemptResult::Failed("coordinated reply dropped".into()),
        }
        // the line this codebase reserved for itself. `nonce` is the ONE id on
        // BOTH sides of a join (lobby::IntroAck.nonce == our token_nonce), so an
        // inviter's log and a joiner's log can finally be read together.
        tracing::debug!(
            target: "ducktape::join",
            node = %label,
            nonce = %noded::hex_bytes(&token_nonce[..token_nonce.len().min(8)]),
            peer = %noded::hex_bytes(&candidate.key.as_ref()[..4]),
            via = "coordinated",
            attempt,
            iters,
            "first-contact intro not yet acked — retrying"
        );
        tokio::time::sleep(RETRY_INTERVAL).await;
    }
    tracing::warn!(
        target: "ducktape::join",
        node = %label,
        nonce = %noded::hex_bytes(&token_nonce[..token_nonce.len().min(8)]),
        peer = %noded::hex_bytes(&candidate.key.as_ref()[..4]),
        via = "coordinated",
        attempts = iters,
        "first-contact candidate EXHAUSTED — no ack within the join window"
    );
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
            intro: None,
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
    async fn race_admits_the_first_to_settle_without_waiting_on_the_rest() {
        let winner = key(1);
        let candidates = vec![
            Candidate { key: winner.clone(), wg: [1; 32], mesh_port: 1, endpoint: Some("win".into()), intro: None },
            Candidate { key: key(2), wg: [2; 32], mesh_port: 2, endpoint: Some("slow".into()), intro: None },
        ];
        let outcome = race_first_contact(candidates, |c| async move {
            match c.endpoint.as_deref() {
                Some("win") => AttemptResult::Admitted {
                    height: 7,
                    cap: Some(vec![1, 2, 3]),
                },
                // the loser never resolves; the race must not wait on it.
                _ => std::future::pending::<AttemptResult>().await,
            }
        })
        .await;
        match outcome {
            FirstContactOutcome::Admitted { key, via, height, cap } => {
                assert_eq!(key, winner);
                assert_eq!(via, ContactVia::Direct("win".into()));
                assert_eq!(height, 7);
                assert_eq!(cap.as_deref(), Some(&[1u8, 2, 3][..]));
            }
            other => panic!("expected Admitted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_terminal_reject_stops_the_race_instead_of_failing_over() {
        // ADR R2: a terminal refusal (spent nonce, bad token) is network-wide
        // truth — the race must surface it, never churn the remaining
        // candidates toward the same answer.
        let candidates = vec![
            Candidate { key: key(1), wg: [1; 32], mesh_port: 1, endpoint: Some("reject".into()), intro: None },
            Candidate { key: key(2), wg: [2; 32], mesh_port: 2, endpoint: Some("slow".into()), intro: None },
        ];
        let outcome = race_first_contact(candidates, |c| async move {
            match c.endpoint.as_deref() {
                Some("reject") => AttemptResult::Rejected {
                    code: lobby::RejectCode::Spent,
                    detail: "invite already redeemed".into(),
                },
                _ => std::future::pending::<AttemptResult>().await,
            }
        })
        .await;
        match outcome {
            FirstContactOutcome::Rejected { code, detail } => {
                assert_eq!(code, lobby::RejectCode::Spent);
                assert!(detail.contains("already redeemed"), "{detail}");
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn ack_resolution_maps_each_reply_to_its_attempt_step() {
        // Installed = the gate is settling: keep announcing.
        assert_eq!(ack_resolution(lobby::IntroReply::Installed), None);
        // Admitted settles the attempt with the authoritative outcome.
        assert_eq!(
            ack_resolution(lobby::IntroReply::Admitted { height: 3, cap: None }),
            Some(AttemptResult::Admitted { height: 3, cap: None })
        );
        // a TERMINAL reject stops the race; a non-terminal one fails over.
        assert!(matches!(
            ack_resolution(lobby::IntroReply::Rejected {
                code: lobby::RejectCode::Spent,
                detail: "spent".into(),
                terminal: true,
            }),
            Some(AttemptResult::Rejected { code: lobby::RejectCode::Spent, .. })
        ));
        assert!(matches!(
            ack_resolution(lobby::IntroReply::Rejected {
                code: lobby::RejectCode::Busy,
                detail: "settling too slowly".into(),
                terminal: false,
            }),
            Some(AttemptResult::Failed(_))
        ));
        assert!(matches!(
            ack_resolution(lobby::IntroReply::Refused { detail: "no".into() }),
            Some(AttemptResult::Failed(_))
        ));
    }

    #[test]
    fn open_ack_opens_sealed_and_cleartext_and_drops_foreign_nonces() {
        let dir = tempfile::tempdir().unwrap();
        let joiner = reachability::WireGuardKeypair::load_or_generate(&dir.path().join("j.key"))
            .unwrap()
            .0;
        let nonce = vec![9u8; 4];
        let ack = lobby::IntroAck {
            nonce: nonce.clone(),
            reply: lobby::IntroReply::Admitted { height: 1, cap: None },
        };
        let plain = lobby::encode_intro_ack(&ack);
        // sealed (the post-verify shape) opens with the joiner's own key…
        let sealed = reachability::seal(&joiner.public_key().0, &plain);
        assert_eq!(
            open_ack(&joiner, &nonce, &sealed),
            Some(lobby::IntroReply::Admitted { height: 1, cap: None })
        );
        // …and a cleartext pre-verify refusal still decodes via the fallback.
        assert_eq!(
            open_ack(&joiner, &nonce, &plain),
            Some(lobby::IntroReply::Admitted { height: 1, cap: None })
        );
        // another attempt's nonce is not ours to interpret.
        assert_eq!(open_ack(&joiner, &[7u8; 4], &sealed), None);
    }

    #[tokio::test]
    async fn race_returns_honest_terminal_when_all_fail() {
        let candidates = vec![
            Candidate { key: key(1), wg: [1; 32], mesh_port: 1, endpoint: Some("a".into()), intro: None },
            Candidate { key: key(2), wg: [2; 32], mesh_port: 2, endpoint: None, intro: None },
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

    fn direct_candidate(endpoint: &str, intro: Option<&str>) -> Candidate {
        Candidate {
            key: key(1),
            wg: [1; 32],
            mesh_port: 52200,
            endpoint: Some(endpoint.into()),
            intro: intro.map(str::to_string),
        }
    }

    #[test]
    fn intro_dest_defaults_to_underlay_port_plus_one_when_unadvertised() {
        // no advertised intro (a front, or a default-configured inviter) ⇒ the
        // product-wide `wg_port + 1` default, on the endpoint's own host.
        let candidate = direct_candidate("198.51.100.7:51820", None);
        let endpoint_addr = "198.51.100.7:51820".parse().unwrap();
        let dest = resolve_intro_dest(&candidate, endpoint_addr).unwrap();
        assert_eq!(dest, "198.51.100.7:51821".parse().unwrap());
    }

    #[test]
    fn intro_dest_honors_the_advertised_intro_verbatim() {
        // an operator-overridden `invite_listen` advertises an intro on a port
        // that is NOT wg_port + 1 (and even a different host): the joiner must
        // announce there verbatim, not re-derive wg_port + 1. This is the #260
        // regression the fix closes.
        let candidate = direct_candidate("198.51.100.7:51820", Some("203.0.113.9:7000"));
        let endpoint_addr = "198.51.100.7:51820".parse().unwrap();
        let dest = resolve_intro_dest(&candidate, endpoint_addr).unwrap();
        assert_eq!(
            dest,
            "203.0.113.9:7000".parse().unwrap(),
            "the advertised intro endpoint is used verbatim, never wg_port + 1"
        );
    }

    #[test]
    fn build_candidates_carries_the_inviters_advertised_intro() {
        let mut inv = inviter(Some("198.51.100.1:51820"));
        inv.intro = Some("198.51.100.1:7000".into());
        let candidates = build_candidates(Some(inv), &[front(2, Some("198.51.100.2:51820"))]);
        assert_eq!(candidates[0].intro.as_deref(), Some("198.51.100.1:7000"));
        // fronts advertise no separate intro listener.
        assert_eq!(candidates[1].intro, None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_mute_plane_cannot_hang_the_race_past_its_window() {
        // The field failure: the reachability plane ACCEPTS commands but never
        // answers them (its command loop never came up). The parked oneshot
        // reply senders stay alive, so per-attempt awaits never resolve — the
        // window must still terminate the race honestly instead of letting the
        // joiner sit dark forever with zero log output.
        let (reach_tx, mut reach_rx) =
            tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
        let parked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = parked.clone();
        tokio::spawn(async move {
            while let Some(cmd) = reach_rx.recv().await {
                sink.lock().expect("collector mutex").push(cmd);
            }
        });

        let candidates = vec![Candidate {
            key: key(9),
            wg: [9; 32],
            mesh_port: 1,
            endpoint: None, // coordinated: the path that awaits a plane reply
            intro: None,
        }];
        let window = Duration::from_secs(90);
        let dir = tempfile::tempdir().unwrap();
        let keypair = Arc::new(
            reachability::WireGuardKeypair::load_or_generate(&dir.path().join("j.key"))
                .unwrap()
                .0,
        );
        // Paused clock: with the window enforced this resolves at t=90s of
        // virtual time (milliseconds of wall time); without it nothing ever
        // wakes and the guard timeout below trips.
        let outcome = tokio::time::timeout(
            Duration::from_secs(600),
            drive_first_contact(
                reach_tx,
                candidates,
                vec![1, 2, 3],
                vec![9, 9],
                keypair,
                "mute-plane".into(),
                window,
            ),
        )
        .await
        .expect("the race must resolve within its window even when the plane never answers");
        assert!(
            matches!(outcome, FirstContactOutcome::Terminal { tried: 1, .. }),
            "a mute plane is an exhausted path, never a hang: {outcome:?}"
        );
    }

    #[tokio::test]
    async fn empty_candidate_set_is_terminal_not_a_hang() {
        // a lone coordinated candidate filtered out by TUN leaves nothing to
        // race — an immediate honest terminal, never a hang.
        let planned = plan_race(build_candidates(Some(inviter(None)), &[]), true);
        assert!(planned.is_empty());
        let outcome = race_first_contact(planned, |_c| async move {
            AttemptResult::Admitted {
                height: 1,
                cap: None,
            }
        })
        .await;
        assert!(matches!(
            outcome,
            FirstContactOutcome::Terminal { tried: 0, .. }
        ));
    }
}
