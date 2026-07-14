// A round account avatar: renders the image at a duckfs `path` (loaded over the
// files plane, same read lane as chat image attachments) and falls back to the
// name's initials on any absence or failure. Self-contained — it reads the live
// transport from ConsoleContext like AttachmentChip, so callers pass only the
// path + name.
//
// Security posture mirrors AttachmentChip's image preview: the bytes render
// ONLY via <img src=blobURL> (an image context executes nothing whatever the
// real bytes are), the path is confined to /shared/attachments by the identity
// module + the writer, and every object URL is revoked on unmount.

import { useContext, useEffect, useRef, useState } from "react";

import { readAll, stat } from "../../domain/files-client";
import { ConsoleContext } from "../store/context";
import { color, font } from "../theme/tokens";

/** Up to two initials from a display name; "?" when empty. */
export const initialsOf = (name: string): string => {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  return parts
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
};

/** Avatars load at most this many bytes (256 KiB — the write cap); a larger or
 *  hostile ref falls back to initials rather than pulling it into memory. */
const MAX_AVATAR_BYTES = 256 * 1024;

export function Avatar({
  path,
  name,
  size = 40,
}: {
  path?: string | null;
  name: string;
  size?: number;
}) {
  const store = useContext(ConsoleContext);
  const transport = store?.transport ?? null;
  const [url, setUrl] = useState<string | null>(null);
  const urlRef = useRef<string | null>(null);

  useEffect(() => {
    setUrl(null);
    if (!transport || !path) return;
    let cancelled = false;
    void (async () => {
      try {
        const entry = await stat(transport, { path });
        if (cancelled) return;
        if (!entry || entry.kind !== "file" || entry.size > MAX_AVATAR_BYTES) return;
        const bytes = await readAll(transport, { path });
        if (cancelled) return;
        const next = URL.createObjectURL(new Blob([bytes as BlobPart]));
        urlRef.current = next;
        setUrl(next);
      } catch {
        // absent / decode failure → initials fallback (leave url null).
      }
    })();
    return () => {
      cancelled = true;
      if (urlRef.current) {
        URL.revokeObjectURL(urlRef.current);
        urlRef.current = null;
      }
    };
  }, [transport, path]);

  const box = {
    width: size,
    height: size,
    borderRadius: "50%",
    flexShrink: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  } as const;

  if (url) {
    return (
      <span aria-hidden="true" style={box}>
        <img
          src={url}
          alt=""
          onError={() => setUrl(null)}
          style={{ width: "100%", height: "100%", objectFit: "cover" }}
        />
      </span>
    );
  }
  return (
    <span
      aria-hidden="true"
      style={{
        ...box,
        background: color.iconIdle,
        color: color.muted3,
        font: `600 ${Math.round(size * 0.375)}px ${font.sans}`,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}
