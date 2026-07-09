# WireGuard Tunnel Upgrade Protocol

Status: implemented protocol boundary for the validator-mesh epic. The
`wireguard-upgrade` crate verifies endpoint advertisements, mesh versions,
port policy, signed upgrade request/response/ack messages, replay nonces,
allowed IPs, relay candidates, signed direct-dial failure evidence, and ACK
freshness. A successful validation emits a tunnel install plan that is converted into `defguard_wireguard_rs`
`Peer`/`InterfaceConfiguration` values for the effectful node layer to apply
through `WGApi`.

## Goals

- Upgrade an already-known validator peer relationship into a WireGuard data
  tunnel without introducing a permanent external relay or central controller.
- Derive every control participant, bootnode, and relay candidate from the
  consensus validator set.
- Bind every tunnel decision to a valset epoch, invitation/admission root, mesh
  version, validator identity, endpoint advertisement, and port policy.
- Fail closed when endpoint, port, identity, epoch, signature, or replay checks
  cannot be verified.

## Non-Goals

- No unauthenticated public discovery.
- No wildcard listening address as an advertised endpoint.
- No arbitrary port acceptance from peer-supplied strings.
- No state-sync authorization based only on possession of a tunnel.
- No external relay that is not also in the validator set for the same epoch.

## Rust Backend Choice

The implementation uses `defguard_wireguard_rs`, not the historical
`WireGuard/wireguard-rs` reference repository. The reason is operational:
Defguard exposes a maintained high-level Rust API over native/kernel and
userspace WireGuard implementations. The protocol crate does not shell out to
`wg`; it returns typed Defguard peer/interface configuration after the validator
mesh checks pass.

## Trust Anchors

The trust anchor is the finalized active consensus validator set, not a
node-local config file and not the permissionless valset module by itself. The
current `ValsetMsg::Join` surface accepts any well-formed Ed25519 key as
application state; that is candidate membership only. A key from that module
does not become a WireGuard peer, bootnode, or relay until the consensus/admission
layer includes it in the finalized active validator set for the epoch.

The protocol carries two distinct commitments:

- `valset_root`: the replicated candidate-membership state root.
- `admission_root`: the invitation/admission system's finalized commitment for
  which candidate validators are actually admitted for the epoch.

`admission_root` is mandatory and must not be zero. It is included in endpoint
advertisement records, mesh-version preimages, request/response/ack messages,
and direct-dial failure evidence. A node must reject a tunnel upgrade if it only
has candidate valset membership but no matching finalized admission root.

For epoch `E`, a node accepts a mesh view only when:

1. The validator identity is admitted in the finalized active consensus set for
   `E` under the signed/finalized admission root.
2. The mesh view version is derived from the canonical mesh-version preimage for
   that admitted set and its endpoint records.
3. The advertisement signature verifies under the admitted validator identity.
4. The advertisement is still within its expiry view.
5. Its endpoints satisfy the local port policy.

Node-local config may restrict the policy further, but it must not add
validators or relays outside the finalized active consensus set. Until the
admission layer exists in code, a WireGuard implementation must fail closed and
must not treat permissionless `ValsetMsg::Join` membership as tunnel authority.

## Endpoint Model

Each validator publishes a signed endpoint advertisement:

```text
EndpointAdvertisementV2 {
  domain = "ducktape:wireguard-endpoint:v2"
  namespace
  epoch
  valset_root
  admission_root
  mesh_version
  validator_identity_ed25519
  wireguard_public_key_x25519
  control_endpoint
  wireguard_endpoint
  capabilities
  expires_at_view
  nonce
  signature_ed25519
}
```

V2 adds `wireguard_public_key_x25519`: the validator's WireGuard key lives in
the signed, mesh-versioned advertisement — never in consensus state — so a key
rotation is a re-advertisement (a new mesh version), and the tunnel handshake
below pins its session keys to these records. The layout change bumps the
signature domain from v1; v1 and v2 blobs can never cross-verify.

Endpoint fields are typed before verification. Protocol v1 endpoints use
canonical IP literals only; DNS names are rejected. IPv4-mapped IPv6 literals
must be normalized before policy checks. The `host` field must not contain a
port or zone identifier.

