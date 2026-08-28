# Identity rework — accounts, key associations, no node binding

2026-08-27. Status: **design approved** (six decisions settled with the user,
recorded at the end); implementation plan next. Replaces every earlier
identity spec (2026-07-07 split, 07-10, 07-14, 07-17 passkey/session-keys,
08-26 wallet-keystore §on-chain): where they disagree with this document,
this document wins. Zero live networks, so every change below is a
flag-day replacement — no compat arm, no dual path.

## The three rules

1. **An account is an abstract principal, unrelated to any key.** It is
   created in a workspace with a name and ONE initial public key, and is
   identified from then on by a number, never by a key or a name.
2. **An account's association is the set of keys that speak for it.** Any
   key in the association may add or remove another (Keybase). Keys come in
   three schemes: `Ed25519` (everything native — device keys, node keys,
   SSH keys), `Secp256k1` (an Ethereum wallet), `Secp256r1` (a WebAuthn
   passkey).
3. **There is no account ↔ node binding.** A node key is a node key. It
   never resolves to an account, and no module, plane, or CLI asks "whose
   node is this" of the chain. Account attribution always comes from a
   USER-signed artifact: a frame, an add-key proof, a request
   proof-of-possession, a git push certificate.

## What exists today, and what this keeps

The current `identity` module already has a Keybase-style umbrella:
`member_keys` of mixed kinds, `AddMemberKey`/`RemoveMemberKey` with two
consents, any-member-authorizes, and a working WebAuthn P-256 envelope
verifier (`crates/modules/system/identity/src/scheme.rs`). None of that is
wrong; it is KEPT in shape and renamed. What goes is everything that made
the account *a key* or *a set of nodes*:

| today | after |
|---|---|
| `account_id` = founding pubkey bytes | `number: u64`, monotonic from 1 |
| `BindNode` founds the account as a side effect | explicit `Create { name }` |
| `nodes` map + `node\0` index + `OfNode` | deleted |
| `UnbindNode`, `SetNodeLabel` | deleted |
| `MemberAuth { key, kind, proof }` on every op | the frame origin IS the acting key; only `AddKey` carries an explicit authorizer proof |
| per-account `nonce` on every signed op | deleted; a per-KEY generation counter guards `AddKey` proofs instead |
| `KeyKind { Ed25519, P256, WebauthnP256 }` | `KeyScheme { Ed25519, Secp256k1, Secp256r1 }` |
| `rp_id_hash` pin per passkey | deleted (a passkey is RP-scoped by construction; its pubkey cannot appear under another RP) |
| roster record `accounts` (cap 1024) | deleted; `All` iterates `1..next`; cap raised to 65 536 |
| origin-gated `SetAccountName` (bound node) | member-key-signed `SetName` |
| frame codec: ed25519 only | frame carries a scheme byte; one verifier for all three |

## On-chain model

### `KeyScheme` — one crate, one verifier

New crate `crates/kernel/keyscheme` (moved out of `identity/src/scheme.rs`
so the kernel frame codec, the identity module, forge and the host planes
share it — the frame codec sits below modules and cannot depend on one).
Pure, deterministic, wasm-clean; the identity guest already compiles this
code.

```rust
#[repr(u8)]
pub enum KeyScheme { Ed25519 = 0, Secp256k1 = 1, Secp256r1 = 2 }

impl KeyScheme {
    pub fn tag(self) -> u8;
    pub fn from_tag(u8) -> Option<Self>;
    /// ed25519: 32 bytes. secp256k1 / secp256r1: SEC1 33 or 65 bytes.
    pub fn pubkey_wellformed(self, pubkey: &[u8]) -> bool;
    /// does `proof` show the holder of `pubkey` authorized `preimage` under `ns`?
    pub fn verify(self, pubkey: &[u8], ns: &[u8], preimage: &[u8], proof: &[u8]) -> bool;
}
```

`proof` is scheme-owned bytes; the scheme parses it. A scheme may accept
more than one envelope when the envelope is unambiguous by length or magic:

