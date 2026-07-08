#!/usr/bin/env python3
import importlib.util
import contextlib
import io
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("run_interop.py")
SPEC = importlib.util.spec_from_file_location("run_interop", MODULE_PATH)
interop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(interop)


class FakeRunner:
    def __init__(self):
        self.calls = []
        self.logs = {
            "dtiop-tun": "INTEROP: serving\n",
            "dtiop-socket": "INTEROP: serving\nINTEROP: tcp echo PASS\nINTEROP: udp echo PASS\n",
        }

    def run(self, args, **kwargs):
        args = tuple(str(arg) for arg in args)
        self.calls.append((args, kwargs))
        if args[:2] == ("podman", "image") and args[2:] == ("exists", "localhost/dtinv-base"):
            return interop.Completed(0, "")
        if args[:2] == ("podman", "logs"):
            return interop.Completed(0, self.logs[args[2]])
        if args[0:2] == ("podman", "exec") and args[1:3] == ("exec", "dtiop-tun"):
            if "client" in args and "tcp" in args:
                return interop.Completed(0, "CLIENT tcp PASS\n")
            if "client" in args and "udp" in args:
                return interop.Completed(0, "CLIENT udp PASS\n")
        if args[0:2] == ("podman", "exec") and args[2] == "dtiop-socket":
            return interop.Completed(0, "")
        if len(args) >= 3 and args[1] == "keygen":
            seed = args[2]
            return interop.Completed(0, f"PRIV priv-{seed}\nPUB pub-{seed}\n")
        return interop.Completed(0, "")


class InteropSmokeTest(unittest.TestCase):
    def test_default_config_derives_paths_from_script_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp) / "repo" / "ops" / "wg-smoke"
            scratch.mkdir(parents=True)

            defaulted = interop.default_config(scratch, {})
            overridden = interop.default_config(scratch, {"BIN": "/tmp/wg-interop"})

            self.assertEqual(defaulted.scratch, scratch)
            self.assertEqual(defaulted.log_path, scratch / "interop.log")
            self.assertEqual(
                defaulted.bin_path,
                scratch.parent.parent / "target" / "debug" / "examples" / "wg_interop",
            )
            self.assertEqual(overridden.bin_path, Path("/tmp/wg-interop"))

    def test_parse_keygen_extracts_private_and_public_keys(self):
        self.assertEqual(
            interop.parse_keygen("PRIV abc\nPUB def\n"),
            interop.KeyPair(private="abc", public="def"),
        )

    def test_run_command_reports_missing_executable_without_raising(self):
        result = interop.run_command(["definitely-not-a-real-ducktape-command"])

        self.assertEqual(result.returncode, 127)
        self.assertIn("definitely-not-a-real-ducktape-command", result.stderr)

    def test_run_interop_starts_tun_and_socket_containers_with_expected_privileges(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = FakeRunner()
            config = self.config(tmp)

            with contextlib.redirect_stdout(io.StringIO()):
                rc = interop.run_interop(config, run=runner.run, sleep=lambda _seconds: None)

            self.assertEqual(rc, 0)
            calls = [call[0] for call in runner.calls]
            self.assertIn(("podman", "network", "create", "--subnet", "172.32.0.0/24", "dtiop"), calls)
            tun_run = next(call for call in calls if call[:4] == ("podman", "run", "-d", "--name") and call[4] == "dtiop-tun")
            socket_run = next(call for call in calls if call[:4] == ("podman", "run", "-d", "--name") and call[4] == "dtiop-socket")
            self.assertIn("--cap-add", tun_run)
            self.assertIn("NET_ADMIN", tun_run)
            self.assertIn("--device", tun_run)
            self.assertIn("/dev/net/tun", tun_run)
            self.assertIn("--cap-drop", socket_run)
            self.assertIn("ALL", socket_run)
            self.assertNotIn("--device", socket_run)

    def test_wait_marker_failure_appends_container_logs(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = FakeRunner()
            runner.logs["dtiop-tun"] = "not ready\n"
            config = self.config(tmp)

            with self.assertRaises(interop.SmokeFailure):
                interop.wait_marker(
                    config,
                    "dtiop-tun",
                    "INTEROP: serving",
                    timeout=0,
                    run=runner.run,
                    sleep=lambda _seconds: None,
                )

            self.assertIn("== logs dtiop-tun ==", config.log_path.read_text(encoding="utf-8"))
            self.assertIn("not ready", config.log_path.read_text(encoding="utf-8"))

    def config(self, tmp: str) -> "interop.InteropConfig":
        root = Path(tmp) / "repo"
        scratch = root / "ops" / "wg-smoke"
        scratch.mkdir(parents=True)
        bin_path = root / "target" / "debug" / "examples" / "wg_interop"
        bin_path.parent.mkdir(parents=True)
        bin_path.write_text("#!/bin/sh\n", encoding="utf-8")
        bin_path.chmod(0o755)
        return interop.InteropConfig(scratch=scratch, bin_path=bin_path, log_path=scratch / "interop.log")


if __name__ == "__main__":
    unittest.main()
