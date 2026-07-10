// Key display helpers shared by console views. Keys are hex ed25519 public
// keys as the node emits them; a names map is hex(key bytes) → profile display
// name, projected from `identity` (see `ConsoleState.authorNames`).

/** Canonical lookup form of a hex key: trimmed, no `0x` prefix, lowercase. */
export const normalizeKey = (key: string | null | undefined): string =>
  (key ?? "").trim().replace(/^0x/i, "").toLowerCase();

export const sameKey = (
  left: string | null | undefined,
  right: string | null | undefined,
): boolean => Boolean(normalizeKey(left)) && normalizeKey(left) === normalizeKey(right);

export const shortKey = (hex: string, start = 10, end = 6): string => {
  const clean = hex.trim();
  if (!clean) return "—";
  return clean.length > start + end + 1
    ? `${clean.slice(0, start)}…${clean.slice(-end)}`
    : clean;
};

/** Profile display name for a hex key, or null when the registry has none. */
export const displayNameForKey = (
  key: string,
  names: Record<string, string>,
): string | null => names[key] ?? names[normalizeKey(key)] ?? null;
