#!/usr/bin/env python3
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from typing import Callable


@dataclass(frozen=True)
class Completed:
    returncode: int
    stdout: str = ""
    stderr: str = ""


@dataclass(frozen=True)
class SmokeConfig:
    scratch: Path
    bin_path: Path
    log_path: Path
    image: str = "localhost/dtinv-base"
    net: str = "dtwg-smoke"
    subnet: str = "172.30.0.0/24"
    ip0: str = "172.30.0.10"
    ip1: str = "172.30.0.11"


class SmokeFailure(RuntimeError):
    pass


ENTRY = """
  if [ -f /data/block-underlay ]; then
    PEER=$(cat /data/block-underlay) &&
    iptables -A OUTPUT -d "$PEER" -p tcp -j REJECT &&
    iptables -A INPUT -s "$PEER" -p tcp -j REJECT;
  fi &&
  mkdir -p /run/wireguard &&
  exec ducktape-node --config /data/node.toml"""


def default_config(scratch: Path, env: dict[str, str] = os.environ) -> SmokeConfig:
    repo_root = scratch.parent.parent
    bin_path = Path(env.get("BIN", str(repo_root / "target" / "debug" / "ducktape-node")))
    return SmokeConfig(scratch=scratch, bin_path=bin_path, log_path=scratch / "smoke.log")


def run_command(args: list[str | Path], **kwargs) -> Completed:
    argv = [str(arg) for arg in args]
    try:
        result = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            check=False,
            **kwargs,
        )
    except FileNotFoundError:
        return Completed(127, "", f"{argv[0]}: command not found\n")
    return Completed(result.returncode, result.stdout, result.stderr)


def append_log(config: SmokeConfig, text: str) -> None:
    config.log_path.parent.mkdir(parents=True, exist_ok=True)
    with config.log_path.open("a", encoding="utf-8") as handle:
        handle.write(text)


def note(config: SmokeConfig, message: str) -> None:
    line = f"--- {message}\n"
    print(line, end="")
    append_log(config, line)


def fail(message: str) -> None:
    raise SmokeFailure(f"SMOKE FAIL: {message}")


def checked(run: Callable[..., Completed], args: list[str | Path], message: str, **kwargs) -> Completed:
    result = run(args, **kwargs)
    if result.returncode != 0:
        fail(message)
    return result


def cleanup(config: SmokeConfig, run: Callable[..., Completed] = run_command) -> None:
    run(["podman", "rm", "-f", "dtwg-node0", "dtwg-node1"])
    run(["podman", "network", "rm", config.net])


def ensure_base_image(config: SmokeConfig, run: Callable[..., Completed]) -> None:
    if run(["podman", "image", "exists", config.image]).returncode == 0:
        return
    note(config, "baking base image (arch + openresolv + iptables)")
    run(["podman", "rm", "-f", "dtwg-prep"])
    checked(
        run,
        [
            "podman",
            "run",
            "--name",
            "dtwg-prep",
            "docker.io/library/archlinux:latest",
            "pacman",
            "-Sy",
            "--noconfirm",
            "openresolv",
            "iptables",
        ],
        "image prep (pacman)",
    )
    checked(run, ["podman", "commit", "dtwg-prep", config.image], "image commit")
    run(["podman", "rm", "dtwg-prep"])


