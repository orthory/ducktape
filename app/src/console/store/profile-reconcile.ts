// Per-network profile propagation. The account's GLOBAL profile (name, avatar,
// bio — held app-local in account-profile.ts) is pushed to each joined
// network's identity module. Two entry points mirror the existing name flow:
//
//  - reconcileProfile(): the ON-CONNECT idempotent SET-propagation pass, the
//    exact shape of autoBindUserIdentity — re-derive the network's on-chain
//    profile each connect, push only SET fields that differ, no-op when
//    converged, best-effort (never throws). It NEVER clears an on-chain field
//    from an empty local store (a fresh device must not wipe the account) and
//    self-heals its local store from on-chain instead.
//  - pushProfileEdit(): the AUTHORITATIVE direct write from the profile panel
//    (the active network), which DOES clear (the user explicitly removed a
//    field). Mirrors setDisplayName's direct-write half.
//
// Avatar bytes ride the duckfs files plane (same plane as chat attachments,
// #541) under /shared/attachments/avatars, at a CONTENT-ADDRESSED path so
// reconciliation is a path compare — the on-chain ref already encodes the
// source-image identity, so "already there" needs no re-upload.

import {
  accountOfNode,
  setAccountName,
  setAccountProfile,
} from "../../domain/identity-client";
import type { AccountView } from "../../domain/identity-client";
import type { NodeTransport } from "../../domain/transport";
import { base64ToBytes, uploadFile } from "../../domain/files-client";
import { ATTACHMENTS_ROOT } from "../../domain/duck-uri";
import { keyHex } from "../../domain/chat-client";
import { hasNativeShell } from "../../domain/node-bootstrap";
import { normalizeKey } from "../../domain/names";
import { identityState } from "../../domain/user-identity-client";
import { loadAccountProfile, saveAccountProfile } from "./account-profile";

/** OWNERSHIP GUARD: true iff `account` is the LOCAL USER'S OWN account — this
 *  machine's user key is in its member set. Both propagation directions hang on
 *  this: without it, connecting to a FOREIGN node (client mode) would adopt the
 *  foreign account's name/bio into the local global profile, and the push half
 *  would write the local profile onto the foreign account. Unverifiable
 *  (web build, locked key) counts as NOT ours — the next connect after unlock
 *  retries, exactly like auto-bind. */
const isOwnAccount = async (account: AccountView): Promise<boolean> => {
  if (!hasNativeShell()) return false;
  const { state, pubkey } = await identityState();
  if ((state !== "unlocked" && state !== "plaintext") || !pubkey) return false;
  const mine = normalizeKey(pubkey);
  return account.member_keys.some((key) => keyHex(key.pubkey) === mine);
};
/** Raster image mimes accepted for avatars → their duckfs extension. Restricted
 *  to the set the attachment chip previews inline (svg is script-bearing and
 *  deliberately excluded). The panel validates against this before storing. */
const AVATAR_MIME_EXT: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/avif": "avif",
};

export const isAvatarMime = (mime: string): boolean => mime in AVATAR_MIME_EXT;

/** A `data:<mime>;base64,<…>` avatar URL, decoded and content-addressed. The
 *  path is `/shared/attachments/avatars/<sha16>.<ext>` — the first 16 hex of
 *  SHA-256 over the bytes, so the same image always maps to the same path
 *  (idempotent reconcile) and different images never collide. */
export interface DerivedAvatar {
  path: string;
  bytes: Uint8Array<ArrayBuffer>;
  mime: string;
}

export const deriveAvatar = async (dataUrl: string): Promise<DerivedAvatar> => {
  const match = /^data:([^;,]+);base64,(.*)$/s.exec(dataUrl);
  if (!match) throw new Error("avatar is not a base64 data URL");
  const mime = match[1];
  const ext = AVATAR_MIME_EXT[mime];
  if (!ext) throw new Error(`unsupported avatar type: ${mime}`);
  const bytes = base64ToBytes(match[2]);
  const digest = await crypto.subtle.digest("SHA-256", bytes as BufferSource);
  const sha16 = Array.from(new Uint8Array(digest).subarray(0, 8))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return { path: `${ATTACHMENTS_ROOT}/avatars/${sha16}.${ext}`, bytes, mime };
};

/** Upload the avatar bytes to duckfs if not already at `path`. Idempotent: the
 *  content-addressed path means an existing object is the same bytes, so a
 *  matching on-chain ref skips the commit entirely. */
