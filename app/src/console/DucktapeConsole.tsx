// Console root: provider (owns the node transport) → window frame → body. The
// body is one of three surfaces: the onboarding gate (no active workspace),
// the join waiting room (a workspace whose node is still parking), or the shell.

import { DucktapeProvider } from "./store/DucktapeProvider";
import { useDucktape } from "./store/use-ducktape";
import { ConsoleShell } from "./layout/ConsoleShell";
import { WindowFrame } from "./layout/WindowFrame";
import { OnboardingGate } from "./views/onboarding/OnboardingGate";
import { JoinProgress } from "./views/onboarding/JoinProgress";
import { NodeFailed } from "./views/onboarding/NodeFailed";

function ConsoleBody() {
  const { state } = useDucktape();
  if (state.needsOnboarding) return <OnboardingGate />;
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
        <ConsoleBody />
      </WindowFrame>
    </DucktapeProvider>
  );
}
