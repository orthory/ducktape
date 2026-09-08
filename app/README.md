# Ducktape desktop

Native Chat + Pages client, with its UI declared in
`src/ui/app.ice` through [Ice](https://github.com/byeongsu-hong/ducktape-ui-lang).

```bash
cargo build -p node-bin
cargo run -p ducktape-app
```

The RPC endpoint stays editable in the app and wins whenever it is set; left
empty it resolves `DUCKTAPE_NODE`, else the active workspace's http port from
the CLI registry (`<ducktape home>/registry.json`), else
`http://127.0.0.1:8844`. Chat + Pages hydrate over HTTP after the resumable
`module:chat` and `module:pages` WebSocket topics are active, then rehydrate on
committed changes. Writes sign with the encrypted key at `DUCKTAPE_USER_KEY`,
else the keystore's active wallet (`<ducktape home>/keys/<name>.key`); no
active wallet is a refusal, not a guess — pick one in the launch window.
`<ducktape home>` is `$DUCKTAPE_HOME` when set, else `~/.ducktape`. Set
`DUCKTAPE_BIN` when the `ducktape` CLI is neither beside the app binary nor on
`PATH`.

## Module-owned views

The Approvals, Members, Agents, Node, Explorer, Settings, Chat, Files,
Pages, Forge and Shell tabs are not native: each is an Ice application under `crates/views`
(`governance`, `members`, `agents`, `node`, `explorer`, `settings`, `chat`, `files`,
`pages`, `forge`, `shell`) compiled
for the `tree` target and wrapped as an `ice:view` component that the app
loads from a file at runtime (`src/module_view.rs`).
`make views` builds every view under `crates/views` and stages it as
`target/views/<module>_view.wasm`, where a built binary looks for it
(`DUCKTAPE_VIEWS_DIR` overrides; the bundle's `resources` metadata carries the
directory into `Ducktape.app` beside the executable, and the Linux
`make install-app` copies it beside the binary); `make dev` and `make app` run
it first. A tab whose view is not
staged says so in its place.

A view is a pure function of the props the app pushes it (`<module>.props`,
one JSON item per change) and speaks back only in intents (`governance.vote`,
`members.propose`, …) that the tab's handler signs and submits exactly as the
native screen did; a view with nothing to write, like Agents, declares none. The guest sees no key, no endpoint and no clock, and a view
that traps shows why in its place instead of taking the window with it. A
view may leave a slot for something only the host can draw: the Node view's
Activity tab declares `node_log_timeline` as a host surface, and the app
paints its own retained log ring there (`surfaces_of` in `module_view.rs`),
queuing what the reader does in it for the handler to drain. A view that
needs the network asks through an intent and reads the answer off its props:
the Explorer's workspace search is run by the app on the view's behalf. A
view keeps its own drafts and hands the app only what the reader submitted:
Settings' rename, key and ticket fields cross as intents, the signing seat
crosses in as a flag (the password never leaves the app), and a committed op
tells the view which drafts it consumed. A view may also leave the host a
whole editor: the Pages document is the app's own `page_document` (its
buffer, history and save tick never cross), painted into the view's slot
from what the tab was last drawn with (`pages/surface.rs`); the view's page,
search and comment drafts leave with the act that reads them, and the app
hands one back only by moving `seed_rev`.
The Shell view goes further: its
composer, its terminal and its answer Markdown are host surfaces, so a task's
words never cross the wire — the host's composer raises the `send` intent
itself.
A view may also hand data to a
host surface that reads it: Forge's code browse leaves slots for the
decoded picture, the document-aware Markdown reader and the highlighted
code reader, each painted by the app from the arguments the view passes,
and the reader's links come back to the view's own handler. What cannot
cross the wire stays native beside the view: Forge's discussion note
composer edits an editor the app holds, so the app docks it under the view
while an item is open.
A view whose screen needs a widget
the tree wire does not carry leaves that widget to the host too: the Chat
view declares `chat_composer` as a host surface per room and per thread,
and the app paints its rich composer there (`src/composer_surface.rs`),
keeps every box's words for the life of the process, and hears a submit as
the view's `composer` intent — the words themselves never cross the wire.
The views workspace pins the same `ducktape-ui` rev as this crate; `make views`
refuses when they differ.

## Release build (macOS: signed and notarized)

