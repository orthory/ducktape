# Sandbox path anonymization + egress allowlist

Date: 2026-07-24
Crate: `crates/modules/system/capability-host`
Status: design approved, ready for planning

## Problem

A remote account that enters a lent-credential Claude Code PTY session on this
node sees real host paths inside the container: the cwd is
`/home/eddy/.ducktape/provider-runs/<run>/workspace`, PATH entries and the
skills tree are mounted at their identical host paths, and `$HOME` points at a
host directory layout. This leaks the operator's username, directory
structure, and node layout to an untrusted guest.

Second hole: the container runs with `--network=slirp4netns`, whose NAT reaches
every routable destination the host can. `host.containers.internal` resolves to
the host's real routable IP (that is how the child reaches its broker + node
RPC), so the guest can equally reach the host's LAN, the tailnet
(`100.64.0.0/10`), link-local, and any service the host exposes on a routable
interface. A private netns is not isolation as long as its NAT is unfiltered.

Both holes live behind one seam: `CliProvider::podman_command` /
`sandbox::wrap_podman_managed` in `capability-host`, used by BOTH headless runs
and interactive PTY sessions (`interactive.rs` calls the same
`podman_command`). One fix covers both.

Scope note (No-Legacy doctrine): there are zero live networks. The current
"identical container paths" contract is replaced outright — no dual path, no
compat flag. Session identity keyed on the workspace path moves to the new
fixed path in the same change.

## Part A — Neutral container paths

Drop the "mount every path at its identical host path" rule for the Podman
backend. Mount at fixed, host-blind guest paths and translate env values + argv
the same way the Tart backend already does (`translate_value`, mount tags).

| host path | container path | mode |
|---|---|---|
| run workdir (`.../provider-runs/<run>/workspace`) | `/ducktape/workspace` | rw, `-w` |
| executor bin | `/ducktape/bin/<filename>` | ro |
| each `ro_paths` entry (PATH dirs, skills tree) | `/ducktape/ro<i>` (skills: `/ducktape/skills`) | ro |
| each `rw_dirs` entry (CLI auth/state under `~/`) | `/ducktape/home/<rel-to-HOME>` | rw |
| workspace-parent context doc (outside workdir) | `/ducktape/<filename>` (one level above `/ducktape/workspace`) | ro |
| `HOME` env | `/ducktape/home` | — |

Translation rules (mirroring Tart, hoisted to shared code so both backends use
one implementation):

- Build an ordered host→guest mount table. Env values and argv strings get
  longest-host-prefix-first string replacement. This covers the codex
  `projects.<workdir>` TOML key inside `-c` args and `DUCKTAPE_RUN_SKILLS`.
- `PATH` is split on `:`, each entry translated, rejoined.
- `HOME` is emitted as `/ducktape/home` (not translated per-mount).
- The run-action URL rewrite (`127.0.0.1` → `host.containers.internal`) is
  unchanged — it is a URL, not a mount.

Result: the guest's cwd is `/ducktape/workspace`; `env`, `pwd`, and every tool
path show `/ducktape/...`. No operator username, no `.ducktape` layout, no
node data-dir hints.

The host-only cidfile (`~/.ducktape/provider-runs/podman/...`) is already never
mounted and is asserted so; unchanged.

Session/run identity: any key derived from the workspace path (e.g. codex
resume `projects.<key>`, `io.ducktape.run` label) uses the stable
`/ducktape/workspace`. Because every run mounts its own distinct host workdir at
that same guest path, the guest-side key is constant across runs by design; run
uniqueness is already carried by the host-side cidfile/labels, not the guest
path. No live network depends on the old value.

## Part B — Egress allowlist in the container netns

Keep `--network=slirp4netns` but make its flags explicit and add a firewall
that runs inside the container's netns before the workload starts.

### Netns flags (tighten the knobs)

```
--network=slirp4netns:allow_host_loopback=false,enable_ipv6=false
--cap-drop=net_admin --cap-drop=net_raw
```

- `allow_host_loopback=false`: removes the `10.0.2.2 → host 127.0.0.1`
  shortcut. The broker/node are reached via the host's routable IP through
  `host.containers.internal`, not this shortcut, so this only closes an extra
  door.