def prepare_node_dirs(config: SmokeConfig) -> None:
    node0 = config.scratch / "node0"
    node1 = config.scratch / "node1"
    node0.mkdir(parents=True, exist_ok=True)
    node1.mkdir(parents=True, exist_ok=True)
    shutil.rmtree(node0 / "storage", ignore_errors=True)
    shutil.rmtree(node1 / "storage", ignore_errors=True)
    (node0 / "block-underlay").unlink(missing_ok=True)
    (node1 / "block-underlay").unlink(missing_ok=True)
    (node0 / "node.toml").write_text(
        "\n".join(
            [
                "id = 0",
                'namespace = "wgsmoke"',
                "peer_seeds = [0, 1]",
                'listen = "[::]:41000"',
                'advertised = "overlay"',
                f'wireguard_listen = "{config.ip0}:51820"',
                'wireguard_effect = "tun"',
                'rpc_listen = "127.0.0.1:41100"',
                'storage_dir = "/data/storage"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    (node1 / "node.toml").write_text(
        "\n".join(
            [
                "id = 1",
                'namespace = "wgsmoke"',
                "peer_seeds = [0, 1]",
                f'bootstrapper_addr = "{config.ip0}:41000"',
                'listen = "[::]:41000"',
                'advertised = "overlay"',
                f'wireguard_listen = "{config.ip1}:51820"',
                'wireguard_effect = "socket"',
                'rpc_listen = "127.0.0.1:41100"',
                'storage_dir = "/data/storage"',
                "",
            ]
        ),
        encoding="utf-8",
    )


def start_nodes(config: SmokeConfig, run: Callable[..., Completed]) -> None:
    checked(
        run,
        [
            "podman",
            "run",
            "-d",
            "--name",
            "dtwg-node0",
            "--network",
            config.net,
            "--ip",
            config.ip0,
            "--cap-add",
            "NET_ADMIN",
            "--device",
            "/dev/net/tun",
            "-v",
            f"{config.bin_path}:/usr/local/bin/ducktape-node:ro",
            "-v",
            f"{config.scratch / 'node0'}:/data",
            config.image,
            "bash",
            "-c",
            ENTRY,
        ],
        "start node0",
    )
    checked(
        run,
        [
            "podman",
            "run",
            "-d",
            "--name",
            "dtwg-node1",
            "--network",
            config.net,
            "--ip",
            config.ip1,
            "--cap-add",
            "NET_ADMIN",
            "-v",
            f"{config.bin_path}:/usr/local/bin/ducktape-node:ro",
            "-v",
            f"{config.scratch / 'node1'}:/data",
            config.image,
            "bash",
            "-c",
            ENTRY,
        ],
        "start node1",
    )


def wait_marker(
    config: SmokeConfig,
    container: str,
    marker: str,
    *,
    timeout: int,
    run: Callable[..., Completed] = run_command,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    deadline = time.monotonic() + timeout
    pattern = re.compile(marker)
    while True:
        logs = run(["podman", "logs", container])
        if pattern.search(logs.stdout):
            return
        if time.monotonic() >= deadline:
            append_log(config, f"== logs {container} ==\n{logs.stdout}{logs.stderr}")
            fail(f"{container} never printed: {marker}")
        sleep(2)


def wait_marker_count(
    config: SmokeConfig,
    container: str,
    marker: str,
    *,
    count: int,
    timeout: int,
    run: Callable[..., Completed] = run_command,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    deadline = time.monotonic() + timeout
    while True:
        logs = run(["podman", "logs", container])
        if logs.stdout.count(marker) >= count:
            return
        if time.monotonic() >= deadline:
            append_log(config, f"== logs {container} ==\n{logs.stdout}{logs.stderr}")
            fail(f"{container}: fewer than {count} of: {marker}")
        sleep(3)


def parse_height(output: str) -> int | None:
    try:
        value = json.loads(output).get("height")
        return int(value)
    except Exception:
        match = re.search(r'"height":([0-9]+)', output)
        return int(match.group(1)) if match else None


def height(container: str, run: Callable[..., Completed]) -> int | None:
    result = run(
        [
            "podman",
            "exec",
            container,
            "bash",
            "-c",
            'exec 3<>/dev/tcp/127.0.0.1/41100 && echo "{\\"cmd\\":\\"status\\"}" >&3 && head -1 <&3',
        ]
    )
    return parse_height(result.stdout)


def wait_height_past(
    container: str,
    floor: int,
    timeout: int,
    *,
    run: Callable[..., Completed],
    sleep: Callable[[float], None],
) -> int | None:
    deadline = time.monotonic() + timeout
    while True:
        value = height(container, run)
        if value is not None and value > floor:
            return value
        if time.monotonic() >= deadline:
            return None
        sleep(3)


def append_command_output(config: SmokeConfig, result: Completed) -> None:
    if result.stdout:
        print(result.stdout, end="")
        append_log(config, result.stdout)
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
        append_log(config, result.stderr)


def run_smoke(
    config: SmokeConfig,
    *,
    run: Callable[..., Completed] = run_command,
    sleep: Callable[[float], None] = time.sleep,
) -> int:
    cleanup(config, run)
    config.log_path.parent.mkdir(parents=True, exist_ok=True)
    config.log_path.write_text("", encoding="utf-8")
    if not (config.bin_path.is_file() and os.access(config.bin_path, os.X_OK)):
        fail(f"no node binary at {config.bin_path} (cargo build -p node-bin)")

    ensure_base_image(config, run)
    checked(run, ["podman", "network", "create", "--subnet", config.subnet, config.net], "network create")
    prepare_node_dirs(config)
    start_nodes(config, run)

    note(config, "waiting for tunnels on both nodes (1 peer each - a 0-peer apply is a FAILED epoch)")
    wait_marker(config, "dtwg-node0", r"tunnels applied on dt-.*\(1 peer", timeout=480, run=run, sleep=sleep)
    wait_marker(
        config,
        "dtwg-node1",
        r"tunnels applied on dt-.*\(1 peer\(s\); userspace socket backend",
        timeout=480,
        run=run,
        sleep=sleep,
    )
    note(config, "tunnels applied with peers (node0 tun, node1 socket)")

    note(config, "baseline liveness (heights advance pre-cut, across the mixed pair)")
    h0 = wait_height_past("dtwg-node0", 2, 120, run=run, sleep=sleep)
    if h0 is None:
        fail("node0 height stuck pre-cut")
    h1 = wait_height_past("dtwg-node1", 2, 120, run=run, sleep=sleep)
    if h1 is None:
        fail("node1 height stuck pre-cut")
    note(config, f"pre-cut heights: node0={h0} node1={h1}")

    note(config, "cutting underlay TCP both directions (WG UDP stays open)")
    checked(
        run,
        [
            "podman",
            "exec",
            "dtwg-node0",
            "bash",
            "-c",
            f"iptables -A OUTPUT -d {config.ip1} -p tcp -j REJECT && iptables -A INPUT -s {config.ip1} -p tcp -j REJECT",
        ],
        "iptables node0",
    )
    checked(
        run,
        [
            "podman",
            "exec",
            "dtwg-node1",
            "bash",
            "-c",
            f"iptables -A OUTPUT -d {config.ip0} -p tcp -j REJECT && iptables -A INPUT -s {config.ip0} -p tcp -j REJECT",
        ],
        "iptables node1",
    )

    note(config, "waiting for mesh to re-dial over the tunnel and consensus to resume")
    ha = wait_height_past("dtwg-node0", h0 + 3, 180, run=run, sleep=sleep)
    if ha is None:
        fail("node0 did not advance after the underlay cut")
    hb = wait_height_past("dtwg-node1", h1 + 3, 180, run=run, sleep=sleep)
    if hb is None:
        fail("node1 did not advance after the underlay cut")
    note(config, f"post-cut heights: node0={ha} node1={hb} - consensus rides the mixed-mode tunnel")

    note(config, "evidence: node0 carries the overlay on a real interface")
    append_command_output(config, run(["podman", "exec", "dtwg-node0", "ss", "-6", "-t", "state", "established"]))
    append_command_output(config, run(["podman", "exec", "dtwg-node0", "ip", "-6", "addr", "show"]))
    note(config, "evidence: node1 carries it with NO interface at all (userspace backend)")
    checked(run, ["podman", "exec", "dtwg-node1", "sh", "-c", "test ! -e /dev/net/tun"], "node1 has a TUN device")
    checked(run, ["podman", "exec", "dtwg-node1", "sh", "-c", '! ip link show | grep -q "dt-"'], "node1 grew a dt- interface")
    print("PHASE 1-3 PASS: mesh traffic flows tun<->socket over WireGuard after the underlay cut")

    note(config, "cold restart: stopping BOTH nodes (tunnels die with the processes)")
    checked(run, ["podman", "stop", "dtwg-node0", "dtwg-node1"], "stop")
    (config.scratch / "node0" / "block-underlay").write_text(f"{config.ip1}\n", encoding="utf-8")
    (config.scratch / "node1" / "block-underlay").write_text(f"{config.ip0}\n", encoding="utf-8")

    if not (config.scratch / "node0" / "storage" / "mesh-state.json").is_file():
        fail("node0 never persisted its mesh")
    if not (config.scratch / "node1" / "storage" / "mesh-state.json").is_file():
        fail("node1 never persisted its mesh")
    note(config, "persisted mesh state present on both nodes")

    checked(run, ["podman", "start", "dtwg-node0", "dtwg-node1"], "restart")
    wait_marker(config, "dtwg-node0", r"persisted mesh \(epoch .*\) restored on dt-", timeout=240, run=run, sleep=sleep)
    wait_marker(config, "dtwg-node1", r"persisted mesh \(epoch .*\) restored on dt-", timeout=240, run=run, sleep=sleep)
    note(config, "both nodes restored tunnels from disk with zero TCP paths (node1 into the socket backend)")
    wait_marker(config, "dtwg-node0", r"1 mesh dial seed\(s\) from the persisted mesh", timeout=60, run=run, sleep=sleep)
    note(config, "node0 seeded its dialer from the persisted mesh")

    note(config, "waiting for live assembly to replace the restored mesh")
    wait_marker_count(config, "dtwg-node0", "tunnels applied on dt-", count=2, timeout=300, run=run, sleep=sleep)
    wait_marker_count(config, "dtwg-node1", "tunnels applied on dt-", count=2, timeout=300, run=run, sleep=sleep)

    note(config, "post-restart liveness (heights must pass their pre-restart values)")
    hc = wait_height_past("dtwg-node0", ha, 300, run=run, sleep=sleep)
    if hc is None:
        fail("node0 height stuck after cold restart")
    hd = wait_height_past("dtwg-node1", hb, 300, run=run, sleep=sleep)
    if hd is None:
        fail("node1 height stuck after cold restart")
    note(config, f"post-restart heights: node0={hc} node1={hd} (pre-restart: {ha}/{hb})")

    note(config, "evidence: the underlay really is blocked and the mesh rides the overlay")
    append_command_output(config, run(["podman", "exec", "dtwg-node0", "iptables", "-L", "OUTPUT", "-n"]))
    append_command_output(config, run(["podman", "exec", "dtwg-node0", "ss", "-6", "-t", "state", "established"]))
    cleanup(config, run)
    print("SMOKE PASS: mixed-mode (tun<->socket) cold restart healed from the persisted mesh with no TCP ingress")
    return 0


def main(argv: list[str]) -> int:
    config = default_config(Path(__file__).resolve().parent)
    try:
        return run_smoke(config)
    except SmokeFailure as exc:
        print(exc)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
