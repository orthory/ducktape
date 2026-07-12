// The input half of browser back/forward (the entry half is
// store/nav-history.ts): map the inputs users mean by "the browser back/forward
// buttons" onto history traversal.
//
// A full browser wires the mouse's back/forward buttons and Alt+Arrow to
// history navigation in its chrome layer — the embedded engines this app ships
// in (CEF on Linux, WKWebView on macOS) do not, so even with real history
// entries those inputs would stay inert. This installer closes that gap at the
// DOM level, which works identically in every engine:
//
//   • pointer back/forward buttons (MouseEvent.button 3/4) on mouseup —
//     preventDefault()ed, so an engine that DOES implement native traversal
//     (Chromium honors the cancelation) never double-navigates;
//   • Alt+ArrowLeft / Alt+ArrowRight, EXCEPT while an editable element has
//     focus — on macOS Option+Arrow is word-by-word caret movement, and that
//     must keep winning inside inputs, textareas, and contenteditable.
//
// Traversal itself is History API back()/forward(); what a traversal restores
// (and rehydrates) is nav-history.ts's contract.

// ── Types ───────────────────────────────────────────────

/** The two history moves this module can issue — injected so tests observe
 *  traversal without a real (jsdom-shared) history stack. */
export interface HistoryNav {
  back(): void;
  forward(): void;
}

// ── Editable guard ──────────────────────────────────────

const isEditable = (target: EventTarget | null): boolean =>
  target instanceof HTMLElement &&
  (target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target.isContentEditable);

// ── Installer ───────────────────────────────────────────

/** Wire pointer/keyboard back-forward inputs to `nav`. Returns the uninstall. */
export function installHistoryButtons(
  nav: HistoryNav = window.history,
  target: Pick<Window, "addEventListener" | "removeEventListener"> = window,
): () => void {
  const onMouseUp = (event: MouseEvent): void => {
    if (event.button !== 3 && event.button !== 4) return;
    // cancel the engine's own traversal (where one exists) — ours is the only one.
    event.preventDefault();
    if (event.button === 3) nav.back();
    else nav.forward();
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    if (isEditable(event.target)) return;
    event.preventDefault();
    if (event.key === "ArrowLeft") nav.back();
    else nav.forward();
  };

  target.addEventListener("mouseup", onMouseUp);
  target.addEventListener("keydown", onKeyDown);
  return () => {
    target.removeEventListener("mouseup", onMouseUp);
    target.removeEventListener("keydown", onKeyDown);
  };
}
