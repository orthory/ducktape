# Running a node as a service (systemd)

How to keep a `ducktape` node — and the compute / agent / airlock daemons
that serve it — running across reboots and crashes on a Linux host. The
units are `ops/node/ducktape-node@.service` and
`ops/node/ducktape-service@.service`; the log rotation is
`ops/node/ducktape-node.logrotate`. Every path below matches what those
files set.

The node is supervisor-ready: on a validator SIGTERM takes the graceful
checkpoint path (the same one the desktop shell uses on quit; a resident
installs no handler and simply re-syncs at its next boot), it raises its own open-file
soft limit to 65536 (`bin/node/src/resource_limits.rs`), and
`ducktape service run` names systemd as its target (`bin/node/src/services.rs`,
`RunArgs::enable`: "for scripts and systemd units"). What was missing was the
recipe; this is it.

## Where the workspace lives

Everything the node keeps on disk sits under **`DUCKTAPE_HOME`**
(`crates/workspace-config/src/lib.rs`, `ducktape_home`): `$DUCKTAPE_HOME`
when set, else `~/.ducktape`. The units set
`DUCKTAPE_HOME=/var/lib/ducktape` (a systemd `StateDirectory`, owned by the
`ducktape` user), so under them the layout is:

```
/var/lib/ducktape/
  workspaces/<chain-id>/     one dir per network (`ducktape node list`)
    node.toml                the operator file: listeners, storage_dir, [sandbox]
    network.toml             the network descriptor (validators, reach hints)
    identity.key             THIS NODE'S seat key, 0600 — back it up (see backup-and-keys.md)
    wireguard.key            the tunnel keypair (regenerable)
    services.toml            service grants (`ducktape service enable`)
    coord.cap                the coordinator admission capability, when issued
    service-link.token       the node↔daemon link secret, 0600
    daemon.log               `node run`'s tee (append-only)
    <kind>.log               `service run <kind>`'s tee (append-only)
    storage/                 consensus state, blobs, mesh-state.json, airlock-creds/
  modules/                   <id>.component.wasm — what `node init` founds a network from
  executors/                 pinned agent CLIs (`ducktape agent install`)
  keys/                      user wallets + `active` pointer (only if you run wallet verbs as this user)
```

Every operator verb you run against that tree needs the same view of it:

```sh
alias dt='sudo -u ducktape env DUCKTAPE_HOME=/var/lib/ducktape /usr/local/bin/ducktape'
```

## Install

```sh
# 1. Build and install the CLI system-wide (make install-node puts it in
#    ~/.cargo/bin and fills ~/.ducktape/modules for the building user).
make install-node
sudo install -m 0755 ~/.cargo/bin/ducktape /usr/local/bin/ducktape

# 2. A dedicated user. The kvm group is for the service daemons: compute and
#    agent open /dev/kvm per run; `node run` itself never does.
sudo useradd --system --home-dir /var/lib/ducktape --shell /usr/sbin/nologin ducktape
sudo usermod -aG kvm ducktape
sudo install -d -o ducktape -g ducktape -m 0700 /var/lib/ducktape

# 3. The module set the network is founded from.
sudo install -d -o ducktape -g ducktape /var/lib/ducktape/modules
sudo cp ~/.ducktape/modules/*.component.wasm /var/lib/ducktape/modules/
sudo chown -R ducktape:ducktape /var/lib/ducktape/modules

# 4. Units and log rotation.
sudo cp ops/node/ducktape-node@.service ops/node/ducktape-service@.service /etc/systemd/system/
sudo install -m 0644 ops/node/ducktape-node.logrotate /etc/logrotate.d/ducktape-node
sudo systemctl daemon-reload

# 5. Found or join the network AS the service user, so the files land where
#    the unit will look for them.
dt node init --name mynet                 # founder
dt node join '<invite blob>'              # ...or a joiner
dt node list                              # the chain id the instance names
```

`node init`/`node join` probe the host and write the `[sandbox]` table when
`/dev/kvm` opens; a host that gained KVM later runs `dt node sandbox` once.
Firecracker, `mke2fs`, `debugfs` AND `nft` must be on the service's `PATH`
— `/usr/local/bin`, `/usr/sbin` and `/sbin` are searched
(`crates/services/sandbox/src/host_tools.rs`). `nft` is not optional: the
Firecracker backend lists it unconditionally (`sandbox.rs`,
`required_tools`) and the compute/agent daemon refuses to boot without it
(`nft is not executable on PATH`), tap-networked run or not — install
`nftables` alongside `e2fsprogs`.

## Enable and start

The instance name is the workspace selector `ducktape node run -n` takes:
the chain id or any unique prefix. A chain id carries `#`, which a unit name
cannot, so use the name half or escape it (the unit passes `%I`, the
unescaped instance, to `-n`, so both forms select the workspace):

```sh
sudo systemctl enable --now ducktape-node@mynet
# or, with the full id:
sudo systemctl enable --now "ducktape-node@$(systemd-escape 'mynet#d0cdf950')"

dt node status                            # height + root hash, once it serves
```

