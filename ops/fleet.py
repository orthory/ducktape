#!/usr/bin/env python3
import datetime
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Callable


AGENT_APP_ID = "com.ducktape.app"


@dataclass(frozen=True)
class ParsedWorktree:
    path: str
    branch: str | None


@dataclass(frozen=True)
class FleetWorktree:
    path: str
    branch: str
    id: str


@dataclass(frozen=True)
class FleetInstance:
    row: FleetWorktree
    slot: int
    display: str
    vite_port: int
    vnc_port: int
    home: Path
    runtime_dir: Path
    endpoint: Path
    app: Path
    token_file: Path


@dataclass(frozen=True)
class FleetConfig:
    self_dir: Path
    console_dir: Path
    dist: Path
    real_home: Path
    prefix: Path
    state: Path
    tokens: Path
    node_bin: Path
    x11vnc: Path
    xdo: Path
    novnc: Path
    main_root: Path
    base_branch: str
    disp_base: int
    vite_base: int
    vnc_base: int
    web_port: int
    screen: str
    tsip: str

    @property
    def slots_path(self) -> Path:
        return self.state / "slots.json"


def log(message: str) -> None:
    print(f"  {message}")


def command_args(args: tuple) -> list[str | Path]:
    if len(args) == 1 and isinstance(args[0], (list, tuple)):
        return list(args[0])
    return list(args)


def run_output(*args, cwd: str | Path | None = None, timeout: int = 8) -> str:
    try:
        return subprocess.run(
            [str(arg) for arg in command_args(args)],
            cwd=str(cwd) if cwd is not None else None,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        ).stdout.strip()
    except Exception:
        return ""


def run_to_log(
    args: list[str | Path],
    log_path: Path,
    *,
    cwd: str | Path | None = None,
    env: dict[str, str] | None = None,
) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("wb") as handle:
        return subprocess.run(
            [str(arg) for arg in args],
            cwd=str(cwd) if cwd is not None else None,
            env=env,
            stdout=handle,
            stderr=subprocess.STDOUT,
            check=False,
        ).returncode


