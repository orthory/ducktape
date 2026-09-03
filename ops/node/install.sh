#!/usr/bin/env bash
# The executable form of docs/deploy/node-service.md's Install/Enable
# sections. Idempotent: every step is safe to re-run (useradd -m is not used,
# `install -d`/`cp` are unconditional, `systemctl enable --now` on an already
# enabled+running unit is a no-op). Linux/systemd only — the units and paths
# this script writes (`/etc/systemd/system`, `/var/lib/ducktape`) have no
# other-platform equivalent; run macos-preflight.sh for a dev-loop macOS host
# instead.
#
# Usage:
#   ops/node/install.sh --workspace <name> --init [-- <node init args...>]
#   ops/node/install.sh --workspace <name> --join <invite>
#   ops/node/install.sh --dry-run --workspace <name> --init
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

log(){ printf '\033[36m[install]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[install] %s\033[0m\n' "$*" >&2; exit 1; }

DRY_RUN=0
WORKSPACE=""
MODE=""       # "init" or "join"
INVITE=""
INIT_ARGS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --workspace) WORKSPACE="${2:?--workspace needs a value}"; shift 2 ;;
    --init) MODE="init"; shift ;;
    --join) MODE="join"; INVITE="${2:?--join needs an invite blob}"; shift 2 ;;
    --) shift; INIT_ARGS=("$@"); break ;;
    *) die "unknown argument: $1" ;;
  esac
done

[ -n "$WORKSPACE" ] || die "--workspace <name> is required"
[ -n "$MODE" ] || die "one of --init or --join <invite> is required"

# run() either prints the command (--dry-run) or executes it. sudo_run()
# is the same but only the lines that touch root-owned paths need it.
run(){
  if [ "$DRY_RUN" = 1 ]; then
    printf '+ %s\n' "$*"
  else
    "$@"
  fi
}
sudo_run(){ run sudo "$@"; }

if [ "$DRY_RUN" = 0 ]; then
  [ "$(uname -s)" = "Linux" ] || die "refusing: this installs a systemd service, not available on $(uname -s)"
  command -v systemctl >/dev/null 2>&1 || die "refusing: systemctl not found (no systemd on this host)"
fi

DUCK_HOME=/var/lib/ducktape
MODULES_SRC="${DUCKTAPE_MODULES_DIR:-${DUCKTAPE_HOME:-$HOME/.ducktape}/modules}"

log "1/6 building and installing the ducktape CLI (cargo install --path bin/node --locked)"
run bash -c "cd '$REPO_ROOT' && cargo install --path bin/node --locked"
run mkdir -p "$MODULES_SRC"
CARGO_BIN_DUCKTAPE="${CARGO_HOME:-$HOME/.cargo}/bin/ducktape"
sudo_run install -m 0755 "$CARGO_BIN_DUCKTAPE" /usr/local/bin/ducktape

log "2/6 dedicated user + state dir"
if [ "$DRY_RUN" = 1 ] || ! id ducktape >/dev/null 2>&1; then
  sudo_run useradd --system --home-dir "$DUCK_HOME" --shell /usr/sbin/nologin ducktape
fi
sudo_run usermod -aG kvm ducktape
sudo_run install -d -o ducktape -g ducktape -m 0700 "$DUCK_HOME"

log "3/6 module set"
sudo_run install -d -o ducktape -g ducktape "$DUCK_HOME/modules"
if [ "$DRY_RUN" = 1 ]; then
  run bash -c "sudo cp '$MODULES_SRC'/*.component.wasm '$DUCK_HOME/modules/'"
else
  shopt -s nullglob
  wasm_files=("$MODULES_SRC"/*.component.wasm)
  shopt -u nullglob
  [ "${#wasm_files[@]}" -gt 0 ] || die "no component.wasm files in $MODULES_SRC (cargo install --path bin/node should have populated it)"
  sudo cp "${wasm_files[@]}" "$DUCK_HOME/modules/"
fi
sudo_run chown -R ducktape:ducktape "$DUCK_HOME/modules"

log "4/6 systemd units + log rotation"
sudo_run cp "$SCRIPT_DIR/ducktape-node@.service" "$SCRIPT_DIR/ducktape-service@.service" /etc/systemd/system/
sudo_run install -m 0644 "$SCRIPT_DIR/ducktape-node.logrotate" /etc/logrotate.d/ducktape-node
sudo_run systemctl daemon-reload

log "5/6 founding or joining the network as the service user"
DT=(sudo -u ducktape env "DUCKTAPE_HOME=$DUCK_HOME" /usr/local/bin/ducktape)
case "$MODE" in
  init) run "${DT[@]}" node init --name "$WORKSPACE" "${INIT_ARGS[@]}" ;;
  join) run "${DT[@]}" node join "$INVITE" ;;
esac

log "6/6 enable and start"
sudo_run systemctl enable --now "ducktape-node@$WORKSPACE"

log "done — 'ducktape node status' (as the ducktape user) once it serves; see docs/deploy/node-service.md"
