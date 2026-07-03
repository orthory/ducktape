import type { CSSProperties } from "react";

import { color } from "../../theme/tokens";

export function Toggle({
  on,
  onToggle,
  label,
  disabled,
}: {
  on: boolean;
  onToggle: () => void;
  label: string;
  disabled?: boolean;
}) {
  const track: CSSProperties = {
    all: "unset",
    cursor: disabled ? "not-allowed" : "pointer",
    width: 38,
    height: 22,
    borderRadius: 11,
    background: on ? color.dark : "#d4d4d4",
    position: "relative",
    transition: "background .15s",
    opacity: disabled ? 0.5 : 1,
  };
  const knob: CSSProperties = {
    position: "absolute",
    top: 2,
    left: on ? 18 : 2,
    width: 18,
    height: 18,
    borderRadius: "50%",
    background: color.paper,
    boxShadow: "0 1px 3px rgba(0,0,0,.2)",
    transition: "left .15s",
  };
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={onToggle}
      style={track}
    >
      <span style={knob} />
    </button>
  );
}
