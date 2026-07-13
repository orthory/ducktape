// The Settings screen: composition only. Sections own their content; shared
// row/card primitives live in parts.tsx. Everything with a canonical home
// elsewhere lives THERE — the person (name, custody, member keys, nodes) on
// the Account view, membership on Members, daemon + ops facts on Node.
// Settings keeps preferences and workspace lifecycle, plus link rows.

import { color, font } from "../../theme/tokens";
import { useDucktape } from "../../store/use-ducktape";
import { isClientMode } from "../../store/state";
import { DangerZone } from "./DangerZone";
import {
  ControlRow,
  GroupCard,
  HoverButton,
  outlineButton,
  SectionLabel,
} from "./parts";
import { PreferencesSection } from "./PreferencesSection";
import { WorkspaceSection } from "./WorkspaceSection";

function AccountLinkSection() {
  const { actions } = useDucktape();
  return (
    <>
      <SectionLabel marginTop={18}>ACCOUNT</SectionLabel>
      <GroupCard>
        <ControlRow
          title="Your account"
          desc="Display name, recovery phrase, linked devices, and your nodes."
          last
          control={
            <HoverButton
              ariaLabel="Open Account"
              onClick={() => actions.goHome()}
              hoverBg={color.titlebar}
              style={outlineButton}
            >
              Open Account
            </HoverButton>
          }
        />
      </GroupCard>
    </>
  );
}

export function SettingsView() {
  const { state } = useDucktape();
  return (
    <div
      data-screen-label="Settings"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.canvas,
        padding: 22,
        overflowY: "auto",
      }}
    >
      <div style={{ font: `600 16px ${font.sans}`, color: color.dark }}>
        Settings
      </div>

      <div data-settings-content="full-width" style={{ width: "100%" }}>
        <AccountLinkSection />

        <PreferencesSection />

        <WorkspaceSection />

        {!isClientMode(state) && <DangerZone />}

        <div style={{ height: 22 }} />
      </div>
    </div>
  );
}
