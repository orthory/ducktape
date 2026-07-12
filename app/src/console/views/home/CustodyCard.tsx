// Recovery & security — the machine-scoped custody state machine, moved whole
// from Settings' old DevicesSection: lock state, unlock / lock, set password,
// reveal recovery phrase. Desktop-only (the account key file lives beside the
// workspaces dir); the identity report and its refresh are owned by
// AccountView so the device-keys card shares one fetch. Reveal ALWAYS
// re-prompts for the password — the session cache is never consulted, by
// design (see revealMnemonic).

import { useState, type ReactNode } from "react";

import type { IdentityStateReport } from "../../../domain/user-identity-client";
import {
  encryptLegacy,
  lockIdentity,
  revealMnemonic,
  unlockIdentity,
} from "../../../domain/user-identity-client";
import { shortKey } from "../../../domain/names";
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
} from "../settings/parts";

/** Which inline custody form/grid (if any) is currently expanded below its
 *  trigger row. Mutually exclusive — only one of unlock/set-password/reveal
 *  is ever mid-flow at a time. */
type CustodyPanelKind = "none" | "unlock" | "setPassword" | "reveal";

/** An inline expando block under a custody trigger row. */
export function CustodyPanel({ children, last }: { children: ReactNode; last?: boolean }) {
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

export function CustodyCard({
  identity,
  fetchError,
  onChanged,
}: {
  identity: IdentityStateReport | null;
  /** The identityState() fetch itself rejected (corrupt/unreadable key file —
   *  it is never overwritten silently, so this is the operator's only
   *  signal). */
  fetchError: string | null;
  /** Re-fetch the report after any mutation below lands. */
  onChanged: () => void;
}) {
  const [panel, setPanel] = useState<CustodyPanelKind>("none");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The just-revealed mnemonic, held only long enough to render the grid —
  // cleared on EVERY panel transition and never persisted anywhere else.
  const [mnemonic, setMnemonic] = useState<string | null>(null);

  // EVERY panel change routes through here so a revealed mnemonic can never
  // survive a panel transition: switching away from reveal drops it, and
  // re-opening reveal always starts back at the password step.
  const transitionPanel = (next: CustodyPanelKind) => {
    setPanel(next);
    setError(null);
    setMnemonic(null);
  };

  const closePanel = () => transitionPanel("none");

  const run = (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    action()
      .then(() => {
        closePanel();
        onChanged();
      })
      .catch((err) => setError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  // Plaintext has no password to verify — the underlying verb tolerates an
  // empty one for a legacy key — so this reveals straight through with no
  // prompt. Locked/unlocked ALWAYS re-prompt.
  const handleRevealClick = () => {
    if (!identity) return;
    setError(null);
    if (identity.state === "plaintext") {
      setBusy(true);
      revealMnemonic("")
        .then((revealed) => {
          transitionPanel("reveal");
          setMnemonic(revealed.mnemonic);
        })
        .catch((err) => setError(errMessage(err)))
        .finally(() => setBusy(false));
      return;
    }
    transitionPanel("reveal");
  };

  const handleRevealSubmit = (password: string) => {
    setBusy(true);
    setError(null);
    revealMnemonic(password)
      .then((revealed) => setMnemonic(revealed.mnemonic))
      .catch((err) => setError(errMessage(err)))
      .finally(() => setBusy(false));
  };

  // "absent" (no account key yet) renders nothing — the account gate is what
  // creates one. A fetch error still shows, in red, as the only signal.
  if (!identity && !fetchError) return null;
  if (identity?.state === "absent") return null;

  return (
    <>
      <SectionLabel>RECOVERY &amp; SECURITY</SectionLabel>
      <GroupCard>
        <InfoRow
          label="Account key (this device)"
          last={!identity}
          value={
            <span style={identity?.pubkey ? monoValue : { ...monoValue, color: color.red }}>
              {identity?.pubkey ? shortKey(identity.pubkey) : fetchError}
            </span>
          }
        />
        {identity && (
          <>
            <InfoRow
              label="Password lock"
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
                title="Unlock account"
                desc="Verify your password to sign with this account for this session."
                control={
                  panel === "unlock" ? (
                    <HoverButton
                      ariaLabel="Cancel unlock"
                      onClick={closePanel}
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
                  error={error}
                  submitLabel={busy ? "Unlocking…" : "Unlock"}
                  onSubmit={(password) => run(() => unlockIdentity(password))}
                />
              </CustodyPanel>
            )}
            {identity.state === "unlocked" && (
              <ControlRow
                title="Lock account"
                desc="Drop the cached password — the next signing action needs it again."
                control={
                  <HoverButton
                    onClick={() => run(() => lockIdentity())}
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
                desc="Encrypt this account key at rest so a stolen device can't sign as you."
                control={
                  panel === "setPassword" ? (
                    <HoverButton
                      ariaLabel="Cancel set password"
                      onClick={closePanel}
                      hoverBg={color.titlebar}
                      disabled={busy}
                      style={outlineButton}
                    >
                      Cancel
                    </HoverButton>
                  ) : (
                    <HoverButton
                      onClick={() => transitionPanel("setPassword")}
                      hoverBg={color.filledHover}
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
                  error={error}
                  submitLabel={busy ? "Securing…" : "Set password"}
                  onSubmit={(password) => run(() => encryptLegacy(password))}
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
                    onClick={closePanel}
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
                  <MnemonicGrid mnemonic={mnemonic} onContinue={closePanel} continueLabel="Done" />
                ) : (
                  <PasswordForm
                    mode="confirm"
                    busy={busy}
                    error={error}
                    submitLabel={busy ? "Revealing…" : "Reveal"}
                    onSubmit={handleRevealSubmit}
                  />
                )}
              </CustodyPanel>
            )}
          </>
        )}
      </GroupCard>
    </>
  );
}
