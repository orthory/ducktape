// Typed mirror of the consensus `gateway` module. DuckDNS only resolves a
// human name to AccountId; this module then verifies one signed, monotonic
// account route whose target is either exact DuckFS content or node-local HTTP.

import { ed25519 } from "@noble/curves/ed25519.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { invoke } from "@tauri-apps/api/core";

import type { AccountView } from "./identity-client";
import { isTauri } from "./node-bootstrap";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

export const TARGET = "gateway";
export const ROUTE_FORMAT_VERSION = 1;
export const GATEWAY_ROUTE_NAMESPACE = "ducktape-gateway-route-v1";
export const MAX_CONTENT_FILES = 128;
export const MAX_CONTENT_PATH_BYTES = 512;
export const MAX_CONTENT_SEGMENT_BYTES = 128;
export const MAX_CONTENT_FILE_BYTES = 1024 * 1024;
export const MAX_CONTENT_TOTAL_BYTES = 4 * 1024 * 1024;
export const MAX_REQUEST_BODY_BYTES = 1024 * 1024;
export const MAX_RESPONSE_BODY_BYTES = 4 * 1024 * 1024;

export const ALLOWED_CONTENT_MIME_TYPES = new Set([
  "application/javascript",
  "application/json",
  "application/wasm",
  "font/woff2",
  "image/gif",
  "image/jpeg",
  "image/png",
  "image/svg+xml",
  "image/webp",
  "text/css",
  "text/html",
  "text/plain",
]);

export interface RouteName {
  label: string | null;
}

export type RouteMethod = "get" | "head" | "post" | "put" | "patch" | "delete";

export type RouteAudience =
  | { kind: "owner" }
  | { kind: "network" }
  | { kind: "accounts"; account_ids: number[][] };

export interface RoutePolicy {
  audience: RouteAudience;
  methods: RouteMethod[];
  max_request_bytes: number;
  max_response_bytes: number;
  allow_authorization: boolean;
}

export interface ContentFile {
  path: string;
  mime: string;
  size: number;
  sha256: string;
}

export interface DuckFsContent {
  default_path: string | null;
  files: ContentFile[];
}

export type RouteTarget =
  | { kind: "duck_fs"; content: DuckFsContent }
  | { kind: "loopback_http" };

export interface RouteDefinition {
  target: RouteTarget;
  policy: RoutePolicy;
}

export interface RouteStatement {
  version: number;
  chain_id: string;
  account_id: number[];
  name: RouteName;
  publisher_node: number[];
  revision: number;
  route: RouteDefinition | null;
}

export interface MemberAuthorization {
  signer: number[];
  signature: number[];
}

export interface RouteRecord {
  statement: RouteStatement;
  authorization: MemberAuthorization;
}

export interface RouteSummary {
  name: RouteName;
  publisher_node: number[];
  revision: number;
  target: "duck_fs" | "loopback_http";
}

export interface RouteHealthProbe {
  path: string;
  status: number;
}

interface SetRouteMessage {
  set_route: {
    statement: RouteStatement;
    authorization: MemberAuthorization;
  };
}

const encoder = new TextEncoder();
const METHOD_ORDER: RouteMethod[] = ["get", "head", "post", "put", "patch", "delete"];
const METHOD_ID: Record<RouteMethod, number> = {
  get: 1,
  head: 2,
  post: 3,
  put: 4,
  patch: 5,
  delete: 6,
};

const bytesEqual = (left: ArrayLike<number>, right: ArrayLike<number>): boolean => {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
};

const compareBytes = (left: number[], right: number[]): number => {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return left.length - right.length;
};

export const bytesToHex = (bytes: ArrayLike<number>): string => {
  let out = "";
  for (let index = 0; index < bytes.length; index += 1) {
    out += bytes[index].toString(16).padStart(2, "0");
  }
  return out;
};

const hexToBytes = (hex: string): Uint8Array => {
  if (!/^(?:[0-9a-f]{2})+$/.test(hex)) throw new Error("gateway: expected lowercase hex");
  const out = new Uint8Array(hex.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
};

const validateBytes = (value: number[], length: number | null, label: string): void => {
  if (
    !Array.isArray(value) ||
    (length !== null && value.length !== length) ||
    value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) {
    throw new Error(`gateway: invalid ${label}`);
  }
};

const validateAccountId = (accountId: number[]): void => {
  validateBytes(accountId, null, "account id");
  if (accountId.length === 0 || accountId.length > 128) {
    throw new Error("gateway: invalid account id length");
  }
};

const validateU64 = (value: number, label: string): void => {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`gateway: invalid ${label}`);
};

const pushU64 = (out: number[], value: number): void => {
  validateU64(value, "u64");
  let remaining = BigInt(value);
  for (let index = 0; index < 8; index += 1) {
    out.push(Number(remaining & 0xffn));
    remaining >>= 8n;
  }
};

