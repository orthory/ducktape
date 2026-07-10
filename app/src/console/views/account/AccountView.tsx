// The Account console — the PERSON's surface, a shell-level screen beside
// Settings: profile (name + account id), machine custody (password/recovery
// phrase), the account's member keys with the device-link ceremony, and its
// nodes. The account↔node split made navigable: nothing here is about "this
// node" (that's the Node page) — it is about you and everything you own.
//
// Account data is chain-scoped (the identity module lives on each network),
// so the account cards read the CONNECTED workspace's projections and an
// honest banner says so when nothing is connected; custody and the local
// workspace list are machine-scoped and always render.

import { useCallback, useEffect, useState } from "react";

import { normalizeKey } from "../../../domain/names";
import { identityState } from "../../../domain/user-identity-client";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { isDesktop } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { CustodyCard } from "./CustodyCard";
import { DeviceKeysCard } from "./DeviceKeysCard";
import { NodesCard } from "./NodesCard";
import { ProfileCard } from "./ProfileCard";

export function AccountView() {
  const { state } = useDucktape();

  // One custody fetch feeds the custody card AND the this-device markers.
  // Mutations inside CustodyCard re-fetch through the callback.
  const [identity, setIdentity] = useState<IdentityStateReport | null>(null);
  const [identityFetchError, setIdentityFetchError] = useState<string | null>(null);
  const refreshIdentity = useCallback(() => {
    if (!isDesktop()) return;
    Promise.resolve()
      .then(() => identityState())
      .then((report) => {
        setIdentity(report);
        setIdentityFetchError(null);
      })
      .catch((err: unknown) => setIdentityFetchError(String(err)));
  }, []);
  useEffect(() => {
    refreshIdentity();
  }, [refreshIdentity]);

  // The account this device's node is bound to — the anchor every
  // account-scoped card reads through.
  const nodeKeyNorm = normalizeKey(state.workspace?.pubkey ?? "");
  const bound = nodeKeyNorm ? state.nodeUsers[nodeKeyNorm] : undefined;
  const accountId = bound?.accountId;

  const disconnected = !state.workspace && !state.nodeUrl;

  return (
    <div
      data-screen-label="Account"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
        padding: 22,
        overflowY: "auto",
      }}
    >
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
        Account
      </div>

      <div style={{ maxWidth: 600 }}>
        <ProfileCard accountId={accountId} />

        {disconnected && (
          <div
            style={{
              marginTop: 12,
              padding: "10px 13px",
              borderRadius: radius.md,
              border: `1px solid ${color.border}`,
              background: color.sunken,
              font: `500 11.5px ${font.sans}`,
              color: color.muted,
              lineHeight: 1.5,
            }}
          >
            Account data lives on each network — connect a workspace to see
            this account's keys and nodes there. Device custody below always
            works.
          </div>
        )}

        <CustodyCard
          identity={identity}
          fetchError={identityFetchError}
          onChanged={refreshIdentity}
        />

        <DeviceKeysCard accountId={accountId} identity={identity} />

        <NodesCard accountId={accountId} />

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
