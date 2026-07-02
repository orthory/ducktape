# WireGuard Tunnel Upgrade Protocol

Status: security contract for the validator-mesh epic. The current Rust PR set
only defines mesh membership and state-sync frames; it does not yet configure
WireGuard devices or perform this handshake.

## Goals

- Upgrade an already-known validator peer relationship into a WireGuard data
  tunnel without introducing a permanent external relay or central controller.
- Derive every control participant, bootnode, and relay candidate from the
  consensus validator set.
- Bind every tunnel decision to a valset epoch, mesh version, validator
  identity, endpoint advertisement, and port policy.
- Fail closed when endpoint, port, identity, epoch, signature, or replay checks
  cannot be verified.

## Non-Goals

- No unauthenticated public discovery.
- No wildcard listening address as an advertised endpoint.
- No arbitrary port acceptance from peer-supplied strings.
- No state-sync authorization based only on possession of a tunnel.
- No external relay that is not also in the validator set for the same epoch.

## Trust Anchors

The trust anchor is the finalized valset state, not a node-local config file.
For epoch `E`, a node accepts a mesh view only when:

1. The validator identity is present in the finalized valset for `E`.
2. The mesh view version is derived from the sorted valset identities plus their
   signed endpoint advertisements.
3. The advertisement signature verifies under the validator identity.
4. The advertisement is still within its expiry view.
5. Its endpoints satisfy the local port policy.

Node-local config may restrict the policy further, but it must not add
validators or relays outside the finalized valset.

## Endpoint Model

Each validator publishes a signed endpoint advertisement:

```text
EndpointAdvertisementV1 {
  domain = "ducktape:wireguard-endpoint:v1"
  namespace
  epoch
  valset_root
  mesh_version
  validator_identity_ed25519
  control_endpoint
  wireguard_endpoint
  capabilities
  expires_at_view
  nonce
  signature_ed25519
}
```

Endpoint fields are typed before verification:

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
- port `0`;
- ports outside the active allowlist;
- mismatched transport, for example WireGuard over TCP;
- duplicate endpoint advertisements for one validator and epoch unless the
  replacement has a higher monotonic nonce and is signed by the same validator.

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
  mesh_version
  responder_identity_ed25519
  initiator_identity_ed25519
  responder_wireguard_public_key_x25519
  responder_wireguard_endpoint
  accepted_allowed_ips
  relay_candidates
  keepalive_seconds
  expires_at_view
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
  mesh_version
  initiator_identity_ed25519
  responder_identity_ed25519
  installed_at_view
  nonce
  signature_ed25519
}
```

A node installs WireGuard peer config only after all checks pass:

1. Both identities are validators in the same finalized valset epoch.
2. Both endpoint advertisements are valid for the same mesh version.
3. Request and response signatures verify.
4. The responder echoes the request hash.
5. Both WireGuard public keys are well-formed X25519 public keys.
6. Both endpoints satisfy local port policy.
7. The message has not expired.
8. `(sender_identity, epoch, nonce)` has not been seen before.
9. The requested allowed IPs are within the deterministic overlay assignment for
   those validator identities.

## Overlay Addressing

Overlay IPs are derived from `(namespace, epoch, validator_identity)` and the
validator's stable mesh index. A peer cannot request arbitrary routes. The
implementation must reject:

- default routes such as `0.0.0.0/0` and `::/0`;
- host routes for another validator's overlay address;
- routes outside the mesh overlay CIDR;
- overlapping routes that would steal traffic from another peer.

## Relay Fallback

Relay candidates are the validators whose mesh view capability includes
`relay`. A relay forwards encrypted tunnel traffic or control messages only; it
must not terminate the WireGuard session or decrypt state-sync payloads.

Relay selection rules:

- Candidate must be in the same epoch and mesh version.
- Candidate must satisfy the same endpoint and port policy checks.
- Candidate must not be the only path unless direct dialing failed with a
  recorded dial error.
- External permanent relays are rejected by protocol. A deployment can add a
  relay only by adding it to the validator set for that epoch.

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
  transport, and disallowed ports.
- Signed endpoint advertisement fails for wrong epoch, wrong mesh version,
  unknown validator, duplicate nonce, and expired view.
- Upgrade request/response fail when request hash, policy hash, valset root, or
  identities do not match.
- Overlay route validation rejects default routes, stolen peer routes, and
  routes outside the mesh CIDR.
- Relay fallback uses only validator-set relay candidates and keeps payloads
  encrypted end-to-end.
- Epoch cutover removes departed validators and rotates or revalidates retained
  validator sessions.
