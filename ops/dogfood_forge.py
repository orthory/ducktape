#!/usr/bin/env python3
import json
import os
from dataclasses import dataclass, field
from pathlib import Path
import subprocess
import sys
import tomllib
from typing import Callable
from urllib import request


DEFAULT_BASE_URL = "http://127.0.0.1:8844"


@dataclass(frozen=True)
class Completed:
    returncode: int
    stdout: str = ""
    stderr: str = ""


@dataclass(frozen=True)
class DogfoodConfig:
    repo_root: Path
    home: Path
    forge_repo: str
    forge_remote: str
    src_ref: str
    env: dict[str, str] = field(default_factory=dict)
    base_url: str | None = None


def log(message: str) -> None:
    print(f"\033[36m[dogfood]\033[0m {message}")


def error(message: str) -> None:
    print(f"\033[31m[dogfood]\033[0m {message}", file=sys.stderr)


def config_from_env(repo_root: Path, env: dict[str, str] = os.environ) -> DogfoodConfig:
    return DogfoodConfig(
        repo_root=repo_root,
        home=Path(env.get("HOME", str(Path.home()))),
        forge_repo=env.get("FORGE_REPO", "ducktape"),
        forge_remote=env.get("FORGE_REMOTE", "ducktape-dev"),
        src_ref=env.get("SRC_REF", "HEAD"),
        env=dict(env),
    )


def resolve_base_url(env: dict[str, str], home: Path) -> str:
    if env.get("DUCKTAPE_DEV_FORGE_URL"):
        return env["DUCKTAPE_DEV_FORGE_URL"].rstrip("/")

    registry_path = home / ".ducktape" / "registry.json"
    try:
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
        active = registry.get("active")
    except Exception:
        active = None
    if not active:
        return DEFAULT_BASE_URL

    node_toml = home / ".ducktape" / "workspaces" / str(active) / "node.toml"
    try:
        node_config = tomllib.loads(node_toml.read_text(encoding="utf-8"))
        listen = node_config.get("http_listen")
    except Exception:
        listen = None
    if not listen:
        return DEFAULT_BASE_URL
    return f"http://{listen}"


def run_command(args: list[str], *, cwd: Path) -> Completed:
    result = subprocess.run(
        args,
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )
    return Completed(result.returncode, result.stdout, result.stderr)


def relay_failure(result: Completed) -> None:
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)


def node_is_healthy(base_url: str) -> bool:
    try:
        with request.urlopen(f"{base_url}/v1/status", timeout=5) as response:
            return 200 <= response.status < 300
    except Exception:
        return False


def run_dogfood(
    config: DogfoodConfig,
    *,
    run: Callable[..., Completed] = run_command,
    health_check: Callable[[str], bool] = node_is_healthy,
) -> int:
    base_url = config.base_url or resolve_base_url(config.env, config.home)
    remote_url = f"{base_url}/forge/{config.forge_repo}"

    log(f"node forge endpoint: {remote_url}")
    if not health_check(base_url):
        error(
            "no node responding at "
            f"{base_url} - start the dev app/node first (`make dev`), "
            "or set DUCKTAPE_DEV_FORGE_URL to a running node."
        )
        return 1

    existing = run(
        ["git", "remote", "get-url", config.forge_remote],
        cwd=config.repo_root,
    )
    if existing.returncode == 0:
        current = existing.stdout.strip()
        if current != remote_url:
            log(f"WARNING: '{config.forge_remote}' currently points at {current}")
            log(
                f"         re-pointing to {remote_url} - this remote is SHARED "
                "across all git"
            )
            log("         worktrees of this repo, so this also moves it for other worktrees.")
            result = run(
                ["git", "remote", "set-url", config.forge_remote, remote_url],
                cwd=config.repo_root,
            )
            if result.returncode != 0:
                relay_failure(result)
                return result.returncode
        log(f"remote '{config.forge_remote}' -> {remote_url}")
    else:
        result = run(
            ["git", "remote", "add", config.forge_remote, remote_url],
            cwd=config.repo_root,
        )
        if result.returncode != 0:
            relay_failure(result)
            return result.returncode
        log(f"added remote '{config.forge_remote}' -> {remote_url}")

    log(
        f"pushing '{config.src_ref}' -> {config.forge_remote} main "
        "(whole-repo pack over git-receive-pack)"
    )
    push = run(
        ["git", "push", config.forge_remote, f"{config.src_ref}:refs/heads/main"],
        cwd=config.repo_root,
    )
    if push.returncode != 0:
        relay_failure(push)
        return push.returncode

    log("done. ducktape now hosts itself in forge - browse it in the desktop Forge view.")
    log(f"re-run `make dogfood-forge` (or `git push {config.forge_remote} main`) to update.")
    return 0


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    return run_dogfood(config_from_env(repo_root))


if __name__ == "__main__":
    raise SystemExit(main())
