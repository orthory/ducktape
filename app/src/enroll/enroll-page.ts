// The phone-side enrollment page, served over the LAN by the desktop's
// enroll.rs and bundled (with @noble) to a single file at /e.js.
//
// It runs in an INSECURE context (plain http on a LAN address), so WebAuthn and
// crypto.subtle are both unavailable — the key is a pure-JS @noble P-256 key.
// Flow: generate a key -> POST /payload to get the exact bytes the node says to
// sign (never reconstructed here) -> ECDSA-P256-SHA256 low-S over them ->
// POST /possession. The desktop then authorizes + submits the AddMemberKey; the
// token (from the URL fragment) gates every call.

import { p256 } from "@noble/curves/nist.js";

const token = location.hash.slice(1);

const hex = (b: Uint8Array): string =>
  Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const unhex = (s: string): Uint8Array =>
  Uint8Array.from(s.match(/.{2}/g)!.map((h) => parseInt(h, 16)));

const status = (text: string): void => {
  const el = document.getElementById("s");
  if (el) el.textContent = text;
};

const postJson = (path: string, body: unknown): Promise<Record<string, unknown>> =>
  fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  }).then((r) => (r.ok ? r.json() : Promise.reject(new Error(`${path} → ${r.status}`))));

const enroll = (): Promise<void> =>
  Promise.resolve()
    .then(() => {
      // 1. mint a P-256 key on THIS phone; the compressed SEC1 point is the
      //    member key the chain will store.
      const secret = p256.utils.randomSecretKey();
      const newKey = hex(p256.getPublicKey(secret, true));
      return { secret, newKey };
    })
    .then(({ secret, newKey }) =>
      // 2. ask the desktop (which asks the node) what exact bytes to sign.
      postJson("/payload", { token, new_key: newKey }).then(({ payload }) => {
        // 3. ECDSA-P256-SHA256, low-S, raw R‖S — precisely what the chain verifies.
        const sig = p256.sign(unhex(payload as string), secret, {
          prehash: true,
          lowS: true,
          format: "compact",
        });
        // 4. hand the possession back; the desktop approves + submits it.
        return postJson("/possession", { token, new_key: newKey, sig: hex(sig) });
      }),
    )
    .then(() => {
      status("✓ Key created. Approve it on your desktop to finish.");
    })
    .catch((e) => {
      status("Couldn't add the key: " + String(e));
    });

document.getElementById("go")?.addEventListener("click", () => {
  status("Working…");
  enroll();
});
