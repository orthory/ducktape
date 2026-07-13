// A draggable edge handle that resizes its parent panel by writing a CSS
// width var on <html> — consumers style `width: var(--x, <default>px)`, so a
// drag re-renders nothing but this handle (the huddle dock tracks the chat
// rail's live width through the same var). The width persists in localStorage
// under the var's name; double-click resets to the default; arrow keys resize
// from the keyboard.

import { useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import { accentVar } from "../theme/tokens";

const KEY_STEP = 16;

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const setVar = (name: string, px: number) =>
  document.documentElement.style.setProperty(name, `${px}px`);

/** Re-apply a persisted panel width to its CSS var without mounting the
 *  handle — for surfaces (the huddle dock) that consume the var on screens
 *  where the panel itself isn't rendered. */
export const restorePanelVar = (varName: string, min: number, max: number) => {
  const saved = Number(localStorage.getItem(varName));
  if (saved) setVar(varName, clamp(saved, min, max));
};

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
  const [width, setWidth] = useState(() => {
    const saved = Number(localStorage.getItem(varName));
    return saved ? clamp(saved, min, max) : defaultWidth;
  });
  const [dragging, setDragging] = useState(false);
  const [hovered, setHovered] = useState(false);
  // Live while a drag is in progress; pointer CAPTURE routes every move/up to
  // this handle even outside the window, so a release can never strand the
  // drag (the old window-listener version could).
  const drag = useRef<{ startX: number; startWidth: number } | null>(null);

  useLayoutEffect(() => {
    setVar(varName, width);
  }, [varName, width]);

  const apply = (next: number) => {
    const clamped = clamp(next, min, max);
    setWidth(clamped);
    return clamped;
  };
  const persist = (value: number) => localStorage.setItem(varName, String(value));
  const deltaOf = (clientX: number, startX: number) =>
    side === "right" ? clientX - startX : startX - clientX;

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const panel = event.currentTarget.parentElement;
    if (!panel) return;
    drag.current = { startX: event.clientX, startWidth: panel.offsetWidth };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setDragging(true);
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    apply(drag.current.startWidth + deltaOf(event.clientX, drag.current.startX));
  };
  const endDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    persist(apply(drag.current.startWidth + deltaOf(event.clientX, drag.current.startX)));
    drag.current = null;
    setDragging(false);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    // Spatial: the panel edge follows the arrow — on a right-docked panel
    // (left-edge handle) ArrowRight moves the edge right, shrinking it.
    const toward = event.key === "ArrowRight" ? 1 : -1;
    const grow = side === "right" ? toward : -toward;
    persist(apply(width + grow * KEY_STEP));
  };

  const reset = () => {
    localStorage.removeItem(varName);
    setWidth(defaultWidth);
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize panel"
      aria-valuenow={Math.round(width)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title="Drag to resize · double-click to reset"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={onKeyDown}
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
        touchAction: "none",
        background:
          dragging || hovered
            ? `linear-gradient(to right, transparent 2px, ${accentVar} 2px, ${accentVar} 4px, transparent 4px)`
            : "transparent",
      }}
    />
  );
}
