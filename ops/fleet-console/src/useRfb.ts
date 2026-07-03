import { useEffect, useState, type RefObject } from "react";
import { wsUrlFromLocation } from "./wsUrl";

export type RfbStatus = "connecting" | "connected" | "disconnected";

interface RfbLike {
  viewOnly: boolean;
  scaleViewport: boolean;
  background: string;
  addEventListener(type: string, cb: (e: unknown) => void): void;
  disconnect(): void;
}

// Live noVNC RFB session bound to a container, reconnecting with capped
// exponential backoff. viewOnly=true for the grid thumbnails (can't fat-finger
// N apps); false for the interactive drawer. The RFB core is imported lazily so
// tests can mock it and so it isn't in the initial paint path.
export function useRfb(
  container: RefObject<HTMLDivElement | null>,
  token: string | undefined,
  viewOnly: boolean,
): RfbStatus {
  const [status, setStatus] = useState<RfbStatus>("connecting");

  useEffect(() => {
    const el = container.current;
    if (!token || !el) return;
    let disposed = false;
    let rfb: RfbLike | null = null;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let attempts = 0;

    const connect = async () => {
      const mod = await import("@novnc/novnc");
      if (disposed || !container.current) return;
      const RFB = mod.default as new (
        t: HTMLElement,
        url: string,
      ) => RfbLike;
      container.current.replaceChildren();
      setStatus("connecting");
      rfb = new RFB(
        container.current,
        wsUrlFromLocation(window.location, token),
      );
      rfb.viewOnly = viewOnly;
      rfb.scaleViewport = true;
      rfb.background = "#0b0e14";
      rfb.addEventListener("connect", () => {
        attempts = 0;
        if (!disposed) setStatus("connected");
      });
      rfb.addEventListener("disconnect", () => {
        rfb = null;
        if (disposed) return;
        setStatus("disconnected");
        const delay = Math.min(1000 * 2 ** attempts, 15000);
        attempts += 1;
        retry = setTimeout(connect, delay);
      });
    };

    connect();
    return () => {
      disposed = true;
      if (retry) clearTimeout(retry);
      try {
        rfb?.disconnect();
      } catch {
        /* already gone */
      }
    };
  }, [container, token, viewOnly]);

  return status;
}
