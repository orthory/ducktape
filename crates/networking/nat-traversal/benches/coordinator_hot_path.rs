use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, Verifier as _, ed25519};
use nat_traversal::{AuthPolicy, AuthRequest, Coordinator, Msg, NodeKey, sign_authenticator};

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: this allocator only counts before delegating the unchanged
        // allocation request to the process-wide system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` above with this same layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn addr(last: u8, port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, last)), port)
}

fn node_key(signer: &ed25519::PrivateKey) -> NodeKey {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(signer.public_key().as_ref());
    NodeKey(bytes)
}

fn auth_request(
    signer: &ed25519::PrivateKey,
    caller: NodeKey,
    inner: Msg,
    now: u64,
) -> AuthRequest {
    let auth = sign_authenticator(signer, &inner.encode(), now, None);
    AuthRequest {
        caller,
        inner,
        auth,
    }
}

fn bench(name: &str, iterations: u64, mut operation: impl FnMut()) {
    for _ in 0..iterations.min(1_000) {
        operation();
    }

    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    let elapsed = started.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let nanos = elapsed.as_nanos() as f64 / iterations as f64;
    let ops_per_second = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "{name:24} {nanos:10.1} ns/op  {ops_per_second:12.0} ops/s  {:5.2} allocs/op  {:7.1} B/op",
        allocations as f64 / iterations as f64,
        allocated_bytes as f64 / iterations as f64,
    );
}

fn main() {
    println!(
        "sizes: Coordinator={} B, PublicKey={} B",
        std::mem::size_of::<Coordinator>(),
        std::mem::size_of::<ed25519::PublicKey>(),
    );
    const NOW: u64 = 1_000_000;
    let source = addr(1, 40_001);
    let peer_source = addr(2, 40_002);

    let signer = ed25519::PrivateKey::from_seed(7);
    let caller = node_key(&signer);
    let peer_signer = ed25519::PrivateKey::from_seed(8);
    let peer = node_key(&peer_signer);
    bench("public key decode", 100_000, || {
        black_box(
            ed25519::PublicKey::decode(black_box(caller.0.as_slice()))
                .expect("benchmark key decodes"),
        );
    });
    let public = signer.public_key();
    let message = b"coordinator benchmark message";
    let signature = signer.sign(b"benchmark", message);
    bench("signature verify", 20_000, || {
        black_box(public.verify(b"benchmark", black_box(message), &signature));
    });
    let bind = auth_request(&signer, caller, Msg::BindRequest { from: caller }, NOW);
    let mut authenticated = Coordinator::with_policy(AuthPolicy::Public);
    bench("authenticated bind", 20_000, || {
        black_box(authenticated.handle_auth_replies(source, bind.clone(), NOW));
    });

    let peer_register = auth_request(
        &peer_signer,
        peer,
        Msg::Register {
            key: peer,
            // No live mapping exists yet, so the cookie would need to verify
            // for it to actually admit the key — irrelevant here, this bench
            // only times `handle_auth_replies`'s dispatch cost either way.
            cookie: [0u8; 32],
        },
        NOW,
    );
    authenticated.handle_auth_replies(peer_source, peer_register.clone(), NOW);
    let lookup = auth_request(&signer, caller, Msg::Lookup { key: peer }, NOW);
    bench("authenticated lookup", 20_000, || {
        black_box(authenticated.handle_auth_replies(source, lookup.clone(), NOW));
    });

    let encoded = lookup.encode();
    let mut pipeline = Coordinator::with_policy(AuthPolicy::Public);
    pipeline.handle_auth_replies(peer_source, peer_register, NOW);
    bench("decode + lookup + encode", 20_000, || {
        let request = AuthRequest::decode(black_box(&encoded)).expect("benchmark request decodes");
        for (_, reply) in pipeline.handle_auth_replies(source, request, NOW) {
            black_box(reply.encode_inline());
        }
    });
}
