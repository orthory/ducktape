// Desktop-only: the bind state lives in `state.nodeUsers`, keyed by node hex,
// which only carries a real entry once a workspace node key exists to look up
// (a web build has no local node key at all — degrade by omitting the whole
// section rather than showing a false "Not linked").
// The machine user key lives outside the node entirely (`~/.ducktape/user.key`,
// desktop-only) — derived from identityState()'s pubkey (below) rather than a
// separate legacy fetch, so the row renders even while locked (v2/encrypted
// keys carry their pubkey in the clear). A corrupt-key error (the file is
// never overwritten silently — this is the operator's only signal) surfaces
// right here without touching the console-wide error banner.

import { useCallback, useEffect, useState, type ReactNode } from "react";

import { normalizeKey, shortKey } from "../../../domain/names";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import {
  encryptLegacy,
  identityState,
  lockIdentity,
  revealMnemonic,
  unlockIdentity,
} from "../../../domain/user-identity-client";
import { isDesktop } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color } from "../../theme/tokens";
import { errMessage, MnemonicGrid, PasswordForm } from "../onboarding/IdentityGateForms";
import {
  ControlRow,
  darkButton,
  GroupCard,
  HoverButton,
  InfoRow,
  monoValue,
  outlineButton,
  SectionLabel,
} from "./parts";

type UserKeyStatus = { pubkey: string } | { error: string };

/** Which inline custody form/grid (if any) is currently expanded below its
 *  trigger row. Mutually exclusive — only one of unlock/set-password/reveal
 *  is ever mid-flow at a time. */
type CustodyPanelKind = "none" | "unlock" | "setPassword" | "reveal";

/** An inline expando block under a custody trigger row — same visual family
 *  as the old invite blob row (sunken background, its own padding). */
function CustodyPanel({ children, last }: { children: ReactNode; last?: boolean }) {
  return (
    <div
      style={{
        padding: "10px 15px 13px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
        background: color.sunken,
      }}
    >
      {children}
    </div>
  );
}

