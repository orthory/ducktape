// The huddle device picker — a small popover over the control bar's "⋯" slot.
// Lists the enumerated mic / camera / speaker options and writes the choice
// through `setDevicePrefs` (persisted + applied to the live session). The
// speaker row is hidden where the runtime can't route output (WebKitGTK / macOS
// WKWebView have no setSinkId) so it's never a dead control.
//
// Split in two: `DevicesMenuView` is store-free (options + prefs + onChange as
// props) so the popped-out huddle window — which runs OUTSIDE the store/provider
// — can reuse the exact same picker against its own session; `DevicesMenu` is
// the store-connected wrapper the in-app dock/stage use.

import { useEffect } from "react";
import type { CSSProperties } from "react";

import { canSelectSpeaker } from "../../../domain/media-devices";
import type { DevicePrefs, HuddleDevices, MediaDeviceOption } from "../../../domain/media-devices";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

const rowLabel: CSSProperties = {
  font: `600 10px ${font.sans}`,
  color: color.muted2,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};

const select: CSSProperties = {
  width: "100%",
  marginTop: 3,
  padding: "5px 7px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderSoft}`,
  background: color.sunken,
  color: color.ink,
  font: `500 12px ${font.sans}`,
};

function DeviceRow({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: MediaDeviceOption[];
  value: string | undefined;
  onChange: (id: string | undefined) => void;
}) {
  return (
    <label style={{ display: "block" }}>
      <span style={rowLabel}>{label}</span>
      <select
        style={select}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || undefined)}
      >
        <option value="">System default</option>
        {options.map((o) => (
          <option key={o.deviceId} value={o.deviceId}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/** The store-free picker: options + current prefs in, a merged patch out. Used
 *  directly by the popped window and wrapped by `DevicesMenu` for the dock. */
export function DevicesMenuView({
  options,
  prefs,
  onChange,
  onClose,
}: {
  options: HuddleDevices;
  prefs: DevicePrefs;
  onChange: (patch: Partial<DevicePrefs>) => void;
  onClose: () => void;
}) {
  const showSpeaker = canSelectSpeaker();
  return (
    <div
      role="menu"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 9,
        padding: 10,
        borderRadius: radius.md,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        boxShadow: "0 6px 20px rgba(40,38,34,.14)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ font: `600 11.5px ${font.sans}`, color: color.ink }}>Devices</span>
        <button
          type="button"
          onClick={onClose}
          title="Close"
          style={{ border: "none", background: "transparent", color: color.muted2, cursor: "pointer", font: `600 13px ${font.sans}` }}
        >
          ×
        </button>
      </div>

      <DeviceRow label="Microphone" options={options.mics} value={prefs.micId} onChange={(micId) => onChange({ micId })} />
      <DeviceRow label="Camera" options={options.cameras} value={prefs.cameraId} onChange={(cameraId) => onChange({ cameraId })} />
      {showSpeaker && (
        <DeviceRow label="Speaker" options={options.speakers} value={prefs.speakerId} onChange={(speakerId) => onChange({ speakerId })} />
      )}
    </div>
  );
}

export function DevicesMenu({ onClose }: { onClose: () => void }) {
  const { state, actions } = useDucktape();
  const { deviceOptions, devicePrefs } = state;

  // Re-enumerate when the menu opens — labels populate only after a media grant,
  // and devices can hot-plug.
  useEffect(() => {
    actions.refreshDevices();
  }, [actions]);

  return (
    <DevicesMenuView
      options={deviceOptions}
      prefs={devicePrefs}
      onChange={(patch) => actions.setDevicePrefs({ ...devicePrefs, ...patch })}
      onClose={onClose}
    />
  );
}
