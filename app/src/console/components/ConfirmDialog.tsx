import { useEffect, useId, useState } from "react";
import type { ReactNode } from "react";

import { color, font, radius, shadow } from "../theme/tokens";

function DialogButton({
  children,
  variant = "neutral",
  disabled,
  onClick,
}: {
  children: ReactNode;
  variant?: "neutral" | "primary" | "danger";
  disabled?: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  const filled = variant !== "neutral";
  const activeBg = variant === "danger" ? color.red : color.dark;
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        minWidth: 76,
        height: 32,
        padding: "0 12px",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: radius.sm,
        border: `1px solid ${filled ? activeBg : color.borderStrong}`,
        background: disabled ? color.chip : filled ? activeBg : hover ? color.hover : color.paper,
        color: disabled ? color.muted2 : filled ? color.onDark : color.inkSoft,
        cursor: disabled ? "default" : "pointer",
        font: `600 12px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {children}
    </button>
  );
}

export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  cancelLabel = "Cancel",
  danger = true,
  onConfirm,
  onCancel,
}: {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const titleId = useId();

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 80,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 20,
        background: "rgba(38, 37, 31, 0.18)",
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        style={{
          width: "min(390px, 100%)",
          borderRadius: radius.lg,
          border: `1px solid ${danger ? color.dangerBorder : color.border}`,
          background: color.paper,
          boxShadow: shadow.pop,
          padding: 16,
        }}
      >
        <div id={titleId} style={{ font: `700 15px ${font.sans}`, color: color.dark }}>
          {title}
        </div>
        <div style={{ marginTop: 8, font: `400 12px ${font.sans}`, color: color.muted3, lineHeight: 1.5 }}>
          {children}
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
          <DialogButton onClick={onCancel}>{cancelLabel}</DialogButton>
          <DialogButton variant={danger ? "danger" : "primary"} onClick={onConfirm}>
            {confirmLabel}
          </DialogButton>
        </div>
      </div>
    </div>
  );
}
