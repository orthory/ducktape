// Shared row/card primitives for the Settings sections. Pure presentation —
// no store access; sections compose these and own their own data.

import { useState, type CSSProperties, type ReactNode } from "react";

import { color, font, radius } from "../../theme/tokens";

export const monoValue: CSSProperties = {
  font: `400 12px ${font.mono}`,
  color: color.muted,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  maxWidth: 330,
};

export const smallMono: CSSProperties = {
  font: `400 10.5px ${font.mono}`,
  color: color.muted2,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

export const copyText = (text: string): void => {
  void navigator.clipboard?.writeText(text).catch(() => {});
};

export function SectionLabel({
  children,
  danger,
  marginTop = 20,
}: {
  children: ReactNode;
  danger?: boolean;
  marginTop?: number;
}) {
  return (
    <div
      style={{
        font: `600 9px ${font.mono}`,
        letterSpacing: ".11em",
        color: danger ? color.danger : color.muted2,
        marginTop,
      }}
    >
      {children}
    </div>
  );
}

export function GroupCard({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        overflow: "hidden",
        background: color.paper,
      }}
    >
      {children}
    </div>
  );
}

export function InfoRow({
  label,
  value,
  last,
}: {
  label: string;
  value: ReactNode;
  last?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "13px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <span style={{ font: `500 12.5px ${font.sans}`, color: color.inkSoft }}>
        {label}
      </span>
      <span style={{ marginLeft: "auto", minWidth: 0, textAlign: "right" }}>
        {value}
      </span>
    </div>
  );
}

export function ControlRow({
  title,
  desc,
  control,
  last,
}: {
  title: string;
  desc: string;
  control: ReactNode;
  last?: boolean;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 16,
        padding: "13px 15px",
        borderBottom: last ? undefined : `1px solid ${color.borderSoft}`,
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ font: `500 12.5px ${font.sans}`, color: color.inkSoft }}>
          {title}
        </div>
        <div
          style={{
            font: `400 10.5px ${font.sans}`,
            color: color.muted2,
            marginTop: 1,
            lineHeight: 1.35,
          }}
        >
          {desc}
        </div>
      </div>
      <div style={{ marginLeft: "auto", flexShrink: 0 }}>{control}</div>
    </div>
  );
}

export function HoverButton({
  onClick,
  style,
  hoverBg,
  children,
  ariaLabel,
  disabled,
}: {
  onClick: () => void;
  style: CSSProperties;
  hoverBg: string;
  children: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        ...style,
        cursor: disabled ? "not-allowed" : style.cursor,
        opacity: disabled ? 0.55 : style.opacity,
        background: !disabled && hover ? hoverBg : style.background,
      }}
    >
      {children}
    </button>
  );
}

export const outlineButton: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `500 11.5px ${font.sans}`,
  color: color.muted3,
  border: `1px solid ${color.borderStrong}`,
  borderRadius: 8,
  padding: "7px 13px",
};

export const darkButton: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 11.5px ${font.sans}`,
  color: color.onDark,
  background: color.dark,
  borderRadius: 8,
  padding: "8px 14px",
};
