// A reset button that layers an additive `hoverStyle` on top of its resting
// `style` while the pointer is over it. Inline style objects can't express a
// `:hover` pseudo-class, so this is the JS approximation — every hover
// affordance in the chat surface (toolbar buttons, menu rows, reaction
// pills, channel-rail "+") should route through this one primitive for a
// single consistent hover feel.

import { useState } from "react";
import type { CSSProperties, MouseEventHandler, ReactNode } from "react";

export function HoverButton({
  style,
  hoverStyle,
  onClick,
  onMouseEnter,
  onMouseLeave,
  title,
  disabled,
  children,
}: {
  style?: CSSProperties;
  hoverStyle: CSSProperties;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  onMouseEnter?: MouseEventHandler<HTMLButtonElement>;
  onMouseLeave?: MouseEventHandler<HTMLButtonElement>;
  title?: string;
  disabled?: boolean;
  children: ReactNode;
}) {
  const [hovered, setHovered] = useState(false);

  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={(event) => {
        setHovered(true);
        onMouseEnter?.(event);
      }}
      onMouseLeave={(event) => {
        setHovered(false);
        onMouseLeave?.(event);
      }}
      style={{
        all: "unset",
        boxSizing: "border-box",
        cursor: disabled ? "default" : "pointer",
        ...style,
        ...(hovered && !disabled ? hoverStyle : {}),
      }}
    >
      {children}
    </button>
  );
}