const pushBytes = (out: number[], bytes: ArrayLike<number>): void => {
  pushU64(out, bytes.length);
  for (let index = 0; index < bytes.length; index += 1) out.push(bytes[index]);
};

export const routeName = (label?: string | null): RouteName => ({
  label: label?.trim() ? label.trim().toLowerCase() : null,
});

export const validateRouteName = (name: RouteName): void => {
  if (name.label === null) return;
  const label = name.label;
  if (
    !label ||
    label.length > 63 ||
    label.startsWith("-") ||
    label.endsWith("-") ||
    !/^[a-z0-9-]+$/.test(label)
  ) {
    throw new Error(`gateway: invalid route label: ${label}`);
  }
};

export const validateContentPath = (path: string): void => {
  if (!path || encoder.encode(path).length > MAX_CONTENT_PATH_BYTES || path.startsWith("/")) {
    throw new Error("gateway: content path must be a bounded relative path");
  }
  for (const segment of path.split("/")) {
    if (
      !segment ||
      segment === "." ||
      segment === ".." ||
      encoder.encode(segment).length > MAX_CONTENT_SEGMENT_BYTES ||
      !/^[A-Za-z0-9._-]+$/.test(segment)
    ) {
      throw new Error(`gateway: non-canonical content path: ${path}`);
    }
  }
};

export const validatePolicy = (policy: RoutePolicy): void => {
  if (!Array.isArray(policy.methods) || policy.methods.length === 0) {
    throw new Error("gateway: policy must allow at least one method");
  }
  let previous = -1;
  for (const method of policy.methods) {
    const index = METHOD_ORDER.indexOf(method);
    if (index <= previous) throw new Error("gateway: methods must be sorted and unique");
    previous = index;
  }
  validateU64(policy.max_request_bytes, "request cap");
  validateU64(policy.max_response_bytes, "response cap");
  if (policy.max_request_bytes > MAX_REQUEST_BODY_BYTES) {
    throw new Error("gateway: request cap is too large");
  }
  if (policy.max_response_bytes < 1 || policy.max_response_bytes > MAX_RESPONSE_BODY_BYTES) {
    throw new Error("gateway: response cap is out of range");
  }
  if (
    policy.methods.some((method) => ["post", "put", "patch", "delete"].includes(method)) &&
    policy.max_request_bytes === 0
  ) {
    throw new Error("gateway: body-bearing methods require a request cap");
  }
  if (typeof policy.allow_authorization !== "boolean") {
    throw new Error("gateway: invalid Authorization policy");
  }
  if (policy.audience.kind === "accounts") {
    const accounts = policy.audience.account_ids;
    if (!Array.isArray(accounts) || accounts.length === 0 || accounts.length > 32) {
      throw new Error("gateway: invalid explicit audience");
    }
    let previousAccount: number[] | null = null;
    for (const accountId of accounts) {
      validateAccountId(accountId);
      if (previousAccount && compareBytes(previousAccount, accountId) >= 0) {
        throw new Error("gateway: audience accounts must be sorted and unique");
      }
      previousAccount = accountId;
    }
  } else if (policy.audience.kind !== "owner" && policy.audience.kind !== "network") {
    throw new Error("gateway: unsupported audience");
  }
};

export const validateContent = (content: DuckFsContent): void => {
  if (!Array.isArray(content.files) || content.files.length === 0 || content.files.length > MAX_CONTENT_FILES) {
    throw new Error("gateway: invalid DuckFS file count");
  }
  if (content.default_path !== null) validateContentPath(content.default_path);
  let previous = "";
  let total = 0;
  let defaultExists = content.default_path === null;
  for (const file of content.files) {
    validateContentPath(file.path);
    if (previous && previous >= file.path) throw new Error("gateway: files must be path-sorted");
    previous = file.path;
    if (!ALLOWED_CONTENT_MIME_TYPES.has(file.mime)) {
      throw new Error(`gateway: MIME type is not allowed: ${file.mime}`);
    }
    validateU64(file.size, `size for ${file.path}`);
    if (file.size > MAX_CONTENT_FILE_BYTES) throw new Error(`gateway: ${file.path} is too large`);
    total += file.size;
    if (total > MAX_CONTENT_TOTAL_BYTES) throw new Error("gateway: DuckFS target is too large");
    if (!/^[0-9a-f]{64}$/.test(file.sha256)) {
      throw new Error(`gateway: invalid SHA-256 for ${file.path}`);
    }
    if (file.path === content.default_path) defaultExists = true;
  }
  if (!defaultExists) throw new Error("gateway: default path is not declared");
};

