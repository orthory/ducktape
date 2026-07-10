# Account console + onboarding redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the app-only account↔node split: a unified 3-step onboarding flow, a shell-level Account console (custody, member keys, device linking, node registry with unbind), and Settings/Node rearrangement — per `docs/superpowers/specs/2026-07-10-account-console-onboarding-design.md`.

**Architecture:** Frontend-only (React 19 + inline-style tokens, no router — registry/`resolveScreen`). All signing verbs already exist as Tauri commands; new work is UI + two localStorage flags + one auto-bind guard. Zero consensus surface.

**Tech Stack:** React 19, TypeScript, vitest + @testing-library/react, Tauri `invoke` mocked in tests.

## Global Constraints

- Worktree: `/home/eddy/dev/ducktape/.claude/worktrees/account-console`, branch `feat/account-console`, PR target `dev`.
- Run tests: `cd app && bun run test` · typecheck: `cd app && bun run typecheck`.
- Vocabulary (spec-locked): **Account** (person / recovery phrase), **Device** (machine member key), **Node** (per-workspace daemon key), **Workspace**. The strings "user key" and bare "identity" leave the UI.
- Mono-file mandate: new files split by responsibility, ~600-line soft cap.
- localStorage keys: `ducktape.pendingDisplayName`, `ducktape.accountLinkPending` (helpers in `state.ts`, same try/catch pattern as `loadAccent`).
- Blob prefixes: `ducktape-link-challenge-v1:` / `ducktape-link-response-v1:` over base64(UTF-8 JSON).
- Reuse: `IdentityGateForms` widgets, `settings/parts.tsx` primitives, `ConfirmDialog`.

---

### Task 1: link-device blob codec

**Files:** Create `app/src/console/views/account/link-device.ts` + `link-device.test.ts`.

**Produces:**
```ts
export interface LinkChallenge { chainId: string; accountId: string; nonce: number; name: string | null }
export interface LinkResponse { pubkey: string; kind: "ed25519"; possession: string; label: string | null }
export const encodeLinkChallenge = (c: LinkChallenge): string
export const decodeLinkChallenge = (s: string): LinkChallenge | null
export const encodeLinkResponse = (r: LinkResponse): string
export const decodeLinkResponse = (s: string): LinkResponse | null
```
Decode is strict: exact prefix, valid base64/JSON, `accountId`/`pubkey` lowercase hex, `nonce` a non-negative safe integer, `possession` non-empty string; anything else → `null` (never throws). Encode/decode round-trips; whitespace around the blob is trimmed.

- [ ] Failing tests: round-trip both blobs; reject wrong prefix, bad b64, bad JSON, negative nonce, non-hex account id, response-as-challenge.
- [ ] Implement; tests green; commit.

### Task 2: localStorage flags + auto-bind guard

**Files:** Modify `app/src/console/store/state.ts` (persistence helpers block), `app/src/console/store/auto-bind.ts`, extend `app/src/console/store/auto-bind.test.ts`.

**Produces (state.ts):** `loadPendingDisplayName(): string | null`, `savePendingDisplayName(name: string)`, `clearPendingDisplayName()`, `loadLinkPending(): boolean`, `saveLinkPending()`, `clearLinkPending()`.

**Auto-bind change:** add `"deferred"` to `AutoBindResult`. After resolving `accountOfMember`: if no account and `loadLinkPending()` → return `"deferred"` (never founds a duplicate account while a link is pending); on `"bound"`/`"already"` → `clearLinkPending()`. Existing behavior unchanged when the flag is absent.

- [ ] Failing tests: flag set + no membership → `"deferred"`, no submit invoked; flag set + membership present → binds and clears flag; flag absent → founds as today.
- [ ] Implement; tests green; commit.

### Task 3: onboarding chrome (stepper)

**Files:** Create `app/src/console/views/onboarding/OnboardingChrome.tsx`.

**Produces:**
```ts
export function StepRail({ active }: { active: 1 | 2 | 3 })          // ACCOUNT · WORKSPACE · CONNECT indicator
export function OnboardingChrome({ step, children }: { step: 1 | 2 | 3 | null; children: ReactNode })
```
`OnboardingChrome` owns the centered outer wrapper (moves `outerStyle` out of `GateCard`); `step: null` renders no rail (returning-user gates, workspace switcher). `GateCard` loses its outer wrapper (chrome owns it) — its only consumers are IdentityGate screens, all rendered inside the chrome after Task 4.

