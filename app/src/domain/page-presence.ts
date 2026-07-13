// A tiny client for the off-consensus Pages presence socket. The node repeats
// our last cursor at 1 Hz over the authenticated mesh; this client reconnects,
// replays recipients/current cursor, and leaves staleness projection to the UI.

export interface PageCursor {
  blockId: string | null;
  anchor: number;
  head: number;
}

export interface RemotePageCursor extends PageCursor {
  peer: string;
  atMs: number;
}

export interface PagePresenceSession {
  start(url: string): void;
  setRecipients(peers: string[]): void;
  setCursor(cursor: PageCursor): void;
  stop(): void;
}

const RECONNECT_MS = 3_000;
const isOffset = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 0;

export const parsePeerCursor = (
  raw: unknown,
  atMs = Date.now(),
): RemotePageCursor | null => {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Record<string, unknown>;
  if (
    value.type !== "peerCursor" ||
    typeof value.peer !== "string" ||
    !/^[0-9a-f]{64}$/i.test(value.peer) ||
    !(
      value.blockId === null ||
      (typeof value.blockId === "string" && value.blockId.length > 0 && value.blockId.length <= 256)
    ) ||
    !isOffset(value.anchor) ||
    !isOffset(value.head)
  ) {
    return null;
  }
  return {
    peer: value.peer.toLowerCase(),
    blockId: value.blockId,
    anchor: value.anchor,
    head: value.head,
    atMs,
  };
};

export const createPagePresenceSession = (
  onPeerCursor: (cursor: RemotePageCursor) => void,
): PagePresenceSession => {
  let socket: WebSocket | null = null;
  let url: string | null = null;
  let stopped = true;
  let reconnect: ReturnType<typeof setTimeout> | null = null;
  let recipients: string[] = [];
  let cursor: PageCursor = { blockId: null, anchor: 0, head: 0 };

  const send = (message: unknown) => {
    if (socket?.readyState === 1) socket.send(JSON.stringify(message));
  };
  const flush = () => {
    send({ type: "recipients", peers: recipients });
    send({ type: "cursor", ...cursor });
  };
  const dial = () => {
    if (stopped || !url) return;
    const next = new WebSocket(url);
    socket = next;
    next.onopen = flush;
    next.onmessage = (event) => {
      if (typeof event.data !== "string") return;
      try {
        const parsed = parsePeerCursor(JSON.parse(event.data));
        if (parsed) onPeerCursor(parsed);
      } catch {
        // Unknown/malformed control is isolated to this beacon.
      }
    };
    next.onclose = () => {
      if (socket === next) socket = null;
      if (!stopped) reconnect = setTimeout(dial, RECONNECT_MS);
    };
  };

  return {
    start(nextUrl) {
      if (!stopped) return;
      stopped = false;
      url = nextUrl;
      dial();
    },
    setRecipients(peers) {
      recipients = [...new Set(peers.map((peer) => peer.toLowerCase()))];
      send({ type: "recipients", peers: recipients });
    },
    setCursor(next) {
      cursor = next;
      send({ type: "cursor", ...cursor });
    },
    stop() {
      stopped = true;
      if (reconnect !== null) clearTimeout(reconnect);
      reconnect = null;
      socket?.close();
      socket = null;
    },
  };
};
