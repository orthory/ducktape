// Which toggles are collapsed, per page — a pure view preference, so it lives
// in localStorage exactly like the rail's tree collapse (PageRail.tsx), never
// on the wire. It used to be component state, which meant every remount (a tab
// switch, a reload) re-expanded every toggle in the document.

const KEY = "ducktape.pageBlocksCollapsed";

type Store = Record<string, string[]>;

const read = (): Store => {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Store)
      : {};
  } catch {
    return {};
  }
};

export const loadCollapsed = (pageId: string | null): ReadonlySet<string> => {
  if (!pageId) return new Set();
  const ids = read()[pageId];
  return new Set(Array.isArray(ids) ? ids.filter((id) => typeof id === "string") : []);
};

export const saveCollapsed = (pageId: string | null, ids: ReadonlySet<string>): void => {
  if (!pageId) return;
  try {
    const all = read();
    if (ids.size === 0) delete all[pageId];
    else all[pageId] = [...ids];
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    // best-effort, exactly like the rail's
  }
};
