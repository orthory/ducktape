import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  createPagePresenceSession,
  type PageCursor,
  type PagePresenceSession,
  type RemotePageCursor,
} from "../../../domain/page-presence";
import { pagePresenceSocketUrl } from "../../../domain/transport";

const STALE_MS = 5_000;

export const usePagePresence = ({
  nodeUrl,
  pageId,
  selfNode,
  recipients,
}: {
  nodeUrl: string | null;
  pageId: string | null;
  selfNode: string | null;
  recipients: string[];
}) => {
  const [peers, setPeers] = useState<Record<string, RemotePageCursor>>({});
  const session = useRef<PagePresenceSession | null>(null);
  const current = useRef<PageCursor>({ blockId: null, anchor: 0, head: 0 });
  const currentPage = useRef(pageId);
  const recipientKey = useMemo(
    () => [...new Set(recipients.map((peer) => peer.toLowerCase()))].sort().join(","),
    [recipients],
  );

  useEffect(() => {
    setPeers({});
    if (currentPage.current !== pageId) {
      currentPage.current = pageId;
      current.current = { blockId: null, anchor: 0, head: 0 };
    }
    if (!nodeUrl || !pageId || !selfNode) return;
    const live = createPagePresenceSession((cursor) => {
      if (cursor.peer === selfNode.toLowerCase()) return;
      setPeers((before) => ({ ...before, [cursor.peer]: cursor }));
    });
    session.current = live;
    live.setRecipients(recipientKey ? recipientKey.split(",") : []);
    live.setCursor(current.current);
    live.start(pagePresenceSocketUrl(nodeUrl, pageId));
    return () => {
      live.stop();
      if (session.current === live) session.current = null;
    };
  }, [nodeUrl, pageId, selfNode, recipientKey]);

  useEffect(() => {
    const timer = setInterval(() => {
      const cutoff = Date.now() - STALE_MS;
      setPeers((before) => {
        const next = Object.fromEntries(
          Object.entries(before).filter(([, cursor]) => cursor.atMs >= cutoff),
        );
        return Object.keys(next).length === Object.keys(before).length ? before : next;
      });
    }, 1_000);
    return () => clearInterval(timer);
  }, []);

  const publishCursor = useCallback((cursor: PageCursor) => {
    current.current = cursor;
    session.current?.setCursor(cursor);
  }, []);

  return { peers: Object.values(peers), publishCursor };
};
