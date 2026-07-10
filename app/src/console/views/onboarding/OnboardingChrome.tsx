// The shared first-run chrome: a centered column carrying the three-step rail
// (Account → Workspace → Connect) above whatever card the current stage
// renders. The SAME screens double as returning-user gates and the workspace
// switcher — there `step` is null and the rail stays hidden, because a
// numbered first-run stepper would lie to someone who is merely unlocking or
// switching. JoinProgress renders StepRail inside its own (wider) column
// instead of this wrapper.

import type { ReactNode } from "react";

import { color, font, tint } from "../../theme/tokens";

const STEP_LABELS = ["Account", "Workspace", "Connect"] as const;

export function StepRail({ active }: { active: 1 | 2 | 3 }) {
  return (
    <div
      role="list"
      aria-label="Onboarding steps"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
      }}
    >
      {STEP_LABELS.map((label, i) => {
        const step = (i + 1) as 1 | 2 | 3;
        const done = step < active;
        const current = step === active;
        return (
          <div
            key={label}
            role="listitem"
            style={{ display: "flex", alignItems: "center", gap: 9 }}
          >
            <span
              aria-hidden="true"
              style={{
                width: 17,
                height: 17,
                borderRadius: "50%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                background: current ? color.dark : done ? tint(color.green).bg : "transparent",
                border: `1px solid ${current ? color.dark : done ? tint(color.green).border : color.borderStrong}`,
                font: `600 9px ${font.mono}`,
                color: current ? color.onDark : done ? tint(color.green).text : color.muted2,
              }}
            >
              {done ? "✓" : step}
            </span>
            <span
              style={{
                font: `600 10.5px ${font.sans}`,
                letterSpacing: ".03em",
                color: current ? color.ink : color.muted2,
              }}
            >
              {label}
            </span>
            {step < STEP_LABELS.length && (
              <span
                aria-hidden="true"
                style={{ width: 22, height: 1, background: color.borderStrong }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

export function OnboardingChrome({
  step,
  children,
}: {
  step: 1 | 2 | 3 | null;
  children: ReactNode;
}) {
  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 18,
        background: color.paper,
        padding: 24,
        overflowY: "auto",
      }}
    >
      {step !== null && <StepRail active={step} />}
      {children}
    </div>
  );
}
