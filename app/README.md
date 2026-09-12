# Ducktape desktop

Native GPUI desktop shell with dynamically loaded, Rust-authored WASM views.

```bash
cargo build -p node-bin
make views                  # the tabs: wasm views staged under target/views
cargo run -p ducktape-app
```

The launch window lists the workspaces under the ducktape home (one
directory per network, `$DUCKTAPE_HOME` when set, else `~/.ducktape`) and the
remote endpoints it has saved; picking one opens that workspace's keystore.
An RPC endpoint typed in wins whenever it is set; left empty it resolves
`DUCKTAPE_NODE`, else the one workspace under the home, else
`http://127.0.0.1:8844`. Chat + Pages hydrate over HTTP after the resumable
`module:chat` and `module:pages` WebSocket topics are active, then rehydrate on
committed changes. Writes sign with the encrypted key at `DUCKTAPE_USER_KEY`,
else the picked workspace's active wallet (`<workspace>/keys/<name>.key`); no
active wallet is a refusal, not a guess — unlock one in the launch window.
The app's own state — its preferences, its log, its forge mirrors — lives in
the platform's application directories (`~/.config`, `~/.local/state` and
`~/.cache` on Linux, `~/Library/Application Support`, `~/Library/Logs` and
`~/Library/Caches` on macOS; an `XDG_*` variable wins on either), never under
the home.

## Module-owned views

The Approvals, Members, Agents, Node, Explorer, Settings, Chat, Files,
Pages and Forge tabs keep their state and behavior in WASM views under `crates/views`
(`governance`, `members`, `agents`, `node`, `explorer`, `settings`, `chat`, `files`,
`pages`, `forge`). Rust cdylibs compile to `wasm32-unknown-unknown`; wasm-tools wraps their embedded WIT exports as components that the app
loads from a file at runtime (`src/module_view.rs`).
`make views` builds every view under `crates/views` and stages it as
`target/views/<module>_view.wasm`, where a built binary looks for it
(`DUCKTAPE_VIEWS_DIR` overrides; the native packaging script carries the
directory into `Ducktape.app` as resources linked beside the executable, and the Linux
`make install-app` copies it beside the binary); `make dev` and `make app` run
it first. A tab whose view is not
staged says so in its place.

Deployed views follow the module registry's active deployment hash on block
events. The host fetches and verifies a candidate, compiles it away from the
window thread, then snapshots the seated guest and restores that state into
the candidate. Installation rechecks the seated instance and snapshot tick.
The chain and existing view keep running during preparation; a rejected
candidate leaves the seated view in place. Input routes belong to the seated
instance, not to an in-progress load attempt. Matching native input values,
selections and focus can survive installation without retaining old callbacks.

`ducktape module update` accepts `--view` and `--assets` alongside the module
component and optional index. A deployment is a complete set: omitting its
view or index removes that part on activation, rather than keeping an older
copy. See [`../docs/records/architecture/wasm-module-authoring.md`](../docs/records/architecture/wasm-module-authoring.md)
for module packaging and registry operations.

A view owns its state, receives session and domain props (`<module>.props`,
one JSON item per change), and emits a wire tree rendered by native gpui-kit
controls. It can emit intents (`governance.vote`, `members.propose`, …) to
the app or request operations through the host kernel. Signing secrets stay
in the app; network access and timers run through the kernel rather than
direct guest OS access. A view that traps shows why in its place instead of
taking the window with it. A
view can also request `host.widget` operations on its own mounted tree and nested overlays:
focus traversal, targeted focus and focus queries; native text-input cursor
and selection operations; and scroll offsets, relative movement, end snapping
and keyed-row reveals. Requests are validated and bounded, run only after the
matching native frame is laid out and editor work has drained, and are refused
if their frame was replaced. They cannot address another view's widgets.
A view may leave a slot for something only the host can draw: the Node view's
Activity tab declares `node_log_timeline` as a host surface, and the app
paints its own retained log ring there (`src/module_view/surfaces.rs`),
queuing what the reader does in it for the handler to drain. A view that
needs the network can use the kernel's bounded request/reply interface:
Explorer queries through `rpc.query` and `rpc.view`. A
view keeps its own drafts and hands the app only what the reader submitted:
Settings' rename, key and ticket fields cross as intents, the signing seat
crosses in as a flag (the password never leaves the app), and a committed op
tells the view which drafts it consumed. Files keeps an unsaved text edit with its
original file and network when navigation changes. Returning to that file
resumes the same edit; only an explicit discard or that save's successful
reply clears it. Saving uses the snapshot that supplied the original text,
so an intervening edit is refused without losing the draft. Pages owns its
Markdown editor, selection and undo history in the guest. Initial source arrives
in bounded chunks, and only an accepted canonical revision updates the app's
save buffer. A replacement preserves that editor and undo history; old-instance
notifications cannot overwrite it. Presentation that exceeds its bounds uses a
plain editor with a visible notice, retaining all text and history. Page, search
and comment drafts leave with the act that reads them; the app hands one back
only by moving `seed_rev`.
A view may also hand data to a
host surface that reads it: Forge's code browse leaves slots for the
decoded picture, the document-aware Markdown reader and the highlighted
code reader, each painted by the app from the arguments the view passes,
and the reader's links come back to the view's own handler. Forge's
discussion note composer is the chat composer as a host surface
(`forge_composer`) over the item's channel, so a note's words stay in the
app and only its send crosses.
Forge and Files project large read-only text and lists before sending props.
The display counts omitted rows and labels shortened text; routing identifiers
are kept intact. If the fixed metadata itself is too large, the view shows a
bounded explanation instead of a partial, ambiguous browser. Files keeps the
complete read separately as its editor seed: shortening the preview neither
marks the read truncated nor disables Edit. These projections budget incoming
facts, not arbitrary editor or input drafts.

