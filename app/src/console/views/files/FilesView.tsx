// The files surface over the node's `files` module — content-addressed
// manifests. Upload chunks + stages bytes into the blob store then commits a
// manifest (consensus write); download reassembles the bytes from the blob
// store and verifies every chunk against the committed manifest before
// handing them to the browser; delete tombstones the manifest (owner-gated).
//
// The chunk BYTES never enter consensus — only the manifest (identity, size,
// ordered chunk digests) does, so this view is a thin shell over
// `actions.uploadFile` / `downloadFile` / `removeFile`.

import { useEffect, useRef, useState } from "react";
import type { ChangeEvent } from "react";

import type { Manifest } from "../../../domain/files-client";
import { FinalizationMark } from "../../components/FinalizationMark";
import { Icon } from "../../components/Icon";
import { opKey } from "../../store/finalization";
import type { OpRecord } from "../../store/finalization";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius, shadow } from "../../theme/tokens";

/** How long an armed delete-confirm or an upload hint stays visible before it
 *  resets on its own (in case the follow-up interaction never lands). */
const CONFIRM_TIMEOUT_MS = 3000;
const UPLOAD_HINT_TIMEOUT_MS = 20_000;

const humanBytes = (n: number): string => {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rendered = unit === 0 ? String(Math.round(value)) : value.toFixed(value < 10 ? 1 : 0);
  return `${rendered} ${units[unit]}`;
};

const shortOwner = (owner: string): string =>
  owner.length > 10 ? `${owner.slice(0, 10)}…` : owner || "—";

