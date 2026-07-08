#!/usr/bin/env python3
import hashlib
import os
from dataclasses import dataclass
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from typing import Callable


@dataclass(frozen=True)
class DevConfig:
    root: Path
    cargo: str
    bun: str
    node_src: Path
    node_bin: Path
    process_marker: str | None = None


def log(message: str) -> None:
    print(f"\033[36m[dev]\033[0m {message}")


def port_probe(port: int) -> bool:
    sock = socket.socket()
    sock.settimeout(0.25)
    try:
        return sock.connect_ex(("127.0.0.1", port)) == 0
    finally:
        sock.close()


def node_pids(node_bin: Path, process_marker: str | None = None) -> list[int]:
    marker = f"{node_bin} --config"
    proc = Path("/proc")
    if proc.is_dir():
        pids = []
        for cmdline in proc.glob("[0-9]*/cmdline"):
            try:
                cmd = cmdline.read_bytes().replace(b"\0", b" ").decode("utf-8", "replace")
            except OSError:
                continue
            if marker in cmd and (process_marker is None or process_marker in cmd):
                pids.append(int(cmdline.parent.name))
        return pids

    result = subprocess.run(
        ["ps", "-eo", "pid=,args="],
        capture_output=True,
        text=True,
        check=False,
    )
    pids = []
    for line in result.stdout.splitlines():
        stripped = line.strip()
        if (
            not stripped
            or marker not in stripped
            or "awk" in stripped
            or (process_marker is not None and process_marker not in stripped)
        ):
            continue
        pid, _sep, _args = stripped.partition(" ")
        if pid.isdigit():
            pids.append(int(pid))
    return pids