`make app` builds `Ducktape.app` and `Ducktape-<version>-<arch>.dmg` under
`target/ice-bundle/` and signs both **ad-hoc**, which runs on the machine that
built it and nowhere else — Gatekeeper refuses an ad-hoc bundle that arrived
over the network. A bundle that leaves this Mac is signed with a Developer ID
identity and notarized by Apple. `cargo-ice bundle` does both itself, off four
environment variables; `make app` inherits the environment, so exporting them
is the whole configuration.

1. **The signing identity.** A "Developer ID Application" certificate from the
   Apple Developer Program, in the login keychain. The exact string is what
   `ICE_CODESIGN_IDENTITY` takes:

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
   export ICE_CODESIGN_IDENTITY="Developer ID Application: Example (TEAMID)"
   export ICE_NOTARY_KEY="$HOME/.appstoreconnect/AuthKey_XXXXXXXXXX.p8"
   export ICE_NOTARY_KEY_ID=XXXXXXXXXX
   export ICE_NOTARY_ISSUER=00000000-0000-0000-0000-000000000000
   make app-release          # refuses if ICE_CODESIGN_IDENTITY is unset
   ```

   The three `ICE_NOTARY_*` go together: all three set adds `xcrun notarytool
   submit --wait` on the DMG followed by `xcrun stapler staple`, so the ticket
   travels inside the image and a first launch with no network still passes.
   Set without `ICE_CODESIGN_IDENTITY`, cargo-ice refuses before the upload
   rather than after Apple's wait. Set none and the build prints the identity
   it used and says what to export to notarize.

4. **Verify** — on the built artifacts, before shipping them:

   ```sh
   spctl -a -vv target/ice-bundle/Ducktape.app      # accepted, source=Notarized Developer ID
   xcrun stapler validate target/ice-bundle/Ducktape-*.dmg
   codesign -dv --verbose=4 target/ice-bundle/Ducktape.app   # Authority + TeamIdentifier
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

The canonical shared UI uses warm ink-on-paper neutrals and a sparse
terracotta brand role. Content stays opaque; functional chrome uses three
translucent tiers over the native-blurred window (thin rail/sidebar, regular
titlebar/popovers, sheet modals). Ice owns the opacity roles while blur remains
renderer-owned. Depth comes from surface steps and warm shadows.

| Token | Value | Use |
| --- | --- | --- |
| `bg` | `#fdfdfb` | the app canvas |
| `surface` | `#ffffff` | cards and controls |
| `muted_bg` | `#f6f5f2` | recessed wells and quiet regions |
| `sidebar` | `#fbfbf9` | opaque utility bars inside content |
| `elevated` | `#f3f2ef` | panels one notch above the canvas |
| `row_hover` | `#f8f7f3` | ordinary row hover |
| `fg` / `muted` | `#2c2b27` / `#6b6962` | warm ink and its secondary |
| `primary` | `#26251f` | neutral primary actions and focus |
| `brand` | `#a05a3c` | mentions, unread state, and action links |
| `glass_thin` | `rgba(253,252,250,.50)` | rail and sidebar |
| `glass_regular` | `rgba(253,252,250,.62)` | titlebar and floating chrome |
| `glass_sheet` | `rgba(253,252,250,.86)` | modal and sheet surfaces |

- Brand is sparse; danger, success, and warning colors are reserved for status.
- Selected navigation uses neutral `#ecebe6`; brand is for mentions, unread
  state, and action links.
- Hover changes fill, border, and foreground only; it never changes geometry.
- Reveal contextual row actions on hover and keep them visible while selected.
- Ice theme tokens are compile-time constants, so the app ships the shared
  default light palette.

## Design system

Shared color, shape, recipes, and components come from the pinned
`ducktape-ui` source vendored under `src/ui/ducktape-ui/`. The local `design`
crate owns only application font assets and the product type scale; drift
guards hold the Ice sources to both authorities.

- Faces: **Geist** (UI), **Geist Mono** (machine values, metadata, field
  labels, and badges).
  The files are embedded from `crates/design/assets/fonts/` at build time.
- Scale: 22 display · 20 screen title · 16 section · 14 pane header · 13.5
  body · 13 list · 12.5 caption · 12 machine value · 11/10.5 meta · 10 field
  label · 9.5 navigation · 9 badge.
- Frame: 1280×800 default, 40px titlebar, 74px permanent rail, 236px module
  sidebar, flexible content, and a 300px detail panel when present.
- Depth: cards stay paper-flat; floating bars/popovers use `0 3px 12px /.13`,
  brand tiles/toasts use `0 6px 18px /.22`, and modal sheets use
  `0 24px 60px /.30` with warm `#282622` ink.
