# W6 — Account Profile Propagation (plan)

Spec: `../specs/2026-07-14-w6-profile-propagation-spec.md`.

## Rust — `crates/system/identity`

1. `interface.rs`: add `avatar`/`bio: Option<String>` to `AccountView`; add
   `SetProfile { avatar, bio }` to `IdentityMsg`; add `MAX_BIO_LEN`,
   `MAX_AVATAR_REF_LEN`; extend the msg-codec roundtrip test.
2. `lib.rs`: add `avatar`/`bio` to `AccountRecord`; encode them in
   `encode_state` (after `display_name`); decode in `decode_snapshot` (bump
   `MIN_ACCOUNT_BYTES` 41→43); surface in `account_view`; add `set_profile`
   handler (origin-gated, trims-empty-clears, length-caps, no nonce bump);
   route it in `execute`.
3. `tests.rs`: `set_profile_is_origin_gated_and_caps`; extend the snapshot
   roundtrip to carry an avatar + bio.

## Wasm fixture

Rebuild ONLY identity (single-sourced adapter), not the whole `wasm-modules`
target (avoids toolchain-drift churn on untouched blobs):
- `cd crates/examples/identity-wasm && cargo build --target wasm32-unknown-unknown --release`
- `wasm-tools component new .../identity_wasm.wasm -o crates/examples/identity-wasm/component.wasm`
- `cp` → `crates/kernel/host/tests/fixtures/identity.component.wasm`
- confirm `wasm_identity_parity` green.

## TS — `app/src`

4. `domain/identity-client.ts`: `avatar`/`bio` on `AccountView`;
   `setAccountProfile(transport, { avatar, bio, origin })`.
5. `console/store/account-profile.ts` (new): localStorage `{ name?, bio?,
   avatar? }` load/save/clear (avatar = data URL). Small, single responsibility.
6. `console/store/profile-reconcile.ts` (new): `reconcileProfile(transport,
   target)` — idempotent compare-and-push; content-addressed avatar upload via
   `files-client`. Pure over injected deps; unit-testable.
7. `console/store/actions.ts`: call `reconcileProfile` in `adopt()` +
   `identityUnlocked()`; `setDisplayName` also persists name to the store;
   `setProfile(bio, avatar)` action for the panel (persist + reconcile active).
8. `console/store/hydration.ts` + `state.ts`: project `authorAvatars` +
   `authorBios` (keyed like `authorNames`); thread through snapshot.
9. `console/components/Avatar.tsx` (new): duckfs-path → `<img>`, initials
   fallback. Reused in `ProfileCard` + `MembersView`.
10. `console/views/home/AccountProfilePanel.tsx` (new, self-contained): avatar
    picker + bio editor; mount in `HomeView`.
11. Render: `ProfileCard` own avatar via `Avatar`; `MembersView` member avatars.

## Tests

- Rust: module ops (above).
- TS: `app/src/test/sim/profile.test.tsx` — reconcile idempotence
  (no-op when converged, pushes when dirty, uploads avatar once).

## Gates

- `cargo clippy -p identity --tests --no-deps` (touch a .rs first).
- `cargo test -p host --test wasm_identity_parity` (fixture rebuilt).
- files crate untouched → its wasm gate not required, but run if touched.
- `bun test` for the sim suite.
