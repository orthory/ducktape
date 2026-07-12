// Console root: provider (owns the node transport) → window frame → identity
// gate → body. The identity gate is machine-scoped and orthogonal to any
// workspace/node, so it renders AHEAD of everything else (desktop only, driven
// by its own boot fetch — see IdentityGate.tsx's header for why it doesn't
// thread through the store like `needsOnboarding` below does). Once it
// resolves (or on web, where it never gates), the body is one of three
// surfaces: the onboarding gate (no active workspace), the join waiting room
// (a workspace whose node is still parking), or the shell.

import { DucktapeProvider } from "./store/DucktapeProvider";
import { hasNodeContext } from "./store/state";
import { useDucktape } from "./store/use-ducktape";
import { ConsoleShell } from "./layout/ConsoleShell";
import { WindowFrame } from "./layout/WindowFrame";
import { HomeView } from "./views/home/HomeView";
import { IdentityGate } from "./views/onboarding/IdentityGate";
import { OnboardingGate } from "./views/onboarding/OnboardingGate";
import { JoinProgress } from "./views/onboarding/JoinProgress";
import { NodeFailed } from "./views/onboarding/NodeFailed";

function ConsoleBody() {
  const { state } = useDucktape();
  if (state.needsOnboarding) return <OnboardingGate />;
  // The account-centric Home sits AHEAD of the shell but is NOT a disconnect
  // (see state.atHome / goHome). With a workspace/node context it renders as
  // the shell's body layer — the sidebar must stay navigable from Home — and
  // owns the window only when nothing is connected (smart boot with no active
  // workspace), where a rail with nothing to show would be dead chrome.
  if (state.atHome) {
    return hasNodeContext(state) ? <ConsoleShell /> : <HomeView />;
  }
  // a managed node that failed to start gets a dedicated, actionable body with
  // the real reason + Retry — never a hollow disconnected shell (see BootError).
  if (state.bootError) return <NodeFailed />;
  if (state.onboardingPhase) return <JoinProgress />;
  return <ConsoleShell />;
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
