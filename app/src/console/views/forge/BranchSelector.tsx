// The code/commits branch picker: a dropdown in the repo-picker's visual
// language (pill-shaped trigger + pop menu) over the local repo's
// refs/heads/* — selecting one re-reads tree/log/files at that reference.

import { useState } from "react";

import type { BranchInfo } from "../../../domain/forge-git-client";
import { Icon } from "../../components/Icon";
import { color, font, radius, shadow } from "../../theme/tokens";
import { panelLabel, shortHash } from "./ui";

export function BranchSelector({
  branches,
  current,
  open,
  onToggle,
  onSelect,
}: {
  branches: BranchInfo[];
  /** The branch being browsed (the repo default when none is picked). */
  current: string;
  open: boolean;
  onToggle: () => void;
  onSelect: (branch: string) => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <span style={{ position: "relative", display: "inline-flex" }}>
      <button
        type="button"
        aria-label="Switch branch"
        onClick={onToggle}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          all: "unset",
          cursor: "pointer",
          display: "inline-flex",
          alignItems: "center",
          gap: 6,
          height: 20,
          padding: "0 8px",
          borderRadius: radius.sm,
          border: `1px solid ${hover || open ? color.borderStrong : color.border}`,
          background: hover || open ? color.sunken : color.paper,
          font: `600 10px ${font.mono}`,
          letterSpacing: ".04em",
          color: color.ink,
        }}
      >
        <span style={{ width: 8, height: 8, borderRadius: "50%", background: color.green, flexShrink: 0 }} />
        {current}
        <Icon
          name="chevronRight"
          size={10}
          color="currentColor"
          strokeWidth={2.2}
          style={{ transform: `rotate(${open ? -90 : 90}deg)` }}
        />
      </button>

      {open && (
        <div
          style={{
            position: "absolute",
            left: 0,
            top: 26,
            zIndex: 14,
            width: 250,
            background: color.paper,
            border: `1px solid ${color.borderStrong}`,
            borderRadius: radius.lg,
            boxShadow: shadow.pop,
            padding: 6,
          }}
        >
          <div style={{ ...panelLabel, padding: "6px 9px 4px" }}>BRANCHES - {branches.length}</div>
          {branches.length === 0 && (
            <div style={{ padding: "7px 9px", font: `400 11px ${font.sans}`, color: color.muted2 }}>
              No local branches
            </div>
          )}
          {branches.map((branch) => (
            <BranchMenuItem
              key={branch.name}
              branch={branch}
              active={branch.name === current}
              onSelect={() => onSelect(branch.name)}
            />
          ))}
        </div>
      )}
    </span>
  );
}

function BranchMenuItem({
  branch,
  active,
  onSelect,
}: {
  branch: BranchInfo;
  active: boolean;
  onSelect: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onSelect}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 9,
        width: "100%",
        boxSizing: "border-box",
        padding: "8px 9px",
        borderRadius: radius.sm,
        background: hover || active ? color.panel : "transparent",
      }}
    >
      <span style={{ font: `600 12px ${font.mono}`, color: color.ink }}>{branch.name}</span>
      <span title={branch.head} style={{ marginLeft: "auto", font: `500 10px ${font.mono}`, color: color.muted2 }}>
        {shortHash(branch.head)}
      </span>
    </button>
  );
}
