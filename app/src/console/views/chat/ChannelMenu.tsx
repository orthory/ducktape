// The "…" channel-settings menu in the chat header: rename (inline input) and
// archive / unarchive. The module owner-gates both ops, so a non-owner's submit
// simply fails via the ops ledger — the menu itself is always offered. Archiving
// asks for confirmation (it hides the channel from the rail); unarchiving is a
// direct, reversible toggle.

import { useEffect, useState } from "react";
import type { FormEvent } from "react";

import type { Channel } from "../../../domain/chat-client";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";

function DotsGlyph() {
  return (
    <svg width={15} height={15} viewBox="0 0 24 24" fill="currentColor" stroke="none" aria-hidden>
      <circle cx="5" cy="12" r="1.7" />
      <circle cx="12" cy="12" r="1.7" />
      <circle cx="19" cy="12" r="1.7" />
    </svg>
  );
}

function MenuRow({
  label,
  danger = false,
  onClick,
}: {
  label: string;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <HoverButton
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
      style={{
        display: "flex",
        alignItems: "center",
        width: "100%",
        padding: "7px 9px",
        borderRadius: radius.sm,
        font: `400 12.5px ${font.sans}`,
        color: danger ? color.danger : color.inkSoft,
      }}
      hoverStyle={{ background: danger ? color.dangerSoft : color.hover }}
    >
      {label}
    </HoverButton>
  );
}

export function ChannelMenu({ channel }: { channel: Channel }) {
  const { actions } = useDucktape();
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState(channel.name);
  const [confirmArchive, setConfirmArchive] = useState(false);

  // Escape / outside-click dismiss the dropdown; the click listener attaches one
  // tick late so the click that OPENED the menu doesn't immediately close it (the
  // MessageItem overflow menu uses the same idiom).
  useEffect(() => {
    if (!open) return;
    const dismiss = () => {
      setOpen(false);
      setRenaming(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") dismiss();
    };
    document.addEventListener("keydown", onKey);
    const timer = setTimeout(() => document.addEventListener("click", dismiss), 0);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("click", dismiss);
      clearTimeout(timer);
    };
  }, [open]);

  const submitRename = (event: FormEvent) => {
    event.preventDefault();
    actions.renameChannel(channel.id, draft);
    setOpen(false);
    setRenaming(false);
  };

  return (
    <div style={{ position: "relative", display: "flex" }}>
      <HoverButton
        onClick={(event) => {
          event.stopPropagation();
          setDraft(channel.name);
          setRenaming(false);
          setOpen((value) => !value);
        }}
        title="Channel settings"
        style={{
          width: 26,
          height: 26,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: radius.sm,
          color: color.muted,
        }}
        hoverStyle={{ background: color.hover, color: color.ink }}
      >
        <DotsGlyph />
      </HoverButton>

      {open && (
        <div
          onClick={(event) => event.stopPropagation()}
          style={{
            position: "absolute",
            top: 30,
            right: 0,
            width: 190,
            zIndex: 5,
            background: color.paper,
            border: `1px solid ${color.borderSoft}`,
            borderRadius: radius.md,
            boxShadow: shadow.pop,
            padding: 4,
          }}
        >
          {renaming ? (
            <form onSubmit={submitRename} style={{ padding: 4, display: "flex", flexDirection: "column", gap: 6 }}>
              <input
                autoFocus
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
                placeholder="Channel name"
                style={{
                  width: "100%",
                  boxSizing: "border-box",
                  padding: "6px 9px",
                  borderRadius: radius.sm,
                  border: `1px solid ${color.borderSoft}`,
                  background: color.paper,
                  font: `400 12.5px ${font.sans}`,
                  color: color.ink,
                }}
              />
              <button
                type="submit"
                disabled={!draft.trim() || draft.trim() === channel.name}
                style={{
                  all: "unset",
                  cursor: draft.trim() && draft.trim() !== channel.name ? "pointer" : "not-allowed",
                  textAlign: "center",
                  padding: "6px 0",
                  borderRadius: radius.sm,
                  background: draft.trim() && draft.trim() !== channel.name ? color.dark : color.borderSoft,
                  color: draft.trim() && draft.trim() !== channel.name ? color.onDark : color.muted2,
                  font: `600 12px ${font.sans}`,
                }}
              >
                Rename channel
              </button>
            </form>
          ) : (
            <>
              <MenuRow label="Rename channel" onClick={() => setRenaming(true)} />
              {channel.archived ? (
                <MenuRow
                  label="Unarchive channel"
                  onClick={() => {
                    actions.setChannelArchived(channel.id, false);
                    setOpen(false);
                  }}
                />
              ) : (
                <MenuRow
                  label="Archive channel"
                  danger
                  onClick={() => {
                    setConfirmArchive(true);
                    setOpen(false);
                  }}
                />
              )}
            </>
          )}
        </div>
      )}

      {confirmArchive && (
        <ConfirmDialog
          title="Archive channel"
          confirmLabel="Archive"
          onConfirm={() => {
            actions.setChannelArchived(channel.id, true);
            setConfirmArchive(false);
          }}
          onCancel={() => setConfirmArchive(false)}
        >
          Archiving <strong>{channel.name}</strong> hides it from the channel list and blocks new
          posts, reactions, and huddles. It stays readable under the rail's Archived section, and
          this menu can unarchive it from there.
        </ConfirmDialog>
      )}
    </div>
  );
}
