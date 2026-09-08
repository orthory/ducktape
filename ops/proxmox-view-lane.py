#!/usr/bin/env python3
"""Provision and roll out three owned Proxmox CTs, recording runtime IDs/IPs.

All existing-resource mutations require the exact recorded host, container names,
PVE descriptions and inner owner markers. Records stay outside disposable state.
"""
import argparse
import fcntl
from datetime import datetime, timezone
import json
import hashlib
import ipaddress
import tarfile
import tempfile
import uuid
import re
import shlex
import subprocess
from pathlib import Path

ROOT = "/var/lib/ducktape-view-lane"
SERVICE = "ducktape-view-lane.service"


def validate_record(record):
    owner = record.get("owner", "")
    if not isinstance(record.get("host"), str) or not record["host"]:
        raise ValueError("record requires its original Proxmox host")
    nodes = record.get("nodes", [])
    if not isinstance(owner, str) or not re.fullmatch(r"ducktape-view-lane-[a-z0-9-]+", owner):
        raise ValueError("invalid lane owner")
    if len(nodes) != 3:
        raise ValueError("lane requires exactly three containers")
    ids, names = set(), set()
    for node in nodes:
        node_id, name = node.get("id"), node.get("name", "")
        # IDs 100-123 are known existing resources, including this workstation.
        if type(node_id) is not int or not 200 <= node_id <= 999999999:
            raise ValueError("container ID must be in the dedicated range >= 200")
        if not isinstance(name, str) or not re.fullmatch(r"dt-view-[a-z0-9-]+", name):
            raise ValueError("invalid dedicated container name")
        if node_id in ids or name in names:
            raise ValueError("duplicate container ID or name")
        ids.add(node_id)
        names.add(name)


def validate_owner(record, node, config):
    if (config.get("hostname") != node["name"]
            or config.get("description") != record["owner"]):
        raise ValueError(f"ownership mismatch for container {node['id']}")


def preflight(record, read_config):
    validate_record(record)
    for node in record["nodes"]:
        validate_owner(record, node, read_config(node["id"]))


def remote(host, *args):
    result = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
         "-o", "ConnectTimeout=10", "--", host, shlex.join(map(str, args))],
        check=True, capture_output=True, text=True, timeout=300,
    )
    return result.stdout


def save_record(path, record):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(record, indent=2) + "\n")
    temporary.replace(path)


def inventory(host):
    return json.loads(remote(host, "pvesh", "get", "/cluster/resources", "--type", "vm", "--output-format", "json"))


def choose_nodes(resources, owner):
    used = {int(item["vmid"]) for item in resources}
    available = (i for i in range(200, 10000) if i not in used)
    return [{"id": next(available), "name": f"dt-view-{owner.removeprefix('ducktape-view-lane-')}-{n}"}
            for n in range(1, 4)]


def send_file(host, destination, data):
    subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
                    "-o", "ConnectTimeout=10", "--", host,
                    "umask 077; cat > " + shlex.quote(destination)],
                   input=data, check=True, capture_output=True, timeout=180)


def remote_stage(host, owner):
    prefix = "/var/tmp/" + owner + "."
    directory = remote(host, "mktemp", "-d", prefix + "XXXXXX").strip()
    if not re.fullmatch(re.escape(prefix) + r"[A-Za-z0-9]{6}", directory):
        raise ValueError("invalid private staging directory")
    return directory


