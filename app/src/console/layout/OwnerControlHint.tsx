// ADR A5: an OWNER whose node's control surface is unreachable gets a one-line
// hint — ownership is a public on-chain fact, so the app may say so. A non-owner
// sees nothing at all (the predicate hides all control chrome). This is the
// remote-owner-with-unreachable-admin case; a local managed node is covered by
// the control predicate's process-plane disjunct.

import { type CSSProperties } from "react";

import { ownerControlUnreachable } from "../store/state";
import { useDucktape } from "../store/use-ducktape";
import { color, font } from "../theme/tokens";

const bar: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 13px",
  background: color.panel,
  color: color.muted,
  font: `500 11px ${font.sans}`,
  flexShrink: 0,
};

export function OwnerControlHint() {
  const { state } = useDucktape();
  if (!ownerControlUnreachable(state)) return null;
  return (
    <div style={bar} title="This node's owner-gated control surface did not answer.">
      <span
        style={{ width: 6, height: 6, borderRadius: "50%", background: color.muted, flexShrink: 0 }}
      />
      <span>You own this node, but its control surface is not reachable.</span>
    </div>
  );
}
