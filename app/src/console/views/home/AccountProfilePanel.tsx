// The account's global profile editor: avatar + bio/status, defined ONCE on the
// account and propagated to each joined network (see profile-reconcile.ts). The
// display name lives in ProfileCard (its existing input); this panel owns the
// two new fields. Self-contained on purpose — W1 rebuilds the account home this
// mounts into, so it takes only an `accountId` and drives everything through the
// store, letting final placement reconcile on the epic branch.

import { useEffect, useRef, useState } from "react";

import { MAX_AVATAR_BYTES } from "../../store/account-profile";
import { isAvatarMime } from "../../store/profile-reconcile";
import { useDucktape } from "../../store/use-ducktape";
import { Avatar } from "../../components/Avatar";
import { color, font, radius } from "../../theme/tokens";
import { outlineButton } from "../settings/parts";

/** Identity module bio cap (MAX_BIO_LEN), mirrored for the counter. */
const MAX_BIO_LEN = 280;

/** Read a picked image file into a base64 data URL, or reject with a reason. */
const readAvatarFile = (file: File): Promise<string> =>
  new Promise((resolve, reject) => {
    if (!isAvatarMime(file.type)) {
      reject(new Error("pick a PNG, JPEG, GIF, WebP, or AVIF image"));
      return;
    }
    if (file.size > MAX_AVATAR_BYTES) {
      reject(new Error(`image exceeds ${Math.floor(MAX_AVATAR_BYTES / 1024)} KiB`));
      return;
    }
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("could not read the image"));
    reader.readAsDataURL(file);
  });

export function AccountProfilePanel({ accountId }: { accountId: string | undefined }) {
  const { state, actions } = useDucktape();
  const onChainBio = accountId ? state.authorBios[accountId] : undefined;
  const onChainAvatar = accountId ? state.authorAvatars[accountId] : undefined;

  const [bio, setBio] = useState(onChainBio ?? "");
  // `undefined` = keep the current avatar; a data URL = new image; `null` = remove.
  const [avatarEdit, setAvatarEdit] = useState<string | null | undefined>(undefined);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  // Re-seed the bio when the account/on-chain value changes (a switch or a
  // landed reconcile) — but not while the user is mid-edit of a new avatar.
  useEffect(() => {
    setBio(onChainBio ?? "");
    setAvatarEdit(undefined);
    setError(null);
  }, [accountId, onChainBio]);

  const pickFile = (file: File | undefined) => {
    if (!file) return;
    setError(null);
    readAvatarFile(file)
      .then((dataUrl) => setAvatarEdit(dataUrl))
      .catch((err: Error) => setError(err.message));
  };

  const previewUrl = typeof avatarEdit === "string" ? avatarEdit : null;
  const showOnChain = avatarEdit === undefined && onChainAvatar;

  const save = () => {
    setSaving(true);
    setError(null);
    actions
      .saveProfile({ bio, avatar: avatarEdit })
      .then(() => setAvatarEdit(undefined))
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setSaving(false));
  };

  const dirty = avatarEdit !== undefined || bio !== (onChainBio ?? "");

  return (
    <div
      data-panel="account-profile"
      style={{
        marginTop: 9,
        border: `1px solid ${color.border}`,
        borderRadius: radius.lg,
        padding: 15,
        background: color.paper,
      }}
    >
      <div style={{ font: `600 12px ${font.sans}`, color: color.ink, marginBottom: 11 }}>
        Profile
      </div>

      <div style={{ display: "flex", gap: 13, alignItems: "flex-start" }}>
        {previewUrl ? (
          <img
            src={previewUrl}
            alt=""
            style={{ width: 56, height: 56, borderRadius: "50%", objectFit: "cover", flexShrink: 0 }}
          />
        ) : (
          <Avatar
            path={showOnChain ? onChainAvatar : null}
            name={state.author}
            size={56}
          />
        )}

        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
            <input
              ref={fileRef}
              type="file"
              accept="image/png,image/jpeg,image/gif,image/webp,image/avif"
              aria-label="Choose avatar image"
              style={{ display: "none" }}
              onChange={(event) => pickFile(event.target.files?.[0])}
            />
            <button
              type="button"
              disabled={!accountId}
              onClick={() => fileRef.current?.click()}
              style={outlineButton}
            >
              Change avatar
            </button>
            {(previewUrl || onChainAvatar) && avatarEdit !== null && (
              <button
                type="button"
                disabled={!accountId}
                onClick={() => {
                  setAvatarEdit(null);
                  setError(null);
                }}
                style={outlineButton}
              >
                Remove
              </button>
            )}
          </div>
          <div style={{ ...bioHint }}>
            Global to your account — shown on every network you join.
          </div>
        </div>
      </div>

      <label
        style={{
          display: "block",
          marginTop: 13,
          font: `500 11px ${font.sans}`,
          color: color.muted,
        }}
      >
        Bio / status
        <textarea
          aria-label="Bio"
          value={bio}
          disabled={!accountId}
          maxLength={MAX_BIO_LEN}
          placeholder="A short line about you"
          onChange={(event) => setBio(event.target.value)}
          rows={2}
          style={{
            display: "block",
            width: "100%",
            marginTop: 5,
            boxSizing: "border-box",
            resize: "vertical",
            border: `1px solid ${color.border}`,
            borderRadius: radius.md,
            background: color.sunken,
            padding: "7px 9px",
            font: `400 12.5px ${font.sans}`,
            color: color.ink,
          }}
        />
      </label>

      <div style={{ display: "flex", alignItems: "center", gap: 9, marginTop: 9 }}>
        <button
          type="button"
          disabled={!accountId || saving || !dirty}
          aria-label="Save profile"
          onClick={save}
          style={{
            ...outlineButton,
            cursor: !accountId || saving || !dirty ? "not-allowed" : "pointer",
            opacity: !accountId || saving || !dirty ? 0.55 : 1,
          }}
        >
          {saving ? "Saving…" : "Save"}
        </button>
        <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted }}>
          {bio.length}/{MAX_BIO_LEN}
        </span>
        {!accountId && (
          <span style={{ font: `400 10.5px ${font.sans}`, color: color.muted }}>
            Bind this node to an account first.
          </span>
        )}
      </div>

      {error && (
        <div role="alert" style={{ marginTop: 6, font: `500 10.5px ${font.sans}`, color: color.danger }}>
          {error}
        </div>
      )}
    </div>
  );
}

const bioHint = {
  marginTop: 6,
  font: `400 11px ${font.sans}`,
  color: color.muted,
} as const;