def provision(args):
    if args.record.exists():
        raise ValueError("record already exists; inspect any partial provision, never adopt existing CTs")
    if not all([args.template, args.storage, args.bridge, args.ssh_key]):
        raise ValueError("provision requires --template, --storage, --bridge, --ssh-key")
    owner = "ducktape-view-lane-" + uuid.uuid4().hex[:12]
    record = {"owner": owner, "host": args.host, "nodes": choose_nodes(inventory(args.host), owner), "created": []}
    validate_record(record)
    public_key = args.ssh_key.read_bytes()
    if not public_key.startswith((b"ssh-ed25519 ", b"ssh-rsa ", b"ecdsa-sha2-")):
        raise ValueError("--ssh-key must be a public SSH key")
    args.record.parent.mkdir(parents=True, exist_ok=True)
    save_record(args.record, record)
    stage_dir = remote_stage(args.host, owner)
    key_path = stage_dir + "/key.pub"
    try:
        send_file(args.host, key_path, public_key)
        for node in record["nodes"]:
            remote(args.host, "pct", "create", node["id"], args.template,
                   "--hostname", node["name"], "--description", owner,
                   "--rootfs", args.storage + ":12", "--memory", "4096", "--cores", "2",
                   "--unprivileged", "1", "--features", "nesting=1",
                   "--net0", f"name=eth0,bridge={args.bridge},ip=dhcp",
                   "--ssh-public-keys", key_path)
            record["created"].append(node["id"])
            save_record(args.record, record)
            remote(args.host, "pct", "start", node["id"])
            remote(args.host, "pct", "exec", node["id"], "--", "mkdir", "-p", ROOT)
            remote(args.host, "pct", "exec", node["id"], "--", "sh", "-c",
                   "printf '%s\\n' " + shlex.quote(owner) + " > " + ROOT + "/owner")
            remote(args.host, "pct", "exec", node["id"], "--", "sh", "-ec",
                   "command -v sshd >/dev/null || { apt-get update; DEBIAN_FRONTEND=noninteractive apt-get install -y openssh-server; }; systemctl enable --now ssh")
    finally:
        remote(args.host, "rm", "-rf", "--", stage_dir)
    return record


def live_addresses(host, record):
    addresses = []
    for node in record["nodes"]:
        interfaces = json.loads(remote(host, "pct", "exec", node["id"], "--",
                                       "ip", "-j", "-4", "address", "show", "dev", "eth0"))
        found = [entry["local"] for interface in interfaces for entry in interface["addr_info"]
                 if entry["scope"] == "global"]
        if len(found) != 1:
            raise ValueError(f"container {node['id']} needs exactly one actual eth0 IPv4 address")
        address = ipaddress.IPv4Address(found[0])
        if address.is_loopback or address.is_unspecified or address.is_multicast:
            raise ValueError("invalid node LAN address")
        addresses.append(str(address))
    if len(set(addresses)) != 3:
        raise ValueError("node LAN addresses must be distinct")
    return addresses


def node_config(record, addresses, index):
    # Same dev-shape contract as bin/node/tests/common::Cluster::config_toml.
    fields = dict(id=index + 1, namespace=record["owner"], peer_seeds=[1, 2, 3],
                  validator_seeds=[1, 2, 3], modules=ROOT + "/current/modules",
                  peer_addrs=[f"{ip}:26656" for ip in addresses],
                  listen=f"{addresses[index]}:26656", storage_dir=ROOT + "/network/storage",
                  rpc_listen="127.0.0.1:26657", http_listen="127.0.0.1:8844",
                  wireguard_listen=f"{addresses[index]}:51820",
                  invite_listen=f"{addresses[index]}:51821", primary_coordinator="none",
                  block_time_ms=500)
    return "".join(f"{key} = {json.dumps(value)}\n" for key, value in fields.items())


