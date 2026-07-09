import { useEffect, useMemo, useState } from "react";

import {
  LOGS_TOPIC,
  isLogTailItem,
} from "../../../domain/stream";
import type { NodeTransport } from "../../../domain/transport";
import { splitLines } from "./log-lines";

const MAX_LOG_LINES = 2_000;

const gapLine = (cursor: string): string =>
  `[log stream gap: dropped older lines before cursor ${cursor}]`;

export function useLogStream(
  transport: NodeTransport | null | undefined,
  backfill: string | null,
) {
  const [streamLines, setStreamLines] = useState<string[]>([]);
  const [sawStream, setSawStream] = useState(false);

  useEffect(() => {
    setStreamLines([]);
    setSawStream(false);
    if (!transport) return;
    return transport.subscribe([LOGS_TOPIC], {
      onTail: (frame) => {
        const item = frame.item;
        if (frame.topic !== LOGS_TOPIC || !isLogTailItem(item)) return;
        setSawStream(true);
        setStreamLines((prev) => [...prev, item.line].slice(-MAX_LOG_LINES));
      },
      onLagged: (topic, cursor) => {
        if (topic !== LOGS_TOPIC) return;
        setSawStream(true);
        setStreamLines((prev) =>
          [...prev, gapLine(cursor)].slice(-MAX_LOG_LINES),
        );
      },
    });
  }, [transport]);

  return useMemo(() => {
    const backfillLines = splitLines(backfill ?? "").map((line) => line.text);
    const rawLines = sawStream
      ? [...backfillLines, ...streamLines].slice(-MAX_LOG_LINES)
      : backfillLines.slice(-MAX_LOG_LINES);
    return {
      ready: Boolean(transport) || backfill !== null,
      text: rawLines.join("\n"),
    };
  }, [backfill, sawStream, streamLines, transport]);
}
