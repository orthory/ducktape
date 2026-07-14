// The account's devices, aggregated across every joined network. A "device" in
// this list is a per-network BindNode record (a node); the same physical
// machine has a distinct node key per network. Under the single-active premise
// there is no live cross-network query — the CONNECTED network's rows are read
// live from the store (nodeUsers + valset standing) and captured to the device
// cache on every refresh; every OTHER network renders its last-known cached
// rows, read-only, with a hint to switch there to manage it.
//
// The connected network's group is actionable: rename a device (its on-chain
// label, SetNodeLabel — origin-gated, no signing) and Unbind a lost/retired one
// (UnbindNode, per-network; there is deliberately no bulk "remove everywhere").
// Recovery is one line pointing at the recovery phrase in the custody card
// below — the restore path for bringing your account onto a new device.
//
// Self-contained (W5): mounts in Home today; final placement into W1's account
// home reconciles on the epic branch.

import { useEffect, useMemo, useState } from "react";

import { normalizeKey, sameKey, shortKey } from "../../../domain/names";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import {
  forgetNetworkDevices,
  loadDeviceCache,
  saveNetworkDevices,
  type DeviceRow,
  type NetworkDevices,
} from "../../store/state";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, tint } from "../../theme/tokens";
import { errMessage, errorTextStyle } from "../onboarding/IdentityGateForms";
import {
  GroupCard,
  HoverButton,
  InfoRow,
  monoValue,
  outlineButton,
  SectionLabel,
} from "../settings/parts";
import { CustodyPanel } from "./CustodyCard";

type Standing = DeviceRow["standing"];

function StandingChip({ standing }: { standing: Standing }) {
  const palette =
    standing === "Validator"
      ? { fg: color.onDark, bg: color.dark, bd: color.dark }
      : standing === "Resident"
        ? { fg: tint(color.green).text, bg: tint(color.green).bg, bd: tint(color.green).border }
        : { fg: color.muted2, bg: color.sunken, bd: color.border };
  return (
    <span
      style={{
        font: `600 9px ${font.mono}`,
        color: palette.fg,
        background: palette.bg,
        border: `1px solid ${palette.bd}`,
        borderRadius: 4,
        padding: "2px 6px",
        letterSpacing: ".04em",
        whiteSpace: "nowrap",
      }}
    >
      {standing.toUpperCase()}
    </span>
  );
}

/** Coarse "last seen" for a cached (not-currently-connected) network. */
function timeAgo(at: number): string {
  const secs = Math.max(0, Math.round((Date.now() - at) / 1000));
  if (secs < 60) return "just now";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

const inlineInput: React.CSSProperties = {
  font: `500 12px ${font.sans}`,
  color: color.ink,
  background: color.paper,
  border: `1px solid ${color.border}`,
  borderRadius: radius.sm,
  padding: "5px 7px",
  minWidth: 150,
};

/** One live, actionable device row: rename its label, view standing, unbind. */
function LiveDeviceRow({
  row,
  last,
  onUnbind,
  onLabel,
}: {
  row: DeviceRow;
  last: boolean;
  onUnbind: () => void;
  onLabel: (label: string | null) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(row.label ?? "");

  const startEdit = () => {
    setDraft(row.label ?? "");
    setEditing(true);
  };
  const commit = () => {
    const trimmed = draft.trim();
    setEditing(false);
    if (trimmed !== (row.label ?? "")) onLabel(trimmed.length > 0 ? trimmed : null);
  };

  return (
    <InfoRow
      last={last}
      label={
        row.label ??
        (row.isThisDevice ? "This device" : "Device")
      }
      value={
        <span style={{ display: "inline-flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          {editing ? (
            <input
              autoFocus
              aria-label={`Label for node ${shortKey(row.nodeHex)}`}
              value={draft}
              placeholder="e.g. Kim's laptop"
              maxLength={64}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={commit}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
                if (event.key === "Escape") setEditing(false);
              }}
              style={inlineInput}
            />
          ) : (
            <span style={monoValue}>
              {shortKey(row.nodeHex)}
              {row.isThisDevice ? " · this device" : ""}
            </span>
          )}
          <StandingChip standing={row.standing} />
          {!editing && (
            <HoverButton
              ariaLabel={`Rename node ${shortKey(row.nodeHex)}`}
              onClick={startEdit}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              {row.label ? "Rename" : "Label"}
            </HoverButton>
          )}
          <HoverButton
            ariaLabel={`Unbind node ${shortKey(row.nodeHex)}`}
            onClick={onUnbind}
            hoverBg={color.dangerSoft}
            style={{ ...outlineButton, color: color.red }}
          >
            Unbind
          </HoverButton>
        </span>
      }
    />
  );
}