def package_release(binary, modules, revision, ui_revision, output):
    for value in [revision, ui_revision]:
        if not re.fullmatch(r"[0-9a-f]{40}", value or ""):
            raise ValueError("provide exact 40-character repository and UI revisions")
    if modules.is_symlink() or not modules.is_dir():
        raise ValueError("modules must be a real directory")
    files = {"ducktape": binary}
    for path in sorted(modules.rglob("*")):
        if path.is_symlink():
            raise ValueError("release inputs must not contain symlinks")
        if path.is_file():
            files["modules/" + path.relative_to(modules).as_posix()] = path
    if not any(name.endswith(".component.wasm") for name in files):
        raise ValueError("release needs runtime component files")
    hashes = {}
    with tarfile.open(output, "w") as archive:
        for name, path in files.items():
            if path.is_symlink() or not path.is_file() or any(ord(c) < 32 or c == "\\" for c in name):
                raise ValueError("release requires regular files with safe names")
            hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
            archive.add(path, arcname=name, recursive=False)
        manifest = {"revision": revision, "ui_revision": ui_revision, "files": hashes}
        with tempfile.TemporaryDirectory() as directory:
            checks = Path(directory) / "SHA256SUMS"
            checks.write_text("".join(f"{digest}  {name}\n" for name, digest in hashes.items()))
            archive.add(checks, arcname="SHA256SUMS")
    return manifest


