import { color, font } from "../../theme/tokens";
import { SandboxTab } from "../status/SandboxTab";

export function SandboxView() {
  return (
    <div
      data-screen-label="Sandbox"
      data-sandbox-layout="full-width"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
      }}
    >
      <header
        style={{
          flexShrink: 0,
          width: "100%",
          boxSizing: "border-box",
          padding: "20px 22px 16px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <h1 style={{ margin: 0, font: `600 20px ${font.sans}`, color: color.dark }}>Sandbox</h1>
        <p style={{ margin: "5px 0 0", font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
          Choose how this node executes agent work, verify the host, and apply changes with a guarded restart.
        </p>
      </header>

      <main
        style={{
          flex: 1,
          minWidth: 0,
          minHeight: 0,
          width: "100%",
          boxSizing: "border-box",
          overflowY: "auto",
          padding: "18px 22px 22px",
        }}
      >
        <SandboxTab />
      </main>
    </div>
  );
}
