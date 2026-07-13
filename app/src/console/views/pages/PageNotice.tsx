// The strip between the doc header and the canvas: a capped paste, a delete's
// Undo, a restore that could not land. One chrome, so the messages read as one
// rail rather than three ad-hoc bars.

import type { ReactNode } from "react";

import { color, font } from "../../theme/tokens";

export function PageNotice({
  role = "status",
  tone = "neutral",
  children,
}: {
  role?: "status" | "alert";
  tone?: "neutral" | "danger";
  children: ReactNode;
}) {
  return (
    <div
      role={role}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "7px 22px",
        borderBottom: `1px solid ${tone === "danger" ? color.dangerBorder : color.borderSoft}`,
        background: tone === "danger" ? color.dangerSoft : color.sunken,
        color: tone === "danger" ? color.danger : color.muted3,
        font: `500 11.5px ${font.sans}`,
      }}
    >
      {children}
    </div>
  );
}