def rollout(args, record):
    if not all([args.binary, args.modules, args.revision, args.ui_revision, args.reason]):
        raise ValueError("rollout requires --binary --modules --revision --ui-revision --reason")
    addresses = live_addresses(args.host, record)
    with tempfile.TemporaryDirectory() as directory:
        archive = Path(directory) / "release.tar"
        manifest = package_release(args.binary, args.modules, args.revision, args.ui_revision, archive)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        manifest.update(archive_sha256=digest, addresses=addresses, reason=args.reason)
        stage_dir = remote_stage(args.host, record["owner"])
        stage = stage_dir + "/release.tar"
        try:
            send_file(args.host, stage, archive.read_bytes())
            # Stage and verify all three before stopping any healthy process.
            for node in record["nodes"]:
                release = ROOT + "/releases/" + digest
                remote(args.host, "pct", "push", node["id"], stage, ROOT + "/release.tar")
                remote(args.host, "pct", "exec", node["id"], "--", "sh", "-ec",
                       f"printf '%s\\n' '{digest}  {ROOT}/release.tar' | sha256sum -c -; "
                       f"mkdir -p {release}; tar -xf {ROOT}/release.tar -C {release} --no-same-owner; "
                       f"cd {release}; sha256sum -c SHA256SUMS; chmod 755 ducktape")
            with args.record.with_suffix(".events.jsonl").open("a") as journal:
                journal.write(json.dumps({"time": datetime.now(timezone.utc).isoformat(),
                                          "action": "rollout", "result": "staged", "release": manifest}) + "\n")
            record["pending_release"] = manifest
            save_record(args.record, record)
            for node in record["nodes"]:
                remote(args.host, "pct", "exec", node["id"], "--", "sh", "-ec",
                       f"if test -f /etc/systemd/system/{SERVICE}; then systemctl stop {SERVICE}; fi")
            for index, node in enumerate(record["nodes"]):
                release = ROOT + "/releases/" + digest
                config = node_config(record, addresses, index)
                unit = ("[Unit]\nAfter=network-online.target\n[Service]\n"
                        f"ExecStart={ROOT}/current/ducktape node run --config {ROOT}/network/node.toml\n"
                        f"Environment=DUCKTAPE_MODULES_DIR={ROOT}/current/modules\n"
                        f"Environment=DUCKTAPE_HOME={ROOT}/network/home\n"
                        f"WorkingDirectory={ROOT}/network\n"
                        "Restart=on-failure\nTimeoutStopSec=120\nLimitNOFILE=65536\n"
                        "[Install]\nWantedBy=multi-user.target\n")
                remote(args.host, "pct", "exec", node["id"], "--", "sh", "-ec",
                       f"mkdir -p {ROOT}/network; ln -sfn {release} {ROOT}/current; "
                       f"printf %s {shlex.quote(config)} > {ROOT}/network/node.toml; "
                       f"printf %s {shlex.quote(unit)} > /etc/systemd/system/{SERVICE}; systemctl daemon-reload")
            for node in record["nodes"]:
                remote(args.host, "pct", "exec", node["id"], "--", "systemctl", "enable", "--now", SERVICE)
            # Starting a service is not convergence; observe is a separate gate.
            record["release"] = record.pop("pending_release")
            save_record(args.record, record)
            with args.record.with_suffix(".events.jsonl").open("a") as journal:
                journal.write(json.dumps({"time": datetime.now(timezone.utc).isoformat(),
                                          "action": "rollout", "result": "started_unverified", "release": manifest}) + "\n")
        finally:
            remote(args.host, "rm", "-rf", "--", stage_dir)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="root@zk")
    parser.add_argument("--record", required=True, type=Path)
    parser.add_argument("--template")
    parser.add_argument("--storage")
    parser.add_argument("--bridge")
    parser.add_argument("--ssh-key", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--modules", type=Path)
    parser.add_argument("--revision")
    parser.add_argument("--ui-revision")
    parser.add_argument("--reason", help="required explanation for resetting network data")
    parser.add_argument("action", choices=["inventory", "provision", "rollout", "check", "status", "reset-network"])
    args = parser.parse_args()
    if args.action == "inventory":
        execute(args)
        return
    args.record.parent.mkdir(parents=True, exist_ok=True)
    with args.record.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        execute(args)


def execute(args):
    if args.action == "inventory":
        print(json.dumps(inventory(args.host), indent=2))
        print(remote(args.host, "pvesm", "status"))
        print("Inspect template storage from pvesm status, then: pveam list <storage>")
        return
    if args.action == "provision":
        print(json.dumps(provision(args), indent=2))
        return
    if args.action == "reset-network" and not args.reason:
        raise ValueError("reset-network requires --reason")
    record = json.loads(args.record.read_text())
    if record.get("host") != args.host:
        raise ValueError("record belongs to a different Proxmox host")

    def config(node_id):
        text = remote(args.host, "pct", "config", node_id, "--current", "1")
        return dict(line.split(": ", 1) for line in text.splitlines() if ": " in line)

    preflight(record, config)
    # Check every inner marker before the first mutation, too. A copied PVE
    # description alone is not permission to clear some other installed node.
    for node in record["nodes"]:
        marker = remote(args.host, "pct", "exec", node["id"], "--", "cat", ROOT + "/owner")
        if marker.rstrip("\n") != record["owner"]:
            raise ValueError(f"inner ownership mismatch for container {node['id']}")
    if args.action == "rollout":
        rollout(args, record)
    elif args.action == "reset-network":
        # The local lane record/evidence are outside every CT's network state.
        # Write intent before mutation so an interrupted reset remains visible.
        event = {"time": datetime.now(timezone.utc).isoformat(),
                 "action": args.action, "reason": args.reason, "lane": record,
                 "result": "started"}
        journal = args.record.with_suffix(".events.jsonl")
        with journal.open("a") as output:
            output.write(json.dumps(event, sort_keys=True) + "\n")
        for node in record["nodes"]:
            remote(args.host, "pct", "exec", node["id"], "--", "systemctl", "stop", SERVICE)
        for node in record["nodes"]:
            # Fixed path below the owner marker. Never accept a caller-supplied
            # removal path, and never remove/recreate the containers themselves.
            remote(args.host, "pct", "exec", node["id"], "--", "rm", "-rf", "--", ROOT + "/network")
        event["result"] = "network_data_removed"
        with journal.open("a") as output:
            output.write(json.dumps(event, sort_keys=True) + "\n")
    elif args.action == "status":
        for node in record["nodes"]:
            status = remote(args.host, "pct", "exec", node["id"], "--",
                            "systemctl", "show", SERVICE,
                            "--property=ActiveState,SubState,MainPID,ExecMainStatus")
            print(f"container {node['id']} ({node['name']}):\n{status}", end="")
    print(f"{args.action}: all three ownership checks passed")


if __name__ == "__main__":
    main()
