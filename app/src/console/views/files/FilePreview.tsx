// The selected-file panel: authoritative meta (size/exec/kind/object/meta), a
// capped text preview fetched with a single `read`, and a download that pages
// the whole file with `readAll`. A historical snapshot is read-only (no delete).

import { useEffect, useRef, useState } from "react";

import { base64ToBytes, readAll, read } from "../../../domain/files-client";
import type { FileEntry } from "../../../domain/files-client";
import type { NodeTransport } from "../../../domain/transport";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";
import { errMsg, humanBytes, shortHash } from "./files-format";

/** How many bytes the inline text preview fetches — one `read` page, well under
 *  the module's 1 MiB cap; larger files are download-only for preview. */
const PREVIEW_BYTES = 64 * 1024;

/** A file is treated as binary (no text preview) if its head holds a NUL. */
const looksBinary = (bytes: Uint8Array): boolean => bytes.some((b) => b === 0);

function MetaRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: "flex", gap: 10, alignItems: "baseline" }}>
      <span style={{ width: 64, flexShrink: 0, font: `600 10.5px ${font.sans}`, color: color.muted2 }}>
        {label}
      </span>
      <span
        style={{
          font: `400 11.5px ${mono ? font.mono : font.sans}`,
          color: color.muted3,
          wordBreak: "break-all",
        }}
      >
        {value}
      </span>
    </div>
  );
}

export function FilePreview({
  transport,
  entry,
  snapshot,
  readOnly,
  deleting,
  onClose,
  onDelete,
}: {
  transport: NodeTransport;
  entry: FileEntry;
  snapshot: string | null;
  readOnly: boolean;
  deleting: boolean;
  onClose: () => void;
  onDelete: () => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const [binary, setBinary] = useState(false);
  const [truncated, setTruncated] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const req = useRef(0);

  // Fetch a capped preview whenever the target entry (or snapshot) changes.
  useEffect(() => {
    const token = ++req.current;
    setText(null);
    setBinary(false);
    setTruncated(false);
    setError(null);
    setLoading(true);
    setConfirming(false);
    read(transport, {
      path: entry.path,
      snapshot: snapshot ?? undefined,
      offset: 0,
      len: PREVIEW_BYTES,
    })
      .then((range) => {
        if (req.current !== token) return;
        const bytes = base64ToBytes(range.b64);
        if (looksBinary(bytes)) {
          setBinary(true);
        } else {
          setText(new TextDecoder("utf-8", { fatal: false }).decode(bytes));
          setTruncated(!range.eof);
        }
        setLoading(false);
      })
      .catch((err) => {
        if (req.current !== token) return;
        setError(errMsg(err));
        setLoading(false);
      });
  }, [transport, entry.path, snapshot]);

  const handleDownload = () => {
    if (downloading) return;
    const objectUrl = transport.filesObjectUrl?.({
      path: entry.path,
      snapshot: snapshot ?? undefined,
      size: entry.size,
    });
    if (objectUrl) {
      const a = document.createElement("a");
      a.href = objectUrl;
      a.download = entry.path.split("/").pop() || "download";
      a.click();
      return;
    }
    setDownloading(true);
    readAll(transport, { path: entry.path, snapshot: snapshot ?? undefined })
      .then((bytes) => {
        const blob = new Blob([bytes], { type: entry.meta.mime || "application/octet-stream" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = entry.path.split("/").pop() || "download";
        a.click();
        URL.revokeObjectURL(url);
      })
      .catch((err) => setError(errMsg(err)))
      .finally(() => setDownloading(false));
  };

  const name = entry.path.split("/").pop() || entry.path;
  const metaEntries = Object.entries(entry.meta);

  return (
    <div
      style={{
        width: 340,
        flexShrink: 0,
        borderLeft: `1px solid ${color.border}`,
        background: color.paper,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "12px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <Icon name="files" size={15} strokeWidth={1.7} />
        <span
          title={entry.path}
          style={{
            flex: 1,
            minWidth: 0,
            font: `600 13px ${font.sans}`,
            color: color.ink,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {name}
        </span>
        <button
          type="button"
          aria-label="Close file panel"
          onClick={onClose}
          style={{
            all: "unset",
            cursor: "pointer",
            color: color.muted2,
            display: "inline-flex",
            padding: 2,
          }}
        >
          <Icon name="close" size={14} strokeWidth={1.9} />
        </button>
      </div>

      <div style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 6 }}>
        <MetaRow label="Size" value={humanBytes(entry.size)} />
        <MetaRow label="Kind" value={entry.exec ? `${entry.kind} · exec` : entry.kind} />
        <MetaRow label="Object" value={shortHash(entry.object)} mono />
        {metaEntries.map(([key, value]) => (
          <MetaRow key={key} label={key} value={value} />
        ))}
      </div>

      <div style={{ display: "flex", gap: 8, padding: "0 14px 12px" }}>
        <button
          type="button"
          aria-label={`Download ${name}`}
          aria-busy={downloading || undefined}
          disabled={downloading}
          onClick={handleDownload}
          style={{
            all: "unset",
            boxSizing: "border-box",
            height: 28,
            padding: "0 12px",
            display: "inline-flex",
            alignItems: "center",
            borderRadius: radius.sm,
            border: `1px solid ${color.borderStrong}`,
            background: downloading ? color.sunken : color.paper,
            color: color.inkSoft,
            cursor: downloading ? "default" : "pointer",
            font: `600 11px ${font.sans}`,
          }}
        >
          {downloading ? "Downloading…" : "Download"}
        </button>
        {!readOnly && (
          <button
            type="button"
            aria-label={confirming ? `Confirm delete ${name}` : `Delete ${name}`}
            aria-busy={deleting || undefined}
            disabled={deleting}
            onClick={() => (confirming ? onDelete() : setConfirming(true))}
            onBlur={() => setConfirming(false)}
            style={{
              all: "unset",
              boxSizing: "border-box",
              height: 28,
              padding: "0 12px",
              display: "inline-flex",
              alignItems: "center",
              borderRadius: radius.sm,
              border: `1px solid ${confirming ? color.dangerBorder : color.borderStrong}`,
              background: confirming ? color.dangerSoft : color.paper,
              color: confirming ? color.danger : color.inkSoft,
              cursor: "pointer",
              font: `600 11px ${font.sans}`,
            }}
          >
            {deleting ? "Deleting…" : confirming ? "Confirm" : "Delete"}
          </button>
        )}
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflow: "auto",
          borderTop: `1px solid ${color.borderSoft}`,
          background: color.sunken,
        }}
      >
        {loading ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            Loading preview…
          </div>
        ) : error ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.danger }}>
            {error}
          </div>
        ) : binary ? (
          <div style={{ padding: 14, font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
            Binary file — download to view.
          </div>
        ) : (
          <pre
            style={{
              margin: 0,
              padding: 14,
              font: `400 11px ${font.mono}`,
              color: color.muted3,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {text}
            {truncated ? "\n\n… preview truncated — download for the full file." : ""}
          </pre>
        )}
      </div>
    </div>
  );
}
