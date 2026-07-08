#!/usr/bin/env python3
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("run_smoke.py")
SPEC = importlib.util.spec_from_file_location("run_smoke", MODULE_PATH)
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class FakeRunner:
    def __init__(self):
        self.calls = []
        self.node_dirs = {}
        self.logs = {
            "dtwg-node0": (
                "tunnels applied on dt-abc(1 peer\n"
                "persisted mesh (epoch 2) restored on dt-abc\n"
                "1 mesh dial seed(s) from the persisted mesh\n"
                "tunnels applied on dt-abc\n"
            ),
            "dtwg-node1": (
                "tunnels applied on dt-def(1 peer(s); userspace socket backend\n"
                "persisted mesh (epoch 2) restored on dt-def\n"
                "tunnels applied on dt-def\n"
            ),
        }
        self.heights = {
            "dtwg-node0": [3, 7, 10, 11],
            "dtwg-node1": [3, 7, 10, 11],
        }

    def run(self, args, **kwargs):
        args = tuple(str(arg) for arg in args)
        self.calls.append((args, kwargs))
        if args[:3] == ("podman", "image", "exists"):
            return smoke.Completed(0, "")
        if args[:2] == ("podman", "logs"):
            return smoke.Completed(0, self.logs[args[2]])
        if args[:5] == ("podman", "run", "-d", "--name", "dtwg-node0"):
            self.node_dirs["dtwg-node0"] = self.extract_data_dir(args)
            return smoke.Completed(0, "")
        if args[:5] == ("podman", "run", "-d", "--name", "dtwg-node1"):
            self.node_dirs["dtwg-node1"] = self.extract_data_dir(args)
            return smoke.Completed(0, "")
        if args[:2] == ("podman", "stop"):
            for path in self.node_dirs.values():
                storage = path / "storage"
                storage.mkdir(parents=True, exist_ok=True)
                (storage / "mesh-state.json").write_text("{}", encoding="utf-8")
            return smoke.Completed(0, "")
        if args[:3] == ("podman", "exec", "dtwg-node0") and "status" in " ".join(args):
            return smoke.Completed(0, self.next_height("dtwg-node0"))
        if args[:3] == ("podman", "exec", "dtwg-node1") and "status" in " ".join(args):
            return smoke.Completed(0, self.next_height("dtwg-node1"))
        return smoke.Completed(0, "")

    def next_height(self, container):
        value = self.heights[container].pop(0)
        return f'{{"height":{value}}}\n'

    def extract_data_dir(self, args):
        for index, value in enumerate(args):
            if value == "-v" and index + 1 < len(args) and args[index + 1].endswith(":/data"):
                return Path(args[index + 1].removesuffix(":/data"))
        raise AssertionError(f"missing /data volume: {args}")


class WgSmokeTest(unittest.TestCase):
    def test_default_config_derives_paths_from_script_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp) / "repo" / "ops" / "wg-smoke"
            scratch.mkdir(parents=True)

            defaulted = smoke.default_config(scratch, {})
            overridden = smoke.default_config(scratch, {"BIN": "/tmp/ducktape-node"})

            self.assertEqual(defaulted.scratch, scratch)
            self.assertEqual(defaulted.log_path, scratch / "smoke.log")
            self.assertEqual(defaulted.bin_path, scratch.parent.parent / "target" / "debug" / "ducktape-node")
            self.assertEqual(overridden.bin_path, Path("/tmp/ducktape-node"))

    def test_prepare_node_dirs_writes_configs_and_clears_stale_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = self.config(tmp)
            for node in ("node0", "node1"):
                (config.scratch / node / "storage").mkdir(parents=True)
                (config.scratch / node / "block-underlay").write_text("stale", encoding="utf-8")

            smoke.prepare_node_dirs(config)

            self.assertFalse((config.scratch / "node0" / "storage").exists())
            self.assertFalse((config.scratch / "node1" / "block-underlay").exists())
            node0 = (config.scratch / "node0" / "node.toml").read_text(encoding="utf-8")
            node1 = (config.scratch / "node1" / "node.toml").read_text(encoding="utf-8")
            self.assertIn('wireguard_effect = "tun"', node0)
            self.assertIn('wireguard_effect = "socket"', node1)
            self.assertIn(f'bootstrapper_addr = "{config.ip0}:41000"', node1)

    def test_run_smoke_starts_nodes_with_expected_privileges_and_writes_restart_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = FakeRunner()
            config = self.config(tmp)
            (config.scratch / "node0" / "storage").mkdir(parents=True)
            (config.scratch / "node1" / "storage").mkdir(parents=True)

            with contextlib.redirect_stdout(io.StringIO()):
                rc = smoke.run_smoke(config, run=runner.run, sleep=lambda _seconds: None)

            self.assertEqual(rc, 0)
            calls = [call[0] for call in runner.calls]
            node0_run = next(call for call in calls if call[:4] == ("podman", "run", "-d", "--name") and call[4] == "dtwg-node0")
            node1_run = next(call for call in calls if call[:4] == ("podman", "run", "-d", "--name") and call[4] == "dtwg-node1")
            self.assertIn("--cap-add", node0_run)
            self.assertIn("NET_ADMIN", node0_run)
            self.assertIn("--device", node0_run)
            self.assertIn("/dev/net/tun", node0_run)
            self.assertIn("--cap-add", node1_run)
            self.assertIn("NET_ADMIN", node1_run)
            self.assertNotIn("--device", node1_run)
            self.assertEqual((config.scratch / "node0" / "block-underlay").read_text(encoding="utf-8"), f"{config.ip1}\n")
            self.assertEqual((config.scratch / "node1" / "block-underlay").read_text(encoding="utf-8"), f"{config.ip0}\n")

    def test_wait_marker_count_failure_appends_logs(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = self.config(tmp)
            runner = FakeRunner()
            runner.logs["dtwg-node0"] = "tunnels applied on dt-abc\n"

            with self.assertRaises(smoke.SmokeFailure):
                smoke.wait_marker_count(
                    config,
                    "dtwg-node0",
                    "tunnels applied on dt-",
                    count=2,
                    timeout=0,
                    run=runner.run,
                    sleep=lambda _seconds: None,
                )

            log = config.log_path.read_text(encoding="utf-8")
            self.assertIn("== logs dtwg-node0 ==", log)
            self.assertIn("tunnels applied on dt-abc", log)

    def test_parse_height_extracts_status_height(self):
        self.assertEqual(smoke.parse_height('{"height":42}\n'), 42)
        self.assertIsNone(smoke.parse_height("not json"))

    def test_run_command_reports_missing_executable_without_raising(self):
        result = smoke.run_command(["definitely-not-a-real-ducktape-command"])

        self.assertEqual(result.returncode, 127)
        self.assertIn("definitely-not-a-real-ducktape-command", result.stderr)

    def config(self, tmp: str) -> "smoke.SmokeConfig":
        root = Path(tmp) / "repo"
        scratch = root / "ops" / "wg-smoke"
        scratch.mkdir(parents=True)
        bin_path = root / "target" / "debug" / "ducktape-node"
        bin_path.parent.mkdir(parents=True)
        bin_path.write_text("#!/bin/sh\n", encoding="utf-8")
        bin_path.chmod(0o755)
        return smoke.SmokeConfig(scratch=scratch, bin_path=bin_path, log_path=scratch / "smoke.log")


if __name__ == "__main__":
    unittest.main()
