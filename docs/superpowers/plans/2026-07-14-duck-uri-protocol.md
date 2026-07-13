# duck:// URI Protocol (v1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Protocolize `duck://<module>/<path>` — one grammar + module table (page/files/forge/channel) with chat chip rendering and click-through deep links onto the store's existing navigation actions.

**Architecture:** A new protocol core `app/src/domain/duck-uri.ts` owns the `DuckRef` union and `classifyDuckRef` (the module table). The chat tokenizer (`chat/duck-ref.ts`) keeps the markdown-link grammar but delegates classification to the core. A thin `openDuckRef(ref, actions)` adapter (`console/store/open-duck-ref.ts`) maps refs onto existing actions (`openPage`, `openFiles`, `openForgeItem`, `selectChannel`, `focusMessage`). Rust (`runs/inject.rs`) is deliberately untouched — the fuzz-verified "Rust refs ⊆ TS refs" invariant is preserved by only widening TS with navigation-only modules.

**Tech Stack:** TypeScript/React (app console), vitest (run from `app/`), no new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-14-duck-uri-protocol-design.md`

## Global Constraints

- Worktree `<primary>/.worktree/duck-uri-protocol` off `origin/dev`; PR against `dev`.
- Vitest MUST run from `app/` (jsdom env lives in `app/vite.config.ts`).
- If `app/node_modules` is missing: `npm install --legacy-peer-deps && npm install --no-save --legacy-peer-deps @testing-library/dom`.
- No wire change: refs stay plain markdown text; existing `page`/`files` URI behavior byte-identical (regexes for those two modules copied verbatim).
- files confinement stays exactly `/shared/attachments/<dir>/<name>` (2 segments, no dot-segments).
- Chip faces come from canonical/store data, never the markdown label (anti-spoof).
- No `cargo` gates needed: zero Rust files touched.

---

### Task 1: Protocol core — `domain/duck-uri.ts`

**Files:**
- Create: `app/src/domain/duck-uri.ts`
- Create: `app/src/domain/duck-uri.test.ts`

**Interfaces:**
- Produces: `DuckRef = { page: PageRef } | { file: FileRef } | { forge: ForgeRef } | { channel: ChannelRef }`, `classifyDuckRef(url: string, label: string, embed: boolean): DuckRef | null`, `ATTACHMENTS_ROOT`, `UNSAFE_NAME_CHARS`, `displayName(name: string): string`. `ForgeRef = { repo: string; number: number | null; seq?: number }`, `ChannelRef = { id: string; seq?: number }`; `PageRef`/`FileRef` shapes identical to today's `chat/duck-ref.ts`.

- [ ] **Step 1: Write the failing test** (`app/src/domain/duck-uri.test.ts`)

```ts
import { describe, expect, it } from "vitest";

import { classifyDuckRef } from "./duck-uri";