Service daemons: the instance is the kind (`compute`, `agent`, `airlock`).
A daemon needs the node up and publishing its mesh identity at ITS boot; it
exits loudly otherwise and the unit's `Restart=always` retries every 5 s, so
co-starting both at boot converges on its own (once serving, it rides out a
node restart — see below). Consent is a separate act — the daemon
signals and parks until you grant it, and it reads the grant at boot:

```sh
sudo systemctl enable --now ducktape-service@compute   # signals, "not enabled"
dt service enable compute                              # the consent screen; needs the node
sudo systemctl restart ducktape-service@compute        # picks up the grant and serves
dt service status
```

For unattended hosts put `DUCKTAPE_SERVICE_ARGS=--enable` in
`/etc/ducktape/node.env` and the daemon mints its own grant without asking.
The same file may carry `DUCKTAPE_NODE_ARGS`, `RUST_LOG`, and a `TMPDIR` on
a disk with room for run images (they are as large as a run's inputs, and
the unit's private `/tmp` is whatever the host's `/tmp` is).

To pin a daemon to one node's lifecycle:

```sh
sudo systemctl edit ducktape-service@compute
# [Unit]
# After=ducktape-node@mynet.service
# BindsTo=ducktape-node@mynet.service
```

## Logs

Two sinks, one filter: stderr (the journal) and the tee file in the
workspace. They never disagree about what was recorded.

```sh
journalctl -fu ducktape-node@mynet
tail -f /var/lib/ducktape/workspaces/<chain-id>/daemon.log
tail -f /var/lib/ducktape/workspaces/<chain-id>/compute.log

# turn one plane up on the LIVE node — never restart to look at a wedged state
curl -XPOST 127.0.0.1:8844/v1/log-filter -d 'info,ducktape::join=debug'
```

The tee files are opened append-only once and never reopened, which is why
the logrotate drop-in uses `copytruncate` (weekly, or at 256 MB, eight kept).

## Restart, stop, upgrade

```sh
sudo systemctl restart ducktape-node@mynet      # SIGTERM → final checkpoint → exit → back up
sudo systemctl stop ducktape-node@mynet         # stays down until started
sudo install -m 0755 target/release/ducktape /usr/local/bin/ducktape && \
  sudo systemctl restart ducktape-node@mynet ducktape-service@compute
```

A running daemon survives a node restart, so do not restart it for one: its
link task re-dials forever (`bin/node/src/compute/link.rs`,
`bin/node/src/agent/link.rs`; the agent link re-reads `service-link.token`
on every attach, since a node restart mints a fresh one), and its hello
heartbeat keeps signaling — `warn` `hello_failed` at attempt 1, then every
30th, carrying `attempts` (`bin/node/src/services.rs`, `heartbeat`). Restart
`ducktape-service@<kind>` only for a new binary or a new grant. The one
daemon that exits is one that BOOTS while the node is down: the first hello
must land (`services.rs`, `send_hello` in `run`), and `Restart=always`
retries it until it does. Read
`docs/records/admission/validator-onboarding.md` before stopping a
validator: below four validators every seat must be live to finalize, so a
restart of one of three halts the chain for the restart's duration.

`curl 127.0.0.1:8844/v1/status | jq .version` prints the build the node
runs (`<cargo version>+<git short sha>[-<dirty digest>]`), which is how two
hosts confirm they run the same binary.

## Listen ports

Defaults from `crates/workspace-config/src/node_toml.rs`, written into
`node.toml` by `init`/`join` and overridable with the plumbing flags
(`--listen`, `--http`, `--rpc`, `--wireguard-listen`, `--invite-listen`):

| Plane | Default | Proto | Inbound rule? |
| --- | --- | --- | --- |
| p2p control mesh (`listen`) | `[::]:8846` | TCP | **Yes** for a node others dial directly (a founder, a `Direct`-hinted member). A member that advertises `"overlay"` is dialed over the WireGuard tunnel instead. |
| WireGuard tunnel plane (`wireguard_listen`) | `0.0.0.0:51820` | UDP | **Yes** for an inviter / a node without a coordinator; the plane hole-punches through a coordinator otherwise. Bind the concrete IP on a LAN or VPS without a coordinator — an unspecified bind advertises an endpoint-less record and joiner↔joiner tunnels stay dark. |
| invite intro (`invite_listen`) | WireGuard port + 1 → `0.0.0.0:51821` | UDP | **Yes** on any node that mints invites (a joiner rings this doorbell first). |
| node HTTP API (`http_listen`) | `127.0.0.1:8844` | TCP | No — loopback only. `/v1` is unauthenticated; never bind it wider. |
| operator rpc (`rpc_listen`) | `127.0.0.1:8845` | TCP | No — loopback only. |
| browser gateway (`gateway_listen`) | `127.0.0.1:0` | TCP | No — port 0, printed at boot, re-read per session. |

Outbound: UDP 3478 + TCP 443 to the coordinator
(`relay.ducktape.industries` by default) for rendezvous and the
first-contact relay fallback; HTTPS to `auth.ducktape.industries` from the
CLI for passkey/wallet ceremonies (not from the node).