/** One cached (read-only) device row from a network that isn't connected. */
function CachedDeviceRow({ row, last }: { row: DeviceRow; last: boolean }) {
  return (
    <InfoRow
      last={last}
      label={row.label ?? (row.isThisDevice ? "This device" : "Device")}
      value={
        <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
          <span style={monoValue}>{shortKey(row.nodeHex)}</span>
          <StandingChip standing={row.standing} />
        </span>
      }
    />
  );
}

export function DevicePanel({ accountId }: { accountId: string | undefined }) {
  const { state, actions } = useDucktape();
  const [cache, setCache] = useState<Record<string, NetworkDevices>>(() => ({}));
  const [pendingUnbind, setPendingUnbind] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);

  const workspace = state.workspace;
  const connectedChainId = workspace?.chainId;

  // The connected network's device rows, live from the store.
  const liveRows = useMemo<DeviceRow[]>(() => {
    if (!accountId) return [];
    const validators = new Set(state.members.map(normalizeKey));
    const residents = new Set(state.residents.map(normalizeKey));
    return Object.entries(state.nodeUsers)
      .filter(([, owner]) => owner.accountId === accountId)
      .map(([nodeHex, owner]) => ({
        nodeHex,
        label: owner.label ?? null,
        standing: validators.has(normalizeKey(nodeHex))
          ? ("Validator" as const)
          : residents.has(normalizeKey(nodeHex))
            ? ("Resident" as const)
            : ("No seat" as const),
        isThisDevice: workspace ? sameKey(nodeHex, workspace.pubkey) : false,
      }))
      .sort((a, b) => a.nodeHex.localeCompare(b.nodeHex));
  }, [accountId, state.nodeUsers, state.members, state.residents, workspace]);

  // Capture the connected network's rows as this ACCOUNT's last-known state
  // (only on an actual change), and keep the cache snapshot the render reads
  // fresh. Account keying means a previous identity's rows never surface here.
  useEffect(() => {
    if (!accountId) {
      setCache({});
      return;
    }
    if (workspace && connectedChainId) {
      const prev = loadDeviceCache(accountId)[connectedChainId];
      if (!prev || JSON.stringify(prev.rows) !== JSON.stringify(liveRows)) {
        saveNetworkDevices(accountId, connectedChainId, {
          name: workspace.name,
          at: Date.now(),
          rows: liveRows,
        });
      }
    }
    setCache(loadDeviceCache(accountId));
  }, [accountId, connectedChainId, workspace, liveRows]);

  // Prune cached entries whose workspace is no longer registered (forgotten).
  useEffect(() => {
    if (!accountId) return;
    const known = new Set(state.workspaces.map((w) => w.chainId));
    for (const chainId of Object.keys(cache)) {
      if (chainId !== connectedChainId && !known.has(chainId)) {
        forgetNetworkDevices(accountId, chainId);
      }
    }
  }, [accountId, state.workspaces, cache, connectedChainId]);

  const cachedOthers = Object.entries(cache)
    .filter(([chainId]) => chainId !== connectedChainId)
    .sort((a, b) => b[1].at - a[1].at);

  const hasConnected = Boolean(accountId && workspace);
  // Nothing to show at all: no connected account and nothing cached.
  if (!hasConnected && cachedOthers.length === 0) return null;

  const unbind = (nodeHex: string) => {
    setOpError(null);
    setPendingUnbind(nodeHex);
  };
  const relabel = (nodeHex: string, label: string | null) => {
    setOpError(null);
    // The module caps the label at 64 BYTES; the input's maxLength counts
    // UTF-16 units, so a multibyte label can pass the field yet bounce
    // on-chain. Validate bytes here instead of round-tripping an opError.
    if (label && new TextEncoder().encode(label).length > 64) {
      setOpError("label is too long — keep it under 64 bytes");
      return;
    }
    actions.accountSetNodeLabel(nodeHex, label).catch((err) => setOpError(errMessage(err)));
  };

  return (
    <>
      <SectionLabel>YOUR DEVICES</SectionLabel>

      {hasConnected && (
        <GroupCard>
          <InfoRow
            label={workspace?.name ?? "This network"}
            value={
              <span
                style={{
                  font: `600 9px ${font.mono}`,
                  color: color.onDark,
                  background: color.dark,
                  border: `1px solid ${color.dark}`,
                  borderRadius: 4,
                  padding: "2px 6px",
                  letterSpacing: ".04em",
                }}
              >
                CONNECTED
              </span>
            }
          />
          {liveRows.map((row, i) => (
            <LiveDeviceRow
              key={row.nodeHex}
              row={row}
              last={i === liveRows.length - 1 && !opError}
              onUnbind={() => unbind(row.nodeHex)}
              onLabel={(label) => relabel(row.nodeHex, label)}
            />
          ))}
          {liveRows.length === 0 && (
            <InfoRow
              label="Devices"
              last={!opError}
              value={<span style={monoValue}>none bound yet</span>}
            />
          )}
          {opError && (
            <CustodyPanel last>
              <span style={errorTextStyle}>{opError}</span>
            </CustodyPanel>
          )}
        </GroupCard>
      )}

      {cachedOthers.map(([chainId, net]) => (
        <div key={chainId} style={{ marginTop: 8 }}>
          <GroupCard>
            <InfoRow
              label={net.name}
              value={
                <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted }}>
                  last seen {timeAgo(net.at)}
                </span>
              }
            />
            {net.rows.map((row, i) => (
              <CachedDeviceRow key={row.nodeHex} row={row} last={i === net.rows.length - 1} />
            ))}
            {net.rows.length === 0 && (
              <InfoRow label="Devices" last value={<span style={monoValue}>none bound</span>} />
            )}
          </GroupCard>
          <div
            style={{
              font: `500 10.5px ${font.sans}`,
              color: color.muted,
              padding: "6px 4px 0",
            }}
          >
            Switch to {net.name} to rename or unbind its devices.
          </div>
        </div>
      ))}

      <div
        style={{
          marginTop: 10,
          padding: "9px 12px",
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          font: `500 11px ${font.sans}`,
          color: color.muted,
          lineHeight: 1.5,
        }}
      >
        Lost a device? Unbind it on the network it was on, then reveal your
        recovery phrase below to restore your account on the replacement.
      </div>

      {pendingUnbind && (
        <ConfirmDialog
          title={`Unbind ${shortKey(pendingUnbind)}?`}
          confirmLabel="Unbind device"
          onCancel={() => setPendingUnbind(null)}
          onConfirm={() => {
            const target = pendingUnbind;
            setPendingUnbind(null);
            actions.accountUnbindNode(target).catch((err) => setOpError(errMessage(err)));
          }}
        >
          For a lost or retired device: the node keeps running, but it stops
          being yours — its writes no longer resolve to this account, and any
          captured bind certificates die with the nonce bump. Its validator seat
          (if any) is separate; retire that from the Members view.
        </ConfirmDialog>
      )}
    </>
  );
}
