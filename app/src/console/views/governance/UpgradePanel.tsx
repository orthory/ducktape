// Node-upgrade panel for the governance surface. Reads the `upgrade` module's
// status (current version, the single pending ScheduledUpgrade, and the
// per-validator readiness verdict) and lets an eligible operator AUTHORIZE a
// schedule/cancel — which opens a ScheduleUpgrade / CancelUpgrade governance
// proposal, listed and voted like any other. Governance only SCHEDULES; the
// upgrade arms only once every boundary member has signalled ready (R = n),
// which this panel surfaces but never drives.

import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";

import { Icon } from "../../components/Icon";
import { HoverButton, darkButton, outlineButton } from "../settings/parts";
import { keyHex } from "../../../domain/chat-client";
import { displayNameForKey, shortKey } from "../../../domain/names";
import { status as fetchStatus, type UpgradeStatus } from "../../../domain/upgrade-client";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow, tint } from "../../theme/tokens";

const sectionLabel: CSSProperties = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".1em",
  color: color.muted2,
};

const inputStyle: CSSProperties = {
  boxSizing: "border-box",
  border: `1px solid ${color.borderStrong}`,
  borderRadius: radius.sm,
  background: color.sunken,
  color: color.ink,
  font: `500 11.5px ${font.sans}`,
  padding: "8px 9px",
};

/** Pure gate for the schedule form — the module also enforces monotonicity /
 *  future-height / at-most-one, but validating here keeps the proposal honest.
 *  Returns an error string, or null when the input is submittable. */
export function validateScheduleForm(input: {
  name: string;
  toVersion: number;
  activationHeight: number;
  currentVersion: number;
  currentHeight: number;
}): string | null {
  if (!input.name.trim()) return "Name the upgrade.";
  if (!Number.isSafeInteger(input.toVersion) || input.toVersion <= input.currentVersion) {
    return `Target version must be greater than the current version (${input.currentVersion}).`;
  }
  if (
    !Number.isSafeInteger(input.activationHeight) ||
    input.activationHeight <= input.currentHeight
  ) {
    return `Activation height must be past the current height (${input.currentHeight}).`;
  }
  return null;
}

