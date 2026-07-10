import { useEffect, useState } from "react";

import {
  duckDnsStatus,
  installDuckDns,
  removeDuckDns,
  repairDuckDns,
  type DuckDnsStatus,
} from "../../../domain/duckdns-client";
import { isDesktop } from "../../../domain/workspace-client";
import { color, font } from "../../theme/tokens";
import {
  ControlRow,
  GroupCard,
  HoverButton,
  outlineButton,
  SectionLabel,
} from "./parts";

export function DuckDnsSection() {
  const desktop = isDesktop();
  const [status, setStatus] = useState<DuckDnsStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [detail, setDetail] = useState<string | null>(null);

  useEffect(() => {
    if (!desktop) return;
    let live = true;
    void duckDnsStatus()
      .then((next) => {
        if (live) setStatus(next);
      })
      .catch((error: unknown) => {
        if (live) setDetail(String(error));
      });
    return () => {
      live = false;
    };
  }, [desktop]);

  const installation = status?.installation;
  const installed = status?.installed === true;
  const healthy =
    (installation?.healthy ?? status?.snapshot !== null) && status?.error == null;
  const state = !desktop
    ? "Desktop only"
    : status === null
      ? "Checking…"
      : !installed
        ? "Not installed"
        : healthy
          ? "Installed"
          : "Needs repair";
  const description = installed
    ? status?.snapshot?.state === "active"
      ? `${status.snapshot.names} published name${status.snapshot.names === 1 ? "" : "s"} in the active workspace.`
      : "Device DNS and HTTPS trust are installed; no workspace is currently registered."
    : "Opt in to device-wide HTTPS names for services explicitly published by the active workspace.";

  const run = async (operation: "install" | "repair" | "remove") => {
    if (
      operation === "remove" &&
      !window.confirm(
        "Remove DuckDNS split DNS and its device root certificate? Published .duck names will stop opening on this device.",
      )
    ) {
      return;
    }
    setBusy(true);
    setDetail(null);
    try {
      const result = await (operation === "install"
        ? installDuckDns()
        : operation === "repair"
          ? repairDuckDns()
          : removeDuckDns());
      setDetail(result.warnings.join(" · ") || null);
      setStatus(await duckDnsStatus());
    } catch (error) {
      setDetail(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <SectionLabel>DUCKDNS</SectionLabel>
      <GroupCard>
        <ControlRow
          title={`.duck access — ${state}`}
          desc={description}
          control={
            desktop ? (
              <div style={{ display: "flex", gap: 7 }}>
                {installed && (
                  <HoverButton
                    ariaLabel="Remove DuckDNS"
                    disabled={busy}
                    onClick={() => void run("remove")}
                    hoverBg={color.titlebar}
                    style={outlineButton}
                  >
                    Remove
                  </HoverButton>
                )}
                <HoverButton
                  ariaLabel={installed ? "Repair DuckDNS" : "Install DuckDNS"}
                  disabled={busy}
                  onClick={() => void run(installed ? "repair" : "install")}
                  hoverBg={color.titlebar}
                  style={outlineButton}
                >
                  {busy ? "Working…" : installed ? "Repair" : "Install"}
                </HoverButton>
              </div>
            ) : (
              <span style={{ font: `400 10.5px ${font.sans}`, color: color.muted2 }}>
                Native app
              </span>
            )
          }
        />
        <div
          style={{
            padding: "10px 15px 12px",
            font: `400 10.5px ${font.sans}`,
            lineHeight: 1.45,
            color: detail || status?.error ? color.red : color.muted2,
          }}
        >
          {detail ?? status?.error ??
            ".duck is a private suffix, not an ICANN-reserved TLD; a future public delegation could collide with these local names."}
        </div>
      </GroupCard>
    </>
  );
}