- `enable_ipv6=false`: eliminate the v6 path rather than firewall it twice.
- `--cap-drop=net_admin,net_raw`: the workload cannot alter the nft rules or
  open raw sockets. The rules are installed by a host-side hook (below), which
  needs no in-container capability.

### The firewall — an OCI `createRuntime` hook (fails closed)

Podman runs `createRuntime` hooks after the container's netns exists but
BEFORE the workload's exec. A non-zero hook aborts the container — so a firewall
that fails to install means the run never starts. That is the fail-closed
property a post-`podman run` `nsenter` cannot guarantee (workload could send
before rules land).

- Add `--hooks-dir <ducktape-owned-dir>` and
  `--annotation io.ducktape.egress=1` to the podman argv. The hook JSON in that
  dir has `when.annotations = { "io.ducktape.egress": "1" }` and
  `stages = ["createRuntime"]`, so it fires only for our containers and does not
  touch any other podman usage on the host.
- The hook command is a hidden `ducktape` subcommand (no new binary). Podman
  pipes the OCI container state JSON to it on stdin: `{ pid, bundle, ... }`.
  The subcommand reads `pid`, reads `<bundle>/config.json` for the run's
  annotations (allowed ports + host IP), then:
  `nsenter --preserve-credentials -U --net -t <pid> -- nft -f -` with the
  generated ruleset. Running on the host (only the netns is entered), it needs
  no container capabilities.
- Ports/host-IP are passed as annotations set on the podman argv:
  `io.ducktape.egress.host=<ip>`, `io.ducktape.egress.ports=<broker>,<node>`.
  The broker port comes from the broker endpoint `base_url`; the node RPC port
  from the run-action URL; the host IP is what `host.containers.internal`
  resolves to (host default-route source address, computed host-side).

### Ruleset (nft, applied in the container netns, in order)

```
table inet ducktape {
  chain output {
    type filter hook output priority 0; policy accept;
    oifname "lo" accept
    ip daddr <host-ip> tcp dport { <broker>, <node> } accept   # broker + node RPC
    udp dport 53 accept                                        # DNS (slirp resolver)
    tcp dport 53 accept
    ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, 169.254.0.0/16, 127.0.0.0/8 } drop
    # everything else (public internet) falls through to policy accept
  }
}
```

Order is load-bearing: the broker/node accept precedes the private-range drop,
so the two allowed host:port pairs survive even though the host IP is itself in
a dropped range. `100.64.0.0/10` is the tailnet/CGNAT block. Public egress is
allowed per the chosen policy (package installs keep working); the broker still
mediates all provider-API traffic regardless.

### Boot probe

`SandboxBackend::probe()` gains `nft` and `nsenter` to its executable-on-PATH
checks, alongside the existing `slirp4netns` requirement — a missing firewall
tool is a loud boot error, never a silent unfirewalled run.

## What this is NOT

- Tart (macOS guest) already gets neutral paths via its mount tags; a Tart-side
  egress equivalent is a different mechanism (VM NAT, not netns nft) and is out
  of scope here. Noted as deferred.
- No default-deny of public internet (chosen policy: public-only egress).
- No IP-address pinning of the broker beyond host-IP + port; the per-run
  random broker port and the opaque run bearer already gate it.

## Testing

- Pure unit tests (no podman): `wrap_podman_managed` emits `/ducktape/...`
  mount targets, `/ducktape/workspace` as `-w`, translated env/argv, the
  `--network=slirp4netns:allow_host_loopback=false,enable_ipv6=false` flag,
  `--cap-drop` pair, `--hooks-dir`, and the three annotations. A dedicated test
  asserts no host-username path (`/home/`, `.ducktape/provider-runs`) survives
  into any mount/env/arg.
- Pure unit test of the nft ruleset generator: given ports + host IP, the
  emitted ruleset accepts the two host:port pairs and DNS, drops each private
  range including `100.64.0.0/10`, in the required order.
- Live QA on a real node: enter a session, confirm `pwd` = `/ducktape/workspace`
  and `env | grep -i home` shows `/ducktape/home`; `curl` to a tailnet IP and a
  LAN IP both fail; the provider round-trip (broker PONG) still works; a public
  `curl`/`npm` fetch succeeds.
```