def node_config_of(pid: int) -> str:
    cmdline = Path("/proc") / str(pid) / "cmdline"
    if cmdline.is_file():
        try:
            parts = cmdline.read_bytes().split(b"\0")
        except OSError:
            return ""
        for index, part in enumerate(parts):
            if part == b"--config" and index + 1 < len(parts):
                return parts[index + 1].decode("utf-8", "replace")
        return ""

    result = subprocess.run(
        ["ps", "-o", "command=", "-p", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    marker = " --config "
    command = result.stdout.strip()
    if marker not in command:
        return ""
    return command.split(marker, 1)[1].split(" ", 1)[0]


def file_digest(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return ""


def stage_node(config: DevConfig) -> bool:
    try:
        config.node_bin.parent.mkdir(parents=True, exist_ok=True)
        config.node_bin.unlink(missing_ok=True)
        shutil.copy2(config.node_src, config.node_bin)
        return True
    except OSError:
        return False


def process_alive(pid: int) -> bool:
    probe = subprocess.run(
        ["kill", "-0", str(pid)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if probe.returncode != 0:
        return False
    state = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(pid)],
        capture_output=True,
        text=True,
        check=False,
    )
    return not state.stdout.strip().startswith("Z")


def kill_pid(pid: int, signal: str = "-TERM") -> None:
    subprocess.run(
        ["kill", signal, str(pid)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def spawn_node(config: DevConfig, config_path: Path) -> subprocess.Popen:
    workspace = config_path.parent
    workspace.mkdir(parents=True, exist_ok=True)
    log_path = workspace / "daemon.log"
    handle = log_path.open("ab")
    try:
        proc = subprocess.Popen(
            [
                str(config.node_bin),
                "--config",
                str(config_path),
                *(["--process-marker", config.process_marker] if config.process_marker else []),
            ],
            stdin=subprocess.DEVNULL,
            stdout=handle,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    finally:
        handle.close()
    try:
        (workspace / "node.pid").write_text(f"{proc.pid}\n", encoding="utf-8")
    except OSError:
        pass
    return proc


def tail(path: Path, count: int) -> list[str]:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return []
    return lines[-count:]


def process_exited_during_startup(
    proc: subprocess.Popen,
    *,
    sleep: Callable[[float], None],
    checks: int = 20,
    interval: float = 0.1,
) -> bool:
    for _ in range(checks):
        if proc.poll() is not None:
            proc.wait()
            return True
        sleep(interval)
    return False


def restart_node(
    config: DevConfig,
    *,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    log("rust changed -> rebuilding ducktape-node...")
    before = file_digest(config.node_src)
    build = subprocess.run([config.cargo, "build", "-p", "node-bin"], check=False)
    if build.returncode != 0:
        log("build failed - leaving the running node up")
        return

    after = file_digest(config.node_src)
    if before and before == after:
        log("node binary unchanged - skipping restart")
        return

    pids = node_pids(config.node_bin, config.process_marker)
    pid = pids[0] if pids else None
    if not stage_node(config):
        log(f"could not stage the fresh node to {config.node_bin} - leaving the running node up")
        return
    if pid is None:
        log("built + staged; no live node - the app will spawn the fresh binary itself")
        return

    config_path = node_config_of(pid)
    if not config_path:
        log("could not read node --config; skipping restart")
        return

    workspace = Path(config_path).parent
    log(f"restarting node (pid {pid}) on {config_path}...")
    kill_pid(pid)
    for _ in range(60):
        if not process_alive(pid):
            break
        sleep(0.1)

    if process_alive(pid):
        log("old node ignored SIGTERM after 6s - sending SIGKILL")
        kill_pid(pid, "-KILL")
        for _ in range(30):
            if not process_alive(pid):
                break
            sleep(0.1)

    spawned = spawn_node(config, Path(config_path))
    if process_exited_during_startup(spawned, sleep=sleep):
        log("rebuilt node exited on start - last log lines:")
        for line in tail(workspace / "daemon.log", 20):
            print(f"    {line}")
        return
    log(f"node back (pid {spawned.pid}) on the fresh binary; app reconnects on its next heartbeat")


def watch_rust(config: DevConfig, stamp_file: Path) -> None:
    stamp_file.touch()
    while True:
        result = subprocess.run(
            [
                "find",
                "bin",
                "crates",
                "-name",
                "*.rs",
                "-newer",
                str(stamp_file),
                "-print",
                "-quit",
            ],
            cwd=config.root,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.stdout.strip():
            stamp_file.touch()
            restart_node(config)
        time.sleep(2)


def make_config(root: Path, env: dict[str, str] = os.environ) -> DevConfig:
    cargo = env.get("CARGO", "cargo")
    bun = env.get("BUN", "bun")
    node_src = root / "target" / "debug" / "ducktape-node"
    tag = subprocess.run(
        ["cksum"],
        input=str(root),
        capture_output=True,
        text=True,
        check=False,
    ).stdout.split(" ", 1)[0]
    if not tag:
        tag = hashlib.sha256(str(root).encode()).hexdigest()[:10]
    tmpdir = Path(env.get("TMPDIR", tempfile.gettempdir()))
    node_bin = tmpdir / f"ducktape-dev-node-{os.getuid()}-{tag}" / "ducktape-node"
    return DevConfig(root=root, cargo=cargo, bun=bun, node_src=node_src, node_bin=node_bin)


def stop_stale_nodes(config: DevConfig) -> None:
    stale = node_pids(config.node_bin, config.process_marker)
    if not stale:
        return
    for pid in stale:
        kill_pid(pid)
    joined = " ".join(str(pid) for pid in stale)
    log(f"stopped this worktree's stale node(s): {joined} ")
    time.sleep(0.5)


def write_tauri_config(config: DevConfig) -> Path:
    path = Path(os.environ.get("TMPDIR", tempfile.gettempdir())) / f"ducktape-dev-tauri-{os.getpid()}.json"
    try:
        path.write_text(
            f'{{"build":{{"beforeDevCommand":"{config.bun} run dev"}}}}\n',
            encoding="utf-8",
        )
    except OSError:
        log(f"could not write the dev tauri config to {path} (check TMPDIR/disk)")
        raise SystemExit(1)
    return path


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    config = make_config(root)
    os.environ["DUCKTAPE_NODE_BIN"] = str(config.node_bin)
    os.environ["DUCKTAPE_DISABLE_HEARTBEAT"] = "1"

    log("building ducktape-node (debug)...")
    build = subprocess.run([config.cargo, "build", "-p", "node-bin"], check=False)
    if build.returncode != 0:
        log("initial node build failed")
        return build.returncode
    if not stage_node(config):
        log(f"could not stage the dev node to {config.node_bin}")
        return 1

    if port_probe(1430):
        log(":1430 is already in use - another 'tauri dev'? Stop it first. Nothing was killed.")
        return 1

    stop_stale_nodes(config)
    cfg_override = write_tauri_config(config)
    stamp_file = Path(tempfile.mkstemp(prefix="ducktape-dev-stamp-")[1])
    watch_proc: subprocess.Popen | None = None
    try:
        if shutil.which("find"):
            watch_proc = subprocess.Popen(
                [sys.executable, __file__, "watch", str(stamp_file)],
                env={**os.environ, "DUCKTAPE_DEV_ROOT": str(root)},
                start_new_session=True,
            )
        else:
            log("'find' not on PATH - Rust hot-reload disabled")

        log("launching tauri dev (frontend hot-reload; Ctrl-C to stop)...")
        app = root / "app"
        if not app.is_dir():
            log(f"app/ not found from {root}")
            return 1
        return subprocess.run(
            [config.bun, "run", "tauri", "dev", "--config", str(cfg_override)],
            cwd=app,
            check=False,
        ).returncode
    finally:
        if watch_proc is not None:
            watch_proc.terminate()
        cfg_override.unlink(missing_ok=True)
        stamp_file.unlink(missing_ok=True)


def main_watch(argv: list[str]) -> int:
    root = Path(os.environ.get("DUCKTAPE_DEV_ROOT", str(Path(__file__).resolve().parent.parent)))
    config = make_config(root)
    watch_rust(config, Path(argv[2]))
    return 0


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "watch":
        raise SystemExit(main_watch(sys.argv))
    raise SystemExit(main(sys.argv))
