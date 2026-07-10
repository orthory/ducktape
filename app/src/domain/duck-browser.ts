// `.duck` address resolution for Ducktape's browser.
//
// `net.duck` is the reserved network-owned DuckFS root and renders as inert,
// sanitized markup. Every account address resolves through DuckDNS to one
// Identity account and then one signed gateway route. Its target type is not
// encoded in the address: DuckFS and loopback HTTP use the same isolated,
// route-scoped browser session.

import * as duckdns from "./duckdns-client";
import * as files from "./files-client";
import * as gateway from "./gateway-client";
import * as identity from "./identity-client";
import { isTauri } from "./node-bootstrap";
import type { FileEntry, NodeTransport } from "./transport";

export const NETWORK_CONTENT_ROOT = "/shared/.duck/net";

export interface DuckAddress {
  kind: "network" | "account";
  handle: string;
  name: gateway.RouteName;
  hostname: string;
  pathAndQuery: string;
  canonical: string;
}

export interface LoadedDuckPage {
  address: DuckAddress;
  hosting: "network" | "gateway";
  target?: "duck_fs" | "loopback_http";
  accountId?: string;
  publisherNode?: string;
  signer?: string;
  revision?: number;
  snapshot?: string;
  title: string;
  srcDoc?: string;
  srcUrl?: string;
  fileCount: number;
  totalBytes: number;
}

