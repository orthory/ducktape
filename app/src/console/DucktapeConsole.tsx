// Console root: provider (owns the node transport) → window frame → identity
// gate → body. The identity gate is machine-scoped and orthogonal to any
// network/node, so it renders AHEAD of everything else (desktop only, driven by
// its own boot fetch — see IdentityGate.tsx's header). Once it resolves (or on
// web, where it never gates), the body is the app frame: the far-left network
// rail (epic W1) beside the base surface — the account home when nothing is
// connected, otherwise the network shell — with the connect panel as a modal
// over it. A joiner's waiting room and a managed node's boot failure are
// full-screen surfaces that pre-empt the frame.

import { DucktapeProvider } from "./store/DucktapeProvider";
import { hasNodeContext } from "./store/state";
import { useDucktape } from "./store/use-ducktape";
import { ConsoleShell } from "./layout/ConsoleShell";
import { NetworkRail } from "./layout/NetworkRail";
import { WindowFrame } from "./layout/WindowFrame";
import { HomeView } from "./views/home/HomeView";
import { IdentityGate } from "./views/onboarding/IdentityGate";
import { ConnectPanel } from "./views/onboarding/ConnectPanel";
import { JoinProgress } from "./views/onboarding/JoinProgress";
import { NodeFailed } from "./views/onboarding/NodeFailed";

function AppFrame() {
  const { state } = useDucktape();
  // Nothing connected → the account home IS the surface (no module nav to show).
  // With a network/remote context, the shell owns the body (and renders the
  // account home as a layer over the routed screen when state.atHome).
  const base = hasNodeContext(state) ? <ConsoleShell /> : <HomeView />;
  return (
    <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
      {/* The rail frames every build: on web (no local registry) it carries the
          account "me" chip and the single remote seat; the "+" that mints a
          local network is desktop-only (see NetworkRail). */}
      <NetworkRail />
      <div style={{ flex: 1, minWidth: 0, display: "flex", position: "relative" }}>
        {base}
        {/* the add-a-network modal (create/join/remote) floats over whatever
            base is showing — always dismissible, the account home is behind it. */}
        {state.needsOnboarding && <ConnectPanel />}
      </div>
    </div>
  );
}

function ConsoleBody() {
  const { state } = useDucktape();
  // A joiner's live park→promote waiting room and a managed node that failed to
  // start are self-contained, actionable full-screen surfaces — they pre-empt
  // the rail frame.
  if (state.onboardingPhase) return <JoinProgress />;
  if (state.bootError) return <NodeFailed />;
  return <AppFrame />;
}

export function DucktapeConsole() {
  return (
    <DucktapeProvider>
      <WindowFrame>
        <IdentityGate>
          <ConsoleBody />
        </IdentityGate>
      </WindowFrame>
    </DucktapeProvider>
  );
}