```text
ControlEndpoint {
  host
  port
  transport = "tcp"
}

WireGuardEndpoint {
  host
  port
  transport = "udp"
}
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
- duplicate endpoint advertisements for one validator and epoch unless the
  replacement has a higher monotonic nonce and is signed by the same validator.

Future protocol versions may allow DNS only if resolution is authenticated,
pinned for the dial attempt, and rechecked immediately before use. DNS rebinding
between verification and dial is a hard failure.

## Mesh Version

`mesh_version` is not self-referential. It is computed before advertisement
signing from a preimage that excludes `mesh_version` and `signature_ed25519`:

```text
EndpointRecordV2 {
  namespace
  epoch
  valset_root
  admission_root
  validator_identity_ed25519
  wireguard_public_key_x25519
  control_endpoint
  wireguard_endpoint
  capabilities
  expires_at_view
  nonce
}

mesh_version =
  HASH("ducktape:validator-mesh-version:v2" ||
       namespace ||
       epoch ||
       valset_root ||
       admission_root ||
       SORT_ASC(endpoint_record_hashes))
```

The v2 preimage differs from v1 only in each record hash covering
`wireguard_public_key_x25519` (and the bumped domain string): the mesh version
commits to the WireGuard key set, so rotating any validator's key produces a
new mesh version.

The endpoint advertisement signs the full `EndpointAdvertisementV1`, including
the computed `mesh_version`, but the signature is not part of the mesh-version
preimage. Implementations must ship fixed test vectors for this preimage so that
independent nodes produce the same mesh version from the same admitted set.

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

Recommended profiles:

```text
production:
  allowed_control_tcp_ports = {443}
  allowed_wireguard_udp_ports = {51820}
  allow_loopback = false
  allow_private_ip = false unless the deployment network is explicitly private

local-dev:
  allowed_control_tcp_ports = {7000, 7001, 7002, 7003, 7004}
  allowed_wireguard_udp_ports = {51820, 51821, 51822, 51823, 51824}
  allow_loopback = true
  allow_private_ip = true
