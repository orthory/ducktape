// Shared transport for the iced-agent CLI and MCP server.
// Zero npm deps — Bun's built-in TCP + file APIs only.

export const DEFAULT_APP_ID = "com.ducktape.app";

export function endpointPath(appId: string): string {
  // Must mirror the bridge's base_dir order (XDG_RUNTIME_DIR|TMPDIR|TMP|/tmp).
  const base =
    process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || process.env.TMP || "/tmp";
  return `${base}/iced-agent/${appId}/endpoint.json`;
}

export interface Response {
  id: number;
  ok: boolean;
  result?: unknown;
  error?: string;
}

async function resolveEndpoint(appId: string): Promise<{ host: string; port: number }> {
  const p = endpointPath(appId);
  const file = Bun.file(p);
  if (!(await file.exists())) {
    throw new Error(`no endpoint.json at ${p} — is the app running (dev build)?`);
  }
  const ep = JSON.parse(await file.text());
  if (typeof ep.port !== "number") throw new Error(`endpoint.json has no tcp port: ${p}`);
  return { host: ep.host ?? "127.0.0.1", port: ep.port };
}

// One JSON line out, one JSON line back, over a fresh loopback connection.
function roundtrip(host: string, port: number, request: unknown): Promise<Response> {
  const line = JSON.stringify(request) + "\n";
  return new Promise((resolve, reject) => {
    let buf = "";
    let settled = false;
    Bun.connect({
      hostname: host,
      port,
      socket: {
        open(socket) {
          socket.write(line);
        },
        data(socket, chunk) {
          buf += chunk.toString();
          const nl = buf.indexOf("\n");
          if (nl >= 0 && !settled) {
            settled = true;
            socket.end();
            try {
              resolve(JSON.parse(buf.slice(0, nl)));
            } catch (e) {
              reject(e);
            }
          }
        },
        error(_socket, err) {
          if (!settled) {
            settled = true;
            reject(err);
          }
        },
        close() {
          if (!settled) {
            settled = true;
            reject(new Error("connection closed before a response line"));
          }
        },
      },
    }).catch((e) => {
      if (!settled) {
        settled = true;
        reject(e);
      }
    });
  });
}

let counter = 0;

// Send one protocol command and return its Response.
export async function send(appId: string, cmd: object): Promise<Response> {
  const ep = await resolveEndpoint(appId);
  return roundtrip(ep.host, ep.port, { id: ++counter, cmd });
}