function Badge({
  label,
  pill,
}: {
  label: string;
  pill: { text: string; bg: string; border: string };
}) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: radius.sm,
        border: `1px solid ${pill.border}`,
        background: pill.bg,
        color: pill.text,
        padding: "3px 8px",
        font: `600 10.5px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </span>
  );
}

function ReadinessRows({ status }: { status: UpgradeStatus }) {
  const { state } = useDucktape();
  const readySet = useMemo(
    () => new Set(status.ready.map((key) => keyHex(key))),
    [status.ready],
  );
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 9 }}>
      {status.members.map((member) => {
        const hex = keyHex(member);
        const ready = readySet.has(hex);
        const label = displayNameForKey(hex, state.authorNames) ?? shortKey(hex);
        return (
          <div
            key={hex}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              font: `500 11px ${font.sans}`,
              color: color.inkSoft,
            }}
          >
            <Icon
              name={ready ? "check" : "node"}
              size={12}
              color={ready ? color.accentAlt2 : color.muted2}
            />
            <span title={hex}>{label}</span>
            <span style={{ marginLeft: "auto", font: `500 10px ${font.mono}`, color: color.muted2 }}>
              {ready ? "ready" : "arming"}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function PendingCard({
  status,
  canPropose,
  onCancel,
}: {
  status: UpgradeStatus;
  canPropose: boolean;
  onCancel: () => void;
}) {
  const pending = status.pending!;
  return (
    <div
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        padding: "12px 13px",
        boxShadow: shadow.card,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 9 }}>
        <span style={{ font: `600 12.5px ${font.sans}`, color: color.ink }}>{pending.name}</span>
        {status.armed ? (
          <Badge label="armed" pill={tint(color.green)} />
        ) : (
          <Badge label="awaiting readiness" pill={tint(color.amber)} />
        )}
      </div>
      <div
        style={{
          marginTop: 7,
          display: "flex",
          gap: 16,
          flexWrap: "wrap",
          font: `500 11px ${font.mono}`,
          color: color.inkSoft,
        }}
      >
        <span>v{status.current_version} → v{pending.to_version}</span>
        <span>activates at #{pending.activation_height.toLocaleString()}</span>
        <span>
          ready {status.ready_count}/{status.member_count}
        </span>
      </div>

      <ReadinessRows status={status} />

      <div style={{ marginTop: 11, display: "flex" }}>
        <span style={{ marginLeft: "auto" }}>
          <HoverButton
            onClick={onCancel}
            disabled={!canPropose}
            ariaLabel="Propose cancel upgrade"
            style={{ ...outlineButton, color: color.danger, border: `1px solid ${color.dangerBorder}` }}
            hoverBg={color.dangerSoft}
          >
            Propose cancel
          </HoverButton>
        </span>
      </div>
    </div>
  );
}

function ScheduleForm({
  status,
  currentHeight,
  onSchedule,
}: {
  status: UpgradeStatus;
  currentHeight: number;
  onSchedule: (params: { name: string; toVersion: number; activationHeight: number }) => void;
}) {
  const [name, setName] = useState("");
  const [toVersion, setToVersion] = useState("");
  const [activationHeight, setActivationHeight] = useState("");
  const [error, setError] = useState<string | null>(null);

  const runSchedule = () => {
    const params = {
      name,
      toVersion: Number(toVersion),
      activationHeight: Number(activationHeight),
    };
    const err = validateScheduleForm({
      ...params,
      currentVersion: status.current_version,
      currentHeight,
    });
    if (err) {
      setError(err);
      return;
    }
    setError(null);
    onSchedule({ name: name.trim(), toVersion: params.toVersion, activationHeight: params.activationHeight });
    setName("");
    setToVersion("");
    setActivationHeight("");
  };
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    runSchedule();
  };

  return (
    <form
      aria-label="Schedule upgrade"
      onSubmit={submit}
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        background: color.paper,
        padding: "12px 13px",
      }}
    >
      <div style={{ font: `600 12.5px ${font.sans}`, color: color.inkSoft }}>Schedule upgrade</div>
      <div style={{ marginTop: 2, font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
        On v{status.current_version}, current height #{currentHeight.toLocaleString()}. Governance
        authorizes; the upgrade arms once every validator signals ready.
      </div>
      <div style={{ display: "flex", gap: 8, marginTop: 10, flexWrap: "wrap" }}>
        <input
          aria-label="Upgrade name"
          value={name}
          placeholder="Upgrade name"
          onChange={(event) => setName(event.target.value)}
          style={{ ...inputStyle, flex: 1, minWidth: 160 }}
        />
        <input
          aria-label="Target version"
          value={toVersion}
          placeholder={`Target version (> ${status.current_version})`}
          inputMode="numeric"
          onChange={(event) => setToVersion(event.target.value)}
          style={{ ...inputStyle, width: 165 }}
        />
        <input
          aria-label="Activation height"
          value={activationHeight}
          placeholder={`Activation height (> ${currentHeight})`}
          inputMode="numeric"
          onChange={(event) => setActivationHeight(event.target.value)}
          style={{ ...inputStyle, width: 185 }}
        />
        <HoverButton
          onClick={runSchedule}
          ariaLabel="Propose upgrade"
          style={{ ...darkButton, display: "inline-flex", alignItems: "center", gap: 7 }}
          hoverBg={color.filledHover}
        >
          <Icon name="plus" size={13} color={color.onDark} />
          Propose
        </HoverButton>
      </div>
      {error ? (
        <div role="alert" style={{ marginTop: 8, color: color.danger, font: `500 10.5px ${font.sans}` }}>
          {error}
        </div>
      ) : null}
    </form>
  );
}

export function UpgradePanel({ canPropose }: { canPropose: boolean }) {
  const { state, actions, transport } = useDucktape();
  const [status, setStatus] = useState<UpgradeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const currentHeight = state.lastBlock ?? 0;

  useEffect(() => {
    if (!transport) return;
    let alive = true;
    fetchStatus(transport)
      .then((next) => {
        if (!alive) return;
        setError(null);
        setStatus(next);
      })
      .catch((e) => {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [transport, state.lastBlock]);

  return (
    <section
      aria-label="Node upgrade"
      style={{
        flexShrink: 0,
        padding: "12px 22px",
        borderBottom: `1px solid ${color.borderSoft}`,
        background: color.paper,
      }}
    >
      <div style={{ ...sectionLabel, display: "flex", alignItems: "center", gap: 7 }}>
        <Icon name="governance" size={13} color={color.muted2} />
        NODE UPGRADE
        {status ? (
          <Badge label={`node v${status.current_version}`} pill={tint(color.green)} />
        ) : null}
      </div>

      {error ? (
        <div style={{ marginTop: 9, font: `500 11px ${font.sans}`, color: color.muted2 }}>
          Upgrade status unavailable: {error}
        </div>
      ) : status === null ? (
        <div style={{ marginTop: 9, font: `400 11px ${font.sans}`, color: color.muted2 }}>
          Loading upgrade status…
        </div>
      ) : status.pending ? (
        <PendingCard
          status={status}
          canPropose={canPropose}
          onCancel={() => actions.proposeCancelUpgrade(status.pending!.name)}
        />
      ) : canPropose ? (
        <ScheduleForm
          status={status}
          currentHeight={currentHeight}
          onSchedule={actions.proposeScheduleUpgrade}
        />
      ) : (
        <div style={{ marginTop: 9, font: `400 11px ${font.sans}`, color: color.muted2 }}>
          No upgrade scheduled. Only an eligible validator can propose one.
        </div>
      )}
    </section>
  );
}
