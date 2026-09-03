# WireGuard Tunnel Upgrade Protocol

The protocol that upgrades an already-known member relationship into a
WireGuard data tunnel. `crates/networking/wireguard` owns it: the crate root
verifies endpoint advertisements, mesh versions, port policy, the signed
upgrade request/response/ack messages, replay nonces, allowed IPs and ack
freshness, and a successful validation emits a tunnel install plan that
`wireguard::effect` reduces to one `InterfaceConfig` (own key, listen port,
overlay addresses, every peer relationship) and pushes through a
`WireGuardEffect` (a deterministic fake in tests, the in-process userspace
backend in real runs). How the node drives it — gossip, invites, cold
restart, rendezvous — is `../architecture/reachability.md`.

## Goals

- Upgrade an already-known validator peer relationship into a WireGuard data
  tunnel without introducing any relay or central controller.
- Derive every control participant and bootnode from the consensus validator
  set.
- Bind every tunnel decision to a valset epoch, admission root, mesh version,
  validator identity, endpoint advertisement, and port policy.
- Fail closed when endpoint, port, identity, epoch, signature, or replay checks
  cannot be verified.

## Non-Goals

- No unauthenticated public discovery.
- No wildcard listening address as an advertised endpoint.
- No arbitrary port acceptance from peer-supplied strings.
- No state-sync authorization based only on possession of a tunnel.
- No relay tier of any kind: the coordinator is a STUN-style rendezvous only
  and never carries tunnel traffic; a failed direct dial is terminal, never
  rerouted through a third party, and no dial-failure evidence exists in the
  protocol.

## Backend

The implementation runs WireGuard in-process on the BoringTun noise core
(`crates/networking/overlay-net/src/userspace`), with no kernel interface, TUN
device, or `wg` binary involved. The protocol crate itself is pure: it returns
the typed `InterfaceConfig` after the validator mesh checks pass, and the
backend consumes that directly.

## Trust Anchors

The trust anchor is the finalized active consensus validator set, not a
node-local config file and not the permissionless valset module by itself.
`ValsetMsg::Join` accepts any well-formed Ed25519 key as application state;
that is candidate membership only. A key from that module does not become a
WireGuard peer or bootnode until the consensus/admission layer includes it in
the finalized active validator set for the epoch.

The protocol carries two distinct commitments:

- `valset_root`: the replicated candidate-membership state root.
- `admission_root`: the invitation/admission system's finalized commitment for
  which candidate validators are actually admitted for the epoch.

`admission_root` is mandatory and must not be zero. It is included in endpoint
records, mesh-version preimages, and request/response/ack messages. A node
rejects a tunnel upgrade if it only has candidate valset membership but no
matching finalized admission root.

For epoch `E`, a node accepts a mesh view only when:

1. The validator identity is admitted in the finalized active consensus set for
   `E` under the finalized admission root.
2. The mesh view version is derived from the canonical mesh-version preimage for
   that admitted set and its endpoint records.
3. The advertisement signature verifies under the admitted validator identity.
4. Its endpoints satisfy the local port policy.

Node-local config may restrict the policy further, but it never adds
validators outside the finalized active consensus set.

## Endpoint Model

Each member publishes a signed endpoint record. The record is one struct
(`EndpointRecord`) that travels in two signed envelopes with distinct domains,
so the two blobs can never cross-verify:

```text
EndpointRecord {
  namespace                      (the chain id)
  epoch
  valset_root
  admission_root
  validator_identity             (ed25519)
  wireguard_public_key           (x25519)
  control_endpoint
  wireguard_endpoint             (optional)
  nonce
}

EndpointAdvertisement            domain "ducktape:wireguard-endpoint:v1"
  { record, mesh_version, signature }

SignedEndpointRecord             domain "ducktape:wireguard-endpoint-record:v1"
  { record, signature }
```

The advertisement is the epoch-locked form: it commits to the mesh version
the record was assembled under. The self-signed record is the gossip form,
for paths where the transport authenticates the forwarder rather than the
record's owner (live re-advertisement, standby pre-warm, the persisted mesh).

The member's WireGuard key lives in the signed record — never in consensus
state — so a key rotation is a re-advertisement (a new mesh version), and the
handshake below pins its session keys to these records. A record is signed
once per epoch and re-offered verbatim; its lifetime is the epoch tuple
itself. Only handshake messages carry an expiry.

`wireguard_endpoint` is optional: a NAT'd member with no dialable underlay
address advertises `None`. Peers install its tunnel without an endpoint and
wait — the endpoint-less side holds every peer's endpoint from these records,
so it initiates, and WireGuard's roaming pins the observed source.

Endpoint fields are typed before verification. Endpoints use canonical IP
literals only; DNS names are rejected. IPv4-mapped IPv6 literals are
normalized before policy checks. The `host` field must not contain a port or
zone identifier.

```text
Endpoint { host, port, transport = "tcp" | "udp" }
```

