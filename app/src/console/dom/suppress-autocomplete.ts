// Browsers (and WebKitGTK, which the desktop shell runs on) pop an
// autocomplete / input-history dropdown over text fields by default. In a
// desktop app whose inputs are app state — not web forms — that dropdown is
// noise, and the team was papering over it by hand-writing autoComplete="off"
// on field after field. Make "off" the default instead: this installer stamps
// autocomplete="off" on every <input>/<textarea> that appears without an
// explicit autocomplete value.
//
// "without an explicit value" is the whole trick — deliberate opt-ins survive
// untouched. The identity gate's password-custody fields set
// autoComplete="new-password"/"current-password" on purpose (so a password
// manager can save/fill them); React writes that attribute before the node is
// inserted, so by the time we see it hasAttribute("autocomplete") is already
// true and we leave it alone.

const FIELD_SELECTOR = "input, textarea";

function suppress(el: Element): void {
  if (
    (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) &&
    !el.hasAttribute("autocomplete")
  ) {
    el.setAttribute("autocomplete", "off");
  }
}

function suppressWithin(node: Node): void {
  if (!(node instanceof Element)) return;
  suppress(node);
  node.querySelectorAll(FIELD_SELECTOR).forEach(suppress);
}

/**
 * Default every input/textarea under `root` to autocomplete="off", now and as
 * React mounts more of them. Explicit autocomplete values are preserved.
 *
 * Returns a disposer that stops watching; for the app root it lives for the
 * window's lifetime, so callers can ignore it.
 */
export function installAutocompleteDefault(root: ParentNode = document): () => void {
  root.querySelectorAll(FIELD_SELECTOR).forEach(suppress);

  const observer = new MutationObserver((records) => {
    for (const record of records) {
      record.addedNodes.forEach(suppressWithin);
    }
  });
  // A Document can't be observed directly — watch its element tree.
  const target = root instanceof Document ? root.documentElement : (root as Node);
  observer.observe(target, { childList: true, subtree: true });
  return () => observer.disconnect();
}
