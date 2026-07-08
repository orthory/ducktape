#!/usr/bin/env python3
from dataclasses import dataclass
import os
from pathlib import Path
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
class KeyPair:
    private: str
    public: str


@dataclass(frozen=True)
class InteropConfig:
    scratch: Path
    bin_path: Path
    log_path: Path
    image: str = "localhost/dtinv-base"
    net: str = "dtiop"
    ip_tun: str = "172.32.0.10"
    ip_socket: str = "172.32.0.11"
    ula_tun: str = "fda2:8ad3:eaee::1"
    ula_socket: str = "fda2:8ad3:eaee::2"
    wg_port: int = 51820


class SmokeFailure(RuntimeError):
    pass


def default_config(scratch: Path, env: dict[str, str] = os.environ) -> InteropConfig:
    repo_root = scratch.parent.parent
    bin_path = Path(env.get("BIN", str(repo_root / "target" / "debug" / "examples" / "wg_interop")))
    return InteropConfig(scratch=scratch, bin_path=bin_path, log_path=scratch / "interop.log")


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


def append_log(config: InteropConfig, text: str) -> None:
    config.log_path.parent.mkdir(parents=True, exist_ok=True)
    with config.log_path.open("a", encoding="utf-8") as handle:
        handle.write(text)


def note(config: InteropConfig, message: str) -> None:
    line = f"--- {message}\n"
    print(line, end="")
    append_log(config, line)


def fail(message: str) -> None:
    raise SmokeFailure(f"INTEROP FAIL: {message}")


def checked(run: Callable[..., Completed], args: list[str | Path], message: str, **kwargs) -> Completed:
    result = run(args, **kwargs)
    if result.returncode != 0:
        fail(message)
    return result


def cleanup(config: InteropConfig, run: Callable[..., Completed] = run_command) -> None:
    run(["podman", "rm", "-f", "dtiop-tun", "dtiop-socket"])
    run(["podman", "network", "rm", config.net])


def parse_keygen(output: str) -> KeyPair:
    private = ""
    public = ""
    for line in output.splitlines():
        if line.startswith("PRIV "):
            private = line[5:]
        elif line.startswith("PUB "):
            public = line[4:]
    if not private or not public:
        fail("keygen")
    return KeyPair(private=private, public=public)


def keygen(config: InteropConfig, seed: int, run: Callable[..., Completed]) -> KeyPair:
    result = checked(run, [config.bin_path, "keygen", str(seed)], "keygen")
    return parse_keygen(result.stdout)


