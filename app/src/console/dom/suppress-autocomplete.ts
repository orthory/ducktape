// The desktop shell's webview — WebKitGTK on Linux, WKWebView on macOS — layers
// three "helpful" completion behaviors onto text fields by default:
//
//   • autocomplete   — a form-autofill / input-history dropdown
//   • autocorrect    — WebKit's as-you-type substitution (on macOS, typing
//                      "test" gets "corrected" to "Test"); this is what reads
//                      as macOS input auto-completion
//   • autocapitalize — first-letter capitalization
//
// In a desktop app whose inputs are app state — names, ids, search text, not web
// forms — all three are noise, and the team was papering over them by
// hand-writing autoComplete/autoCorrect/autoCapitalize="off" on field after
// field. Make "off" the default instead: this installer stamps those three
// attributes on every <input>/<textarea> that appears without an explicit value.
//
// "without an explicit value" is the whole trick — deliberate opt-ins survive
// untouched. The identity gate's password-custody fields set
// autoComplete="new-password"/"current-password" on purpose (so a password
// manager can save/fill them), and a prose textarea can set autoCorrect="on".
// React writes those attributes before the node is inserted, so by the time we
// see it hasAttribute(...) is already true and we leave it alone.
//
// spellcheck is deliberately NOT defaulted here: the red squiggle is a prose
// affordance (chat, pages, governance proposals), not a completion, and those
// fields opt in with spellCheck explicitly.

const FIELD_SELECTOR = "input, textarea";

// attribute → value we stamp when the field has not set one itself.
const DEFAULTS: ReadonlyArray<readonly [string, string]> = [
  ["autocomplete", "off"],
  ["autocorrect", "off"],
  ["autocapitalize", "off"],
];

function suppress(el: Element): void {
  if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return;
  for (const [name, value] of DEFAULTS) {
    if (!el.hasAttribute(name)) el.setAttribute(name, value);
  }
}

function suppressWithin(node: Node): void {
  if (!(node instanceof Element)) return;
  suppress(node);
  node.querySelectorAll(FIELD_SELECTOR).forEach(suppress);
}

/**
 * Default every input/textarea under `root` to autocomplete / autocorrect /
 * autocapitalize = "off", now and as React mounts more of them. Explicit
 * values are preserved.
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