const utf8 = new TextDecoder("utf-8", { fatal: true });
const BLOCKED_CSS = /url\s*\(|@import|@namespace|expression\s*\(|behavior\s*:|-moz-binding|javascript\s*:|data\s*:/i;
const ALLOWED_TAGS = new Set([
  "html", "head", "body", "title", "style", "main", "section", "article",
  "header", "footer", "nav", "aside", "div", "span", "p", "h1", "h2",
  "h3", "h4", "h5", "h6", "ul", "ol", "li", "dl", "dt", "dd",
  "blockquote", "pre", "code", "strong", "em", "b", "i", "u", "s",
  "small", "mark", "sub", "sup", "br", "hr", "a", "img", "figure",
  "figcaption", "table", "thead", "tbody", "tfoot", "tr", "th", "td",
  "caption",
]);
const SAFE_ATTRIBUTES = new Set(["id", "class", "title", "lang", "dir", "role"]);
const NETWORK_CSP = [
  "default-src 'none'",
  "script-src 'none'",
  "connect-src 'none'",
  "img-src data:",
  "style-src 'unsafe-inline'",
  "font-src 'none'",
  "media-src 'none'",
  "frame-src 'none'",
  "object-src 'none'",
  "form-action 'none'",
  "base-uri 'none'",
].join("; ");

const validateOriginForm = (pathAndQuery: string): void => {
  if (
    !pathAndQuery.startsWith("/") ||
    pathAndQuery.startsWith("//") ||
    pathAndQuery.length > 2048 ||
    /[\r\n\\#]/.test(pathAndQuery) ||
    !/^[\x21-\x7e]+$/.test(pathAndQuery)
  ) {
    throw new Error("Enter a bounded .duck path without a fragment.");
  }
};

export const parseDuckAddress = (input: string): DuckAddress => {
  let value = input.trim();
  if (/^duck:\/\//i.test(value)) value = value.slice("duck://".length);
  if (!value || /[#@\\\s]/.test(value)) {
    throw new Error("Enter a direct .duck address without credentials or a fragment.");
  }
  const slash = value.indexOf("/");
  const hostname = (slash < 0 ? value : value.slice(0, slash)).toLowerCase();
  const tail = slash < 0 ? "" : value.slice(slash + 1);
  const labels = hostname.split(".");
  if (!hostname.endsWith(".duck") || (labels.length !== 2 && labels.length !== 3)) {
    throw new Error("Use <account>.duck or <label>.<account>.duck.");
  }
  if (labels[labels.length - 1] !== "duck") throw new Error("Address must end in .duck.");
  const isNetwork = hostname === "net.duck";
  if (labels.length === 3 && labels[1] === "net") {
    throw new Error("net.duck is reserved and has no account subdomains.");
  }
  const handle = labels.length === 3 ? labels[1] : labels[0];
  const name = gateway.routeName(labels.length === 3 ? labels[0] : null);
  if (!isNetwork) {
    const problem = duckdns.handleError(handle);
    if (problem) throw new Error(problem);
    gateway.validateRouteName(name);
  } else if (labels.length !== 2) {
    throw new Error("net.duck is the only network-owned address.");
  }
  const pathAndQuery = `/${tail}`;
  validateOriginForm(pathAndQuery);
  if (isNetwork && pathAndQuery.includes("?")) {
    throw new Error("net.duck content does not accept query strings.");
  }
  return {
    kind: isNetwork ? "network" : "account",
    handle,
    name,
    hostname,
    pathAndQuery,
    canonical: tail ? `${hostname}/${tail}` : hostname,
  };
};

const mimeForPath = (entry: FileEntry, path: string): string => {
  const lower = path.toLowerCase();
  const inferred = lower.endsWith(".html") ? "text/html"
    : lower.endsWith(".css") ? "text/css"
      : lower.endsWith(".js") || lower.endsWith(".mjs") ? "application/javascript"
        : lower.endsWith(".json") ? "application/json"
          : lower.endsWith(".wasm") ? "application/wasm"
            : lower.endsWith(".woff2") ? "font/woff2"
              : lower.endsWith(".gif") ? "image/gif"
                : lower.endsWith(".png") ? "image/png"
                  : lower.endsWith(".jpg") || lower.endsWith(".jpeg") ? "image/jpeg"
                    : lower.endsWith(".svg") ? "image/svg+xml"
                      : lower.endsWith(".webp") ? "image/webp"
                        : lower.endsWith(".txt") ? "text/plain"
                          : null;
  if (!inferred || !gateway.ALLOWED_CONTENT_MIME_TYPES.has(inferred)) {
    throw new Error(`Gateway does not publish this file type: ${path}`);
  }
  if (entry.meta.mime && entry.meta.mime !== inferred) {
    throw new Error(`DuckFS MIME metadata disagrees with ${path}.`);
  }
  return inferred;
};

const readExact = async (
  transport: NodeTransport,
  path: string,
  size: number,
  snapshot: string,
): Promise<Uint8Array<ArrayBuffer>> => {
  if (size > gateway.MAX_CONTENT_FILE_BYTES) throw new Error(`${path} exceeds the file cap.`);
  const range = await files.read(transport, {
    path,
    snapshot,
    offset: 0,
    len: Math.max(1, size),
  });
  const bytes = files.base64ToBytes(range.b64);
  if (!range.eof || bytes.length !== size) throw new Error(`${path} changed while reading.`);
  return bytes;
};

export const buildContentDefinition = async (
  transport: NodeTransport,
  publisherNode: string,
  name: gateway.RouteName,
  defaultPath = "index.html",
): Promise<gateway.DuckFsContent> => {
  gateway.validateContentPath(defaultPath);
  const root = gateway.contentRoot(publisherNode, name);
  const snapshot = (await files.refs(transport)).head;
  if (!snapshot) throw new Error("DuckFS is empty.");
  const entries: FileEntry[] = [];
  let after: string | undefined;
  do {
    const page = await files.find(transport, {
      prefix: `${root}/`,
      snapshot,
      after,
      limit: 256,
    });
    entries.push(...page.entries);
    if (entries.length > gateway.MAX_CONTENT_FILES * 2) {
      throw new Error("Gateway content tree is too large.");
    }
    after = page.next ?? undefined;
  } while (after);
  if (entries.some((entry) => entry.kind === "symlink")) {
    throw new Error("Gateway content cannot contain symlinks.");
  }
  const fileEntries = entries.filter((entry) => entry.kind === "file");
  if (fileEntries.length === 0 || fileEntries.length > gateway.MAX_CONTENT_FILES) {
    throw new Error(`Gateway content requires 1..${gateway.MAX_CONTENT_FILES} files.`);
  }
  let total = 0;
  const declarations: gateway.ContentFile[] = [];
  for (const entry of fileEntries) {
    if (!entry.path.startsWith(`${root}/`)) throw new Error("Gateway content escaped its root.");
    const path = entry.path.slice(root.length + 1);
    gateway.validateContentPath(path);
    const bytes = await readExact(transport, entry.path, entry.size, snapshot);
    total += bytes.length;
    if (total > gateway.MAX_CONTENT_TOTAL_BYTES) throw new Error("Gateway content is too large.");
    declarations.push({
      path,
      mime: mimeForPath(entry, path),
      size: bytes.length,
      sha256: gateway.sha256Hex(bytes),
    });
  }
  // Rust's canonical manifest uses bytewise `String` ordering. Every accepted
  // path is ASCII, so an explicit code-unit comparison is byte-identical;
  // locale collation is deliberately forbidden here.
  declarations.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  const content = { default_path: defaultPath, files: declarations };
  gateway.validateContent(content);
  return content;
};

const sanitizeNetworkDocument = (html: string): { srcDoc: string; title: string } => {
  const parsed = new DOMParser().parseFromString(html, "text/html");
  for (const element of [...parsed.querySelectorAll("*")]) {
    const tag = element.tagName.toLowerCase();
    if (!ALLOWED_TAGS.has(tag)) {
      element.remove();
      continue;
    }
    for (const attribute of [...element.attributes]) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value;
      const safeLink = tag === "a" && name === "href" && value.startsWith("#");
      const safeImage = tag === "img" && name === "src" && /^data:image\/(?:gif|jpeg|png|webp);base64,/i.test(value);
      const safeAlt = tag === "img" && name === "alt";
      if (!SAFE_ATTRIBUTES.has(name) && !safeLink && !safeImage && !safeAlt) {
        element.removeAttribute(attribute.name);
      }
    }
    if (tag === "style" && BLOCKED_CSS.test(element.textContent ?? "")) element.remove();
  }
  for (const existing of [...parsed.head.querySelectorAll('meta[http-equiv="Content-Security-Policy"]')]) {
    existing.remove();
  }
  const csp = parsed.createElement("meta");
  csp.httpEquiv = "Content-Security-Policy";
  csp.content = NETWORK_CSP;
  parsed.head.prepend(csp);
  return {
    srcDoc: `<!doctype html>${parsed.documentElement.outerHTML}`,
    title: parsed.title.trim().slice(0, 128) || "net.duck",
  };
};

const loadNetworkPage = async (
  transport: NodeTransport,
  address: DuckAddress,
): Promise<LoadedDuckPage> => {
  const relative = address.pathAndQuery === "/"
    ? "index.html"
    : address.pathAndQuery.slice(1);
  gateway.validateContentPath(relative);
  if (!relative.toLowerCase().endsWith(".html")) throw new Error("net.duck opens HTML documents only.");
  const snapshot = (await files.refs(transport)).head;
  if (!snapshot) throw new Error("net.duck has no DuckFS snapshot.");
  const absolute = `${NETWORK_CONTENT_ROOT}/${relative}`;
  const entry = await files.stat(transport, { path: absolute, snapshot });
  if (!entry || entry.kind !== "file") throw new Error(`net.duck page not found: ${relative}`);
  if (mimeForPath(entry, relative) !== "text/html") throw new Error("net.duck page is not HTML.");
  const bytes = await readExact(transport, absolute, entry.size, snapshot);
  const rendered = sanitizeNetworkDocument(utf8.decode(bytes));
  return {
    address,
    hosting: "network",
    snapshot,
    title: rendered.title,
    srcDoc: rendered.srcDoc,
    fileCount: 1,
    totalBytes: bytes.length,
  };
};

const safeSessionUrl = (raw: string, pathAndQuery: string): string => {
  const root = new URL(raw);
  if (
    root.protocol !== "http:" ||
    !/^[0-9a-f]{32}\.localhost$/.test(root.hostname) ||
    !root.port ||
    root.username ||
    root.password ||
    root.hash ||
    root.pathname !== "/" ||
    root.search
  ) {
    throw new Error("Node returned an unsafe gateway session origin.");
  }
  return new URL(pathAndQuery, root.origin).toString();
};

const loadAccountRoute = async (
  transport: NodeTransport,
  address: DuckAddress,
): Promise<LoadedDuckPage> => {
  if (!isTauri()) throw new Error("Account routes require the isolated desktop browser window.");
  if (!transport.gatewaySession) throw new Error("This node has no active gateway browser plane.");
  const resolved = await duckdns.resolve(transport, { handle: address.handle });
  if (!resolved) throw new Error(`${address.handle}.duck is not registered.`);
  const accountId = gateway.bytesToHex(resolved.account_id);
  const [account, record] = await Promise.all([
    identity.getAccount(transport, accountId),
    gateway.getRoute(transport, resolved.account_id, address.name),
  ]);
  if (!account) throw new Error("The resolved account no longer exists.");
  if (!record?.statement.route) throw new Error(`${address.hostname} has no published gateway route.`);
  gateway.verifyRecord(record, account);

  const session = await transport.gatewaySession({
    accountId: resolved.account_id,
    name: address.name,
    revision: record.statement.revision,
  });
  const srcUrl = safeSessionUrl(session.url, address.pathAndQuery);

  // Close the resolution/session race. The publisher also re-resolves on each
  // request, but the UI should not present a session minted for stale authority.
  const [latestResolution, latestAccount, latestRecord] = await Promise.all([
    duckdns.resolve(transport, { handle: address.handle }),
    identity.getAccount(transport, accountId),
    gateway.getRoute(transport, resolved.account_id, address.name),
  ]);
  if (
    !latestResolution ||
    gateway.bytesToHex(latestResolution.account_id) !== accountId ||
    !latestAccount ||
    !latestRecord?.statement.route
  ) {
    throw new Error("Gateway authority changed while the session was opening.");
  }
  gateway.verifyRecord(latestRecord, latestAccount);
  if (
    latestRecord.statement.revision !== record.statement.revision ||
    gateway.bytesToHex(latestRecord.statement.publisher_node) !==
      gateway.bytesToHex(record.statement.publisher_node)
  ) {
    throw new Error("Gateway route changed while the session was opening. Reload it.");
  }

  return {
    address,
    hosting: "gateway",
    target: record.statement.route.target.kind,
    accountId,
    publisherNode: gateway.bytesToHex(record.statement.publisher_node),
    signer: gateway.bytesToHex(record.authorization.signer),
    revision: record.statement.revision,
    title: address.hostname,
    srcUrl,
    fileCount: record.statement.route.target.kind === "duck_fs"
      ? record.statement.route.target.content.files.length
      : 0,
    totalBytes: record.statement.route.target.kind === "duck_fs"
      ? record.statement.route.target.content.files.reduce((total, file) => total + file.size, 0)
      : 0,
  };
};

export const loadDuckPage = async (
  transport: NodeTransport,
  input: string,
): Promise<LoadedDuckPage> => {
  const address = parseDuckAddress(input);
  return address.kind === "network"
    ? loadNetworkPage(transport, address)
    : loadAccountRoute(transport, address);
};

export const starterDocument = (name: string, address: string): Uint8Array<ArrayBuffer> =>
  new TextEncoder().encode(`<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>${name.replace(/[<>&"]/g, "")}</title>
  <style>body{font:16px system-ui;max-width:720px;margin:12vh auto;padding:24px;color:#242422}code{background:#eee;padding:2px 5px;border-radius:4px}</style>
</head>
<body>
  <h1>${name.replace(/[<>&"]/g, "")}</h1>
  <p>This route is published at <code>${address.replace(/[<>&"]/g, "")}</code>.</p>
  <p>Static files and same-route API calls share one signed gateway policy.</p>
</body>
</html>`);
