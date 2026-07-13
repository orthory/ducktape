// A draggable edge handle that resizes its parent panel by writing a CSS
// width var on <html> — consumers style `width: var(--x, <default>px)`, so a
// drag re-renders nothing (the huddle dock tracks the chat rail's live width
// through the same var). The width persists in localStorage under the var's
// name; double-click resets to the default.

import { useLayoutEffect, useState } from "react";
import type { PointerEvent } from "react";

import { accentVar } from "../theme/tokens";

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const setVar = (name: string, px: number) =>
  document.documentElement.style.setProperty(name, `${px}px`);

export function PanelResizer({
  varName,
  defaultWidth,
  min,
  max,
  side,
}: {
  /** The CSS var this handle writes — also its localStorage key. */
  varName: string;
  defaultWidth: number;
  min: number;
  max: number;
  /** The panel edge the handle sits on. A LEFT-edge handle (right-docked
   *  panel) grows the panel as the pointer moves left. */
  side: "left" | "right";
}) {
  const [dragging, setDragging] = useState(false);
  const [hovered, setHovered] = useState(false);

  // Restore the saved width before first paint so the panel doesn't jump.
  useLayoutEffect(() => {
    const saved = Number(localStorage.getItem(varName));
    if (saved) setVar(varName, clamp(saved, min, max));
  }, [varName, min, max]);

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const panel = event.currentTarget.parentElement;
    if (!panel) return;
    const startX = event.clientX;
    const startWidth = panel.offsetWidth;
    setDragging(true);
    const widthAt = (ev: globalThis.PointerEvent) =>
      clamp(startWidth + (side === "right" ? ev.clientX - startX : startX - ev.clientX), min, max);
    const move = (ev: globalThis.PointerEvent) => setVar(varName, widthAt(ev));
    const up = (ev: globalThis.PointerEvent) => {
      localStorage.setItem(varName, String(widthAt(ev)));
      setDragging(false);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const reset = () => {
    localStorage.removeItem(varName);
    setVar(varName, defaultWidth);
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      title="Drag to resize · double-click to reset"
      onPointerDown={onPointerDown}
      onDoubleClick={reset}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        position: "absolute",
        top: 0,
        bottom: 0,
        ...(side === "right" ? { right: -3 } : { left: -3 }),
        width: 7,
        cursor: "col-resize",
        zIndex: 10,
        background:
          dragging || hovered
            ? `linear-gradient(to right, transparent 2px, ${accentVar} 2px, ${accentVar} 4px, transparent 4px)`
            : "transparent",
      }}
    />
  );
}