const ensureAvatarUploaded = async (
  transport: NodeTransport,
  avatar: DerivedAvatar,
  onChainPath: string | null,
): Promise<void> => {
  if (onChainPath === avatar.path) return; // already reconciled
  await uploadFile(transport, {
    path: avatar.path,
    bytes: avatar.bytes,
    meta: { mime: avatar.mime },
    message: "avatar",
  });
};

export type ReconcileOutcome =
  | "unbound"
  /** The node is bound to an account that is NOT this machine's user — a
   *  foreign/client-mode connection. Nothing adopted, nothing pushed. */
  | "foreign"
  | "reconciled"
  | "skipped";

/** ON-CONNECT idempotent pass: propagate the account's SET profile fields to
 *  this network, adopting on-chain values into an empty local store rather than
 *  clearing them. Never throws — a failure resolves "skipped" and the next
 *  connect retries (identical contract to autoBindUserIdentity). */
export const reconcileProfile = async (
  transport: NodeTransport,
  deps: { nodePub: string },
): Promise<ReconcileOutcome> => {
  try {
    const account = await accountOfNode(transport, deps.nodePub);
    // Unbound: no account to push to yet. The bind pass runs first on connect;
    // the next connect/refresh reconciles once it lands.
    if (!account) return "unbound";
    // Foreign account (or ownership unverifiable): neither adopt nor push.
    if (!(await isOwnAccount(account))) return "foreign";

    // Self-heal: seed the local store from on-chain when unset. Stops a fresh
    // device (empty localStorage) from wiping the account, and carries the
    // name/bio forward so a later network gets them.
    let profile = loadAccountProfile();
    const seed: { name?: string; bio?: string } = {};
    if (!profile.name && account.display_name) seed.name = account.display_name;
    if (!profile.bio && account.bio) seed.bio = account.bio;
    if (seed.name !== undefined || seed.bio !== undefined) {
      profile = saveAccountProfile(seed);
    }

    // NAME via the existing origin-gated SetAccountName (idempotent guard).
    if (profile.name && profile.name !== account.display_name) {
      await setAccountName(transport, {
        displayName: profile.name,
        origin: deps.nodePub,
      });
    }

    // AVATAR + BIO via SetProfile — push only SET fields that differ; preserve
    // the other field by defaulting it to its on-chain value.
    let avatarPath = account.avatar;
    let bio = account.bio;
    let changed = false;
    if (profile.avatar) {
      const derived = await deriveAvatar(profile.avatar);
      if (account.avatar !== derived.path) {
        await ensureAvatarUploaded(transport, derived, account.avatar);
        avatarPath = derived.path;
        changed = true;
      }
    }
    if (profile.bio && profile.bio !== account.bio) {
      bio = profile.bio;
      changed = true;
    }
    if (changed) {
      await setAccountProfile(transport, {
        avatar: avatarPath,
        bio,
        origin: deps.nodePub,
      });
    }
    return "reconciled";
  } catch {
    return "skipped";
  }
};

/** AUTHORITATIVE direct write from the profile panel to the active node. Unlike
 *  reconcile it DOES clear (the user explicitly removed a field). `avatar`:
 *  a data URL sets a new image, `null` removes, `undefined` keeps the current
 *  on-chain avatar. Throws on failure so the panel can show it inline.
 *
 *  ponytail: a CLEAR only reaches the ACTIVE network here; propagating a clear
 *  to inactive networks would need a tombstone in the local store — add that if
 *  users report a removed avatar lingering on a network they later switch to. */
export const pushProfileEdit = async (
  transport: NodeTransport,
  edit: { nodePub: string; bio: string | null; avatar: string | null | undefined },
): Promise<void> => {
  const account = await accountOfNode(transport, edit.nodePub);
  if (!account) throw new Error("this node isn't linked to an account yet");
  if (!(await isOwnAccount(account))) {
    throw new Error("this node is bound to someone else's account — profile not written");
  }

  let avatarPath = account.avatar;
  if (edit.avatar === null) {
    avatarPath = null;
  } else if (edit.avatar) {
    const derived = await deriveAvatar(edit.avatar);
    await ensureAvatarUploaded(transport, derived, account.avatar);
    avatarPath = derived.path;
  }
  const bio = edit.bio && edit.bio.trim() ? edit.bio.trim() : null;
  await setAccountProfile(transport, {
    avatar: avatarPath,
    bio,
    origin: edit.nodePub,
  });
};