def background(
    args: list[str | Path],
    log_path: Path,
    *,
    cwd: str | Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.Popen:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    handle = log_path.open("ab")
    try:
        return subprocess.Popen(
            [str(arg) for arg in args],
            cwd=str(cwd) if cwd is not None else None,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=handle,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    finally:
        handle.close()


def config_from_env(env: dict[str, str] = os.environ) -> FleetConfig:
    self_dir = Path(__file__).resolve().parent
    real_home = Path(env.get("HOME", str(Path.home())))
    prefix = real_home / ".local" / "opt" / "remote-tauri"
    common_dir = run_output(
        ["git", "-C", self_dir, "rev-parse", "--path-format=absolute", "--git-common-dir"]
    )
    main_root = Path(common_dir).parent if common_dir else self_dir.parent
    tsip = run_output(["tailscale", "ip", "-4"]).splitlines()
    state = prefix / "fleet"
    tokens = state / "tokens"
    state.mkdir(parents=True, exist_ok=True)
    tokens.mkdir(parents=True, exist_ok=True)

    ld_library_path = f"{prefix}/root/usr/lib/x86_64-linux-gnu"
    if env.get("LD_LIBRARY_PATH"):
        ld_library_path = f"{ld_library_path}:{env['LD_LIBRARY_PATH']}"
    os.environ["LD_LIBRARY_PATH"] = ld_library_path

    return FleetConfig(
        self_dir=self_dir,
        console_dir=self_dir / "fleet-console",
        dist=self_dir / "fleet-console" / "dist",
        real_home=real_home,
        prefix=prefix,
        state=state,
        tokens=tokens,
        node_bin=prefix / "bin" / "ducktape-node",
        x11vnc=prefix / "root" / "usr" / "bin" / "x11vnc",
        xdo=prefix / "root" / "usr" / "bin" / "xdotool",
        novnc=prefix / "noVNC",
        main_root=main_root,
        base_branch=env.get("FLEET_BASE", "dev"),
        disp_base=int(env.get("FLEET_DISP_BASE", "110")),
        vite_base=int(env.get("FLEET_VITE_BASE", "1430")),
        vnc_base=int(env.get("FLEET_VNC_BASE", "5910")),
        web_port=int(env.get("FLEET_WEB_PORT", "6090")),
        screen=env.get("FLEET_SCREEN", "1400x900x24"),
        tsip=tsip[0] if tsip else "127.0.0.1",
    )


def slug(value: str) -> str:
    return re.sub(r"-+", "-", re.sub(r"[^a-z0-9]", "-", value.lower())).strip("-")


def parse_worktree_porcelain(raw: str) -> list[ParsedWorktree]:
    worktrees: list[ParsedWorktree] = []
    current: dict[str, str | None] = {}

    def flush() -> None:
        nonlocal current
        if current:
            worktrees.append(
                ParsedWorktree(
                    path=str(current.get("path", "")),
                    branch=current.get("branch"),
                )
            )
            current = {}

    for line in raw.splitlines():
        if line.startswith("worktree "):
            current = {"path": line[9:]}
        elif line.startswith("branch "):
            current["branch"] = line[7:].replace("refs/heads/", "")
        elif line == "detached":
            current["branch"] = None
        elif line == "":
            flush()
    flush()
    return worktrees


def discover_app_worktrees(
    main: str | Path,
    *,
    sh: Callable[..., str] = run_output,
    include_detached: bool = False,
) -> list[FleetWorktree]:
    raw = sh("git", "-C", str(main), "worktree", "list", "--porcelain")
    rows: list[FleetWorktree] = []
    for worktree in parse_worktree_porcelain(raw):
        branch = worktree.branch
        if branch is None:
            if not include_detached:
                continue
            branch = "DETACHED"
        if not os.path.isdir(os.path.join(worktree.path, "app")):
            continue
        rows.append(FleetWorktree(worktree.path, branch, slug(branch)))
    return rows


def select_worktrees(
    rows: list[FleetWorktree],
    wanted: list[str],
) -> list[FleetWorktree]:
    if not wanted:
        return rows
    wanted_set = set(wanted)
    return [row for row in rows if row.branch in wanted_set or row.id in wanted_set]


def format_tsv(rows: list[FleetWorktree]) -> str:
    return "".join(f"{row.path}\t{row.branch}\t{row.id}\n" for row in rows)


def load_slots(path: Path) -> dict[str, int]:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except Exception:
        return {}


def save_slots(path: Path, slots: dict[str, int]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(slots, handle)


def slot_for(path: Path | str, worktree_id: str) -> int:
    path = Path(path)
    slots = load_slots(path)
    if worktree_id not in slots:
        used = set(slots.values())
        slot = 0
        while slot in used:
            slot += 1
        slots[worktree_id] = slot
        save_slots(path, slots)
    return slots[worktree_id]


def instance_for(config: FleetConfig, row: FleetWorktree, slot: int) -> FleetInstance:
    runtime_dir = config.state / row.id
    return FleetInstance(
        row=row,
        slot=slot,
        display=f":{config.disp_base + slot}",
        vite_port=config.vite_base + slot,
        vnc_port=config.vnc_base + slot,
        home=runtime_dir / "home",
        runtime_dir=runtime_dir,
        endpoint=runtime_dir / "tauri-agent" / AGENT_APP_ID / "endpoint.json",
        app=Path(row.path) / "app",
        token_file=config.tokens / row.id,
    )


def resolve_instance(config: FleetConfig, row: FleetWorktree) -> FleetInstance:
    return instance_for(config, row, slot_for(config.slots_path, row.id))


def socket_open(host: str, port: int) -> bool:
    sock = socket.socket()
    sock.settimeout(0.25)
    try:
        return sock.connect_ex((host, port)) == 0
    finally:
        sock.close()


def port_up(config: FleetConfig, port: int) -> bool:
    if shutil.which("ss"):
        out = run_output(["ss", "-ltn"])
        needles = (
            f"127.0.0.1:{port} ",
            f"{config.tsip}:{port} ",
            f"0.0.0.0:{port} ",
            f"*:{port} ",
        )
        return any(needle in out for needle in needles)
    return socket_open("127.0.0.1", port) or (
        config.tsip != "127.0.0.1" and socket_open(config.tsip, port)
    )


def process_exists(pattern: str) -> bool:
    if shutil.which("pgrep"):
        return subprocess.run(
            ["pgrep", "-f", pattern],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode == 0
    out = run_output(["ps", "-eo", "args="])
    return pattern in out


def pkill(pattern: str) -> None:
    if shutil.which("pkill"):
        subprocess.run(
            ["pkill", "-f", pattern],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )


def ensure_node_bin(config: FleetConfig) -> bool:
    if (
        config.node_bin.exists()
        and os.access(config.node_bin, os.X_OK)
        and config.node_bin.stat().st_size > 0
    ):
        return True
    config.node_bin.parent.mkdir(parents=True, exist_ok=True)
    log("staging ducktape-node (cargo build -p node-bin)...")
    if run_to_log(
        ["cargo", "build", "-p", "node-bin"],
        config.state / "node-build.log",
        cwd=config.main_root,
    ) != 0:
        log(f"node-bin build FAILED - see {config.state / 'node-build.log'}")
        return False
    shutil.copy2(config.main_root / "target" / "debug" / "ducktape-node", config.node_bin)
    log(f"staged {config.node_bin}")
    return True


def allocate_ports(count: int) -> list[int]:
    sockets = [socket.socket() for _ in range(count)]
    try:
        for sock in sockets:
            sock.bind(("127.0.0.1", 0))
        return [sock.getsockname()[1] for sock in sockets]
    finally:
        for sock in sockets:
            sock.close()


def node_output(config: FleetConfig, args: list[str | Path]) -> str:
    return run_output([config.node_bin, *args], timeout=30).splitlines()[-1:]


def last_line(lines: list[str]) -> str:
    return lines[-1] if lines else ""


def seed_workspace(config: FleetConfig, worktree_id: str, home: Path) -> None:
    registry = home / ".ducktape" / "registry.json"
    workspace_dir = home / ".ducktape" / "workspaces" / worktree_id
    if registry.exists():
        return
    workspace_dir.mkdir(parents=True, exist_ok=True)
    listen, http, rpc = allocate_ports(3)
    chain = last_line(
        node_output(
            config,
            [
                "init",
                "--name",
                worktree_id,
                "--dir",
                workspace_dir,
                "--listen",
                f"127.0.0.1:{listen}",
                "--advertised",
                f"127.0.0.1:{listen}",
                "--http",
                f"127.0.0.1:{http}",
                "--rpc",
                f"127.0.0.1:{rpc}",
            ],
        )
    )
    pubkey = last_line(
        node_output(config, ["keygen", "--out", workspace_dir / "identity.key"])
    )
    doc = {
        "version": 1,
        "active": worktree_id,
        "workspaces": [
            {
                "id": worktree_id,
                "name": worktree_id,
                "chainId": chain,
                "pubkey": pubkey,
                "founder": True,
                "member": True,
                "ports": {"listen": listen, "http": http, "rpc": rpc},
            }
        ],
    }
    registry.parent.mkdir(parents=True, exist_ok=True)
    with registry.open("w", encoding="utf-8") as handle:
        json.dump(doc, handle, separators=(",", ":"))
    log(f"[{worktree_id}] seeded solo workspace (own node http 127.0.0.1:{http})")


def instance_env(config: FleetConfig, instance: FleetInstance) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "HOME": str(instance.home),
            "CARGO_HOME": str(config.real_home / ".cargo"),
            "RUSTUP_HOME": str(config.real_home / ".rustup"),
            "XDG_CACHE_HOME": str(config.real_home / ".cache"),
            "BUN_INSTALL_CACHE_DIR": str(config.real_home / ".bun" / "install" / "cache"),
            "PATH": f"{config.real_home / '.local' / 'bin'}:{env.get('PATH', '')}",
            "DISPLAY": instance.display,
            "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
            "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
            "LIBGL_ALWAYS_SOFTWARE": "1",
            "GDK_BACKEND": "x11",
            "DUCKTAPE_TAURI_DEV_PORT": str(instance.vite_port),
            "XDG_RUNTIME_DIR": str(instance.runtime_dir),
            "DUCKTAPE_NODE_BIN": str(config.node_bin),
        }
    )
    return env


def write_tauri_dev_config(instance: FleetInstance) -> Path:
    path = instance.runtime_dir / "no-before.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        f'{{ "build": {{ "beforeDevCommand": null, "devUrl": "http://localhost:{instance.vite_port}" }} }}\n',
        encoding="utf-8",
    )
    return path


def write_vnc_token(instance: FleetInstance) -> None:
    instance.token_file.parent.mkdir(parents=True, exist_ok=True)
    instance.token_file.write_text(
        f"{instance.row.id}: 127.0.0.1:{instance.vnc_port}\n",
        encoding="utf-8",
    )


def prepare_instance_runtime(instance: FleetInstance) -> None:
    instance.home.mkdir(parents=True, exist_ok=True)
    instance.runtime_dir.mkdir(parents=True, exist_ok=True)
    instance.runtime_dir.chmod(0o700)


def ensure_seeded_workspace(config: FleetConfig, instance: FleetInstance) -> None:
    if ensure_node_bin(config):
        seed_workspace(config, instance.row.id, instance.home)


def ensure_xvfb(config: FleetConfig, instance: FleetInstance) -> None:
    if not process_exists(f"Xvfb {instance.display} "):
        background(
            ["Xvfb", instance.display, "-screen", "0", config.screen, "-nolisten", "tcp"],
            instance.runtime_dir / "xvfb.log",
        )
    time.sleep(1)


def ensure_frontend_dependencies(config: FleetConfig, instance: FleetInstance) -> None:
    if not (instance.app / "node_modules").is_dir():
        env = os.environ.copy()
        env["BUN_INSTALL_CACHE_DIR"] = str(config.real_home / ".bun" / "install" / "cache")
        run_to_log(
            ["bun", "install"],
            instance.runtime_dir / "bun-install.log",
            cwd=instance.app,
            env=env,
        )


def ensure_vite_server(config: FleetConfig, instance: FleetInstance) -> None:
    if not port_up(config, instance.vite_port):
        env = os.environ.copy()
        env["DUCKTAPE_TAURI_DEV_PORT"] = str(instance.vite_port)
        env["BUN_INSTALL_CACHE_DIR"] = str(config.real_home / ".bun" / "install" / "cache")
        background(["bun", "run", "dev"], instance.runtime_dir / "vite.log", cwd=instance.app, env=env)


def ensure_tauri_app(config: FleetConfig, instance: FleetInstance) -> None:
    tauri_config = write_tauri_dev_config(instance)

    if instance.endpoint.exists() and not port_up(config, instance.vnc_port):
        instance.endpoint.unlink()

    if not instance.endpoint.exists():
        background(
            [
                "dbus-run-session",
                "--",
                "bunx",
                "tauri",
                "dev",
                "--config",
                tauri_config,
                "--no-dev-server-wait",
            ],
            instance.runtime_dir / "tauri.log",
            cwd=instance.app,
            env=instance_env(config, instance),
        )
        log(f"[{instance.row.id}] app starting (compiling if cold)...")


def ensure_x11vnc(config: FleetConfig, instance: FleetInstance) -> None:
    if not port_up(config, instance.vnc_port):
        subprocess.run(
            [
                str(config.x11vnc),
                "-display",
                instance.display,
                "-nopw",
                "-listen",
                "127.0.0.1",
                "-rfbport",
                str(instance.vnc_port),
                "-forever",
                "-shared",
                "-noxdamage",
                "-ncache",
                "0",
                "-bg",
                "-o",
                str(instance.runtime_dir / "x11vnc.log"),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )


def up_one(config: FleetConfig, row: FleetWorktree) -> None:
    instance = resolve_instance(config, row)
    prepare_instance_runtime(instance)
    log(
        f"[{row.id}] slot {instance.slot}  disp {instance.display}  "
        f"vite {instance.vite_port}  vnc {instance.vnc_port}"
    )
    ensure_seeded_workspace(config, instance)
    ensure_xvfb(config, instance)
    ensure_frontend_dependencies(config, instance)
    ensure_vite_server(config, instance)
    ensure_tauri_app(config, instance)
    ensure_x11vnc(config, instance)
    write_vnc_token(instance)


def focus_fill(config: FleetConfig, display: str) -> None:
    width, height = config.screen.split("x", 2)[:2]
    env = os.environ.copy()
    env["DISPLAY"] = display
    out = subprocess.run(
        [str(config.xdo), "search", "--name", "^Ducktape$"],
        env=env,
        capture_output=True,
        text=True,
        timeout=4,
        check=False,
    ).stdout.strip()
    window_ids = [line for line in out.splitlines() if line.strip()]
    if not window_ids:
        return
    window_id = window_ids[-1]
    for args in (
        [config.xdo, "windowmove", window_id, "0", "0"],
        [config.xdo, "windowsize", window_id, width, height],
        [config.xdo, "windowfocus", window_id],
    ):
        subprocess.run(
            [str(arg) for arg in args],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )


def up_web(config: FleetConfig) -> bool:
    if port_up(config, config.web_port):
        log(f"web :{config.web_port} already up")
        return True
    if not (config.dist / "index.html").is_file():
        log(f"console not built - run '{sys.argv[0]} build-console' first")
        return False
    background(
        [
            "python3",
            "-m",
            "websockify",
            "--web",
            config.dist,
            "--token-plugin",
            "websockify.token_plugins.TokenFile",
            "--token-source",
            config.tokens,
            f"{config.tsip}:{config.web_port}",
        ],
        config.state / "websockify.log",
        cwd=config.novnc / "utils" / "websockify",
    )
    time.sleep(2)
    log(f"web :{config.web_port} up (console + token router)")
    return True


def parse_int(value: str) -> int:
    try:
        return int(value or 0)
    except ValueError:
        return 0


def recent_commits(command: Callable[..., str], path: str) -> list[dict[str, str]]:
    commits = []
    raw = command("git", "-C", path, "log", "-4", "--pretty=%h\x1f%s\x1f%cr")
    for line in raw.splitlines():
        parts = line.split("\x1f")
        if len(parts) == 3:
            commits.append({"sha": parts[0], "subject": parts[1], "age": parts[2]})
    return commits


def agent_observe_contract(instance: FleetInstance, relative_path: str) -> dict:
    return {
        "protocol": "tauri-agent-observe-ndjson",
        "cwd": relative_path,
        "env": {"XDG_RUNTIME_DIR": str(instance.runtime_dir)},
        "argv": [
            "app/scripts/tauri-agent",
            "observe",
            "--app",
            AGENT_APP_ID,
            "--format",
            "ndjson",
        ],
    }


def fleet_node_for_row(
    config: FleetConfig,
    row: FleetWorktree,
    *,
    slots: dict[str, int],
    sh: Callable[..., str],
    port_open: Callable[[int], bool],
) -> dict:
    path, branch, worktree_id = row.path, row.branch, row.id
    sha = sh("git", "-C", path, "rev-parse", "--short", "HEAD")
    subject = sh("git", "-C", path, "log", "-1", "--pretty=%s")
    ahead = sh("git", "-C", path, "rev-list", "--count", f"{config.base_branch}..HEAD")
    behind = sh("git", "-C", path, "rev-list", "--count", f"HEAD..{config.base_branch}")
    dirty = len(
        [
            line
            for line in sh("git", "-C", path, "status", "--porcelain").splitlines()
            if line.strip()
        ]
    )
    relative_path = os.path.relpath(path, config.main_root)
    node = {
        "id": worktree_id,
        "branch": branch,
        "path": relative_path,
        "head": {"sha": sha, "subject": subject},
        "parent": config.base_branch if branch != config.base_branch else None,
        "ahead": parse_int(ahead),
        "behind": parse_int(behind),
        "activity": {"dirty": dirty, "commits": recent_commits(sh, path)},
        "status": "down",
    }

    if worktree_id in slots:
        instance = instance_for(config, row, slots[worktree_id])
        node.update(
            {
                "slot": instance.slot,
                "display": instance.display,
                "vncPort": instance.vnc_port,
                "token": worktree_id,
                "agent": {
                    "appId": AGENT_APP_ID,
                    "runtimeDir": str(instance.runtime_dir),
                    "endpointPath": str(instance.endpoint),
                    "endpointReady": instance.endpoint.exists(),
                    "observe": agent_observe_contract(instance, relative_path),
                },
            }
        )
        if instance.endpoint.exists() and port_open(instance.vnc_port):
            node["status"] = "up"
        elif (instance.runtime_dir / "tauri.log").is_file():
            node["status"] = "building"
    return node


def build_fleet_doc(
    config: FleetConfig,
    *,
    slots: dict[str, int] | None = None,
    sh: Callable[..., str] = run_output,
    port_open: Callable[[int], bool] | None = None,
    now: Callable[[], datetime.datetime] | None = None,
) -> dict:
    slots = load_slots(config.slots_path) if slots is None else slots
    port_open = port_open or (lambda port: port_up(config, port))
    now = now or (lambda: datetime.datetime.now(datetime.timezone.utc))
    nodes = []
    for row in discover_app_worktrees(config.main_root, sh=sh):
        nodes.append(
            fleet_node_for_row(
                config,
                row,
                slots=slots,
                sh=sh,
                port_open=port_open,
            )
        )

    nodes.sort(
        key=lambda node: (
            node["branch"] != config.base_branch,
            node.get("slot", 999),
            node["branch"],
        )
    )
    return {
        "generatedAt": now().isoformat(timespec="seconds"),
        "host": config.tsip,
        "webPort": config.web_port,
        "base": config.base_branch,
        "worktrees": nodes,
    }


def write_fleet_json(config: FleetConfig) -> str:
    doc = build_fleet_doc(config)
    config.dist.mkdir(parents=True, exist_ok=True)
    output = config.dist / "fleet.json"
    with output.open("w", encoding="utf-8") as handle:
        json.dump(doc, handle, indent=2)
    return f"fleet.json: {len(doc['worktrees'])} worktree(s) -> {output}"


def selected_rows(config: FleetConfig, wanted: list[str]) -> list[FleetWorktree]:
    return select_worktrees(
        discover_app_worktrees(config.main_root, include_detached=True),
        wanted,
    )


def cmd_up(config: FleetConfig, args: list[str]) -> int:
    print("bringing up fleet...")
    rows = selected_rows(config, args)
    for row in rows:
        up_one(config, row)
    time.sleep(3)
    for row in rows:
        focus_fill(config, resolve_instance(config, row).display)
    print(write_fleet_json(config))
    up_web(config)
    print()
    return cmd_url(config, [])


def cmd_down(config: FleetConfig, args: list[str]) -> int:
    print("tearing down...")
    for row in selected_rows(config, args):
        instance = resolve_instance(config, row)
        pkill("target/debug/ducktape-desktop")
        subprocess.run(
            [str(config.x11vnc), "-R", "stop"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        pkill(f"rfbport {instance.vnc_port}")
        pkill(f"Xvfb {instance.display} ")
        instance.token_file.unlink(missing_ok=True)
        instance.endpoint.unlink(missing_ok=True)
        log(f"[{row.id}] stopped")
    if not args:
        pkill("python3 -m websockify")
        log("web stopped")
    print(write_fleet_json(config))
    return 0


def cmd_refresh(config: FleetConfig, _args: list[str]) -> int:
    print(write_fleet_json(config))
    return 0


def cmd_build_console(config: FleetConfig, _args: list[str]) -> int:
    env = os.environ.copy()
    env["BUN_INSTALL_CACHE_DIR"] = str(config.real_home / ".bun" / "install" / "cache")
    install = subprocess.run(["bun", "install"], cwd=config.console_dir, env=env, check=False)
    if install.returncode != 0:
        return install.returncode
    build = subprocess.run(
        ["bun", "run", "build"],
        cwd=config.console_dir,
        env=env,
        check=False,
    )
    if build.returncode != 0:
        return build.returncode
    log(f"console built -> {config.dist}")
    return 0


def cmd_status(config: FleetConfig, _args: list[str]) -> int:
    print(f"fleet status (base {config.base_branch}):")
    for row in discover_app_worktrees(config.main_root, include_detached=True):
        instance = resolve_instance(config, row)
        state = "up" if port_up(config, instance.vnc_port) else "down"
        print(
            f"  {row.branch:<28} slot {instance.slot:<2} "
            f"vnc {instance.vnc_port:<5} {state}"
        )
    if port_up(config, config.web_port):
        log(f"web :{config.web_port} listening")
    else:
        log(f"web :{config.web_port} down")
    print()
    return cmd_url(config, [])


def cmd_url(config: FleetConfig, _args: list[str]) -> int:
    print("dashboard (open on any tailnet device):")
    log(f"http://{config.tsip}:{config.web_port}/")
    return 0


def main(argv: list[str]) -> int:
    config = config_from_env()
    cmd = argv[1] if len(argv) > 1 else "status"
    args = argv[2:]
    commands = {
        "up": cmd_up,
        "down": cmd_down,
        "status": cmd_status,
        "refresh": cmd_refresh,
        "build-console": cmd_build_console,
        "url": cmd_url,
    }
    if cmd not in commands:
        print(
            f"usage: {argv[0]} {{up|down|status|refresh|build-console|url}} [branch...]",
            file=sys.stderr,
        )
        return 1
    return commands[cmd](config, args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