function CenterState({
  title,
  detail,
  muted,
}: {
  title: string;
  detail: string;
  muted?: boolean;
}) {
  return (
    <div
      style={{
        minHeight: 280,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: muted ? color.sunken : "#eef5f0",
          color: muted ? color.muted : color.green,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="files" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.55,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

function MimeChip({ mime }: { mime: string }) {
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: radius.sm,
        border: `1px solid ${color.borderSoft}`,
        background: color.sunken,
        color: color.muted3,
        padding: "2px 7px",
        font: `500 10px ${font.mono}`,
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
      {mime || "unknown"}
    </span>
  );
}

function RowButton({
  label,
  ariaLabel,
  busy,
  disabled,
  tone,
  onClick,
  onBlur,
}: {
  label: string;
  ariaLabel: string;
  busy?: boolean;
  disabled?: boolean;
  tone?: "danger";
  onClick: () => void;
  onBlur?: () => void;
}) {
  const [hover, setHover] = useState(false);
  const danger = tone === "danger";
  const isDisabled = Boolean(disabled) || Boolean(busy);

  return (
    <button
      type="button"
      aria-label={ariaLabel}
      aria-busy={busy || undefined}
      disabled={isDisabled}
      onClick={onClick}
      onBlur={onBlur}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        height: 28,
        padding: "0 10px",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        borderRadius: radius.sm,
        border: `1px solid ${danger ? color.dangerBorder : color.borderStrong}`,
        background: danger
          ? color.dangerSoft
          : isDisabled
            ? color.sunken
            : hover
              ? color.hover
              : color.paper,
        color: danger ? color.danger : isDisabled ? color.muted2 : color.inkSoft,
        cursor: isDisabled ? "default" : "pointer",
        font: `600 11px ${font.sans}`,
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
      {label}
    </button>
  );
}

function FileRow({
  manifest,
  downloading,
  op,
  onDownload,
  onDelete,
}: {
  manifest: Manifest;
  downloading: boolean;
  /** The manifest's finalization record — the meta line draws the mark. */
  op: OpRecord | undefined;
  onDownload: () => void;
  onDelete: () => void;
}) {
  const [hover, setHover] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const confirmTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (confirmTimer.current !== null) window.clearTimeout(confirmTimer.current);
    },
    [],
  );

  const resetConfirm = () => {
    if (confirmTimer.current !== null) {
      window.clearTimeout(confirmTimer.current);
      confirmTimer.current = null;
    }
    setConfirming(false);
  };

  const handleDeleteClick = () => {
    if (confirming) {
      resetConfirm();
      onDelete();
      return;
    }
    setConfirming(true);
    if (confirmTimer.current !== null) window.clearTimeout(confirmTimer.current);
    confirmTimer.current = window.setTimeout(() => setConfirming(false), CONFIRM_TIMEOUT_MS);
  };

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        borderBottom: `1px solid ${color.borderSoft}`,
        background: hover ? color.sidebar : "transparent",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 13,
          padding: "13px 16px",
        }}
      >
        <span
          style={{
            width: 30,
            height: 30,
            borderRadius: radius.sm,
            border: `1px solid ${color.border}`,
            background: color.sunken,
            color: color.muted3,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          <Icon name="files" size={15} strokeWidth={1.7} />
        </span>

        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
            <span
              title={manifest.name}
              style={{
                font: `600 14px ${font.sans}`,
                color: color.ink,
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {manifest.name}
            </span>
            <MimeChip mime={manifest.mime} />
          </div>
          <div
            style={{
              marginTop: 5,
              display: "flex",
              alignItems: "center",
              gap: 6,
              flexWrap: "wrap",
              font: `400 11px ${font.mono}`,
              color: color.muted2,
            }}
          >
            <span>{humanBytes(manifest.size)}</span>
            <span>·</span>
            <span>{manifest.chunks.length} chunks</span>
            <span>·</span>
            <span title={manifest.owner}>{shortOwner(manifest.owner)}</span>
            <span>·</span>
            <span>h{manifest.created_at_height}</span>
            <FinalizationMark op={op} />
          </div>
        </div>

        <button
          type="button"
          aria-label={expanded ? `Hide digest for ${manifest.name}` : `Show digest for ${manifest.name}`}
          aria-expanded={expanded}
          onClick={() => setExpanded((v) => !v)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            width: 24,
            height: 24,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: radius.sm,
            color: color.muted2,
            cursor: "pointer",
            flexShrink: 0,
          }}
        >
          <span
            style={{
              display: "inline-flex",
              transform: `rotate(${expanded ? 90 : 0}deg)`,
              transition: "transform 120ms ease",
            }}
          >
            <Icon name="chevronRight" size={13} strokeWidth={1.9} />
          </span>
        </button>

        <RowButton
          label={downloading ? "Downloading…" : "Download"}
          ariaLabel={`Download ${manifest.name}`}
          busy={downloading}
          onClick={onDownload}
        />
        <RowButton
          label={confirming ? "Confirm" : "Delete"}
          ariaLabel={confirming ? `Confirm delete ${manifest.name}` : `Delete ${manifest.name}`}
          tone={confirming ? "danger" : undefined}
          onClick={handleDeleteClick}
          onBlur={resetConfirm}
        />
      </div>

      {expanded ? (
        <div style={{ padding: "0 16px 13px 59px" }}>
          <div
            style={{
              padding: "8px 10px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderSoft}`,
              background: color.sunken,
              font: `400 10.5px ${font.mono}`,
              color: color.muted3,
              wordBreak: "break-all",
            }}
          >
            {manifest.digest}
          </div>
        </div>
      ) : null}
    </div>
  );
}

export function FilesView() {
  const { state, actions } = useDucktape();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [uploadingName, setUploadingName] = useState<string | null>(null);
  const uploadTimer = useRef<number | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [uploadHover, setUploadHover] = useState(false);

  const loading = state.status === null;
  const backed = Boolean(state.status?.modules.some((m) => m.id === "files"));

  useEffect(
    () => () => {
      if (uploadTimer.current !== null) window.clearTimeout(uploadTimer.current);
    },
    [],
  );

  useEffect(() => {
    if (!uploadingName) return;
    if (state.files.some((f) => f.name === uploadingName)) {
      if (uploadTimer.current !== null) {
        window.clearTimeout(uploadTimer.current);
        uploadTimer.current = null;
      }
      setUploadingName(null);
    }
  }, [uploadingName, state.files]);

  const armUploadHint = (name: string) => {
    setUploadingName(name);
    if (uploadTimer.current !== null) window.clearTimeout(uploadTimer.current);
    uploadTimer.current = window.setTimeout(() => {
      setUploadingName(null);
      uploadTimer.current = null;
    }, UPLOAD_HINT_TIMEOUT_MS);
  };

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const input = event.target;
    const file = input.files?.[0] ?? null;
    input.value = "";
    if (!file || !backed) return;
    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    armUploadHint(file.name);
    actions.uploadFile({ name: file.name, mime: file.type || "application/octet-stream", bytes });
  };

  const handleDownload = async (fileId: string) => {
    if (downloadingId) return;
    setDownloadingId(fileId);
    try {
      const res = await actions.downloadFile(fileId);
      if (!res) return;
      const blob = new Blob([res.bytes], { type: res.manifest.mime || "application/octet-stream" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = res.manifest.name;
      a.click();
      URL.revokeObjectURL(url);
    } finally {
      setDownloadingId(null);
    }
  };

  return (
    <div
      data-screen-label="Files"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Files</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {state.files.length}
        </span>

        <div style={{ marginLeft: "auto" }}>
          <input
            ref={fileInputRef}
            type="file"
            onChange={handleFileChange}
            style={{ display: "none" }}
          />
          <button
            type="button"
            disabled={!backed}
            onClick={() => fileInputRef.current?.click()}
            onMouseEnter={() => setUploadHover(true)}
            onMouseLeave={() => setUploadHover(false)}
            style={{
              all: "unset",
              boxSizing: "border-box",
              height: 32,
              padding: "0 13px",
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              borderRadius: radius.sm,
              border: `1px solid ${backed ? color.borderStrong : color.borderSoft}`,
              background: backed ? (uploadHover ? color.hover : color.paper) : color.sunken,
              color: backed ? color.inkSoft : color.muted2,
              cursor: backed ? "pointer" : "default",
              font: `600 12px ${font.sans}`,
              whiteSpace: "nowrap",
            }}
          >
            <Icon name="plus" size={13} strokeWidth={1.9} />
            Upload
          </button>
        </div>
      </div>

      <div
        style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 18, background: color.sidebar }}
      >
        <div
          style={{
            minHeight: "100%",
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            boxShadow: shadow.card,
            overflow: "hidden",
          }}
        >
          {loading ? (
            <CenterState
              title="Loading files…"
              detail="Waiting for this node's committed file manifests."
              muted
            />
          ) : !backed ? (
            <CenterState
              title="Files module is not available"
              detail="This node did not report a files module, so uploads and downloads are disabled."
              muted
            />
          ) : (
            <>
              {uploadingName ? (
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 9,
                    padding: "10px 16px",
                    borderBottom: `1px solid ${color.borderSoft}`,
                    background: color.sunken,
                    font: `500 12px ${font.sans}`,
                    color: color.muted3,
                  }}
                >
                  <Icon name="refresh" size={13} strokeWidth={1.9} />
                  uploading {uploadingName}…
                </div>
              ) : null}

              {state.files.length === 0 ? (
                <CenterState
                  title="No files yet"
                  detail="No files yet — upload one to store it on this node."
                />
              ) : (
                state.files.map((manifest) => (
                  <FileRow
                    key={manifest.file_id}
                    manifest={manifest}
                    downloading={downloadingId === manifest.file_id}
                    op={state.ops[opKey.file(manifest.file_id)]}
                    onDownload={() => handleDownload(manifest.file_id)}
                    onDelete={() => actions.removeFile(manifest.file_id)}
                  />
                ))
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
