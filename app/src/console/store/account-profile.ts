// The account-level profile, held APP-LOCAL and GLOBAL: one display name,
// avatar image, and bio/status defined once on the account (no per-network
// overrides). It is the desired-state source the reconcile pass pushes to each
// joined network's identity module on connect (see profile-reconcile.ts).
//
// Persisted in localStorage next to the onboarding hand-offs (state.ts). The
// avatar is stored as a DATA URL (the raw image bytes, base64) — the account's
// global source image; the per-network duckfs upload derives a content-address
// path from it at reconcile time. Every access is best-effort: storage being
// unavailable degrades to "no profile", never an error.

const PROFILE_KEY = "ducktape.accountProfile";

/** Byte cap on the stored avatar image: kept at the files module's inline
 *  commit budget so the reconcile upload rides ONE inline commit (no chunking)
 *  and the data URL stays comfortably inside localStorage's quota. */
export const MAX_AVATAR_BYTES = 256 * 1024;

export interface AccountProfile {
  /** Display name — mirrored here from the on-chain name so it propagates to
   *  networks joined later, not only the active one. */
  name?: string;
  /** Bio/status line. */
  bio?: string;
  /** Avatar image as a `data:<mime>;base64,<…>` URL (the global source image). */
  avatar?: string;
}

/** The stored account profile, or an empty object when none/unavailable. */
export const loadAccountProfile = (): AccountProfile => {
  try {
    const raw = localStorage.getItem(PROFILE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as AccountProfile;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
};

/** Merge `patch` into the stored profile and persist. A field set to `null`
 *  is REMOVED (cleared); `undefined` leaves it untouched. Returns the merged
 *  result so callers can act on it without a re-read. */
export const saveAccountProfile = (
  patch: { name?: string | null; bio?: string | null; avatar?: string | null },
): AccountProfile => {
  const next = loadAccountProfile();
  for (const key of ["name", "bio", "avatar"] as const) {
    const value = patch[key];
    if (value === undefined) continue;
    if (value === null || value.trim?.() === "") delete next[key];
    else next[key] = value;
  }
  try {
    localStorage.setItem(PROFILE_KEY, JSON.stringify(next));
  } catch {
    // best-effort; a failed write just loses the update until next time.
  }
  return next;
};
