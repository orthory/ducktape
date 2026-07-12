// The page icon, without a wire change.
//
// A real `icon` field on the Block would move the committed bytes — an app-hash
// flag day across every validator — for a decoration. So the icon simply IS the
// leading emoji of the page title (`root.text`): parsed out for display, written
// back by the picker. The title the user edits is the rest.

// A leading emoji, with its variation selector (U+FE0F), skin-tone modifiers
// and ZWJ (U+200D) joins — so "👩‍💻" and "👍🏽" come out whole. Regional-
// indicator flags are not Extended_Pictographic and are deliberately NOT icons.
const HEAD = new RegExp(
  "^(\\p{Extended_Pictographic}" +
    "(?:\\uFE0F|\\p{Emoji_Modifier}|\\u200D\\p{Extended_Pictographic}\\uFE0F?)*" +
    ")\\s*",
  "u",
);

/** Split a raw page title into its icon and the title proper. */
export function splitTitleEmoji(raw: string): { icon: string | null; title: string } {
  const match = HEAD.exec(raw);
  if (!match) return { icon: null, title: raw };
  return { icon: match[1], title: raw.slice(match[0].length) };
}

/** The raw title an icon + title compose back into — the inverse of the split,
 *  and exactly what gets committed. */
export const composeTitle = (icon: string | null, title: string): string =>
  icon ? (title ? `${icon} ${title}` : icon) : title;