def wait_marker(
    config: InteropConfig,
    container: str,
    marker: str,
    *,
    timeout: int,
    run: Callable[..., Completed] = run_command,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    deadline = time.monotonic() + timeout
    while True:
        logs = run(["podman", "logs", container])
        if marker in logs.stdout:
            return
        if time.monotonic() >= deadline:
            append_log(config, f"== logs {container} ==\n{logs.stdout}{logs.stderr}")
            fail(f"{container} never printed: {marker}")
        sleep(2)


def ensure_base_image(config: InteropConfig, run: Callable[..., Completed]) -> None:
    if run(["podman", "image", "exists", config.image]).returncode == 0:
        return
    note(config, "baking base image (arch + openresolv + iptables)")
    run(["podman", "rm", "-f", "dtiop-prep"])
    checked(
        run,
        [
            "podman",
            "run",
            "--name",
            "dtiop-prep",
            "docker.io/library/archlinux:latest",
            "pacman",
            "-Sy",
            "--noconfirm",
            "openresolv",
            "iptables",
        ],
        "image prep (pacman)",
    )
    checked(run, ["podman", "commit", "dtiop-prep", config.image], "image commit")
    run(["podman", "rm", "dtiop-prep"])


def run_interop(
    config: InteropConfig,
    *,
    run: Callable[..., Completed] = run_command,
    sleep: Callable[[float], None] = time.sleep,
) -> int:
    cleanup(config, run)
    config.log_path.parent.mkdir(parents=True, exist_ok=True)
    config.log_path.write_text("", encoding="utf-8")
    if not (config.bin_path.is_file() and os.access(config.bin_path, os.X_OK)):
        fail(f"no probe binary at {config.bin_path} (cargo build -p overlay-net --example wg_interop)")

    ensure_base_image(config, run)
    checked(run, ["podman", "network", "create", "--subnet", "172.32.0.0/24", config.net], "network create")

    note(config, "keys: deterministic probe fixtures")
    tun = keygen(config, 11, run)
    socket = keygen(config, 22, run)

    note(config, "starting the TUN-backend container (CAP_NET_ADMIN + /dev/net/tun, passive peer)")
    checked(
        run,
        [
            "podman",
            "run",
            "-d",
            "--name",
            "dtiop-tun",
            "--network",
            config.net,
            "--ip",
            config.ip_tun,
            "--cap-add",
            "NET_ADMIN",
            "--device",
            "/dev/net/tun",
            "-v",
            f"{config.bin_path}:/usr/local/bin/wg-interop:ro",
            config.image,
            "sh",
            "-c",
            (
                "mkdir -p /run/wireguard && exec wg-interop serve --mode tun "
                f"--priv {tun.private} --ula {config.ula_tun} --wg-port {config.wg_port} "
                f"--peer-pub {socket.public} --peer-ula {config.ula_socket}"
            ),
        ],
        "start tun container",
    )

    note(config, "starting the socket-backend container (--cap-drop ALL, no devices, dialing)")
    checked(
        run,
        [
            "podman",
            "run",
            "-d",
            "--name",
            "dtiop-socket",
            "--network",
            config.net,
            "--ip",
            config.ip_socket,
            "--cap-drop",
            "ALL",
            "-v",
            f"{config.bin_path}:/usr/local/bin/wg-interop:ro",
            config.image,
            "wg-interop",
            "serve",
            "--mode",
            "socket",
            "--priv",
            socket.private,
            "--ula",
            config.ula_socket,
            "--wg-port",
            str(config.wg_port),
            "--peer-pub",
            tun.public,
            "--peer-ula",
            config.ula_tun,
            "--peer-endpoint",
            f"{config.ip_tun}:{config.wg_port}",
            "--dial",
        ],
        "start socket container",
    )

    wait_marker(config, "dtiop-tun", "INTEROP: serving", timeout=60, run=run, sleep=sleep)
    wait_marker(config, "dtiop-socket", "INTEROP: serving", timeout=60, run=run, sleep=sleep)
    note(config, "both backends up on one network")

    note(config, "leg A: socket -> tun (handshake initiated by the unprivileged side)")
    wait_marker(config, "dtiop-socket", "INTEROP: tcp echo PASS", timeout=90, run=run, sleep=sleep)
    wait_marker(config, "dtiop-socket", "INTEROP: udp echo PASS", timeout=90, run=run, sleep=sleep)
    note(config, "leg A PASS: smoltcp TCP + UDP echo against the TUN backend")

    note(config, "leg B: tun -> socket (kernel-originated connections into smoltcp)")
    tcp = checked(
        run,
        ["podman", "exec", "dtiop-tun", "wg-interop", "client", "tcp", f"[{config.ula_socket}]:7000"],
        "tun->socket tcp echo",
    )
    append_log(config, tcp.stdout)
    if "CLIENT tcp PASS" not in tcp.stdout:
        fail("tun->socket tcp echo")
    udp = checked(
        run,
        ["podman", "exec", "dtiop-tun", "wg-interop", "client", "udp", f"[{config.ula_socket}]:7002"],
        "tun->socket udp echo",
    )
    append_log(config, udp.stdout)
    if "CLIENT udp PASS" not in udp.stdout:
        fail("tun->socket udp echo")
    note(config, "leg B PASS: kernel TCP + UDP echo against the userspace backend")

    note(config, "leg C: the socket container is genuinely unprivileged")
    checked(
        run,
        ["podman", "exec", "dtiop-socket", "sh", "-c", "test ! -e /dev/net/tun"],
        "socket container has a TUN device",
    )
    note(config, "leg C PASS: no /dev/net/tun, all caps dropped")

    for container in ("dtiop-tun", "dtiop-socket"):
        logs = run(["podman", "logs", container])
        append_log(config, f"== logs {container} ==\n{logs.stdout}{logs.stderr}")
    cleanup(config, run)
    print(f"INTEROP PASS: userspace <-> TUN wire compatibility proven (log: {config.log_path})")
    return 0


def main(argv: list[str]) -> int:
    config = default_config(Path(__file__).resolve().parent)
    try:
        return run_interop(config)
    except SmokeFailure as exc:
        print(exc)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
