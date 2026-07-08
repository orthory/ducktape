// The PR merge box: advisory review tallies + the client-computed merge.
// Merging is the desktop three-step the module contract prescribes —
// forge_build_merge builds the merge commit + pack locally (conflicts are
// terminal: nothing is built), uploadMergePack stages the pack bytes on the
// node's blob plane, and actions.mergeForgePr submits the CAS'd MergePr op
// pinned to both branch heads. Heads come from state.forgeBranches (ListRefs).

import { useEffect, useMemo, useState } from "react";

import { uploadMergePack } from "../../../../domain/forge-client";
import type { ForgeItemDetail } from "../../../../domain/forge-client";
import { forgeBuildMerge, isForgeGitAvailable } from "../../../../domain/forge-git-client";
import { useDucktape } from "../../../store/use-ducktape";
import { color, font, radius } from "../../../theme/tokens";
import { ActionButton, errMsg, panelLabel, shortHash, statusTone } from "../ui";
import { itemNumber, stateTone } from "./shared";

/** Decode the bridge's hex-encoded pack into the bytes putBlob wants. */
function hexToBytes(hex: string): Uint8Array {
  const clean = hex.trim();
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

export function MergeBox({
  repo,
  detail,
  onChanged,
}: {
  repo: string;
  detail: ForgeItemDetail;
  onChanged: () => void;
}) {
  const { state, actions, transport } = useDucktape();
  const [busy, setBusy] = useState(false);
  const [conflicts, setConflicts] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const targetBranch = detail.target_branch || "main";
  const sourceBranch = detail.source_branch ?? "";

  // Branch heads are per-screen store data; refresh on mount so the CAS pins
  // the heads the user is actually looking at.
  useEffect(() => {
    void actions.loadForgeBranches(repo);
    // actions is a stable facade; repo is the only real dependency.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo]);

  const targetHead = useMemo(
    () => state.forgeBranches.find((b) => b.name === targetBranch)?.head ?? null,
    [state.forgeBranches, targetBranch],
  );
  const sourceHead = useMemo(
    () => state.forgeBranches.find((b) => b.name === sourceBranch)?.head ?? null,
    [state.forgeBranches, sourceBranch],
  );

  const approvals = detail.reviews.filter((r) => r.verdict === "approve").length;
  const changeRequests = detail.reviews.filter((r) => r.verdict === "request_changes").length;

  if (detail.state !== "open") {
    const merged = detail.state === "merged";
    const tone = merged ? stateTone.merged : stateTone.closed;
    return (
      <div
        style={{
          marginTop: 20,
          border: `1px solid ${tone.border}`,
          borderRadius: radius.md,
          background: tone.bg,
          padding: "12px 15px",
        }}
      >
        <div style={{ font: `600 13px ${font.sans}`, color: tone.text }}>
          {merged ? "Merged" : "Closed"}
        </div>
        <div style={{ marginTop: 4, font: `400 11.5px ${font.sans}`, color: color.muted3 }}>
          {merged
            ? `This pull request was merged as ${shortHash(detail.merge_oid)} into ${targetBranch}.`
            : "This pull request was closed without merging."}
        </div>
      </div>
    );
  }

  const merge = () => {
    if (!targetHead || !sourceHead || !transport || busy) return;
    setBusy(true);
    setError(null);
    setConflicts(null);
    forgeBuildMerge(
      repo,
      targetHead,
      sourceHead,
      `Merge pull request ${itemNumber(detail.number)} from ${sourceBranch}`,
    )
      .then((result) => {
        if (result.conflicts.length > 0 || !result.mergeOid || !result.packHex) {
          setConflicts(result.conflicts);
          return;
        }
        const { mergeOid } = result;
        return uploadMergePack(transport, hexToBytes(result.packHex))
          .then((packDigest) =>
            Promise.resolve(
              actions.mergeForgePr({
                repo,
                number: detail.number,
                prevTargetOid: targetHead,
                expectedSourceOid: sourceHead,
                mergeOid,
                packDigest,
              }),
            ),
          )
          .then(() => onChanged());
      })
      .catch((e) => setError(errMsg(e)))
      .finally(() => setBusy(false));
  };

  const close = () => {
    if (busy) return;
    setBusy(true);
    Promise.resolve(actions.setForgeItemState({ repo, number: detail.number, open: false }))
      .then(() => onChanged())
      .catch((e) => setError(errMsg(e)))
      .finally(() => setBusy(false));
  };

  const desktop = isForgeGitAvailable();
  const headsKnown = targetHead !== null && sourceHead !== null;
  const canMerge = desktop && headsKnown && transport != null;

  return (
    <div
      style={{
        marginTop: 20,
        border: `1px solid ${color.border}`,
        borderRadius: radius.md,
        background: color.sidebar,
        padding: "13px 15px",
      }}
    >
      <div style={panelLabel}>MERGE</div>
      <div style={{ marginTop: 7, font: `400 12px ${font.sans}`, color: color.muted3 }}>
        <span style={{ color: approvals > 0 ? color.green : color.muted2, fontWeight: 600 }}>
          {approvals} approval{approvals === 1 ? "" : "s"}
        </span>
        {" · "}
        <span style={{ color: changeRequests > 0 ? color.red : color.muted2, fontWeight: 600 }}>
          {changeRequests} change request{changeRequests === 1 ? "" : "s"}
        </span>
        <span style={{ color: color.muted2 }}> — advisory, not blocking</span>
      </div>
      <div style={{ marginTop: 6, font: `400 11px ${font.mono}`, color: color.muted2 }}>
        {sourceBranch} ({shortHash(sourceHead)}) → {targetBranch} ({shortHash(targetHead)})
      </div>

      {conflicts && conflicts.length > 0 && (
        <div
          style={{
            marginTop: 10,
            border: `1px solid ${statusTone.danger.border}`,
            borderRadius: radius.sm,
            background: statusTone.danger.bg,
            padding: "9px 11px",
          }}
        >
          <div style={{ font: `600 11.5px ${font.sans}`, color: statusTone.danger.text }}>
            This branch has conflicts that must be resolved
          </div>
          <div style={{ marginTop: 5, font: `400 11px ${font.mono}`, color: statusTone.danger.text }}>
            {conflicts.map((path) => (
              <div key={path}>{path}</div>
            ))}
          </div>
          <div style={{ marginTop: 6, font: `400 11px ${font.sans}`, color: color.muted3 }}>
            Resolve locally and push the source branch again.
          </div>
        </div>
      )}
      {error && (
        <div style={{ marginTop: 10, font: `500 11px ${font.sans}`, color: color.red }}>{error}</div>
      )}
      {!desktop && (
        <div style={{ marginTop: 10, font: `400 11px ${font.sans}`, color: color.muted2 }}>
          Merging builds the merge commit locally — available in the desktop app only.
        </div>
      )}

      <div style={{ marginTop: 12, display: "flex", alignItems: "center", gap: 9 }}>
        <ActionButton
          label={busy ? "Working..." : "Merge pull request"}
          onClick={merge}
          disabled={!canMerge || busy || (conflicts !== null && conflicts.length > 0)}
          strong
        />
        <ActionButton label="Close pull request" onClick={close} disabled={busy} tone="danger" />
      </div>
    </div>
  );
}
