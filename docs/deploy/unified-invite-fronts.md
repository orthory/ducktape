# Unified invite: member fronts

`ducktape-node invite` now bundles **fronts** — the inviter's reachable
members — alongside the inviter's own WireGuard bootstrap. A joiner races
first contact across the whole union (`{inviter} ∪ {fronts}`), so a fully
NAT'd inviter is no longer a single point of failure: the tunnel can come up
against any offered member.

## What a front carries

Each front is minted from the inviter's persisted mesh state
(`<storage>/mesh-state.json`, itself a set of member-signed, re-verified
adverts) and carries only:

- `member_key` — the member's real ed25519 node identity (the joiner
  authenticates this end-to-end);
- `wireguard_public_key` — the member's **public** X25519 key;
- `mesh_port` — the member's overlay control port;
- `endpoint` — the member's routable WireGuard underlay endpoint
  (`host:wg_port`) when it is host-capable, else `None` (a punchable, NAT'd
  member reached by identity through the joiner's coordinator).

No WireGuard **private** key is ever transported. No coordinator address is
embedded anywhere in the invite — the joiner uses its **own ambient
coordinator** (`primary_coordinator_or_default`, the public product default),
never one baked into the blob.

## Exposure to weigh before minting

An invite blob now names the inviter's reachable members: their node
identities, WireGuard public keys, overlay ports, and — for host-capable
members — their public underlay endpoints. This is the same class of data
those members already advertise inside the signed mesh, but a **leaked invite
widens who sees it** from admitted members to whoever holds the blob. Treat
invites as single-use, short-TTL secrets (they already are — the token is
single-use and the default TTL is 7 days). Fronts stay **outside** the genesis
fingerprint, so they never affect consensus identity or which network a blob
admits to.

## When there are no fronts

A first-boot inviter with no persisted mesh (or with mesh state that holds only
itself) mints an invite with **no fronts** and prints a warning; the join still
works over the inviter's own paths. Re-mint once the mesh has peers to widen
the joiner's first-contact race.
