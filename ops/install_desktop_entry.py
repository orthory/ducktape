#!/usr/bin/env python3
import os
from dataclasses import dataclass
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Callable


@dataclass(frozen=True)
class InstallConfig:
    repo_root: Path
    data_home: Path
    bin_path: Path


def icon_plan(repo_root: Path, data_home: Path) -> list[tuple[Path, Path]]:
    source = repo_root / "app" / "src-tauri" / "icons"
    root = data_home / "icons" / "hicolor"
    return [
        (source / "32x32.png", root / "32x32" / "apps" / "ducktape.png"),
        (source / "64x64.png", root / "64x64" / "apps" / "ducktape.png"),
        (source / "128x128.png", root / "128x128" / "apps" / "ducktape.png"),
        (source / "128x128@2x.png", root / "256x256" / "apps" / "ducktape.png"),
        (source / "icon.png", root / "512x512" / "apps" / "ducktape.png"),
    ]


def desktop_entry(bin_path: Path) -> str:
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Ducktape\n"
        "Comment=Consensus-based workplace super-app\n"
        f"Exec={bin_path}\n"
        "Icon=ducktape\n"
        "Terminal=false\n"
        "Categories=Office;\n"
        "StartupWMClass=ducktape\n"
    )


def install_desktop_entry(
    config: InstallConfig,
    *,
    which: Callable[[str], str | None] = shutil.which,
    run: Callable[..., subprocess.CompletedProcess] = subprocess.run,
) -> Path:
    for source, destination in icon_plan(config.repo_root, config.data_home):
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    apps_dir = config.data_home / "applications"
    apps_dir.mkdir(parents=True, exist_ok=True)
    desktop_path = apps_dir / "ducktape.desktop"
    desktop_path.write_text(desktop_entry(config.bin_path), encoding="utf-8")

    icon_root = config.data_home / "icons" / "hicolor"
    if which("update-desktop-database"):
        run(["update-desktop-database", str(apps_dir)], check=False)
    if which("gtk-update-icon-cache"):
        run(["gtk-update-icon-cache", "-t", str(icon_root)], check=False)

    print(f"installed {desktop_path}")
    return desktop_path


def config_from_env(
    repo_root: Path,
    argv: list[str],
    env: dict[str, str] = os.environ,
) -> InstallConfig:
    if not argv:
        raise ValueError("usage: install-desktop-entry.sh /abs/path/to/ducktape")
    if env.get("XDG_DATA_HOME"):
        data_home = Path(env["XDG_DATA_HOME"])
    else:
        data_home = Path(env.get("HOME", str(Path.home()))) / ".local" / "share"
    return InstallConfig(repo_root=repo_root, data_home=data_home, bin_path=Path(argv[0]))


def main(argv: list[str]) -> int:
    repo_root = Path(__file__).resolve().parent.parent
    try:
        config = config_from_env(repo_root, argv[1:])
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    install_desktop_entry(config)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