- [ ] Implement + snapshot-light test (renders 3 labels, active step highlighted); commit with Task 4 (chrome has no standalone consumer until the gate mounts it).

### Task 4: IdentityGate rework (step 1)

**Files:** Modify `IdentityGate.tsx`, `IdentityGateForms.tsx` (3-mode tabs, name field slot), create `app/src/console/views/onboarding/LinkDeviceFlow.tsx`; update `IdentityGate.test.tsx`.

- `AbsentScreen` modes: **Create / Restore / Link device** (ModeTabs takes `Array<{id, label}>`).
- Create: optional **display name** field above password; on success `savePendingDisplayName(name)`. Copy: "Create your account" / recovery-phrase copy per spec.
- Link device: password step (createIdentity + `confirmMnemonic()` to suppress the resume pester — the account's recovery lives on the other device; a comment says so) → `saveLinkPending()` → `LinkDeviceFlow`.
- `LinkDeviceFlow` (also used by the Account console): paste challenge → `decodeLinkChallenge` → `invoke("user_sign_possession", { chainId, accountId, nonce })` → show `encodeLinkResponse({pubkey, kind:"ed25519", possession, label})` with copy + optional device label input → "Continue" (onDone). Errors inline (`identity-locked` → "unlock first").
- Locked screen skip copy: "Until you unlock, nodes you start stay unlinked to your account." All gate screens render inside `OnboardingChrome` (`absent` → step 1; plaintext/locked/resume/bootError → step null).
- Wrap gate: chrome mounts in `IdentityGate` around every gated return.

- [ ] Update/extend tests: 3 tabs render; create captures pending name; link flow encodes a response from a pasted challenge (invoke mocked); locked copy.
- [ ] Tests green; typecheck; commit.

### Task 5: OnboardingGate + JoinProgress under the chrome (steps 2–3)

**Files:** Modify `OnboardingGate.tsx`, `JoinProgress.tsx`; update `app/src/console/store/onboarding.test.tsx` if copy-coupled.

- OnboardingGate: replace its hand-rolled outer wrapper with `OnboardingChrome`, `step = state.workspaces.length === 0 && !state.workspace && !state.nodeUrl ? 2 : null` (first-run vs switcher). Copy: create-tab subtitle "Found a new network — your account becomes its first member; this device runs its first node."
- JoinProgress: delete the hard-coded `STEP 2 / 3` div → `<StepRail active={3}/>` when `state.workspaces.length <= 1`, else nothing. "YOUR NODE IDENTITY" → "THIS NODE'S KEY". Add an **account link hint row** under the steps, driven by one `identityState()` fetch + `loadLinkPending()`: unlocked/plaintext → "Will be linked to your account when admitted"; locked → "Account locked — unlock in the Account view to link this node"; link-pending no membership → "Waiting for your other device to approve the link".

- [ ] Tests green (update copy assertions); commit.

### Task 6: account store ops

**Files:** Create `app/src/console/store/account-ops.ts` (+ test); modify `actions.ts` + the `Actions` interface to expose four thin delegates; modify `DucktapeProvider`/`connectActive.adopt` for pending display name.

**Produces (account-ops.ts, deps injected):**
```ts
export interface AccountOpsDeps { transport: NodeTransport; chainId: string; nodePub: string }
export const mintLinkChallenge = (d: AccountOpsDeps): Promise<string>            // accountOfNode(nodePub) → encodeLinkChallenge (throws "node not linked" if unbound)
export const addMemberFromResponse = (d: AccountOpsDeps, blob: string): Promise<void>   // decode → accountOfNode for fresh {accountId} but SIGN AT THE CHALLENGE NONCE embedded in the response? No — possession was signed at challenge nonce; re-query account, if nonce ≠ challenge nonce the submit fails server-side; we pass the CURRENT account nonce only if equal, else throw "account changed since the challenge — re-run the link"
export const removeMemberKey = (d: AccountOpsDeps, targetKeyHex: string): Promise<void>
export const unbindNode = (d: AccountOpsDeps, targetNodeHex: string): Promise<void>
```
Wrinkle the code must own: the possession proof is nonce-scoped. The response blob does not carry the nonce; `addMemberFromResponse` therefore takes the challenge (kept in component state on the inviter side) and compares its nonce against the live account nonce before signing `user_sign_add_member` — mismatch throws the re-run error instead of a doomed submit. (Exact signature: `addMemberFromResponse(d, challenge: LinkChallenge, blob: string)`.)

**Actions:** `accountLinkChallenge()`, `accountAddMember(challenge, blob)`, `accountRemoveMember(keyHex)`, `accountUnbindNode(nodeHex)` — each resolves the live transport + active workspace, throws a clear error when disconnected. `user_sign_unbind` gains its first TS caller.

**Pending display name:** in `connectActive`'s `adopt()`, after `autoBindUserIdentity` resolves, if `loadPendingDisplayName()` → apply via the existing `setDisplayName` routing logic and `clearPendingDisplayName()`; also `patch({ author: pending })` immediately so the shell greets by name.

- [ ] Unit tests for account-ops with a mocked transport + mocked invoke (happy path, nonce-drift error, unbound error).
- [ ] Tests green; commit.

### Task 7: Account console screen

**Files:** Create `app/src/console/views/account/AccountView.tsx`, `ProfileCard.tsx`, `CustodyCard.tsx`, `DeviceKeysCard.tsx`, `NodesCard.tsx` (+ `AccountView.test.tsx`); modify `ConsoleShell.tsx` (resolveScreen `"account"` branch), `Sidebar.tsx` (avatar button above the gear: circle with `initialsOf(state.author)`, `setScreen("account")`).

- **ProfileCard**: avatar initials + display-name editor + role/FinalizationMark (ported from settings/IdentityCard) + "Account" row: linked → shortKey(accountId)+copy; unbound → "Not linked".
- **CustodyCard**: the custody machine moved from `DevicesSection` (identity lock / unlock / lock / set password / reveal, `CustodyPanel` included) + "Account key (this device)" row (identityState pubkey; fetch-error in red). Desktop-only (`isDesktop()`).
- **DeviceKeysCard**: member keys of the bound account (KIND_LABEL moves here) + "this device" marker (pubkey === identityState pubkey); per-key **Remove** (ConfirmDialog → `accountRemoveMember`; hidden when it's the last key); **Link a device** ControlRow → inline panel: mint challenge (`accountLinkChallenge`, shown with copy) → paste response textarea → **Approve** (`accountAddMember`) with inline error surface.
- **NodesCard**: "ON THIS NETWORK" — rows from `nodeUsers` entries sharing this account: shortKey, badges Validator/Resident (from `state.members`/`state.residents`), "this device", **Unbind** (ConfirmDialog, copy: lost-device eviction) → `accountUnbindNode`. "ON THIS MACHINE" — `state.workspaces` rows: name, chainId, Active marker, **Open** → `selectWorkspace`.
- **AccountView**: composition + honest disconnected banner (account data is chain-scoped; custody + local nodes still render).

- [ ] Render tests over a stubbed store (linked account with 2 keys/2 nodes; unbound; disconnected).
- [ ] Tests green; typecheck; commit.

### Task 8: Settings thinning + Node facts + rename sweep

**Files:** Modify `settings/SettingsView.tsx` (drop IdentityCard/DevicesSection; add ACCOUNT link-row card), delete `settings/IdentityCard.tsx` + `settings/DevicesSection.tsx` (logic lives in account/ now), modify `settings/WorkspaceSection.tsx` (desc copy only if needed), `status/NodeFactsCard.tsx` (add "Node key" full+copy row and "Owned by" row: account name/short id or "Not linked"), rename `nodeUsers[].userKey` → `accountId` across `state.ts`, `DucktapeProvider.tsx`, `MembersView.tsx`, new account views; sweep remaining "identity/user key" strings in onboarding/settings/status/members copy to the locked vocabulary.

- [ ] Grep gate: `grep -rn "userKey" app/src` returns only domain-level user-identity-client internals (or nothing); `grep -rn "user key" app/src` returns nothing user-facing.
- [ ] Full app test suite + typecheck green; commit.

### Task 9: verification

- [ ] `cd app && bun run test` — all green.
- [ ] `bun run typecheck` — clean.
- [ ] Live QA headless (tauri-debug/fleet): fresh create-account onboarding (stepper visible, name captured), Account console over a live workspace, Settings link row, Node facts rows, unbind confirm renders. Link ceremony: codec + wizard drive at UI level (single box).
- [ ] PR against `dev`; clean-context review; leave merge per confidence policy.