export const validateStatement = (statement: RouteStatement): void => {
  if (statement.version !== ROUTE_FORMAT_VERSION) throw new Error("gateway: unsupported version");
  const chainBytes = encoder.encode(statement.chain_id);
  if (chainBytes.length === 0 || chainBytes.length > 256) throw new Error("gateway: invalid chain id");
  validateAccountId(statement.account_id);
  validateRouteName(statement.name);
  validateBytes(statement.publisher_node, 32, "publisher node");
  validateU64(statement.revision, "revision");
  if (statement.revision < 1) throw new Error("gateway: revision starts at 1");
  if (!statement.route) return;
  validatePolicy(statement.route.policy);
  if (statement.route.target.kind === "duck_fs") {
    validateContent(statement.route.target.content);
    const policy = statement.route.policy;
    if (
      policy.methods.length !== 2 ||
      policy.methods[0] !== "get" ||
      policy.methods[1] !== "head" ||
      policy.max_request_bytes !== 0 ||
      policy.allow_authorization
    ) {
      throw new Error("gateway: DuckFS routes require GET+HEAD without request credentials");
    }
    if (statement.route.target.content.files.some((file) => file.size > policy.max_response_bytes)) {
      throw new Error("gateway: a DuckFS file exceeds the response cap");
    }
  } else if (statement.route.target.kind !== "loopback_http") {
    throw new Error("gateway: unsupported route target");
  }
};

const encodePolicy = (out: number[], policy: RoutePolicy): void => {
  if (policy.audience.kind === "owner") out.push(1);
  else if (policy.audience.kind === "network") out.push(2);
  else {
    out.push(3);
    pushU64(out, policy.audience.account_ids.length);
    for (const accountId of policy.audience.account_ids) pushBytes(out, accountId);
  }
  pushU64(out, policy.methods.length);
  for (const method of policy.methods) out.push(METHOD_ID[method]);
  pushU64(out, policy.max_request_bytes);
  pushU64(out, policy.max_response_bytes);
  out.push(policy.allow_authorization ? 1 : 0);
};

/** Byte-identical mirror of `gateway::route_signing_preimage`. */
export const routeSigningPreimage = (statement: RouteStatement): Uint8Array => {
  validateStatement(statement);
  const out: number[] = [statement.version];
  pushBytes(out, encoder.encode(statement.chain_id));
  pushBytes(out, statement.account_id);
  if (statement.name.label === null) out.push(0);
  else {
    out.push(1);
    pushBytes(out, encoder.encode(statement.name.label));
  }
  pushBytes(out, statement.publisher_node);
  pushU64(out, statement.revision);
  if (!statement.route) {
    out.push(0);
    return Uint8Array.from(out);
  }
  out.push(1);
  encodePolicy(out, statement.route.policy);
  if (statement.route.target.kind === "duck_fs") {
    out.push(1);
    const content = statement.route.target.content;
    if (content.default_path === null) out.push(0);
    else {
      out.push(1);
      pushBytes(out, encoder.encode(content.default_path));
    }
    pushU64(out, content.files.length);
    for (const file of content.files) {
      pushBytes(out, encoder.encode(file.path));
      pushBytes(out, encoder.encode(file.mime));
      pushU64(out, file.size);
      out.push(...hexToBytes(file.sha256));
    }
  } else {
    out.push(2);
  }
  return Uint8Array.from(out);
};

export const verificationPayload = (statement: RouteStatement): Uint8Array => {
  const namespace = encoder.encode(GATEWAY_ROUTE_NAMESPACE);
  if (namespace.length >= 128) throw new Error("gateway: signing namespace is too long");
  const preimage = routeSigningPreimage(statement);
  const out = new Uint8Array(1 + namespace.length + preimage.length);
  out[0] = namespace.length;
  out.set(namespace, 1);
  out.set(preimage, 1 + namespace.length);
  return out;
};

export const verifyRecord = (record: RouteRecord, account: AccountView): void => {
  validateStatement(record.statement);
  validateBytes(record.authorization.signer, 32, "signer");
  validateBytes(record.authorization.signature, 64, "signature");
  if (!bytesEqual(record.statement.account_id, account.account_id)) {
    throw new Error("gateway: route account does not match Identity");
  }
  if (!account.nodes.some((node) => bytesEqual(node, record.statement.publisher_node))) {
    throw new Error("gateway: publisher node is no longer bound to the account");
  }
  if (!account.member_keys.some(
    (member) => member.kind === "ed25519" && bytesEqual(member.pubkey, record.authorization.signer),
  )) {
    throw new Error("gateway: signer is no longer an account member");
  }
  if (!ed25519.verify(
    Uint8Array.from(record.authorization.signature),
    verificationPayload(record.statement),
    Uint8Array.from(record.authorization.signer),
    { zip215: true },
  )) {
    throw new Error("gateway: route signature does not verify");
  }
};

