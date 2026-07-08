#!/usr/bin/env python3
import importlib.util
import contextlib
import io
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("install_desktop_entry.py")
SPEC = importlib.util.spec_from_file_location("install_desktop_entry", MODULE_PATH)
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


class InstallDesktopEntryTest(unittest.TestCase):
    def test_installs_hicolor_icons_and_desktop_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "repo"
            data_home = Path(tmp) / "data"
            self.write_icons(root)
            bin_path = Path(tmp) / "bin" / "ducktape"

            with contextlib.redirect_stdout(io.StringIO()):
                installed = installer.install_desktop_entry(
                    installer.InstallConfig(root, data_home, bin_path),
                    which=lambda _name: None,
                )

            self.assertEqual(installed, data_home / "applications" / "ducktape.desktop")
            for size in ("32x32", "64x64", "128x128"):
                self.assertEqual(
                    (data_home / "icons" / "hicolor" / size / "apps" / "ducktape.png").read_bytes(),
                    f"{size}\n".encode(),
                )
            self.assertEqual(
                (data_home / "icons" / "hicolor" / "256x256" / "apps" / "ducktape.png").read_bytes(),
                b"128x128@2x\n",
            )
            self.assertEqual(
                (data_home / "icons" / "hicolor" / "512x512" / "apps" / "ducktape.png").read_bytes(),
                b"icon\n",
            )
            self.assertEqual(
                installed.read_text(encoding="utf-8"),
                "[Desktop Entry]\n"
                "Type=Application\n"
                "Name=Ducktape\n"
                "Comment=Consensus-based workplace super-app\n"
                f"Exec={bin_path}\n"
                "Icon=ducktape\n"
                "Terminal=false\n"
                "Categories=Office;\n"
                "StartupWMClass=ducktape\n",
            )

    def test_refreshes_desktop_and_icon_caches_when_tools_exist(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "repo"
            data_home = Path(tmp) / "data"
            self.write_icons(root)
            calls = []

            def which(name):
                return f"/usr/bin/{name}"

            def run(args, **kwargs):
                calls.append((tuple(args), kwargs))

                class Completed:
                    returncode = 7

                return Completed()

            with contextlib.redirect_stdout(io.StringIO()):
                installer.install_desktop_entry(
                    installer.InstallConfig(root, data_home, Path("/opt/ducktape")),
                    which=which,
                    run=run,
                )

            self.assertEqual(
                calls,
                [
                    (("update-desktop-database", str(data_home / "applications")), {"check": False}),
                    (("gtk-update-icon-cache", "-t", str(data_home / "icons" / "hicolor")), {"check": False}),
                ],
            )

    def test_config_from_env_uses_xdg_data_home_or_home_default(self):
        root = Path("/repo")
        explicit = installer.config_from_env(
            root,
            ["/bin/ducktape"],
            {"XDG_DATA_HOME": "/tmp/data", "HOME": "/home/user"},
        )
        defaulted = installer.config_from_env(
            root,
            ["/bin/ducktape"],
            {"HOME": "/home/user"},
        )

        self.assertEqual(explicit.data_home, Path("/tmp/data"))
        self.assertEqual(defaulted.data_home, Path("/home/user/.local/share"))

    def write_icons(self, root: Path) -> None:
        icon_dir = root / "app" / "src-tauri" / "icons"
        icon_dir.mkdir(parents=True)
        for name in ("32x32", "64x64", "128x128", "128x128@2x", "icon"):
            (icon_dir / f"{name}.png").write_bytes(f"{name}\n".encode())


if __name__ == "__main__":
    unittest.main()
