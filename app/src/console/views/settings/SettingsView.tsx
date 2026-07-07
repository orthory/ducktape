// The Settings screen: composition only. Sections own their content; shared
// row/card primitives live in parts.tsx. Everything a module view owns
// (membership → Members, daemon + ops facts → Node) lives THERE — Settings
// keeps identity, custody, preferences, and workspace lifecycle.

import { color, font } from "../../theme/tokens";
import { DangerZone } from "./DangerZone";
import { DevicesSection } from "./DevicesSection";
import { IdentityCard } from "./IdentityCard";
import { SectionLabel } from "./parts";
import { PreferencesSection } from "./PreferencesSection";
import { WorkspaceSection } from "./WorkspaceSection";

export function SettingsView() {
  return (
    <div
      data-screen-label="Settings"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: "#fcfcfc",
        padding: 22,
        overflowY: "auto",
      }}
    >
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
        Settings
      </div>

      <div style={{ maxWidth: 600 }}>
        <SectionLabel marginTop={18}>YOUR IDENTITY</SectionLabel>
        <IdentityCard />

        <DevicesSection />

        <PreferencesSection />

        <WorkspaceSection />

        <DangerZone />

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
