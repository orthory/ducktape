#!/usr/bin/env python3
import contextlib
import importlib.util
import io
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
import uuid


MODULE_PATH = Path(__file__).with_name("dev.py")
SPEC = importlib.util.spec_from_file_location("dev", MODULE_PATH)
dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev)


class DevTest(unittest.TestCase):
    def cleanup_process(self, proc: subprocess.Popen) -> None:
        if proc.poll() is None:
            proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)

    def test_port_probe_reports_free_and_bound_local_ports(self):
        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
        sock.close()

        self.assertFalse(dev.port_probe(port))

        listener = socket.socket()
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", port))
        listener.listen(1)
        try:
            self.assertTrue(dev.port_probe(port))
        finally:
            listener.close()

    def test_stage_node_replaces_existing_staged_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "target" / "debug" / "ducktape-node"
            dst = root / "staged" / "ducktape-node"
            src.parent.mkdir(parents=True)
            dst.parent.mkdir(parents=True)
            src.write_text("#!/usr/bin/env bash\necho fresh\n", encoding="utf-8")
            src.chmod(0o755)
            dst.write_text("stale\n", encoding="utf-8")

            config = dev.DevConfig(
                root=root,
                cargo="cargo",
                bun="bun",
                node_src=src,
                node_bin=dst,
            )

            self.assertTrue(dev.stage_node(config))
            self.assertEqual(dst.read_text(encoding="utf-8"), src.read_text(encoding="utf-8"))
            self.assertTrue(os.access(dst, os.X_OK))

    def test_restart_node_reports_dead_respawn_with_daemon_log_reason(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            node_src = root / "node-src"
            node_bin = root / "staged" / "ducktape-node"
            workspace = root / "wsdir"
            cfg = workspace / "node.toml"
            node_bin.parent.mkdir(parents=True)
            workspace.mkdir(parents=True)
            cfg.write_text("id=0\n", encoding="utf-8")

            node_bin.write_text("#!/usr/bin/env bash\nsleep 30\n", encoding="utf-8")
            node_bin.chmod(0o755)
            marker = f"test-{uuid.uuid4()}"
            old = subprocess.Popen([str(node_bin), "--config", str(cfg), "--process-marker", marker])
            self.addCleanup(self.cleanup_process, old)
            self._wait_for_pid(lambda: dev.node_pids(node_bin, marker))

            cargo = root / "cargo-stub"
            cargo.write_text(
                "#!/usr/bin/env bash\n"
                f"printf '%s' \"$RANDOM\" >>'{node_src}'\n"
                "exit 0\n",
                encoding="utf-8",
            )
            cargo.chmod(0o755)

            node_src.write_text(
                "#!/usr/bin/env bash\n"
                "echo 'FATAL bind 127.0.0.1:8844: address already in use' >&2\n"
                "exit 1\n",
                encoding="utf-8",
            )
            node_src.chmod(0o755)
            config = dev.DevConfig(
                root=root,
                cargo=str(cargo),
                bun="bun",
                node_src=node_src,
                node_bin=node_bin,
                process_marker=marker,
            )

            with contextlib.redirect_stdout(io.StringIO()) as stdout:
                dev.restart_node(config)

            output = stdout.getvalue()
            self.assertIn("rebuilt node exited on start", output)
            self.assertIn("address already in use", output)
            self.assertNotIn("node back", output)

    def test_restart_node_skips_bounce_when_binary_is_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            node_src = root / "node-src"
            node_bin = root / "staged" / "ducktape-node"
            workspace = root / "wsdir"
            cfg = workspace / "node.toml"
            node_bin.parent.mkdir(parents=True)
            workspace.mkdir(parents=True)
            cfg.write_text("id=0\n", encoding="utf-8")
            node_src.write_text("#!/usr/bin/env bash\nsleep 30\n", encoding="utf-8")
            node_src.chmod(0o755)
            node_bin.write_text(node_src.read_text(encoding="utf-8"), encoding="utf-8")
            node_bin.chmod(0o755)
            marker = f"test-{uuid.uuid4()}"
            old = subprocess.Popen([str(node_bin), "--config", str(cfg), "--process-marker", marker])
            self.addCleanup(self.cleanup_process, old)
            self._wait_for_pid(lambda: dev.node_pids(node_bin, marker))

            config = dev.DevConfig(
                root=root,
                cargo="true",
                bun="bun",
                node_src=node_src,
                node_bin=node_bin,
                process_marker=marker,
            )

            with contextlib.redirect_stdout(io.StringIO()) as stdout:
                dev.restart_node(config, sleep=lambda _seconds: None)

            self.assertIn("node binary unchanged - skipping restart", stdout.getvalue())
            self.assertIsNone(old.poll())

    def _wait_for_pid(self, producer):
        for _ in range(40):
            value = producer()
            if value:
                return value
            time.sleep(0.05)
        self.fail("timed out waiting for pid")


if __name__ == "__main__":
    unittest.main()