- `Ed25519` — 64 bytes: commonware signature over `union_unique(ns, preimage)`
  (today's native path). `SSHSIG` magic: an OpenSSH signature (namespace
  `git`, SHA-512, signer pubkey embedded in the blob) — used ONLY by forge
  push certificates (phase 6), where the signed message is git's own push
  cert rather than our preimage; forge calls a dedicated
  `sshsig::verify_ed25519(pubkey, namespace, message, blob)` in this crate.
- `Secp256k1` — 65-byte `r‖s‖v` from `personal_sign`: the wallet signs
  `msg = union_unique(ns, preimage)` as an EIP-191 message
  (`keccak256("\x19Ethereum Signed Message:\n" ‖ len(msg) ‖ msg)`); verify =
  recover the pubkey, compare to the stored compressed point. Needs `k256`
  (already in the workspace via `crates/labs`) + `sha3`. Every Ethereum
  wallet produces this with no custom code on the wallet side.
- `Secp256r1` — the existing WebAuthn envelope, framed as three
  length-prefixed fields `authenticator_data ‖ client_data_json ‖ sig(raw R‖S)`.
  Challenge = `SHA-256(ns ‖ preimage)` (today's `webauthn_challenge`), type
  must be `webauthn.get`, User-Present flag required, then pure-Rust p256.

Deleted with the move: `KeyKind::P256` (a raw hardware P-256 signer nobody
has), `MemberProof` (the scheme owns the shape), `webauthn_rp_id_hash`.

### Frame codec (`crates/kernel/node`)

The frame preimage gains ONE leading byte: `(scheme, origin, seq, target,
payload)`. `decode_frame` reads the scheme, then
`KeyScheme::verify(origin, FRAME_NS, preimage, trailing_bytes)`.
`Origin::External(pubkey)` is UNCHANGED — raw pubkey bytes, no tag — so no
origin consumer (chat authors, files `ext:` actors, valset tiers, governance
ballots, forge) moves. A node signs its frames as `Ed25519` (tag 0).

The scheme is not needed downstream: raw pubkey bytes cannot collide across
schemes without solving a discrete log on the other curve, so the identity
index is keyed by raw pubkey and verification under the frame's declared
scheme is proof of possession by itself.

`encode_frame(signer: &ed25519::PrivateKey, ..)` stays for nodes and
ed25519 device keys; `frame_preimage(scheme, origin, seq, msg)` becomes
`pub` so a wallet or passkey client can sign it externally and append the
proof.

### Account state

```rust
pub type AccountNumber = u64;

struct AccountRecord {           // borsh; stored at `acct\0{number LE}`
    name: String,                // MAX_NAME_LEN; display only, NOT unique
    keys: BTreeMap<Vec<u8>, KeyMeta>,   // the association, keyed by raw pubkey
    avatar: Option<String>,
    bio: Option<String>,
    updated_at: u64,
}
struct KeyMeta { scheme: KeyScheme, label: Option<String>, added_at: u64 }
```

State keys: `acct\0{n}` → record; `key\0{pubkey}` → `n`; `gen\0{pubkey}` →
`u64` (how many times this key has been admitted, anywhere; absent = 0);
`next` → next number. `All { from, limit }` reads `acct\0{from..}`
directly (no deletion ever, so no gaps); `MAX_ACCOUNTS = 65_536` caps
`next`. Byte caps and the `store_bounded` discipline are unchanged.

`number` is the only id. Names are display text: identity state is
per-workspace, so a "unique" name would only be a first-come claim inside
one workspace and would still let a stranger take `eddy` on the next one —
the thing that proves the same person across workspaces is the same key in
both associations, never the name. The proof of an `AddKey` is NOT stored
(the block that carried it is the audit log).

### Messages — the frame origin is the acting key

```rust
enum IdentityMsg {
    /// found an account. origin = the initial key (its frame signature is the
    /// possession proof); origin must belong to no account.
    Create { name: String },
    /// add the ORIGIN key (of `scheme`, the frame's scheme) to the account
    /// `authorizer.key` belongs to. the frame signature is the new key's
    /// possession proof; `authorizer.proof` is an existing member's proof over
    /// `add_key_preimage` at the key's CURRENT generation. origin must belong
    /// to no account. on success `gen[origin] += 1`.
    AddKey { label: Option<String>, authorizer: Authorizer },
    /// origin ∈ account. any member removes any key except the last. writes
    /// nothing but the record — a removed key may be re-admitted later (a
    /// wallet or an SSH key cannot be re-minted), at its next generation.
    RemoveKey { key: Vec<u8> },
    /// origin ∈ account.
    SetName { name: String },
    SetProfile { avatar: Option<String>, bio: Option<String> },
}
struct Authorizer { key: Vec<u8>, proof: Vec<u8> }

fn add_key_preimage(chain_id, scheme_tag, new_key, gen: u64) -> Vec<u8>
// ns = b"ducktape-identity-add-key-v1"; a passkey authorizer's challenge
// = SHA-256(ns ‖ preimage). the client and the verifier share this fn.
```

No account number and no account nonce in the preimage, on purpose — the
only state it folds in is the NEW key's own generation:

- The account is resolved from `OfKey(authorizer.key)` — a key belongs to
  exactly one account, so a proof cannot be redirected to another account.
- Replay: a proof authorizes one key `K` at generation `g`; only `K`'s
  holder can build the frame that carries it; once `K` is admitted,
  `gen[K]` is `g+1`, so the same proof never verifies again — including
  after a compromised `K` is removed. A removed key is NOT burned: a
  wallet or an SSH key can be re-linked (to any account) by an authorizer
  signing at the new generation. An authorizer that has itself been
  removed fails the "is a current member" check, so its outstanding proofs
  die with it — no account nonce needed for that either.
- A fresh key has generation 0, which the joining device knows without a
  query. That is what lets QR login run WITHOUT a username (see WebAuthn):
  the challenge is computable before anyone knows which account or which
  passkey will answer. Re-linking a used key costs one `KeyGen` query.
- Two devices adding keys in the same block no longer race.

`RemoveKey`/`SetName`/`SetProfile` need no certificate: the origin's frame
signature is the consent.

Queries: `All { from, limit }`, `Get { number }`, `OfKey { key }`,
`KeyGen { key }` → `IdentityReply::{Accounts, Account(Option<AccountView>), Gen(u64)}`.
`AccountView { number, name, keys: Vec<KeyView{scheme,pubkey,label,added_at}>, avatar, bio, updated_at }`.

Admission: `Create` and `AddKey` come from keys that are not members yet, so
the `identity` ACL target is `Open` — anyone with a valid key founds an
account; the cap, the record byte cap, and the node's `/v1` exposure are the
spam defence, and the ACL policy stays the operator's knob for later.
`Standing::User` becomes `OfKey(origin).is_some()`.

## Consumers — every `OfNode` and every "node → account" read

In-consensus (one PR with the module — the genesis root and the wasm guest
change together):

| site | change |
|---|---|
| `crates/kernel/host/src/lib.rs:1490` `identity_account_holds` | `OfKey` only |
| `governance` `Actor::Account { account_id, nodes }` | `Actor::Account { number }`; in node-electorate mode only node keys vote as themselves; share mode keys on `number` (`account_id: Vec<u8>` → `u64` in `ShareAllocation`, `SetShares`) |
| `gateway/module.rs` `account_of_node`, `origin_node` (32-byte gate), "publisher does not match origin", "account does not own the publisher node" | origin = a member key (`OfKey`); `RouteStatement.publisher_node` is whatever node the account's signature names — the account vouches for it; `authorization` verifies through `KeyScheme::verify` with the member's stored scheme |
| `forge/state.rs` `principal_of_origin` | `OfKey(origin).unwrap_or(origin)`; phase 6 adds the push-cert principal |

Host-side (`bin/node`, `crates/noded`, `crates/airlock`):

| site | change |
|---|---|
| `gateway_plane.rs` `account_of_node(caller)`, `node_is_current`, `x-duck-caller-account` | caller account = the request's user PoP (below); `Owner`/`Accounts` audiences check that account; `Network` = any mesh peer, PoP or not |
| `work_admission.rs` `owner_account`/`account_of_node`, `WorkCaller::Account` | saga callers: `OfKey(saga origin)`; peer callers (`WorkSource::Peer`, the term plane's session create): `ThisNode` or `PeerNode` (no account — admitted by `Anyone`, refused by `Accounts`); `WorkAdmission::Owner` is deleted — the app/operator writes `Accounts([n])` |
| `airlock` grant lanes (`bin/node/src/airlock.rs`, `crates/airlock/src/server.rs::vouched_caller`) | the vouched caller is the mesh-verified NODE key (`x-duck-caller-node`, already minted). Lane (1) "caller's own grant" is deleted (a node holds no grant). Lane (3) becomes `saga.pinned_assignee == caller_node` — a pure compare, no identity read. Lane (4) `OfKey(saga origin)` ∈ owner/grants — works because sagas are user-signed. #1177 cross-host lending survives on (3)+(4) |
| `compute/cred.rs` `account_of_node(record.publisher_node)` | `record.owner_account` — the record already carries it |
| `term_plane.rs` creator / owner | creator = the requesting peer's node key (gate unchanged); owner from the credential record |
| `noded/admin.rs` `resolve_owner` | `OfKey(admin key)` |
| `cli.rs` `node status account=` line, `agent_cli` name→node | deleted |
| `ops/demo-kanban.mjs` "who am I" | reads the caller account the PoP established, `anonymous` without one |

### Request proof-of-possession (gateway callers)

The node daemon holds no user key (the service path never reads one), so a
peer node cannot sign as its user. The APP can: its duck:// scheme handler
already stamps `x-duck-authority` on every request to its local node, and
its signer is unlocked. It adds `x-duck-user-key`, `x-duck-user-ts`,
`x-duck-user-sig` = `sign(GATEWAY_CALLER_NS, publisher_node ‖ account_id ‖
route ‖ method ‖ path ‖ ts)` with 30 s freshness — the exact shape
`/v1/admin`'s `x-ducktape-admin-*` PoP has. The local node copies the three
into `ProxyRequestHead` fields (the proxy refuses caller-supplied `x-duck-*`
headers, so they cannot ride as headers past the first hop); the publisher
verifies with `KeyScheme::verify` under the key's stored scheme and resolves
`OfKey`. A forwarding node cannot forge it — it has no user key. Requests
without a PoP (a CLI `curl`, a peer's service) are `Network`-level callers.

Every CLI verb that submits a USER op moves from `/v1/submit` (node
re-signs → node principal) to `/v1/submit/frame` signed by the user key:
`account *`, `cred add/grant/revoke` (`SetCredential`, the airlock route),
`agent run/sched` (the saga's trigger origin becomes the USER, so
`saga::namespaced_id` composes under the user key — the "sched saga id is
owned by the submitting node" contract flips to the user). `/v1/submit`
stays for node-authored ops only (announce, saga results, compute pump,
unsigned git pushes).

## Clients

### Key storage — unchanged

One device = one key, shared by the app and the CLI on that device: the
2026-08-26 keystore (`~/.ducktape/keys/<name>.key` + `active`,
`DUCKTAPE_USER_KEY` override) stays exactly as it is. Splitting the app and
the CLI onto different keys on the same machine buys nothing (same user,
same disk) and would only add a "register the CLI key" step; the
association is for DIFFERENT devices — a laptop, a server, a phone's
passkey, a wallet.

### CLI

```
ducktape account create --name <name>          # Create; the active key is the initial key
ducktape account show [--number N | --key <hex>]   # Get / OfKey (default: OfKey(active key))
ducktape account key list
ducktape account key add --pubkey <hex> --scheme <s> [--label]   # this device authorizes a pasted key
ducktape account key add --ssh <path/to/id_ed25519.pub>           # phase 6: an SSH key as an Ed25519 member
ducktape account key approve                   # print this device's pubkey + add-key challenge for another device
ducktape account key remove --pubkey <hex>
ducktape account login                         # QR/passkey: add THIS device's key, authorized by a passkey (phase 5)
ducktape account set-name / set-profile
```

Deleted: `user account-init`, `user sign-bind/sign-unbind`, `node label`.
`user sign-add-member` / `webauthn-challenge` / `p256-payload` fold into
`account key approve`.

### App

- `load_account`: `OfKey(local user key)`, not `OfNode(node key)`. "No
  account" renders a Create form (name); Create is a frame signed by the
  app key. Members count only; the nodes count and node-label UI go.
- Settings → Keys: the association list (scheme, label, added_at), add
  (approve another device's pasted challenge / register a passkey / link a
  wallet), remove.
- DM peer directory: key = account number; `dm_channel_id` takes the
  number's decimal string.
- `SetAccountName` → `SetName`, already frame-signed.
- duck:// requests carry the user PoP (above).

### Ops

`ops/demo-seed.sh` and `ops/demo-gateway.mjs`: replace the bind with
`account create --name demo` signed by the demo wallet; the gateway route
is signed by the same key (`publisher_node` = the demo node's key, which
the account now merely names). `demo-gateway`'s `owner` routes keep
working from the owner's app through the PoP.

## WebAuthn — registration, and QR login

Nothing is built for the QR itself: the WebAuthn **hybrid transport** is the
browser's job. A page calls `navigator.credentials.get()`; the browser shows
"use a passkey from another device" with a QR; the phone scans, asserts,
and the browser hands the assertion back. That works only from a real
browser page at a stable origin, because a passkey is scoped to its RP ID
and every device must use the same one.

So: one static page at **`https://auth.ducktape.industries/`** (RP ID
`auth.ducktape.industries`), no backend. The app/CLI opens the system
browser to it with the request in the URL fragment
(`#op=create|get|eth&challenge=…&user=…&cb=http://127.0.0.1:<port>/`), the
page runs the ceremony, and POSTs the result to the loopback callback (the
`gh auth login` shape). The callback listener lives in the app/CLI for the
duration of one ceremony. Changing the domain later invalidates every
passkey (users re-register) — acceptable at zero live networks, not after.

- **Register a passkey** (an existing member device does this): `create()`
  with `user.id` = account number LE bytes, `user.name` = account name;
  the page returns the credential's SEC1 pubkey. Then `AddKey { authorizer:
  this device }` as a frame whose origin is the passkey — so a second
  `get()` over the frame preimage follows. Two touches, once per passkey.
  (A `webauthn.create` attestation does not prove possession — "none"
  attestation carries no signature — so the second ceremony is not optional.)
- **QR login** (a new device with no member key, no username typed): the
  device mints its ed25519 key `K`, computes
  `challenge = SHA-256(ns ‖ add_key_preimage(chain_id, Ed25519, K, 0))` (a
  fresh key is at generation 0 — no query), opens
  the page with `get()` and `allowCredentials: []` (synced passkeys are
  discoverable — the phone offers the right one), receives the assertion
  whose `userHandle` names the account number, `Get { number }`, matches the
  assertion against that account's `Secp256r1` keys, and submits
  `AddKey { authorizer: { key: <that passkey>, proof: <envelope> } }` as a
  frame signed by `K`. One scan.
- **Link an Ethereum wallet**: `op=eth` calls `personal_sign` over the frame
  preimage (`Create` or `AddKey` with the wallet as origin); the page
  recovers the pubkey client-side to display it. A wallet, like a passkey,
  is an identity anchor — daily ops are signed by the device's ed25519 key.

Dev without the domain: the same page served from the node at
`http://localhost:<port>/.duck/auth` gives an RP ID of `localhost`, which
works for platform authenticators on the same machine but not for the
cross-device QR flow.

## Git push — `git push --signed` (phase 6)

Today `POST /forge/{repo}/git-receive-pack` carries no authentication; the
node submits `PushRefs` under its own key and the binding turned that into
the user's account. After the rework an unsigned push is the NODE
principal: feature branches still take it, protected branches refuse it
(safe direction). The user-signed push is stock git:

```
ducktape account key add --ssh ~/.ssh/id_ed25519.pub   # once: the SSH key joins the association
git config gpg.format ssh; git config user.signingkey ~/.ssh/id_ed25519.pub
git config push.gpgSign true                            # every push carries a push certificate
```

- git's push certificate lists `(old sha, new sha, refname)` per update plus
  pusher and nonce, signed by the SSH ed25519 key in OpenSSH `SSHSIG`
  format (namespace `git`); it rides the pack protocol on HTTP and SSH alike.
- `noded/git_http.rs`: advertise `push-cert=<nonce>` in `info/refs`
  (nonce = chain id ‖ repo — cross-chain binding, not freshness), parse the
  cert section of the receive-pack body, and put it on the op:
  `PushRefs { repo, updates, pack_digest, cert: Option<PushCert{cert, sshsig}> }`.
- forge: with a cert, verify the SSHSIG (`keyscheme::sshsig`), parse the
  cert, require its update list to equal `updates`, principal =
  `OfKey(cert signer)`. Without one, principal = frame origin. The node is
  not trusted — every validator re-verifies the cert. Replay is inert: the
  cert names exact old→new CAS moves.
- GPG (OpenPGP) is not supported — the parser is heavy and SSH signing
  covers the flow.

## Phases (each a PR against `dev`)

1. **keyscheme + frame** — new crate, `KeyScheme` with three verifiers and
   test vectors (a MetaMask-produced `personal_sign` vector, a real
   WebAuthn assertion, commonware ed25519), frame scheme byte, `frame_preimage`
   public. Identity's `scheme.rs` is MOVED, not copied: identity adopts
   `KeyScheme` (drop `P256`, `WebauthnP256` → `Secp256r1`, proof → bytes)
   in this PR, which is one wasm regen of its own — no window with two
   verifiers in tree.
2. **identity module + in-consensus consumers** — the model above, wasm
   guest regen, `wasm_identity_parity`, host ACL, governance, gateway
   module, forge; genesis root updates. `SetHandle`'s account id type
   follows (`Vec<u8>` → `u64`).
3. **host planes + CLI + request PoP** — `bin/node` planes per the table,
   `/v1/submit/frame` for user ops, `ducktape account`, deletions, the
   `ProxyRequestHead` PoP fields + publisher verification, ops scripts, docs
   (`docs/dist/*/modules/product-modules`).
4. **app** — account plane, keys settings, DM directory, Create flow, PoP
   stamping on duck:// requests (needed for phase 3's `Owner` audience to
   admit anyone; phases 3 and 4 land together or 4 first).
5. **WebAuthn** — the auth page, register / QR-login / wallet-link flows in
   app and CLI, `account login`.
6. **signed git push** — `keyscheme::sshsig`, `git_http` push-cert,
   `PushRefs.cert`, `account key add --ssh`.

## Decisions (settled 2026-08-27)

1. `name` is display-only, not unique; no per-account nonce; `AddKey`
   proofs are single-use via a per-key generation counter (`gen\0{K}`,
   tombstones rejected — a wallet/SSH key must stay re-linkable); QR login
   is usernameless via `userHandle`.
2. Account creation is open; `MAX_ACCOUNTS` raised to 65 536.
3. Key storage unchanged: one device key shared by app and CLI, the
   2026-08-26 keystore as is.
4. RP ID / auth page: `auth.ducktape.industries`.
5. Peer attribution: the app's request proof-of-possession, in phase 3
   (with phase 4's stamping); airlock lanes re-keyed on the node key and
   the saga origin; `WorkAdmission::Owner` deleted.
6. Git push: `git push --signed` with SSH ed25519 keys as members, in
   phase 6; unsigned pushes are node-principal until then.

## Out of scope (say so, don't build it)

- Session keys with scope/expiry (the 07-17 spec): a device key IS a member;
  revoke with `RemoveKey`. Add scoping when a real need for a key that
  cannot add keys appears.
- Per-key sequence numbers in state: frame replay stays digest-gated.
- Storing proofs on chain, account deletion, name history, unique names,
  a global name registry.
- GPG-signed pushes, a `git-remote-duck` helper.
- Node ownership of any kind. If "whose node" ever comes back it is a
  separate, user-requested design — not a field in this one.
