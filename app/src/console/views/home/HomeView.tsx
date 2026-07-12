// The account-centric Home — a shell-level layer (gated by state.atHome, not a
// disconnect) that IS the person's surface: profile, the workspace table you
// switch networks from, this device's Touch ID + linked keys, and machine
// custody. Re-parented whole from the old in-shell Account view; the cards are
// unchanged except that the workspace table replaces the old Nodes card's
// machine-workspace list.
//
// Account data is chain-scoped (the identity module lives on each network), so
// the account cards read the CONNECTED workspace's projections and an honest
// banner says so when nothing is connected; custody, the Touch ID row, and the
// workspace table are machine-scoped and always render.

import { useCallback, useEffect, useState } from "react";

import { normalizeKey } from "../../../domain/names";
import { identityState } from "../../../domain/user-identity-client";
import type { IdentityStateReport } from "../../../domain/user-identity-client";
import { isDesktop } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
import { CustodyCard } from "./CustodyCard";
import { DevicesCard } from "./DevicesCard";
import { NetworkNodesCard } from "./NetworkNodesCard";
import { ProfileCard } from "./ProfileCard";
import { WorkspacesTable } from "./WorkspacesTable";

export function HomeView() {
  const { state } = useDucktape();

  // One custody fetch feeds the custody card AND the devices card's this-device
  // markers. Mutations inside those cards re-fetch through the callback.
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
      data-screen-label="Home"
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
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Home</div>

      <div style={{ width: "100%", maxWidth: 720, alignSelf: "center" }}>
        <ProfileCard accountId={accountId} />

        <WorkspacesTable />

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
            Account data lives on each network — enter a workspace above to see
            this account&apos;s keys and standing there. Device custody below
            always works.
          </div>
        )}

        <NetworkNodesCard accountId={accountId} />

        <DevicesCard accountId={accountId} identity={identity} />

        <CustodyCard
          identity={identity}
          fetchError={identityFetchError}
          onChanged={refreshIdentity}
        />

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
