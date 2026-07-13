// The renderer half of chat attachments: a `duck://files/shared/attachments/…`
// URI in message text becomes a chip (any file) or an inline image preview
// (image extensions). Bytes come from duckfs over the normal read lane —
// nothing here trusts the message text beyond the path shape the tokenizer
// already validated, and the name renders as React TEXT, never markup.
//
// Security posture (audited before merge):
// - previews are extension-gated and rendered ONLY via <img src=blobURL> — an
//   image context executes nothing, whatever the real bytes are; svg (script-
//   bearing by design) is deliberately NOT in the preview set;
// - downloads use application/octet-stream blobs with a download attribute —
//   never navigation, never text/html blob URLs;
// - every object URL is revoked (unmount for previews, finally for downloads).

import { useContext, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

import { readAll, stat } from "../../../domain/files-client";
import type { NodeTransport } from "../../../domain/transport";
import { Icon } from "../../components/Icon";
import { ConsoleContext } from "../../store/context";
import { accentVar, color, font, radius } from "../../theme/tokens";
import type { AttachmentRef } from "./attachments";
import { isImageName } from "./attachments";

/** Previews load at most this many bytes; a larger image renders as a plain
 *  chip (click still downloads). Keeps a scrollback of pasted screenshots
 *  from holding tens of MiB of decoded bitmaps alive. */
const MAX_PREVIEW_BYTES = 8 * 1024 * 1024;

const chipStyle: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 5,
  padding: "1px 7px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderSoft}`,
  background: color.sunken,
  font: `500 12.5px ${font.sans}`,
  color: accentVar,
  verticalAlign: "baseline",
  maxWidth: 260,
};

const nameStyle: CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  minWidth: 0,
};

/** Fetch the attachment and hand it to the browser as a download. The blob is
 *  typed octet-stream regardless of the stored mime — a download must never
 *  become a same-origin document. */
const download = async (transport: NodeTransport, attachment: AttachmentRef) => {
  const bytes = await readAll(transport, { path: attachment.path });
  const url = URL.createObjectURL(
    new Blob([bytes as BlobPart], { type: "application/octet-stream" }),
  );
  try {
    const a = document.createElement("a");
    a.href = url;
    a.download = attachment.name;
    a.click();
  } finally {
    // the click has consumed the URL synchronously; revoke on the next tick
    // to be safe across webview download managers.
    setTimeout(() => URL.revokeObjectURL(url), 1_000);
  }
};

/** An image attachment's inline preview. Loads once on mount, falls back to
 *  the plain chip on any failure (absent path, over-cap, decode error). */
function ImagePreview({
  transport,
  attachment,
}: {
  transport: NodeTransport;
  attachment: AttachmentRef;
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const urlRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        // stat FIRST — never pull a multi-MiB (or hostile, re-pointed) file
        // into memory just to discover it's over the preview cap. A crafted
        // chip pointing at a large existing object costs nothing here.
        const entry = await stat(transport, { path: attachment.path });
        if (cancelled) return;
        if (!entry || entry.kind !== "file" || entry.size > MAX_PREVIEW_BYTES) {
          setFailed(true);
          return;
        }
        const bytes = await readAll(transport, { path: attachment.path });
        if (cancelled) return;
        const next = URL.createObjectURL(new Blob([bytes as BlobPart]));
        urlRef.current = next;
        setUrl(next);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
      if (urlRef.current) {
        URL.revokeObjectURL(urlRef.current);
        urlRef.current = null;
      }
    };
  }, [transport, attachment.path]);

  if (failed) return <FileChip transport={transport} attachment={attachment} />;
  if (!url) {
    return (
      <span style={{ ...chipStyle, color: color.muted2 }}>
        <Icon name="link" size={12} strokeWidth={1.8} />
        <span style={nameStyle}>{attachment.name}</span>
      </span>
    );
  }
  return (
    <button
      type="button"
      title={`Download ${attachment.name}`}
      aria-label={`Download attachment ${attachment.name}`}
      onClick={(event) => {
        event.stopPropagation();
        void download(transport, attachment);
      }}
      style={{ all: "unset", cursor: "pointer", display: "inline-block", verticalAlign: "top" }}
    >
      <img
        src={url}
        alt={attachment.name}
        onError={() => setFailed(true)}
        style={{
          display: "block",
          maxWidth: 320,
          maxHeight: 240,
          borderRadius: radius.sm,
          border: `1px solid ${color.borderSoft}`,
        }}
      />
    </button>
  );
}

function FileChip({
  transport,
  attachment,
}: {
  transport: NodeTransport | null;
  attachment: AttachmentRef;
}) {
  const body = (
    <>
      <Icon name="link" size={12} strokeWidth={1.8} />
      <span style={nameStyle}>{attachment.name}</span>
    </>
  );
  if (!transport) return <span style={chipStyle}>{body}</span>;
  return (
    <button
      type="button"
      title={`Download ${attachment.name}`}
      aria-label={`Download attachment ${attachment.name}`}
      onClick={(event) => {
        event.stopPropagation();
        void download(transport, attachment);
      }}
      style={{ all: "unset", cursor: "pointer", ...chipStyle }}
    >
      {body}
    </button>
  );
}

/** A `duck://files/shared/attachments/…` reference in message text. */
export function AttachmentChip({ attachment }: { attachment: AttachmentRef }) {
  const store = useContext(ConsoleContext);
  const transport = store?.transport ?? null;
  if (transport && isImageName(attachment.name)) {
    return <ImagePreview transport={transport} attachment={attachment} />;
  }
  return <FileChip transport={transport} attachment={attachment} />;
}