describe("classifyDuckRef — the module table", () => {
  it("classifies page refs (verbatim legacy grammar)", () => {
    expect(classifyDuckRef("duck://page/pg-1", "Plan", false)).toEqual({
      page: { id: "pg-1", label: "Plan" },
    });
    expect(classifyDuckRef("duck://page/a/b", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://page/", "x", false)).toBeNull();
  });

  it("confines file refs to the attachments root (verbatim legacy rules)", () => {
    expect(
      classifyDuckRef("duck://files/shared/attachments/u1/doc.pdf", "doc.pdf", false),
    ).toEqual({ file: { path: "/shared/attachments/u1/doc.pdf", name: "doc.pdf", embed: false } });
    expect(classifyDuckRef("duck://files/shared/skills/x.md", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/a/b/c", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/../etc/pw", "x", false)).toBeNull();
    expect(classifyDuckRef("duck://files/shared/attachments/u1/a.png", "a.png", true)).toEqual({
      file: { path: "/shared/attachments/u1/a.png", name: "a.png", embed: true },
    });
  });

  it("classifies forge refs: repo, item, discussion anchor", () => {
    expect(classifyDuckRef("duck://forge/ducktape", "", false)).toEqual({
      forge: { repo: "ducktape", number: null },
    });
    expect(classifyDuckRef("duck://forge/ducktape/58", "", false)).toEqual({
      forge: { repo: "ducktape", number: 58 },
    });
    expect(classifyDuckRef("duck://forge/ducktape/58#12", "", false)).toEqual({
      forge: { repo: "ducktape", number: 58, seq: 12 },
    });
    // an anchor needs an item; zero ids are not mintable
    expect(classifyDuckRef("duck://forge/ducktape#12", "", false)).toBeNull();
    expect(classifyDuckRef("duck://forge/ducktape/0", "", false)).toBeNull();
    expect(classifyDuckRef("duck://forge/ducktape/58#0", "", false)).toBeNull();
  });

  it("classifies channel refs, including colon ids and message anchors", () => {
    expect(classifyDuckRef("duck://channel/general", "", false)).toEqual({
      channel: { id: "general" },
    });
    expect(classifyDuckRef("duck://channel/forge:ducktape:58", "", false)).toEqual({
      channel: { id: "forge:ducktape:58" },
    });
    expect(classifyDuckRef("duck://channel/general#42", "", false)).toEqual({
      channel: { id: "general", seq: 42 },
    });
    expect(classifyDuckRef("duck://channel/general#0", "", false)).toBeNull();
    expect(classifyDuckRef("duck://channel/", "", false)).toBeNull();
  });

  it("leaves unknown modules and gateway hosts unclassified", () => {
    expect(classifyDuckRef("duck://memory/notes/a.md", "", false)).toBeNull();
    expect(classifyDuckRef("duck://team.duck/index.html", "", false)).toBeNull();
    expect(classifyDuckRef("duck://net.duck", "", false)).toBeNull();
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd app && npx vitest run src/domain/duck-uri.test.ts`
Expected: FAIL — `Cannot find module './duck-uri'`.

- [ ] **Step 3: Implement `app/src/domain/duck-uri.ts`**

Move `ATTACHMENTS_ROOT`, `UNSAFE_NAME_CHARS`, `displayName` here (from `chat/attachments.ts` / `chat/duck-ref.ts`) and add the table:

```ts
// The duck:// protocol core — the ONE module table for module-plane URIs
// (`duck://<module>/<path>[#<fragment>]`, module = single dotless label).
// Dotted authorities (`<name>.duck`) are the gateway plane and are NOT
// classified here — duck-browser.ts owns them. Spec:
// docs/adr/2026-07-14-duck-uri-protocol.mdx.

export const ATTACHMENTS_ROOT = "/shared/attachments";

// Bidi overrides + zero-width chars (label/extension spoofing) — stripped
// everywhere a ref-authored name is shown or saved.
// eslint-disable-next-line no-control-regex
export const UNSAFE_NAME_CHARS =
  /(copy the escaped \uXXXX regex VERBATIM from chat/attachments.ts:39 - do not retype)/g;

/** Received-side display name: authored by ANY sender, so strip control/bidi
 *  chars before it reaches a label or a download filename. */
export const displayName = (name: string): string =>
  name.replace(UNSAFE_NAME_CHARS, "") || "file";

export interface PageRef {
  id: string;
  /** the markdown label — decorative; the chip prefers the live store title. */
  label: string;
}

export interface FileRef {
  /** absolute duckfs path, `/shared/attachments/<dir>/<name>`. */
  path: string;
  /** display/download name — the markdown label, spoof-stripped. */
  name: string;
  /** `![..]` embed form: an image previews inline; a non-image still downloads. */
  embed: boolean;
}

export interface ForgeRef {
  repo: string;
  /** item number; null = a repo-only ref. */
  number: number | null;
  /** `#<seq>` Discussion-message anchor — only meaningful on an item ref. */
  seq?: number;
}

export interface ChannelRef {
  id: string;
  /** `#<seq>` message anchor (jump-to-message). */
  seq?: number;
}

export type DuckRef =
  | { page: PageRef }
  | { file: FileRef }
  | { forge: ForgeRef }
  | { channel: ChannelRef };

/** Classify one module-plane duck:// url into a typed ref, or null when it
 *  doesn't validate (unknown module, malformed path, gateway host). The
 *  page/files rules are byte-identical to the pre-protocol tokenizer. */
export function classifyDuckRef(url: string, label: string, embed: boolean): DuckRef | null {
  const page = url.match(/^duck:\/\/page\/([^/\s)]+)$/);
  if (page) return { page: { id: page[1], label } };

  if (url.startsWith("duck://files")) return classifyFile(url, label, embed);

  const forge = url.match(/^duck:\/\/forge\/([^/\s)#:]+)(?:\/(\d+))?(?:#(\d+))?$/);
  if (forge) {
    const number = forge[2] ? Number(forge[2]) : null;
    const seq = forge[3] ? Number(forge[3]) : undefined;
    if (forge[2] && !number) return null; // item 0 is not mintable
    if (seq !== undefined && (!seq || number === null)) return null; // anchor needs a real item+seq
    return { forge: { repo: forge[1], number, ...(seq ? { seq } : {}) } };
  }

  const channel = url.match(/^duck:\/\/channel\/([^\s)#]+)(?:#(\d+))?$/);
  if (channel) {
    const seq = channel[2] ? Number(channel[2]) : undefined;
    if (channel[2] && !seq) return null; // seqs are 1-based
    return { channel: { id: channel[1], ...(seq ? { seq } : {}) } };
  }

  return null;
}

// duck://files<absolute-path>; the path already carries its leading slash.
// Confinement: exactly /shared/attachments/<dir>/<name> — the tokenizer is
// the only guard against a crafted ref steering a client read elsewhere.
function classifyFile(url: string, label: string, embed: boolean): DuckRef | null {
  const filePath = url.slice("duck://files".length);
  if (!filePath.startsWith(`${ATTACHMENTS_ROOT}/`)) return null;
  const rest = filePath.slice(ATTACHMENTS_ROOT.length + 1);
  const parts = rest.split("/");
  if (parts.length !== 2 || parts.some((p) => p === "" || p === "." || p === "..")) {
    return null;
  }
  const name = displayName(label) || displayName(parts[1]);
  return { file: { path: filePath, name, embed } };
}
```

Note the forge repo charclass excludes `:` (repo names cannot carry the
item-channel separator) and `#` (fragment delimiter).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npx vitest run src/domain/duck-uri.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/duck-uri.ts app/src/domain/duck-uri.test.ts
git commit -m "feat(app): add the duck:// protocol core (module table + classify)"
```

### Task 2: Tokenizer delegation — `chat/duck-ref.ts` + `chat/attachments.ts`

**Files:**
- Modify: `app/src/console/views/chat/duck-ref.ts` (delete local classify/types; delegate)
- Modify: `app/src/console/views/chat/attachments.ts` (import moved names)
- Modify: `app/src/console/views/chat/duck-ref.test.ts` (new cases)
- Check/Modify: any other importer of `displayName`/`ATTACHMENTS_ROOT` from `attachments.ts` (`grep -rn "displayName\|ATTACHMENTS_ROOT" app/src` — update import paths only if the symbol moved out from under them; `attachments.ts` keeps exporting `sanitizeAttachmentName`, `isImageName`, `uploadAttachment`, `MAX_ATTACHMENT_BYTES`).

**Interfaces:**
- Consumes: `classifyDuckRef`, `displayName`, types from Task 1.
- Produces: `DuckSegment = { text: string } | DuckRef`; guards `isPageSeg`, `isFileSeg`, `isForgeSeg`, `isChannelSeg`; `splitDuckRefs(text): DuckSegment[]`; builders `pageRefMarkdown`, `fileRefMarkdown` unchanged. Re-exports `PageRef`/`FileRef`/`ForgeRef`/`ChannelRef`/`DuckRef` types for existing importers.

- [ ] **Step 1: Add failing tests to `duck-ref.test.ts`**

```ts
it("chips forge and channel refs and keeps unknown modules literal", () => {
  const segs = splitDuckRefs(
    "fix in [PR](duck://forge/ducktape/58) discussed in [#general](duck://channel/general#42), " +
      "see [notes](duck://memory/notes.md)",
  );
  expect(segs.filter(isForgeSeg)).toEqual([{ forge: { repo: "ducktape", number: 58 } }]);
  expect(segs.filter(isChannelSeg)).toEqual([{ channel: { id: "general", seq: 42 } }]);
  // unknown module: the whole markdown ref stays in the literal run
  expect(segs.some((s) => "text" in s && s.text.includes("[notes](duck://memory/notes.md)"))).toBe(
    true,
  );
});
```

Also extend the existing lossless round-trip test's source string with a forge ref, e.g. `and [PR](duck://forge/duck/7)` (the rebuild map needs arms for forge/channel segs — forge: `[${label}](duck://forge/...)` can't be rebuilt from the seg alone since the label is dropped; instead assert segment COUNT and literal-run reassembly the way the existing test does for page/file, or simply keep the round-trip test on page/file and assert forge/channel splitting separately as above. Choose the latter — do NOT weaken the existing test.)

- [ ] **Step 2: Run to verify failure**

Run: `cd app && npx vitest run src/console/views/chat/duck-ref.test.ts`
Expected: FAIL — `isForgeSeg` not exported.

- [ ] **Step 3: Rewrite `duck-ref.ts` to delegate**

Keep the header comment (update the grammar list), `mdLabel`, builders, `splitDuckRefs`, `parsePageRefs`. Delete the local `classify`, `ATTACHMENTS_ROOT`, `PageRef`, `FileRef`. New shape:

```ts
import {
  classifyDuckRef,
  displayName,
  type ChannelRef,
  type DuckRef,
  type FileRef,
  type ForgeRef,
  type PageRef,
} from "../../../domain/duck-uri";

export type { ChannelRef, DuckRef, FileRef, ForgeRef, PageRef };

export type DuckSegment = { text: string } | DuckRef;

export const isPageSeg = (s: DuckSegment): s is { page: PageRef } => "page" in s;
export const isFileSeg = (s: DuckSegment): s is { file: FileRef } => "file" in s;
export const isForgeSeg = (s: DuckSegment): s is { forge: ForgeRef } => "forge" in s;
export const isChannelSeg = (s: DuckSegment): s is { channel: ChannelRef } => "channel" in s;

// `!?` embed flag, a label with no `]`/newline, then a module-plane duck://
// url (single dotless label) with no whitespace or `)`.
const DUCK_REF = /(!?)\[([^\]\n]*)\]\((duck:\/\/[a-z][a-z0-9-]*\/[^\s)]+)\)/g;
```

`splitDuckRefs` body unchanged except `classify(...)` → `classifyDuckRef(...)`. `fileRefMarkdown`/`pageRefMarkdown`/`mdLabel` unchanged (mdLabel keeps using `displayName`, now imported from domain).

In `attachments.ts`: delete local `ATTACHMENTS_ROOT`/`UNSAFE_NAME_CHARS`/`displayName`, import them from `../../../domain/duck-uri`, keep re-exporting `ATTACHMENTS_ROOT` and `displayName` ONLY if other files import them from here (grep first; update those imports to domain instead and do not re-export).

- [ ] **Step 4: Run the chat test suite**

Run: `cd app && npx vitest run src/console/views/chat/`
Expected: PASS (all existing + new).

- [ ] **Step 5: Commit**

```bash
git add -A app/src
git commit -m "feat(app): tokenize forge/channel duck refs through the protocol core"
```

### Task 3: Open plane — `store/open-duck-ref.ts`

**Files:**
- Create: `app/src/console/store/open-duck-ref.ts`
- Create: `app/src/console/store/open-duck-ref.test.ts`
- Modify: `app/src/console/store/actions.ts` (one line: widen `openForgeItem` param)

**Interfaces:**
- Consumes: `DuckRef` (Task 1), `ConsoleActions`, `forgeItemTarget` from `domain/forge-client`.
- Produces: `openDuckRef(ref: DuckRef, actions: ConsoleActions): void`.

- [ ] **Step 1: Widen `openForgeItem`** in `ConsoleActions` (actions.ts:124) so a repo-only focus type-checks (the impl already handles null via `forgeFocus`):

```ts
openForgeItem(target: {
  repo: string;
  number: number | null;
  messageId?: string;
  messageSeq?: number;
}): void;
```

(`ForgeItemTarget` stays as-is in forge-client — it remains assignable. If the
`ForgeItemTarget` import in actions.ts becomes unused, remove it.)

- [ ] **Step 2: Write the failing test** (`open-duck-ref.test.ts`)

```ts
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "./actions";
import { openDuckRef } from "./open-duck-ref";

const mockActions = () =>
  ({
    openPage: vi.fn(),
    setScreen: vi.fn(),
    openFiles: vi.fn(),
    openForgeItem: vi.fn(),
    selectChannel: vi.fn(),
    focusMessage: vi.fn(),
  }) as unknown as ConsoleActions;

describe("openDuckRef — the deep-link adapter", () => {
  it("opens a page and lands on the pages screen", () => {
    const a = mockActions();
    openDuckRef({ page: { id: "pg-1", label: "x" } }, a);
    expect(a.openPage).toHaveBeenCalledWith("pg-1");
    expect(a.setScreen).toHaveBeenCalledWith("pages");
  });

  it("opens a file in the files browser", () => {
    const a = mockActions();
    openDuckRef({ file: { path: "/shared/attachments/u/d.pdf", name: "d.pdf", embed: false } }, a);
    expect(a.openFiles).toHaveBeenCalledWith("/shared/attachments/u/d.pdf");
  });

  it("jumps to a forge item with an optional discussion anchor", () => {
    const a = mockActions();
    openDuckRef({ forge: { repo: "ducktape", number: 58, seq: 12 } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: 58, messageSeq: 12 });
    openDuckRef({ forge: { repo: "ducktape", number: null } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: null });
  });

  it("selects a channel, focuses an anchored message, reroutes forge:* ids", () => {
    const a = mockActions();
    openDuckRef({ channel: { id: "general" } }, a);
    expect(a.setScreen).toHaveBeenCalledWith("chat");
    expect(a.selectChannel).toHaveBeenCalledWith("general");
    openDuckRef({ channel: { id: "general", seq: 42 } }, a);
    expect(a.focusMessage).toHaveBeenCalledWith("general", 42);
    openDuckRef({ channel: { id: "forge:ducktape:58", seq: 3 } }, a);
    expect(a.openForgeItem).toHaveBeenCalledWith({ repo: "ducktape", number: 58, messageSeq: 3 });
  });
});
```

- [ ] **Step 3: Run to verify failure**

Run: `cd app && npx vitest run src/console/store/open-duck-ref.test.ts`
Expected: FAIL — module missing.

- [ ] **Step 4: Implement `open-duck-ref.ts`**

```ts
// The duck:// open plane: one typed ref → the store's EXISTING navigation
// vocabulary (the same targets the desktop-notification NavigateTarget
// patches). No new navigation machinery — only a textual address for it.

import type { DuckRef } from "../../domain/duck-uri";
import { forgeItemTarget } from "../../domain/forge-client";
import type { ConsoleActions } from "./actions";

export const openDuckRef = (ref: DuckRef, actions: ConsoleActions): void => {
  if ("page" in ref) {
    // openPage loads the tree but does NOT navigate (SearchModal precedent).
    actions.openPage(ref.page.id);
    actions.setScreen("pages");
  } else if ("file" in ref) {
    actions.openFiles(ref.file.path);
  } else if ("forge" in ref) {
    const { repo, number, seq } = ref.forge;
    actions.openForgeItem({ repo, number, ...(seq ? { messageSeq: seq } : {}) });
  } else {
    const { id, seq } = ref.channel;
    // A forge item's hidden discussion channel is unroutable on the chat
    // surface — route to the item view (the navigate listener's rule).
    const forge = forgeItemTarget(id, { messageSeq: seq });
    if (forge) {
      actions.openForgeItem(forge);
    } else if (seq !== undefined) {
      actions.focusMessage(id, seq); // lands on the chat screen itself
    } else {
      actions.setScreen("chat");
      actions.selectChannel(id);
    }
  }
};
```

- [ ] **Step 5: Run tests, then commit**

Run: `cd app && npx vitest run src/console/store/open-duck-ref.test.ts`
Expected: PASS.

```bash
git add app/src/console/store/open-duck-ref.ts app/src/console/store/open-duck-ref.test.ts app/src/console/store/actions.ts
git commit -m "feat(app): duck:// open plane — refs deep-link through existing nav actions"
```

### Task 4: Chips — `chat/rich-text.tsx`

**Files:**
- Modify: `app/src/console/views/chat/rich-text.tsx`
- Modify: `app/src/console/views/chat/rich-text.test.tsx`

**Interfaces:**
- Consumes: `isForgeSeg`/`isChannelSeg`/`ForgeRef`/`ChannelRef` (Task 2), `openDuckRef` (Task 3), `parseItemChannelId` from `domain/forge-client`.
- Produces: `ForgeRefChip({ forge })`, `ChannelRefChip({ channel })` components; a shared `DuckSeg` renderer used by BOTH `CommentText` and `SpanText` (today they duplicate the seg→component ternary).

- [ ] **Step 1: Add failing tests to `rich-text.test.tsx`** — follow the file's existing store-wrapper pattern (read it first; reuse its provider/mocks). Cases:

```tsx
// forge chip face is canonical repo#n (never the label) and opens the item
it("chips a forge ref and deep-links on click", async () => { /* render body text
  "see [misleading](duck://forge/ducktape/58)" via the existing harness;
  expect screen text "ducktape#58"; click; expect openForgeItem-equivalent
  dispatch per the harness's action-spy idiom */ });

// channel chip face prefers the live store name; forge:* ids face as repo#n
it("chips a channel ref with the live #name face", async () => { /* body
  "[x](duck://channel/general)" with a store channel {id:"general",name:"general"};
  expect "#general"; click → setScreen("chat")+selectChannel("general") */ });
```

- [ ] **Step 2: Run to verify failure**

Run: `cd app && npx vitest run src/console/views/chat/rich-text.test.tsx`
Expected: FAIL (no chip rendered).

- [ ] **Step 3: Implement the chips + shared seg renderer**

Chip style mirrors `PageRefChip` (same style object, different glyph/face). Face rules:

```tsx
export function ForgeRefChip({ forge }: { forge: ForgeRef }) {
  const store = useContext(ConsoleContext);
  const face = forge.number === null ? forge.repo : `${forge.repo}#${forge.number}`;
  // glyph: ⑂ (fork) in the ¶ slot; click → openDuckRef({ forge }, store.actions)
}

export function ChannelRefChip({ channel }: { channel: ChannelRef }) {
  const store = useContext(ConsoleContext);
  const item = parseItemChannelId(channel.id); // forge:* hidden discussion
  const name = store?.state.channels.find((c) => c.id === channel.id)?.name;
  const face = item ? `${item.repo}#${item.number}` : `#${name ?? channel.id}`;
  // click → openDuckRef({ channel }, store.actions)
}
```

Both render a `<button>` when a store exists, else an inert span (PageRefChip precedent, keeps bare component tests working). Extract the seg mapping used by `CommentText` and `SpanText` into one helper so both surfaces gain the new chips at once:

```tsx
function DuckSeg({ seg, onTagClick }: { seg: DuckSegment; onTagClick?: (t: string) => void }) {
  if (isPageSeg(seg)) return <PageRefChip pageId={seg.page.id} />;
  if (isFileSeg(seg)) return <AttachmentChip attachment={seg.file} />;
  if (isForgeSeg(seg)) return <ForgeRefChip forge={seg.forge} />;
  if (isChannelSeg(seg)) return <ChannelRefChip channel={seg.channel} />;
  return <LiteralRun text={seg.text} onTagClick={onTagClick} />;
}
```

(`CommentText` passes no `onTagClick`, same as today.)

- [ ] **Step 4: Run the chat suite**

Run: `cd app && npx vitest run src/console/views/chat/`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/chat/rich-text.tsx app/src/console/views/chat/rich-text.test.tsx
git commit -m "feat(app): forge + channel ref chips with click-through deep links"
```

### Task 5: Browser address-bar handoff — `BrowserView.tsx`

**Files:**
- Modify: `app/src/console/views/browser/BrowserView.tsx` (the `open(input)` submit path)
- Modify: `app/src/console/views/browser/BrowserView.test.tsx`

**Interfaces:**
- Consumes: `classifyDuckRef` (Task 1), `openDuckRef` (Task 3), the view's existing `ConsoleContext` store access (verify how BrowserView reaches the store; if it doesn't already consume ConsoleContext, add `const store = useContext(ConsoleContext)`).

- [ ] **Step 1: Add a failing test** — typing `duck://forge/ducktape/58` in the address form navigates in-app instead of erroring (follow the file's existing harness; assert the store spy saw `openForgeItem` and no browser error rendered).

- [ ] **Step 2: Run to verify failure**

Run: `cd app && npx vitest run src/console/views/browser/BrowserView.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement the handoff** at the top of the submit path, before `parseDuckAddress`:

```ts
const raw = input.trim();
const uri = /^duck:\/\//i.test(raw) ? raw : `duck://${raw}`;
const ref = store ? classifyDuckRef(uri, "", false) : null;
if (ref) {
  openDuckRef(ref, store.actions);
  return;
}
// fall through: gateway plane (parseDuckAddress) exactly as today
```

A dotless authority that does NOT classify (e.g. `duck://forge/x/nope`) falls
through to `parseDuckAddress`, whose existing error message reports it
unresolvable — acceptable v1 (the spec's "address bar reports unresolvable").

- [ ] **Step 4: Run the browser suite**

Run: `cd app && npx vitest run src/console/views/browser/`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/views/browser/BrowserView.tsx app/src/console/views/browser/BrowserView.test.tsx
git commit -m "feat(app): address-bar handoff for module-plane duck:// URIs"
```

### Task 6: ADR + docs + full gate

**Files:**
- Create: `docs/adr/2026-07-14-duck-uri-protocol.mdx` (normative: grammar, plane split, module table incl. reserved `memory`/dotted hosts, canonical-face rule, security invariants, "adding a module = 1 union variant + 1 classify row + 1 chip + 1 openDuckRef arm + 1 table row here"; follow the voice/format of `docs/adr/2026-07-13-join-protocol.mdx`)
- Copy into the worktree & commit: `docs/superpowers/specs/2026-07-14-duck-uri-protocol-design.md`, `docs/superpowers/plans/2026-07-14-duck-uri-protocol.md`

- [ ] **Step 1: Write the ADR** (content distilled from the spec — grammar block, module table, planes, invariants verbatim where normative).

- [ ] **Step 2: Full app test run**

Run: `cd app && npx vitest run`
Expected: PASS (no unrelated regressions).

- [ ] **Step 3: Commit docs**

```bash
git add docs/adr/2026-07-14-duck-uri-protocol.mdx docs/superpowers/
git commit -m "docs: duck:// URI protocol ADR + design spec"
```

### Task 7: PR

- [ ] Push branch, `gh pr create --base dev` with a body covering: what the protocol defines, the four consumers unified, Rust deliberately untouched (parity invariant), test coverage, out-of-scope list from the spec.
