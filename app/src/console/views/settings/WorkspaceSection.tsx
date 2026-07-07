// The active workspace's network facts plus the membership controls, exactly
// as they lived in the SettingsView mono-file. Task 3 of the settings
// overhaul slims this to the workspace card + link rows into Members/Node.

import { useState } from "react";

import { LIVE_JOIN_SUPPORTED } from "../../../domain/workspace-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";
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

const workspaceDataDir = (id: string): string => `~/.ducktape/workspaces/${id}`;

const quorumText = (count: number): string => {
  if (count <= 0) return "not exposed";
  const threshold = Math.floor((count * 2) / 3) + 1;
  return `${threshold} of ${count} validator${count === 1 ? "" : "s"}`;
};

function workspaceRole(workspace: {
  founder: boolean;
  member: boolean;
} | null) {
  if (workspace?.founder) {
    return { role: "genesis validator" } as const;
  }
  if (workspace?.member) {
    return { role: "member validator" } as const;
  }
  return { role: "guest" } as const;
}

function InviteBlob({ value }: { value: string }) {
  return (
    <div
      style={{
        padding: "10px 15px 13px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.sunken,
      }}
    >
      <textarea
        readOnly
        rows={2}
        value={value}
        onFocus={(event) => event.currentTarget.select()}
        style={{
          width: "100%",
          boxSizing: "border-box",
          padding: "9px 10px",
          borderRadius: radius.sm,
          border: `1px solid ${color.borderStrong}`,
          background: color.paper,
          font: `500 10.5px ${font.mono}`,
          color: color.inkSoft,
          resize: "vertical",
        }}
      />
    </div>
  );
}

function AdmitControl() {
  const { actions } = useDucktape();
  const [pubkey, setPubkey] = useState("");
  return (
    <div style={{ display: "flex", gap: 7 }}>
      <input
        aria-label="Joiner pubkey"
        value={pubkey}
        placeholder="joiner pubkey"
        onChange={(event) => setPubkey(event.target.value)}
        style={{
          width: 160,
          boxSizing: "border-box",
          padding: "7px 9px",
          borderRadius: radius.sm,
          border: `1px solid ${color.borderStrong}`,
          background: color.sunken,
          font: `500 11px ${font.mono}`,
          color: color.ink,
        }}
      />
      <HoverButton
        onClick={() => {
          actions.admitMember(pubkey);
          setPubkey("");
        }}
        hoverBg={color.titlebar}
        style={outlineButton}
      >
        Admit
      </HoverButton>
    </div>
  );
}

export function WorkspaceSection() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const role = workspaceRole(workspace);
  const validatorCount = state.members.length || (workspace?.member ? 1 : 0);
  const portLine = workspace
    ? `p2p ${workspace.ports.listen} · http ${workspace.ports.http} · rpc ${workspace.ports.rpc}`
    : "not available";

  return (
    <>
      <SectionLabel>NETWORK</SectionLabel>
      <GroupCard>
        <InfoRow
          label="Network name"
          value={
            <span style={{ font: `500 12px ${font.mono}`, color: color.inkSofter }}>
              {workspace?.name ?? "Remote node"}
            </span>
          }
        />
        <InfoRow
          label="Network ID"
          value={
            <span style={monoValue} title={workspace?.chainId}>
              {workspace?.chainId ?? "not available"}
            </span>
          }
        />
        <InfoRow
          label="Data dir"
          value={
            <span style={monoValue}>
              {workspace ? workspaceDataDir(workspace.id) : "not available"}
            </span>
          }
        />
        <InfoRow label="Ports" value={<span style={monoValue}>{portLine}</span>} />
        <InfoRow
          label="Quorum threshold"
          value={<span style={monoValue}>{quorumText(validatorCount)}</span>}
        />
        <InfoRow
          label="Node role"
          value={<span style={monoValue}>{role.role}</span>}
        />
        <ControlRow
          title="Switch workspace"
          desc="Create, join, or select another local workspace."
          last={!LIVE_JOIN_SUPPORTED}
          control={
            <HoverButton
              ariaLabel="Workspaces"
              onClick={actions.newWorkspace}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Workspaces
            </HoverButton>
          }
        />

        {LIVE_JOIN_SUPPORTED && (
          <>
            <ControlRow
              title="Invite a member"
              desc="Reveal a fresh invite blob for this network."
              control={
                <HoverButton
                  onClick={actions.revealInvite}
                  hoverBg="#38362e"
                  disabled={!workspace}
                  style={darkButton}
                >
                  {state.inviteBlob ? "Refresh invite" : "Reveal invite"}
                </HoverButton>
              }
            />
            {state.inviteBlob && <InviteBlob value={state.inviteBlob} />}
            <ControlRow
              title="Admit a joiner"
              desc="Promote a waiting workspace by its public key."
              last
              control={<AdmitControl />}
            />
          </>
        )}
      </GroupCard>
    </>
  );
}
