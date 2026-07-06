# Secure Private Team Messaging — Design

Date: 2026-07-06 · Status: proposed design of record for making the chat
surface *secure private team messaging* for members of the same private
network.

The team's messaging already runs on consensus (`crates/apps/chat`) inside a
private network whose transport, identity, and membership story is strong
(`2026-07-05-private-cutover-coordinator-design.md`). What is missing is the
privacy layer on top: today every byte of every channel is plaintext in
replicated state, readable by any member node, the read path is completely
ungated, channel membership and hooks are writable by **any** origin, and
there are no DMs and no channel owners. This design closes that gap in the
repo's established trust idiom (the vaults model: crypto is the read barrier,
consensus is the write-integrity barrier) without inventing new planes.

## What already holds (and this design leans on)

- **Transport privacy.** The control mesh is commonware
  `authenticated::discovery` — ed25519-key-authenticated, end-to-end
  encrypted TCP. The WireGuard data plane binds an X25519 tunnel key to the
  member's ed25519 identity via signed endpoint records
  (`crates/system/wireguard-upgrade/src/lib.rs:277-283`). The coordinator is
  rendezvous-only and never on a data path. **No third-party service ever
  carries or stores message data** — that is the headline "private" property
  and it is already true.
- **Authorship integrity.** On the real node lane every submit is an
  ed25519-signed op frame under the workspace key; a claimed origin cannot
  ride a signed frame (`bin/node/src/main.rs:6929-6945`). Chat derives
  `AuthorRef` from the verified origin, never a payload
  (`crates/apps/chat/src/lib.rs:155-164`). One workspace = one node = one
  member identity.
- **Membership = network membership.** The valset module
  (`crates/system/valset/src/lib.rs`) is the consensus registry of members
  (validators + observers); the WireGuard peer set is derived from it, and
  `profiles` maps member keys to display names. "The members in the same
  private network" is a concrete, consensus-committed set — the team roster.
- **Ordering, durability, late-join.** Consensus total order, per-channel
  gap-free sequences, revision history, and statesync give messaging
  semantics Slack has to fake: a new member's node syncs full history
  and every member sees the same order.

## The gap (confirmed in code)

1. Message bodies are plaintext in qmdb; replicated state is readable by
   every node, and `ChatQuery` serves any channel to any caller.
2. `PostPolicy::MembersOnly` gates **writes only**; there is no read gating
   of any kind.