The parser rejects:

- empty host;
- unspecified hosts such as `0.0.0.0` or `::` as advertised endpoints;
- loopback hosts outside an explicit local-dev policy;
- private, link-local, multicast, broadcast, documentation, or otherwise
  non-global addresses unless the active policy explicitly allows that address
  class;
- DNS names, non-canonical IP literals, IPv6 zone identifiers, and endpoint
  strings with embedded ports;
- port `0`;
- ports outside the active allowlist;
- mismatched transport, for example WireGuard over TCP;
- duplicate endpoint records for one validator and epoch unless the
  replacement has a higher monotonic nonce and is signed by the same validator.

## Mesh Version

`mesh_version` is not self-referential. It is computed before advertisement
signing from the record preimage, which excludes `mesh_version` and the
signature:

```text
mesh_version =
  HASH("ducktape:validator-mesh-version:v1" ||
       namespace ||
       epoch ||
       valset_root ||
       admission_root ||
       SORT_ASC(endpoint_record_hashes))
```

Each record hash covers `wireguard_public_key`, so the mesh version commits
to the WireGuard key set and rotating any member's key produces a new mesh
version. The advertisement signs the full `EndpointAdvertisement`, including
the computed `mesh_version`, but the signature is not part of the mesh-version
preimage. Implementations must ship fixed test vectors for this preimage so
that independent nodes produce the same mesh version from the same admitted
set; `crates/networking/wireguard/tests/tunnel_e2e.rs` pins it.

## Port Policy

Port policy is explicit and reject-by-default.

```text
PortPolicy {
  name
  allowed_control_tcp_ports
  allowed_wireguard_udp_ports
  allow_loopback
  allow_private_ip
}
```

`PortPolicy::production()` is `{443}` for control TCP, `{51820}` for
WireGuard UDP, no loopback, no private IPs; a deployment on an explicitly
private network sets `allow_private_ip`. The policy hash is included in the
signed upgrade request, and a peer rejects an upgrade when its local policy
hash differs from the hash in the signed message.

## Upgrade Handshake

The upgrade has three signed messages. All signed bytes use a canonical
length-prefixed encoding; JSON is never signed protocol bytes.

```text
TunnelUpgradeRequest     domain "ducktape:wireguard-upgrade-request:v1"
  namespace, epoch, valset_root, admission_root, mesh_version,
  initiator_identity, responder_identity,
  initiator_wireguard_public_key, initiator_wireguard_endpoint (optional),
  requested_allowed_ips, port_policy_hash, expires_at_view, nonce, signature

TunnelUpgradeResponse    domain "ducktape:wireguard-upgrade-response:v1"
  request_hash, namespace, epoch, valset_root, admission_root, mesh_version,
  responder_identity, initiator_identity,
  responder_wireguard_public_key, responder_wireguard_endpoint (optional),
  accepted_allowed_ips, keepalive_seconds (optional), expires_at_view, nonce,
  signature

TunnelUpgradeAck         domain "ducktape:wireguard-upgrade-ack:v1"
  request_hash, response_hash, namespace, epoch, valset_root, admission_root,
  mesh_version, initiator_identity, responder_identity,
  installed_at_view, expires_at_view, nonce, signature
```

A node installs WireGuard peer config only after all checks pass:

1. Both identities are admitted validators in the same finalized active
   consensus epoch.
2. Both endpoint records are valid for the same mesh version.
3. Request, response, and ack signatures verify.
4. The responder echoes the request hash.
5. The ack echoes both request and response hashes.
6. All three messages carry the same namespace, epoch, valset root,
   admission root, and mesh version.
7. Both WireGuard public keys are well-formed X25519 public keys and equal the
   keys in the initiator's and responder's mesh-view records — a party cannot
   complete a handshake under a fresh key the mesh never versioned.
8. Each message's WireGuard endpoint equals the sender's mesh-view record
   (an endpoint-less record stays endpoint-less in the handshake), and any
   present endpoint satisfies local port policy.
9. No request, response, or ack message has expired.
10. `(sender_identity, epoch, nonce)` has not been seen before for every signed
    message, including duplicate nonces inside the same validation call.
11. Both requested and accepted allowed IPs are within the deterministic overlay
    assignment for those validator identities.
12. The ack's `installed_at_view` is within a symmetric lag window
    (`MAX_ACK_INSTALL_LAG`, 8 views) of the local view — the two ends run
    independent view clocks, so a zero-tolerance future check would fail
    genuine cross-node pairs.

Both sides validate the identical signed triple, each from its own mesh view
and replay cache, and each derives the install plan for its own perspective
(initiator or responder) from that one validation call.

## Overlay Addressing

