// Typed desktop bridge for the opt-in device-local DuckDNS helper. The web
// build has no machine resolver/trust boundary and must never call these.

import { invoke } from "@tauri-apps/api/core";

export type DuckDnsSnapshot =
  | { state: "inactive" }
  | {
      state: "active";
      workspace_id: string;
      ingress: string;
      names: number;
      lease_millis: number;
    };

export interface DuckDnsInstallation {
  installed: boolean;
  healthy: boolean;
  installation_id: string | null;
  root_certificate: string | null;
  problems: string[];
}

export interface DuckDnsStatus {
  installed: boolean;
  installation: DuckDnsInstallation | null;
  snapshot: DuckDnsSnapshot | null;
  error: string | null;
}

export interface DuckDnsInstallResult {
  installation: DuckDnsInstallation;
  warnings: string[];
}

export const duckDnsStatus = (): Promise<DuckDnsStatus> =>
  invoke<DuckDnsStatus>("duckdns_status");

export const installDuckDns = (): Promise<DuckDnsInstallResult> =>
  invoke<DuckDnsInstallResult>("duckdns_install");

export const repairDuckDns = (): Promise<DuckDnsInstallResult> =>
  invoke<DuckDnsInstallResult>("duckdns_repair");

export const removeDuckDns = (): Promise<DuckDnsInstallResult> =>
  invoke<DuckDnsInstallResult>("duckdns_remove");
