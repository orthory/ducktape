import { describe, expect, it } from "vitest";

import {
  ATTACHMENTS_ROOT,
  attachmentUri,
  displayName,
  isAttachment,
  isImageName,
  sanitizeAttachmentName,
  splitAttachments,
} from "./attachments";

const uri = (rest: string) => `duck://files${ATTACHMENTS_ROOT}/${rest}`;

describe("splitAttachments", () => {
  it("splits a uri out of surrounding text, losslessly", () => {
    const text = `look at this ${uri("abc-123/cat.jpg")} !`;
    const segments = splitAttachments(text);
    expect(segments).toEqual([
      { text: "look at this " },
      { attachment: { path: `${ATTACHMENTS_ROOT}/abc-123/cat.jpg`, name: "cat.jpg" } },
      { text: " !" },
    ]);
    // lossless: concatenating segment sources reproduces the input.
    const rebuilt = segments
      .map((s) => (isAttachment(s) ? attachmentUri(s.attachment.path) : s.text))
      .join("");
    expect(rebuilt).toBe(text);
  });

  it("accepts only <dir>/<name> under the root — nothing shallower or deeper", () => {
    expect(splitAttachments(uri("onlyname"))).toEqual([{ text: uri("onlyname") }]);
    expect(splitAttachments(uri("a/b/c"))).toEqual([{ text: uri("a/b/c") }]);
    expect(splitAttachments(uri("a//b"))).toEqual([{ text: uri("a//b") }]);
  });

  it("rejects dot-segments verbatim", () => {
    expect(splitAttachments(uri("../etc"))).toEqual([{ text: uri("../etc") }]);
    expect(splitAttachments(uri("a/.."))).toEqual([{ text: uri("a/..") }]);
    expect(splitAttachments(uri("./x"))).toEqual([{ text: uri("./x") }]);
  });

  it("never chips paths outside the attachments root", () => {
    const other = "duck://files/shared/skills/a/b";
    expect(splitAttachments(other)).toEqual([{ text: other }]);
    const home = "duck://files/home/ext:aa/secret.txt";
    expect(splitAttachments(home)).toEqual([{ text: home }]);
  });

  it("cannot smuggle traversal — encoded, backslash, or NUL stay one segment", () => {
    // %2e%2e / %2f are literal chars in the URI text (single-decoded at the
    // wire by URLSearchParams), so they never split into path separators.
    const enc = uri("d/%2e%2e%2fsecret");
    expect(splitAttachments(enc)).toEqual([
      { attachment: { path: `${ATTACHMENTS_ROOT}/d/%2e%2e%2fsecret`, name: "%2e%2e%2fsecret" } },
    ]);
    // a backslash is not a separator; still exactly <dir>/<name>.
    const back = uri("d/..\\evil");
    expect(splitAttachments(back).filter(isAttachment)).toHaveLength(1);
    // a literal extra slash makes it 3 segments → rejected to literal text.
    expect(splitAttachments(uri("d/a/b"))).toEqual([{ text: uri("d/a/b") }]);
  });

  it("strips bidi/zero-width spoofing from the DISPLAY name, keeps the fetch path verbatim", () => {
    const spoof = uri("d/photo\u202egnp.exe");
    const [seg] = splitAttachments(spoof);
    if (!isAttachment(seg)) throw new Error("expected an attachment");
    // path keeps the raw segment (the node re-canonicalizes on read)...
    expect(seg.attachment.path).toBe(`${ATTACHMENTS_ROOT}/d/photo\u202egnp.exe`);
    // ...but the shown name has the RTL override stripped.
    expect(seg.attachment.name).toBe("photognp.exe");
    expect(seg.attachment.name).not.toContain("\u202e");
  });

  it("ends a uri at whitespace and handles several in one message", () => {
    const text = `${uri("d1/a.png")}\n${uri("d2/b.pdf")}`;
    const segments = splitAttachments(text);
    expect(segments.filter(isAttachment).map((s) => s.attachment.name)).toEqual([
      "a.png",
      "b.pdf",
    ]);
  });

  it("keeps unicode names intact", () => {
    const segments = splitAttachments(uri("d/사진.png"));
    expect(segments).toEqual([
      { attachment: { path: `${ATTACHMENTS_ROOT}/d/사진.png`, name: "사진.png" } },
    ]);
  });
});

describe("sanitizeAttachmentName", () => {
  it("flattens separators and brackets, collapses whitespace", () => {
    expect(sanitizeAttachmentName("a/b\\c d.png")).toBe("a-b-c-d.png");
    expect(sanitizeAttachmentName("x[1].png")).toBe("x-1-.png");
  });

  it("strips leading dots and control chars — no dotfiles, no traversal", () => {
    expect(sanitizeAttachmentName("..")).toBe("file");
    expect(sanitizeAttachmentName("...sneaky")).toBe("sneaky");
    expect(sanitizeAttachmentName(".hidden")).toBe("hidden");
    expect(sanitizeAttachmentName("a\u0000b\u001fc")).toBe("abc");
    // bidi override + zero-width are stripped, not turned into dashes.
    expect(sanitizeAttachmentName("photo\u202egnp.exe")).toBe("photognp.exe");
    expect(sanitizeAttachmentName("a\u200bb.png")).toBe("ab.png");
  });

  it("NFC-normalizes so the node never rejects the commit", () => {
    // NFD "e\u0301" (é) must normalize to the single NFC codepoint.
    const out = sanitizeAttachmentName("cafe\u0301.png");
    expect(out).toBe("caf\u00e9.png");
    expect(out.normalize("NFC")).toBe(out);
  });

  it("falls back on empty and clamps to the name byte cap", () => {
    expect(sanitizeAttachmentName("")).toBe("file");
    expect(sanitizeAttachmentName("   ")).toBe("file");
    const long = sanitizeAttachmentName("한".repeat(200));
    expect(new TextEncoder().encode(long).length).toBeLessThanOrEqual(255);
    expect(long.length).toBeGreaterThan(0);
  });

  it("keeps ordinary unicode names", () => {
    expect(sanitizeAttachmentName("사진 2026.png")).toBe("사진-2026.png");
  });
});

describe("isImageName", () => {
  it("previews common raster images only", () => {
    expect(isImageName("a.png")).toBe(true);
    expect(isImageName("a.JPG")).toBe(true);
    expect(isImageName("a.webp")).toBe(true);
  });

  it("never previews svg or non-images", () => {
    expect(isImageName("a.svg")).toBe(false);
    expect(isImageName("a.pdf")).toBe(false);
    expect(isImageName("a.html")).toBe(false);
    expect(isImageName("png")).toBe(false);
    expect(isImageName(".png")).toBe(false);
  });
});

describe("displayName", () => {
  it("strips control/bidi/zero-width, never rewrites slashes or clamps", () => {
    expect(displayName("a\u202eb")).toBe("ab");
    expect(displayName("x\u200b.png")).toBe("x.png");
    expect(displayName("")).toBe("file");
    // NOT the upload sanitizer: a slash is left alone (the tokenizer guarantees
    // the display segment has none, and this must not invent path structure).
    expect(displayName("plain name.png")).toBe("plain name.png");
  });
});