3. `SetMembership` and `RegisterHook` accept **any non-empty origin**
   (`crates/apps/chat/src/interface.rs:217-233` — "admin gating is future
   work"): anyone can add themselves to any channel or attach a hook.
4. No channel owners, no private channels, no DMs.
5. The legacy `noded` lane stamps an **unauthenticated** origin string.
   Privacy and authorship claims in this design hold only on the signed-frame
   `bin/node` lane, which is what the desktop app runs.

## Threat model

| Actor | Can | Cannot (after this design) |
|-------|-----|----------------------------|
| Outside observer | See ciphertext + coarse traffic timing | Read anything; join; impersonate |
| Coordinator | Rendezvous metadata (who punches whom) | Touch message data at all |
| Network member, not in a private channel | See that the channel exists, its membership, message sizes/timing/reactions (state is replicated) | Read message content (no key) |
| Private-channel member | Read that channel; post as themselves | Forge another author; rewrite history |
| Channel owner | Add/remove members, rotate keys | Read channels they're not in; forge authorship |
| Compromised member node | Everything that member could read (their keys + full plaintext of public channels) | Read private channels the member wasn't wrapped into; retroactively read post-removal messages |

Deliberate non-goals of the threat model: per-message forward secrecy /
post-compromise security (epoch keys, not a ratchet — see "What this does
not provide"), and metadata privacy *inside* the network (membership,
volume, reaction emoji stay visible to member nodes; the team boundary is
the metadata boundary).

## Approaches considered

**A. Governance hardening only (no crypto).** Add owners, gate membership
and hooks, add a `Private` visibility that hides channels from non-member
queries. Cheap, immediately shippable — but "private" channels would be
curtains, not locks: any member node reads the plaintext state directly.
Honest only as a UX tier, not a security tier.

**B. A + encrypted lanes (vaults pattern) — recommended.** Private channels
and DMs store **opaque ciphertext** in consensus state; members encrypt to a
per-channel epoch key wrapped per-recipient with X25519 envelopes. The
on-chain member list is recipient bookkeeping and write-integrity (exactly
`crates/apps/vaults/src/interface.rs`'s documented trust model); the
envelope is the read barrier. Consensus keeps doing what it's good at
(ordering, durability, late-join sync, unforgeable authorship); crypto does
the only thing consensus cannot: confidentiality against replicas.

**C. Off-consensus messaging over the data plane.** `crates/system/data-plane`
is designed for exactly this shape (consensus-derived `AdmissionPolicy`,
default-deny, WireGuard-authenticated peer identity) — but it has **no real
transport arm yet** (sim only), and going off-consensus forfeits ordered
durable history, offline delivery, and new-member sync — the properties that
make team messaging usable. Right substrate later for ephemera (typing,
presence, huddles — voice already lives there); wrong carrier for the
message history itself.

**Decision: B, built as A-then-B slices** (A is a strict subset and ships
first). C is explicitly deferred, not rejected — the `Service` registry keeps
a seam open.

## Design

### 1. Member messaging keys (profiles carries them)

Each workspace gets a **messaging X25519 keypair**, generated by its node on
first use and stored `0600` next to `identity.key` (same pattern as the
WireGuard secret, `crates/system/reachability/src/keys.rs`). The public key
is published through the profiles module:

- `ProfileMsg::SetMessagingKey { x25519_pub: [u8; 32] }` — origin-gated like
  `SetName`, so only the member can publish their own key. The consensus
  authorship of that write **is** the ed25519↔X25519 binding (same trick the
  reachability plane uses for tunnel keys).
- `Profile` gains `messaging_key: Option<Vec<u8>>` (serde-default — old
  records decode).

The profiles registry becomes the crypto roster: whom you *can* wrap to is
exactly who has published a key.

### 2. Channel governance (slice A — no crypto)

The `Channel` record gains, all serde-defaulted for state compatibility:

- `owners: Vec<Vec<u8>>` — creator becomes first owner (vaults pattern).
  New ops `AddOwner`/`RemoveOwner`, owner-gated; the last owner cannot be
  removed.
- `visibility: Public | Private` — set at create, immutable.
- **Owner-gating of the existing un-gated surface:** `SetMembership`,
  `RegisterHook`, `UnregisterHook` (and pinning when it lands) now require an
  owner origin. This closes gap 3 for *all* channels, public included.
- **Read hygiene for private channels:** `Channels` listings omit, and
  channel-scoped queries reject, private channels for non-member callers.
  This is labeled in code and docs as **bookkeeping, not a security
  boundary** — a member node can read its own replica; the envelope (below)
  is the read barrier. It still matters: it keeps honest UIs honest and
  keeps private-channel existence out of casual browsing.

### 3. Encrypted lanes (slice B — private channels and DMs)

**Key model.** Each private channel has a 32-byte symmetric **channel key**
per **epoch**. Epoch 0 is minted at creation; the key is wrapped to each
member with an X25519 sealed envelope (ephemeral ECDH → HKDF-SHA256 →
ChaCha20-Poly1305; `x25519-dalek` and `sha2` are workspace deps already,
`chacha20poly1305`/`hkdf` are standard RustCrypto adds already present in
the dependency graph). Consensus state stores, per `(channel, epoch)`:

```text
ChannelKeyRecord {
  epoch    : u64,
  wrapped  : BTreeMap<member_key_hex, Vec<u8>>,  // sealed envelope per member
}
```

**Message shape.** `PostMessage`/`EditMessage` bodies become a two-arm
concept, encoded state-compatibly (the head keeps `blocks` and gains an
optional `sealed` field, so every pre-upgrade record decodes unchanged;
exactly one of the two is populated):

```text
Body = Plain(Vec<Block>)
     | Sealed { epoch: u64, nonce: [u8; 24], ct: Vec<u8> }
```

`ct` is XChaCha20-Poly1305 over the serialized `Vec<Block>`, with AAD
binding `(channel_id, message_id, epoch, author_key)` — sequence numbers are
assigned at execute time so they cannot be in the AAD; `message_id` plus the
consensus-verified author gives equivalent replay/context binding. The
module stores sealed bodies **opaquely** and enforces: sealed bodies only in
private channels, plain bodies only in public ones, `epoch` ≤ the channel's
current epoch. Private channels are implicitly `MembersOnly` for posting and
reacting; visibility, not `post_policy`, is what a creator chooses. All module-side validation is deterministic; **no crypto runs
in `execute`**. The module cannot verify an envelope decrypts — a malicious
owner or poster can write garbage, which is a liveness nuisance for people
who can already be removed, never a confidentiality failure.

**Membership changes.**

- `AddMember { channel_id, user, wrapped_keys: BTreeMap<epoch, Vec<u8>> }` —
  owner-gated; the owner wraps **all existing epochs** to the joiner (Slack
  semantics: a new member reads history). Epoch count only grows on
  removals, so this stays small.
- `RemoveMember { channel_id, user, next_epoch_wrapped: BTreeMap<member, Vec<u8>> }`
  — owner-gated, and the rotation is **in the same op**: the message that
  removes a member also installs epoch N+1 wrapped to exactly the remaining
  members (module-checked coverage). There is no window in which a removed
  member can read new messages. They keep what they could already read
  (epochs ≤ N) — no retroactive protection, stated plainly.

**DMs.** A DM is a private channel with a deterministic id
`dm:<hex(min(a,b))>:<hex(max(a,b))>`, exactly two members who are both
owners, membership immutable, auto-created by the first message (a dedicated
`SendDm` arm creates-and-posts atomically in one op, so there is no
half-created DM). No owner ops, no roster UI — the DM surface is just "pick
a member from the roster".

### 4. Where the crypto runs: the member's node

The desktop app already delegates **signing** to its local node (the
workspace key never enters the webview). Encryption gets the same treatment:
the node exposes a small workspace-crypto surface on the daemon router
(precedent for non-generic routes: `/v1/files/blob`):

- `GET  /v1/crypto/pubkey` — the workspace messaging pubkey (mints on first
  call, publishes via `SetMessagingKey` if unpublished).
- `POST /v1/crypto/seal`  — `{recipients: [pubkey], plaintext}` → envelopes;
  also `{key: channel_key_env, plaintext, aad}` → sealed body.
- `POST /v1/crypto/open`  — batch: `[{envelope | sealed_body, aad}]` →
  plaintexts.

The app composes module payloads itself (the generic RPC stays generic and
module-blind); it just calls seal/open for the opaque parts. Private keys
never leave the node process. The same surface finally gives the vaults view
a real client-encryption implementation instead of ad-hoc app crypto.
End-to-end here means **member node to member node** — the node is the
member's cryptographic agent, exactly as it already is for authorship.

### 5. App surface

- Channel create dialog: a Private toggle (visibility is immutable after).
- A member roster view sourced from valset + profiles (key, name, has
  messaging key) — doubles as the DM launcher.
- Private channels and DMs render with a lock affordance; sealed bodies are
  opened in batches via `/v1/crypto/open` and cached decrypted **in memory
  only**.
- A member without a published messaging key can't be added to private
  channels/DMs; the roster shows that state and nudges key publication
  (automatic on first app boot after upgrade).

## Semantics, edge cases, limits

- **Key loss.** Losing the messaging secret loses private-channel history;
  recovery is social: an owner re-adds the member's new key (re-wrap). DMs
  are re-created against the new key. No escrow in v1.
- **Caps.** Envelopes are ~100 bytes/member; `ChannelKeyRecord` must respect
  the 256 KiB record bound → cap private-channel membership at 1024 and
  epochs at 256 (removals per channel). Sealed `ct` rides the existing
  64 KiB message-head bound.
- **Structure stays plaintext.** Sequences, threading, reaction emoji +
  reactors, membership, and timing of private channels are visible to member
  nodes (they replicate the state). Only bodies are sealed. Encrypting
  structure buys little inside a team boundary and costs the module its
  deterministic bookkeeping — rejected for v1.
- **Hooks, mentions, agents, search.** Sealed bodies cannot be parsed by
  modules: private channels get **no hook fan-out, no mention routing, no
  agent triggers, no indexer/search** in v1. `RegisterHook` is rejected on
  private channels rather than silently useless. Agents participate in
  public channels as today. (Follow-on if wanted: an agent whose module
  holds a wrapped key — deliberate, visible in the member list.)
- **Rollout.** New msg arms + state fields change module execution: this is
  consensus-breaking and ships via the established upgrade path (lockstep or
  the height-gated dual-path scheme per
  `2026-07-04-no-downtime-node-upgrade-design.md`). All state additions are
  serde-defaulted so existing state decodes without migration.

## What this does not provide (honest ledger)

- **No per-message forward secrecy / PCS.** An epoch key + a member's
  secret decrypts that epoch. MLS/ratcheting was considered and rejected:
  consensus already provides the ordered group-state MLS rebuilds, and the
  team-scale threat model doesn't justify the machinery. The epoch seam
  (rotate-on-remove) is where a ratchet would slot in later.
- **No deniability.** Signed frames are the point — authorship integrity
  was chosen over deniability, repo-wide.
- **No metadata privacy inside the network**; full metadata + content
  privacy outside it.
- **No multi-device story.** One workspace = one node = one key, matching
  the platform's identity model today.

## Testing & acceptance

1. **Module tests (chat, profiles):** owner gating (incl. last-owner rule,
   un-gated-origin regression for `SetMembership`/`RegisterHook`); private
   read hygiene; sealed-body validation (epoch bounds, arm/visibility
   mismatch rejection, caps); `RemoveMember` coverage check (next epoch must
   wrap exactly the survivors); DM id determinism + membership immutability;
   serde compatibility for pre-upgrade state.
2. **Crypto service tests:** seal/open round-trips, wrong-recipient and
   AAD-mismatch failures, batch open, key mint idempotence, on-disk 0600.
3. **End-to-end (fleet):** three workspaces A/B/C. A creates a private
   channel with B; C's node proves via raw query that it holds only
   ciphertext and cannot open it; A removes B → B stops decrypting new
   messages but retains old epochs; A↔B DM round-trip; late-joining node
   syncs and decrypts exactly the channels it was wrapped into.
4. **QA** via the fleet dashboard per the `qa` skill before merge.

## Slices

1. **Governance** — owners, gating, visibility, read hygiene (chat only; no
   crypto; independently valuable — closes the any-origin membership hole).
2. **Keys** — profiles `SetMessagingKey` + node keypair + `/v1/crypto/*`.
3. **Sealed lanes** — `Body::Sealed`, `ChannelKeyRecord`, add/remove+rotate,
   private create; module + e2e tests.
4. **Surface** — DM launcher, roster, lock UI, decrypt cache, fleet QA.

Each slice is a PR into `dev` per the repo's branching rules.

## Vetoable calls (flagged, defaulted, not blocking)

1. **Scope:** full B (encrypted lanes) vs stopping at slice 1 governance.
   Default: full B — governance-only "private" is dishonest as security.
2. **Epoch keys, not MLS** — no per-message FS. Default: epoch keys.
3. **Crypto in the node, not the webview** (keys never in JS). Default: node.
4. **Public channels stay plaintext** so hooks/agents/search keep working;
   private channels lose those in v1. Default: yes.
5. **New members read history** (all epochs wrapped on add). Default: yes —
   Slack semantics; flip to current-epoch-only is a one-line policy change.

## Code anchors

- Chat module + wire surface: `crates/apps/chat/src/lib.rs`,
  `crates/apps/chat/src/interface.rs` (`Channel`, `MessageHead`, caps,
  `SetMembership`/`RegisterHook` gating gap at `interface.rs:217-233`).
- Trust-model precedent: `crates/apps/vaults/src/interface.rs:1-14`.
- Profiles (origin-gated writes): `crates/apps/profiles/src/lib.rs:154-179`.
- Signed op frames / authorship: `crates/kernel/node/src/lib.rs:393-458`,
  `bin/node/src/main.rs:6929-6945`.
- Member registry: `crates/system/valset/src/lib.rs:63-155`; key-binding
  pattern: `crates/system/wireguard-upgrade/src/lib.rs:277-283`,
  `crates/system/reachability/src/keys.rs`.
- Daemon router (crypto routes land here): `bin/noded/src/lib.rs:470-516`.
- App: `app/src/domain/chat-client.ts`, `app/src/domain/transport.ts`,
  `app/src/console/views/chat/`, registry at
  `app/src/console/modules/registry.ts`.
- Deferred substrate for ephemera: `crates/system/data-plane/src/lib.rs`
  (`Service` registry, `AdmissionPolicy` at `plane.rs:44-46`).