export const getRoute = (
  transport: NodeTransport,
  accountId: number[],
  name: RouteName,
): Promise<RouteRecord | null> => {
  validateAccountId(accountId);
  validateRouteName(name);
  return Promise.resolve()
    .then(() => transport.query(TARGET, { get: { account_id: accountId, name } }))
    .then((reply) => replyVariant<RouteRecord | null>(reply, "route"));
};

/** Bounded summaries for every live route of one account, in canonical
 * apex-then-label order. Manifests, policies, signatures, and signed
 * tombstones stay behind getRoute. */
export const listRoutes = (
  transport: NodeTransport,
  accountId: number[],
): Promise<RouteSummary[]> => {
  validateAccountId(accountId);
  return Promise.resolve()
    .then(() => transport.query(TARGET, { list: { account_id: accountId } }))
    .then((reply) => replyVariant<RouteSummary[]>(reply, "routes"));
};

/** Exercise the finalized route through the same authenticated gateway plane
 * as a real caller. The probe is deliberately credential-free and bodyless;
 * routes that did not explicitly sign HEAD are never probed implicitly. */
export const probeRouteHealth = async (
  transport: NodeTransport,
  record: RouteRecord,
): Promise<RouteHealthProbe> => {
  validateStatement(record.statement);
  const route = record.statement.route;
  if (!route) throw new Error("gateway: cannot probe an unpublished route");
  if (!route.policy.methods.includes("head")) {
    throw new Error("gateway: route health requires HEAD in the signed policy");
  }
  if (!transport.gatewayProxy) {
    throw new Error("gateway: this node has no active gateway plane");
  }
  const path = route.target.kind === "duck_fs"
    ? `/${route.target.content.default_path ?? route.target.content.files[0].path}`
    : "/";
  const reply = await transport.gatewayProxy({
    head: {
      account_id: record.statement.account_id,
      name: record.statement.name,
      revision: record.statement.revision,
      method: "head",
      path_and_query: path,
      headers: [],
      body_len: 0,
    },
    body: new Uint8Array(0),
  });
  return { path, status: reply.head.status };
};

export const signStatement = async (statement: RouteStatement): Promise<SetRouteMessage> => {
  validateStatement(statement);
  if (!isTauri()) throw new Error("gateway publishing requires the desktop app");
  const messageJson = await invoke<string>("user_sign_gateway_route", {
    statement: JSON.stringify(statement),
  });
  const parsed = JSON.parse(messageJson) as SetRouteMessage;
  if (!parsed?.set_route) throw new Error("gateway signer returned an invalid message");
  if (!bytesEqual(routeSigningPreimage(statement), routeSigningPreimage(parsed.set_route.statement))) {
    throw new Error("gateway signer changed the route statement");
  }
  return parsed;
};

export const submitStatement = async (
  transport: NodeTransport,
  statement: RouteStatement,
): Promise<BlockEvent> => transport.submit(TARGET, await signStatement(statement));

export const openWindow = async (url: string, title: string): Promise<void> => {
  if (!isTauri()) throw new Error("an isolated gateway window requires the desktop app");
  await invoke<void>("gateway_open_window", { url, title });
};

// ── Inline gateway view (CEF shells only) ───────────────
// On the CEF runtime every gateway session is its own renderer process and the
// inline child webview's label matches no capability, so embedding in the
// Browser pane is as isolated as the separate window. wry shells report
// unsupported and keep the window path.

export interface InlineRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

let inlineSupport: Promise<boolean> | null = null;
export const inlineSupported = (): Promise<boolean> => {
  if (!isTauri()) return Promise.resolve(false);
  inlineSupport ??= invoke<boolean>("gateway_inline_supported").catch(() => false);
  return inlineSupport;
};

// `title` is the .duck route the user navigated to. The shell needs it because
// the session origin is a random loopback token: it is the only honest name a
// permission prompt can put in front of the user (see src-tauri/permissions.rs).
export const openInline = async (url: string, title: string, rect: InlineRect): Promise<void> => {
  await invoke<void>("gateway_open_inline", { url, title, rect });
};

export const placeInline = async (rect: InlineRect): Promise<void> => {
  await invoke<void>("gateway_inline_place", { rect });
};

export const closeInline = async (): Promise<void> => {
  await invoke<void>("gateway_inline_close").catch(() => undefined);
};

export const contentRoot = (
  publisherNode: ArrayLike<number> | string,
  name: RouteName,
): string => {
  validateRouteName(name);
  const nodeHex = typeof publisherNode === "string"
    ? publisherNode.toLowerCase()
    : bytesToHex(publisherNode);
  if (!/^[0-9a-f]{64}$/.test(nodeHex)) throw new Error("gateway: invalid publisher node");
  return `/home/ext:${nodeHex}/.duck/gateway/${name.label ?? "_apex"}`;
};

export const sha256Hex = (bytes: Uint8Array): string => bytesToHex(sha256(bytes));
