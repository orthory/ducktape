export const INCOMPATIBLE_STATE_SCHEMA_MARKER = "DUCKTAPE_STATE_SCHEMA_INCOMPATIBLE";

export type BootErrorKind = "startup_failure" | "incompatible_workspace";

/** Classify a managed-node boot failure from both the shell error and the
 * daemon log. Startup often reports only a timeout after the child exits, so
 * the stable node marker in daemon.log is the authoritative fallback. */
export function classifyBootError(reason: string, logTail: string): BootErrorKind {
  return reason.includes(INCOMPATIBLE_STATE_SCHEMA_MARKER) ||
    logTail.includes(INCOMPATIBLE_STATE_SCHEMA_MARKER)
    ? "incompatible_workspace"
    : "startup_failure";
}