export function DevicesSection() {
  const { state } = useDucktape();
  const workspace = state.workspace;
  // The custody state machine (locked/unlocked/plaintext/absent), driven by
  // identityState() — Task 6's rows are built from this, AND (below) the
  // "User key" row's pubkey: a v2/encrypted key still carries its pubkey in
  // the clear, so this single fetch covers both, even while locked. Never
  // derived from any cached mnemonic or password — every mutator below
  // re-fetches fresh on success.
  const [identity, setIdentity] = useState<IdentityStateReport | null>(null);
  // Set only when the identityState() fetch itself rejects (a corrupt/
  // unreadable user.key — the file is never overwritten silently, so this is
  // the operator's only signal). Surfaced in the "User key" row; the custody
  // block below just stays absent on a failed fetch (not spec'd for
  // Settings — no second error banner).
  const [identityFetchError, setIdentityFetchError] = useState<string | null>(null);
  const [panel, setPanel] = useState<CustodyPanelKind>("none");
  const [busy, setBusy] = useState(false);
  const [identityError, setIdentityError] = useState<string | null>(null);
  // The just-revealed mnemonic, held only long enough to render the grid —
  // cleared on EVERY panel transition (see transitionPanel below) and never
  // persisted anywhere else.
  const [mnemonic, setMnemonic] = useState<string | null>(null);

  // Routed through an extra microtask hop so a synchronous throw inside
  // identityState() (a malformed mock, a bad IPC binding) rejects this
  // promise instead of escaping the effect uncaught.
  const refreshIdentityState = useCallback(
    () =>
      Promise.resolve()
        .then(() => identityState())
        .then((report) => {
          setIdentity(report);
          setIdentityFetchError(null);
        })
        .catch(() => {}),
    [],
  );

  useEffect(() => {
    if (!workspace || !isDesktop()) return;
    let alive = true;
    Promise.resolve()
      .then(() => identityState())
      .then((report) => {
        if (!alive) return;
        setIdentity(report);
        setIdentityFetchError(null);
      })
      .catch((err: unknown) => {
        if (alive) setIdentityFetchError(String(err));
      });
    return () => {
      alive = false;
    };
  }, [workspace]);

  // EVERY panel change routes through here so a revealed mnemonic can never
  // survive a panel transition: switching away from reveal drops it, and
  // re-opening reveal always starts back at the password step (the plaintext
  // path re-sets it fresh from its own revealMnemonic resolution AFTER this
  // clear runs, inside the same handler). A bare setPanel anywhere else would
  // let a stale grid render with no re-prompt — the exact "reveal always
  // re-prompts" violation the spec forbids.
  const transitionPanel = (next: CustodyPanelKind) => {
    setPanel(next);
    setIdentityError(null);
    setMnemonic(null);
  };

  const closeCustodyPanel = () => transitionPanel("none");

  // The async continuations below setState without an alive/mounted guard on
  // purpose: React 18+ makes setState on an unmounted component a silent
  // no-op (the old warning is gone), and threading a mounted-ref through
  // every .then/.catch/.finally here buys nothing but noise. The mount
  // effect above keeps its alive flag only because it predates this block
  // and guards a state pair that renders immediately.
  const handleUnlock = (password: string) => {
    setBusy(true);
    setIdentityError(null);
    unlockIdentity(password)
      .then(() => {
        closeCustodyPanel();
        return refreshIdentityState();
      })
      .catch((err) => setIdentityError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  const handleLock = () => {
    setBusy(true);
    setIdentityError(null);
    lockIdentity()
      .then(() => {
        closeCustodyPanel();
        return refreshIdentityState();
      })
      .catch((err) => setIdentityError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  const handleSetPassword = (password: string) => {
    setBusy(true);
    setIdentityError(null);
    encryptLegacy(password)
      .then(() => {
        closeCustodyPanel();
        return refreshIdentityState();
      })
      .catch((err) => setIdentityError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  // Plaintext has no password to verify — the underlying verb tolerates an
  // empty one for a legacy key — so this reveals straight through with no
  // prompt. Locked/unlocked ALWAYS re-prompt: revealMnemonic never consults
  // the session cache by design, so neither does this button — the mnemonic
  // is only ever set here, fresh, from that call's own resolution.
  const handleRevealClick = () => {
    if (!identity) return;
    setIdentityError(null);
    if (identity.state === "plaintext") {
      setBusy(true);
      revealMnemonic("")
        .then((revealed) => {
          // Order matters: the transition's clear runs first, then the fresh
          // value from THIS resolution lands — never a stale copy.
          transitionPanel("reveal");
          setMnemonic(revealed.mnemonic);
        })
        .catch((err) => setIdentityError(errMessage(err)))
        .finally(() => setBusy(false));
      return;
    }
    transitionPanel("reveal");
  };

  const handleRevealSubmit = (password: string) => {
    setBusy(true);
    setIdentityError(null);
    revealMnemonic(password)
      .then((revealed) => setMnemonic(revealed.mnemonic))
      .catch((err) => setIdentityError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  if (!workspace) return null;

  const nodeKeyNorm = normalizeKey(workspace.pubkey);
  const bound = state.nodeUsers[nodeKeyNorm];
  // This user's OTHER bound nodes — every nodeUsers entry sharing the same
  // userKey, excluding this device itself.
  const otherNodes = bound
    ? Object.entries(state.nodeUsers)
        .filter(([key, user]) => key !== nodeKeyNorm && user.userKey === bound.userKey)
        .map(([key]) => key)
    : [];
  // Driven by identityState()'s pubkey rather than the legacy user_identity_status
  // (which shells the GENERATE verb and errors on a v2/encrypted file) — this
  // works whether the identity is plaintext, locked, or unlocked. A fetch
  // rejection (corrupt/unreadable file) still surfaces here, in red.
  const userKey: UserKeyStatus | null = identity?.pubkey
    ? { pubkey: identity.pubkey }
    : identityFetchError
      ? { error: identityFetchError }
      : null;
  const showUserKey = userKey !== null;
  // "absent" (no user.key yet) renders exactly like non-desktop: no custody
  // rows at all — the identity gate is what's supposed to create one first.
  const showCustody = identity !== null && identity.state !== "absent";

  return (
    <>
      <SectionLabel>DEVICES</SectionLabel>
      <GroupCard>
        <InfoRow
          label="This device"
          value={<span style={monoValue}>{shortKey(workspace.pubkey)}</span>}
        />
        <InfoRow
          label="Bind state"
          last={otherNodes.length === 0 && !showUserKey && !showCustody}
          value={
            <span style={monoValue}>
              {bound ? `Linked to ${bound.name ?? shortKey(bound.userKey)}` : "Not linked"}
            </span>
          }
        />
        {otherNodes.length > 0 ? (
          <InfoRow
            label="Other devices"
            last={!showUserKey && !showCustody}
            value={
              <span style={monoValue}>
                {otherNodes.map((key) => shortKey(key)).join(", ")}
              </span>
            }
          />
        ) : null}
        {userKey ? (
          <InfoRow
            label="User key"
            last={!showCustody}
            value={
              <span style={"pubkey" in userKey ? monoValue : { ...monoValue, color: color.red }}>
                {"pubkey" in userKey ? shortKey(userKey.pubkey) : userKey.error}
              </span>
            }
          />
        ) : null}
        {showCustody && identity ? (
          <>
            <InfoRow
              label="Identity lock"
              value={
                <span style={monoValue}>
                  {identity.state === "locked"
                    ? "Locked"
                    : identity.state === "unlocked"
                      ? "Unlocked"
                      : "Not password-protected"}
                </span>
              }
            />
            {identity.state === "locked" && (
              <ControlRow
                title="Unlock identity"
                desc="Verify your password to use this identity for signing this session."
                control={
                  panel === "unlock" ? (
                    <HoverButton
                      ariaLabel="Cancel unlock"
                      onClick={closeCustodyPanel}
                      hoverBg={color.titlebar}
                      disabled={busy}
                      style={outlineButton}
                    >
                      Cancel
                    </HoverButton>
                  ) : (
                    <HoverButton
                      onClick={() => transitionPanel("unlock")}
                      hoverBg={color.titlebar}
                      disabled={busy}
                      style={outlineButton}
                    >
                      Unlock
                    </HoverButton>
                  )
                }
              />
            )}
            {panel === "unlock" && (
              <CustodyPanel>
                <PasswordForm
                  mode="confirm"
                  busy={busy}
                  error={identityError}
                  submitLabel={busy ? "Unlocking…" : "Unlock"}
                  onSubmit={handleUnlock}
                />
              </CustodyPanel>
            )}
            {identity.state === "unlocked" && (
              <ControlRow
                title="Lock identity"
                desc="Drop the cached password — the next signing action needs it again."
                control={
                  <HoverButton
                    onClick={handleLock}
                    hoverBg={color.titlebar}
                    disabled={busy}
                    style={outlineButton}
                  >
                    {busy ? "Locking…" : "Lock"}
                  </HoverButton>
                }
              />
            )}
            {identity.state === "plaintext" && (
              <ControlRow
                title="Set a password"
                desc="Encrypt this identity key at rest so a stolen device can't sign as you."
                control={
                  panel === "setPassword" ? (
                    <HoverButton
                      ariaLabel="Cancel set password"
                      onClick={closeCustodyPanel}
                      hoverBg={color.titlebar}
                      disabled={busy}
                      style={outlineButton}
                    >
                      Cancel
                    </HoverButton>
                  ) : (
                    <HoverButton
                      onClick={() => transitionPanel("setPassword")}
                      hoverBg="#38362e"
                      disabled={busy}
                      style={darkButton}
                    >
                      Set password
                    </HoverButton>
                  )
                }
              />
            )}
            {panel === "setPassword" && (
              <CustodyPanel>
                <PasswordForm
                  mode="set"
                  busy={busy}
                  error={identityError}
                  submitLabel={busy ? "Securing…" : "Set password"}
                  onSubmit={handleSetPassword}
                />
              </CustodyPanel>
            )}
            <ControlRow
              title="Recovery phrase"
              desc="View your 24-word backup phrase. Always requires your password."
              last={panel !== "reveal"}
              control={
                panel === "reveal" ? (
                  <HoverButton
                    ariaLabel="Cancel reveal"
                    onClick={closeCustodyPanel}
                    hoverBg={color.titlebar}
                    disabled={busy}
                    style={outlineButton}
                  >
                    Cancel
                  </HoverButton>
                ) : (
                  <HoverButton
                    onClick={handleRevealClick}
                    hoverBg={color.titlebar}
                    disabled={busy}
                    style={outlineButton}
                  >
                    Reveal recovery phrase
                  </HoverButton>
                )
              }
            />
            {panel === "reveal" && (
              <CustodyPanel last>
                {mnemonic ? (
                  <MnemonicGrid mnemonic={mnemonic} onContinue={closeCustodyPanel} continueLabel="Done" />
                ) : (
                  <PasswordForm
                    mode="confirm"
                    busy={busy}
                    error={identityError}
                    submitLabel={busy ? "Revealing…" : "Reveal"}
                    onSubmit={handleRevealSubmit}
                  />
                )}
              </CustodyPanel>
            )}
          </>
        ) : null}
      </GroupCard>
    </>
  );
}