A view whose screen needs a widget
the tree wire does not carry leaves that widget to the host too: the Chat
view declares `chat_composer` as a host surface per room and per thread,
and the app paints its rich composer there (`src/composer_surface.rs`),
keeps every box's words for the life of the process, and hears a submit as
the view's `composer` intent — the words themselves never cross the wire.
The views workspace and desktop crate pin the same `ducktape-ui` revision for
their shared wire vocabulary.

## Release build (macOS: signed and notarized)

`make app` builds `Ducktape.app` and `Ducktape-<version>-<arch>.dmg` under
`target/app-bundle/` and signs both **ad-hoc**, which runs on the machine that
built it and nowhere else — Gatekeeper refuses an ad-hoc bundle that arrived
over the network. A bundle that leaves this Mac is signed with a Developer ID
identity and notarized by Apple. `ops/bundle-app-macos.sh` does both itself, off four
environment variables; `make app` inherits the environment, so exporting them
is the whole configuration.

1. **The signing identity.** A "Developer ID Application" certificate from the
   Apple Developer Program, in the login keychain. The exact string is what
   `DUCKTAPE_CODESIGN_IDENTITY` takes:

   ```sh
   security find-identity -v -p codesigning   # "Developer ID Application: … (TEAMID)"
   ```

   With it set, the `.app` and the `.dmg` are signed with `--timestamp
   --options runtime` — the hardened runtime and a trusted timestamp are both
   preconditions for notarization.

2. **The notary key.** An App Store Connect API key: **App Store Connect →
   Users and Access → Integrations → Team Keys**, created with the *Developer*
   role. The `.p8` downloads **once**. Store it outside this repo (a repo copy
   is a leaked signing credential), and keep the Key ID and the Issuer UUID the
   same page shows.

3. **Build.**

   ```sh
   export DUCKTAPE_CODESIGN_IDENTITY="Developer ID Application: Example (TEAMID)"
   export DUCKTAPE_NOTARY_KEY="$HOME/.appstoreconnect/AuthKey_XXXXXXXXXX.p8"
   export DUCKTAPE_NOTARY_KEY_ID=XXXXXXXXXX
   export DUCKTAPE_NOTARY_ISSUER=00000000-0000-0000-0000-000000000000
   make app-release          # refuses if DUCKTAPE_CODESIGN_IDENTITY is unset
   ```

   The three `DUCKTAPE_NOTARY_*` go together: all three set adds `xcrun notarytool
   submit --wait` on the DMG followed by `xcrun stapler staple`, so the ticket
   travels inside the image and a first launch with no network still passes.
   Set without `DUCKTAPE_CODESIGN_IDENTITY`, the packaging script refuses before the upload
   rather than after Apple's wait. Set none and the build prints the identity
   it used and says what to export to notarize.

4. **Verify** — on the built artifacts, before shipping them:

   ```sh
   spctl -a -vv target/app-bundle/Ducktape.app      # accepted, source=Notarized Developer ID
   xcrun stapler validate target/app-bundle/Ducktape-*.dmg
   codesign -dv --verbose=4 target/app-bundle/Ducktape.app   # Authority + TeamIdentifier
   ```

`ops/macos-preflight.sh` reports both halves — the Developer ID identities in
the keychain and whether the three notary variables are exported with the key
file present — as an informational section; it never fails on them, because a
local build needs neither.

The microVM shim signs the same way: `bin/duck-vz-shim/build.sh` takes
`CODESIGN_IDENTITY` (ad-hoc `-` by default) and passes the
`com.apple.security.virtualization` entitlement whichever identity it is given
— Virtualization.framework refuses the binary without that entitlement.

## Visual language

The native shell uses gpui-kit's light and dark semantic themes over an opaque
window. Rectangular navigation and bordered sections separate the permanent
rail, workspace header and content. Selected navigation uses the native primary
button variant; muted surfaces and text distinguish context from actions, and
the destructive color identifies errors. These roles follow the active theme
rather than a separate fixed shell palette.

WASM views retain their authored light and dark faces across the wire boundary.
The renderer maps their layout and presentation onto native controls; their
module-specific styling is separate from the shell's theme tokens.

## Design system

The native shell and wire renderer use `gpui-kit`. WASM views describe their own faces,
layout and editor presentation through the shared wire vocabulary. The local
`design` crate owns application font assets and the product type scale.

- Faces: **Geist** (UI), **Geist Mono** (machine values, metadata, field
  labels, and badges).
  The files are embedded from `crates/views/support/design/assets/fonts/` at build time.
- Guest type scale: 22 display · 20 screen title · 16 section · 14 pane header · 13.5
  body · 13 list · 12.5 caption · 12 machine value · 11/10.5 meta · 10 field
  label · 9.5 navigation · 9 badge.
- Console frame: 1280×800 default, a 184px permanent rail and a 72px workspace
  header. Content fills the remaining space; individual WASM views own their
  sidebars and split panes.