```

The policy hash is included in upgrade messages. A peer must reject an upgrade
when its local policy hash differs from the policy named in the signed message.

## Upgrade Handshake

The upgrade has three signed messages. All signed bytes use a canonical
length-prefixed encoding; JSON is not acceptable for signed protocol bytes.

```text
TunnelUpgradeRequestV1 {
  domain = "ducktape:wireguard-upgrade-request:v1"
  namespace
  epoch
  valset_root
  admission_root
  mesh_version
  initiator_identity_ed25519
  responder_identity_ed25519
  initiator_wireguard_public_key_x25519
  initiator_wireguard_endpoint
  requested_allowed_ips
  port_policy_name
  port_policy_hash
  expires_at_view
  nonce
  signature_ed25519
}
```

```text
TunnelUpgradeResponseV1 {
  domain = "ducktape:wireguard-upgrade-response:v1"
  request_hash
  namespace
  epoch
  valset_root
  admission_root
  mesh_version
  responder_identity_ed25519
  initiator_identity_ed25519
  responder_wireguard_public_key_x25519
  responder_wireguard_endpoint
  accepted_allowed_ips
  relay_candidates
  direct_dial_failure
  keepalive_seconds
  expires_at_view
  nonce
  signature_ed25519
}
```

```text
DirectDialFailureEvidenceV1 {
  domain = "ducktape:wireguard-direct-dial-failure:v1"
  namespace
  epoch
  valset_root
  admission_root
  mesh_version
  observer_identity_ed25519
  target_identity_ed25519
  target_wireguard_endpoint
  failed_at_view
  expires_at_view
  error_hash
  nonce
  signature_ed25519
}
```

```text
TunnelUpgradeAckV1 {
  domain = "ducktape:wireguard-upgrade-ack:v1"
  request_hash
  response_hash
  namespace
  epoch
  valset_root
  admission_root
  mesh_version
  initiator_identity_ed25519
  responder_identity_ed25519
  installed_at_view
  expires_at_view
  nonce
  signature_ed25519
}
```

A node installs WireGuard peer config only after all checks pass:

1. Both identities are admitted validators in the same finalized active
   consensus epoch.
2. Both endpoint advertisements are valid for the same mesh version.
3. Request, response, and ack signatures verify.
4. The responder echoes the request hash.
5. The ack echoes both request and response hashes.
6. All three messages carry the same namespace, epoch, valset root,
   admission root, and mesh version.
7. Both WireGuard public keys are well-formed X25519 public keys.
8. Both endpoints satisfy local port policy.
9. No request, response, or ack message has expired.
10. `(sender_identity, epoch, nonce)` has not been seen before for every signed
    message, including duplicate nonces inside the same validation call.
11. Both requested and accepted allowed IPs are within the deterministic overlay
    assignment for those validator identities.
12. Relay candidates are empty unless the response includes signed direct-dial
    failure evidence from the initiator for the responder's WireGuard endpoint.

## Overlay Addressing

Two deterministic overlay modes exist; a mesh runs exactly one. A peer cannot
request arbitrary routes in either mode.

- **Indexed v4** (the original scheme): overlay IPs derive from the
  validator's stable mesh index inside a v4 block. An address is a function
  of the sorted set, so it moves on membership churn.
- **ULA v6** (the node-driven WireGuard overlay): the mesh owns the /48
  `fd || first 40 bits of HASH("ducktape:overlay-ula:v1" || chain_id)`, and
  each validator's /128 host is
  `first 80 bits of HASH("ducktape:overlay-addr:v1" || chain_id || identity)`.
  An address is a function of `(chain_id, identity)` only — no allocator, no
  index, stable across churn — and fd00::/8 cannot collide with RFC1918 v4 or
  the 100.64.0.0/10 CGNAT block a resident Tailscale occupies, which is what
  lets a dedicated `dt-*` interface coexist with a personal tailnet.

`requested_allowed_ips` is the initiator's proposed route set for the responder
identity. `accepted_allowed_ips` is the responder's route set for the initiator
identity. Each field must equal the canonical route set for that remote identity
or a strict subset explicitly allowed by the mesh policy; supernets and routes
for any other validator are forbidden. The implementation must reject either
field when it contains:

- default routes such as `0.0.0.0/0` and `::/0`;
- host routes for another validator's overlay address;
- routes outside the mesh overlay CIDR;
- overlapping routes that would steal traffic from another peer.

## Relay Fallback

Relay candidates are admitted validators whose mesh view capability includes
`relay`. A relay forwards encrypted tunnel traffic or control messages only; it
must not terminate the WireGuard session or decrypt state-sync payloads.

Relay selection rules:

- Candidate must be in the same admitted active consensus epoch and mesh
  version.
- Candidate must satisfy the same endpoint and port policy checks.
- Candidate must not be used unless direct dialing failed with a signed,
  unexpired `DirectDialFailureEvidenceV1` bound to the same namespace, epoch,
  valset root, mesh version, observer, target, and target WireGuard endpoint.
- External permanent relays are rejected by protocol. A deployment can add a
  relay only by admitting it to the active consensus validator set for that
  epoch.

## Replay, Downgrade, and Cutover

- Every signed message includes a domain string, namespace, epoch, valset root,
  mesh version, expiry view, and nonce.
- Nodes store a bounded replay cache keyed by `(identity, epoch, nonce)` until
  the epoch expires.
- A node rejects older protocol versions unless explicitly configured for a
  one-epoch migration window.
- A valset cutover revokes tunnels for validators not present in the new epoch.
- A validator that remains in the set must rotate its WireGuard session key at
  epoch cutover or prove the previous key is still authorized by a fresh signed
  advertisement for the new mesh version.

## State-Sync Authorization

The WireGuard tunnel is only a transport. State-sync authorization still checks
the state-sync request:

- source is a mesh participant allowed to serve;
- module id is served by that source;
- requested root equals the finalized module root;
- payload kind matches the module sync surface;
- QMDB resolver targets are verified by their own root and content checks.

Possession of a tunnel never bypasses module/root/kind checks.

## Minimum Tests Before Implementation Is Mergeable

- Endpoint parser rejects wildcard, loopback in production, port zero, wrong
  transport, disallowed ports, DNS names, IPv4-mapped loopback/private forms,
  link-local, multicast, embedded-port hosts, and DNS rebinding if a future
  protocol version enables DNS.
- Signed endpoint advertisement fails for wrong epoch, wrong mesh version,
  unknown admitted validator, candidate-only permissionless valset membership,
  duplicate nonce, and expired view.
- Mesh-version fixed vectors cover identical inputs, endpoint changes, valset
  changes, and signatures excluded from the preimage.
- Upgrade request/response/ack fail when request hash, response hash, policy
  hash, valset root, expiry, or identities do not match.
- Overlay route validation rejects default routes, stolen peer routes, and
  routes outside the mesh CIDR for both requested and accepted allowed IPs.
- Relay fallback uses only admitted validator-set relay candidates and keeps
  payloads encrypted end-to-end.
- Epoch cutover removes departed validators and rotates or revalidates retained
  validator sessions.