Overlay addressing is deterministic ULA v6. `OverlayPolicy` carries only the
`chain_id` (which must equal the mesh's advertisement namespace); a peer
cannot request arbitrary routes.

The mesh owns the /48
`fd || first 40 bits of HASH("ducktape:overlay-ula:v1" || chain_id)`, and
each member's /128 host is
`first 80 bits of HASH("ducktape:overlay-addr:v1" || chain_id || identity)`.
An address is a function of `(chain_id, identity)` only — no allocator, no
index, stable across churn — and fd00::/8 cannot collide with RFC1918 v4 or
the 100.64.0.0/10 CGNAT block a resident Tailscale occupies, which is what
lets a dedicated `dt-*` interface coexist with a personal tailnet.

`requested_allowed_ips` is the initiator's proposed route set for the responder
identity. `accepted_allowed_ips` is the responder's route set for the initiator
identity. Each field must equal the canonical route set for that remote identity
or a strict subset explicitly allowed by the mesh policy; supernets and routes
for any other validator are forbidden. The implementation rejects either field
when it contains:

- default routes such as `0.0.0.0/0` and `::/0`;
- host routes for another validator's overlay address;
- routes outside the mesh overlay CIDR;
- overlapping routes that would steal traffic from another peer;
- duplicate entries — the signed preimage sorts and dedups routes, so an
  unchecked vector could repeat one canonical route many times and inflate a
  peer's installed allowed-ips into a memory DoS.

## Replay, Downgrade, and Cutover

- Every signed message includes a domain string, namespace, epoch, valset root,
  and nonce; advertisements add the mesh version, and handshake messages add
  the mesh version and an expiry view (records carry no expiry — their
  lifetime is the epoch tuple).
- Nodes store a bounded replay cache keyed by `(identity, epoch, nonce)` until
  the epoch expires.
- A valset cutover revokes tunnels for validators not present in the new epoch.
- A validator that remains in the set must rotate its WireGuard session key at
  epoch cutover or prove the previous key is still authorized by a fresh signed
  advertisement for the new mesh version.
- A cutover reconfigures the live interface in place rather than rebuilding
  it: a tunnel whose configuration is unchanged keeps its WireGuard sessions
  across the boundary, and a membership change elsewhere in the set never
  drops an established pair.

## Live Re-advertisement and Mid-Epoch Rejoin

The record set a mesh version locks is immutable for the epoch, but members
are not: a member can restart or rebind its address mid-epoch. Two mechanisms
cover this without ever recomputing a locked version:

- **Live re-advertisement.** A `SignedEndpointRecord` whose nonce is above
  the one the epoch locked for its owner is a fresh life, not a stale
  duplicate. Receivers accept it under the same higher-nonce supersession
  rule as every other gossip item, re-point the owner's tunnel in place as a
  layer over the applied base, and flood it onward (accept-gated). A record
  nonce at or below the locked one instead marks the owner as behind in
  assembly and triggers the heal-back of the receiver's own record and
  advertisement. Accepted re-advertisements persist with the mesh, so a cold
  restart restores each member's current life rather than the one the epoch
  happened to lock.
- **Adoption of the peers' locked view.** A member that re-assembles
  mid-epoch (a restart) signs a fresh record, so the set it can lock never
  hashes to the version its peers locked. When every peer's advertisement
  commits to one identical mesh version that differs from the local
  computation, the node adopts the peers' lock outright: their owner-signed,
  epoch-bound records install as the applied base with freshly resolved
  endpoints (exactly like the cold-restart restore), and the node keeps
  re-offering its own fresh record until every peer re-tunnels it through
  the live re-advertisement path. Without a unanimous peer version — several
  nodes re-assembling at once — the epoch fails and the next cutover
  reassembles from scratch.

## State-Sync Authorization

The WireGuard tunnel is only a transport. State-sync authorization still checks
the state-sync request:

- source is a mesh participant allowed to serve;
- module id is served by that source;
- requested root equals the finalized module root;
- payload kind matches the module sync surface;
- QMDB resolver targets are verified by their own root and content checks.

Possession of a tunnel never bypasses module/root/kind checks.

## Pinned by tests

The crate's tests hold every rule above; the load-bearing ones:

- the endpoint parser rejects wildcard, loopback under the production policy,
  port zero, wrong transport, disallowed ports, DNS names, IPv4-mapped
  loopback/private forms, link-local, multicast and embedded-port hosts;
- a signed endpoint advertisement fails for a wrong epoch, a wrong mesh
  version, an unknown admitted validator, candidate-only valset membership,
  and a duplicate nonce;
- the mesh-version fixed vector (`tests/tunnel_e2e.rs`) covers identical
  inputs, endpoint changes, valset changes, and the signature's exclusion
  from the preimage;
- request/response/ack fail when the request hash, response hash, policy
  hash, valset root, expiry, or identities do not match;
- a member record whose nonce beats the epoch's locked record re-tunnels its
  owner in place; a nonce at or below the locked one heals the owner instead;
- overlay route validation rejects default routes, stolen peer routes, and
  routes outside the mesh CIDR for both requested and accepted allowed IPs;
- an epoch cutover removes departed validators and rotates or revalidates
  retained validator sessions.
