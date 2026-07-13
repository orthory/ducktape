// The workspace card: the facts that name the active workspace, the switcher,
// and link rows into the operator surfaces that own everything else — Members
// owns invite/admit, Node owns the daemon and its ops facts (ports, data dir,
// quorum). Settings deliberately does NOT duplicate those controls.

import { useDucktape } from "../../store/use-ducktape";
import { isClientMode } from "../../store/state";
import { color, font } from "../../theme/tokens";
import {
  ControlRow,
  GroupCard,
  HoverButton,
  InfoRow,
  monoValue,
  outlineButton,
  SectionLabel,
} from "./parts";

export function WorkspaceSection() {
  const { state, actions } = useDucktape();
  const workspace = state.workspace;
  const clientMode = isClientMode(state);

  return (
    <>
      <SectionLabel>WORKSPACE</SectionLabel>
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
        <ControlRow
          title="Switch workspace"
          desc={
            clientMode
              ? "Connect to another workspace or remote node."
              : "Create, join, or select another local workspace."
          }
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
        {!clientMode && (
          <ControlRow
            title="Members & invites"
            desc="Invite, admit, and manage members from the Members view."
            control={
              <HoverButton
                ariaLabel="Open Members"
                onClick={() => actions.setScreen("members")}
                hoverBg={color.titlebar}
                style={outlineButton}
              >
                Open Members
              </HoverButton>
            }
          />
        )}
        <ControlRow
          title={clientMode ? "Node overview" : "Node & daemon"}
          desc={
            clientMode
              ? "Inspect the connected node's status, version, and committed roots."
              : "Start or stop the daemon and inspect ports, data dir, and quorum from the Node view."
          }
          last={!clientMode}
          control={
            <HoverButton
              ariaLabel="Open Node"
              onClick={() => actions.setScreen("status")}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Open Node
            </HoverButton>
          }
        />
        {clientMode && (
          <ControlRow
            title="Metrics"
            desc="Inspect read-only health and performance metrics from the connected node."
            last
            control={
              <HoverButton
                ariaLabel="Open Metrics"
                onClick={() => actions.setScreen("metrics")}
                hoverBg={color.titlebar}
                style={outlineButton}
              >
                Open Metrics
              </HoverButton>
            }
          />
        )}
      </GroupCard>
    </>
  );
}
