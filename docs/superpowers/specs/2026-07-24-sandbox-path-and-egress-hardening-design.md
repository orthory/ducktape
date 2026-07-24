# Sandbox hardening: socket-driven podman, neutral paths, egress allowlist

Date: 2026-07-24 (revised 2026-07-25 for the socket pivot)
Crate: `crates/modules/system/capability-host` (+ `bin/node`)
Status: foundation landed + unit-tested; integration UNVERIFIED (no live podman here)

## Problem

A remote account in a lent-credential Claude Code PTY session on this node
sees real host paths — cwd `/home/eddy/.ducktape/provider-runs/<run>/workspace`,
PATH/skills mounted at their identical host paths, `$HOME` a host layout —
leaking the operator's username, directory structure, and node layout. And the
container's `--network=slirp4netns` NAT reaches every routable destination the
host can: the host's LAN, the tailnet (`100.64.0.0/10`), link-local, any host
service on a routable interface.

Both were reported together as "make the sandbox more sandboxed."

## Decision: drive podman over its rootless socket, not the CLI

Beyond the two leaks, the sandbox is moved OFF the `podman` CLI subprocess and
onto the libpod REST API over a **node-owned rootless podman socket**. No
`Command::new("podman")` anywhere in the run path.

- **The node owns a rootless podman service.** It supervises
  `podman system service --time=0 --hooks-dir <datadir>/podman-hooks unix://<sock>`
  and talks to `<sock>`. Starting the service with `--hooks-dir` is how the
  egress hook is delivered — the socket API has no per-run `--hooks-dir`.
- **Rootless user service only.** Containers stay in the node user's
  namespaces, so the egress hook's `nsenter -U --net` into the container pid
  works and the node never talks to a root daemon.
- **A minimal in-tree libpod client** (`podman_api.rs`), hand-rolled over
  `tokio::net::UnixStream` — no new dependency. Endpoints: create, start,
  attach (raw hijacked stream), wait, resize, kill, remove.

No-Legacy doctrine: the CLI-argv path is deleted outright, not kept behind a
flag. There are zero live networks; nothing needs the old mechanism.

## Part A — neutral container paths (`podman_api::plan_mounts` + `translate`)

Every host path maps to a NEUTRAL `/ducktape/*` guest path in the
`SpecGenerator`, and every env value / argv entry is translated to match.

| host path | container path | mode |
|---|---|---|
| run workdir | `/ducktape/workspace` | rw, `work_dir` |
| executor bin | `/ducktape/bin/<name>` | ro |
| each `rw_dir` (CLI auth/state under HOME) | `/ducktape/home/<rel>` | rw |
| workspace-parent context doc (a file in ro_paths) | `/ducktape/<name>` | ro |
| other ro_paths (PATH dirs, skills) | `/ducktape/ro<i>` | ro |
| `HOME` | `/ducktape/home` | — |

`translate` does longest-host-prefix-first substring replacement over the mount
table plus a synthetic `HOME → /ducktape/home`, covering the codex
`projects."<workdir>"` TOML key and any stray `$HOME`-prefixed value. The bind
mount's `source` (host side) still carries the real path — that is invisible to
the guest, which only sees `destination`.

Session identity that keyed on the workspace path now uses the stable
`/ducktape/workspace`; run uniqueness is carried by the container id + labels,
not the guest path.

## Part B — egress allowlist in the container netns

`SpecGenerator`:
- `netns = {nsmode: "slirp4netns"}` + `network_options.slirp4netns =
  ["allow_host_loopback=false", "enable_ipv6=false"]`.
- `cap_drop = ["NET_ADMIN", "NET_RAW"]` — the workload cannot alter the
  firewall or open raw sockets.
- annotations `io.ducktape.egress=1`, `io.ducktape.egress.host=<ip>`,
  `io.ducktape.egress.ports=<broker>,<node>`.

**The firewall is an OCI `createRuntime` hook (fails closed).** Podman runs it
after the netns exists but before the workload execs; a hook failure aborts the
container, so a firewall that fails to install means the run never starts. The
hook is the node binary's hidden `__egress-hook` subcommand: it reads the OCI
state JSON on stdin (pid + bundle), reads the run's annotations, generates the
ruleset with `capability_host::egress_nftables`, and runs
`nsenter -U --net -t <pid> -- nft -f -`.

`egress_nftables(host_ip, ports)` ruleset, order load-bearing:
```
oifname "lo" accept
ip daddr <host_ip> tcp dport { <broker>, <node> } accept   # broker + node RPC
udp/tcp dport 53 accept                                    # DNS (slirp resolver)
ip daddr { 10/8, 172.16/12, 192.168/16, 100.64/10, 169.254/16, 127/8 } drop
# public internet falls through to policy accept
```
The broker/node accept precedes the private-range drop so the two allowed
host:port pairs survive even though `host_ip` is in a dropped range.
`host_ip` is what `host.containers.internal` resolves to (host default-route
source IP, computed host-side via a UDP-connect probe).

`nft` + `nsenter` (found via `/usr/sbin` too, off a non-root PATH) become boot
probes alongside `slirp4netns`.

## Client (`podman_api.rs`) — landed + unit-tested

Hand-rolled HTTP/1.1 over `UnixStream`. `request` uses `Connection: close`
(read-to-EOF) with a `Content-Length`/chunked body parser; `attach` sends the
`Upgrade` request, reads only the response head, and hands back the hijacked
`UnixStream` split into read/write halves. Headless reads demux the 8-byte
Docker mux header (`[stream,0,0,0,len_be]`); a tty session reads raw. Resize is
a `POST .../resize?w=&h=` call (the SIGWINCH relay).

Unit tests (pass, no podman needed): HTTP content-length + chunked parsing,
error-body surfacing, attach frame demux, `plan_mounts` neutrality, `translate`
of a codex arg + `$HOME` sanitization, `SpecGenerator` JSON shape, egress
ruleset ordering.

## Integration (UNVERIFIED — needs real-node QA)

- `SandboxBackend::Podman { image, socket }` carries the node's socket path.
- `podman_command`/`invoke` → build spec, create, start, attach; headless
  demuxes stdout/stderr and `wait`s the exit code; kill/remove on
  timeout/cancel. The cidfile lifecycle is deleted (the container id replaces
  it); reaping lists containers by label over the socket.
- `interactive.rs`: `InteractiveSession` becomes a transport enum — a socket
  attach stream (Podman) or the existing local pty (Tart ssh / vendor login).
  Resize calls the socket; close kills+removes.
- `bin/node`: `__egress-hook` subcommand; supervise the rootless podman
  service with `--hooks-dir`; write the hook JSON (path = current_exe).

## Live QA (real node — this dev box has no working podman)

- `pwd` = `/ducktape/workspace`; `env` shows `HOME=/ducktape/home`; no host
  path visible.
- `curl` to a tailnet IP and a LAN IP both fail; broker PONG works; a public
  `curl`/`npm` fetch succeeds.
- Interactive TUI renders and drives over the attach stream; resize works.
- Hook failure aborts the container (fail-closed): break the ruleset, confirm
  the run refuses to start.
