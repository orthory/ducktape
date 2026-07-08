#!/usr/bin/env python3
import datetime
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("fleet.py")
SPEC = importlib.util.spec_from_file_location("fleet", MODULE_PATH)
fleet = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fleet)


RAW_WORKTREES = """worktree /repo
HEAD abc
branch refs/heads/dev

worktree /repo/.claude/worktrees/feat+agent
HEAD def
branch refs/heads/feat/agent

worktree /repo/no-app
HEAD 123
branch refs/heads/chore/no-app

worktree /repo/detached
HEAD fed
detached
"""


class FleetTest(unittest.TestCase):
    def test_parse_worktree_porcelain_keeps_detached_as_none(self):
        parsed = fleet.parse_worktree_porcelain(RAW_WORKTREES)

        self.assertEqual(
            [(item.path, item.branch) for item in parsed],
            [
                ("/repo", "dev"),
                ("/repo/.claude/worktrees/feat+agent", "feat/agent"),
                ("/repo/no-app", "chore/no-app"),
                ("/repo/detached", None),
            ],
        )

    def test_discover_app_worktrees_can_skip_or_include_detached(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / "repo"
            feature = repo / ".claude" / "worktrees" / "feat+agent"
            detached = repo / "detached"
            for path in (repo / "app", feature / "app", detached / "app"):
                path.mkdir(parents=True)

            raw = RAW_WORKTREES.replace("/repo", str(repo))

            json_rows = fleet.discover_app_worktrees(str(repo), sh=lambda *_: raw)
            launcher_rows = fleet.discover_app_worktrees(
                str(repo),
                sh=lambda *_: raw,
                include_detached=True,
            )

            self.assertEqual(
                [(row.path, row.branch, row.id) for row in json_rows],
                [
                    (str(repo), "dev", "dev"),
                    (str(feature), "feat/agent", "feat-agent"),
                ],
            )
            self.assertEqual(launcher_rows[-1], fleet.FleetWorktree(str(detached), "DETACHED", "detached"))

    def test_select_worktrees_matches_branch_or_id_and_format_tsv_matches_shim(self):
        rows = [
            fleet.FleetWorktree("/repo", "dev", "dev"),
            fleet.FleetWorktree(
                "/repo/.claude/worktrees/feat+agent",
                "feat/agent",
                "feat-agent",
            ),
        ]

        selected = fleet.select_worktrees(rows, ["feat-agent"])

        self.assertEqual(selected, [rows[1]])
        self.assertEqual(
            fleet.format_tsv(selected),
            "/repo/.claude/worktrees/feat+agent\tfeat/agent\tfeat-agent\n",
        )

    def test_slot_for_reuses_existing_or_allocates_lowest_free_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "state" / "slots.json"
            path.parent.mkdir()
            path.write_text(json.dumps({"dev": 0, "feat-b": 2}), encoding="utf-8")

            self.assertEqual(fleet.slot_for(path, "dev"), 0)
            self.assertEqual(fleet.slot_for(path, "feat-a"), 1)
            self.assertEqual(
                json.loads(path.read_text(encoding="utf-8")),
                {"dev": 0, "feat-b": 2, "feat-a": 1},
            )

    def test_instance_for_centralizes_per_worktree_paths_and_ports(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=root / "repo",
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(root / "repo"), "feat/agent", "feat-agent")

            instance = fleet.instance_for(config, row, slot=2)

            self.assertEqual(instance.display, ":112")
            self.assertEqual(instance.vite_port, 1432)
            self.assertEqual(instance.vnc_port, 5912)
            self.assertEqual(instance.home, config.state / "feat-agent" / "home")
            self.assertEqual(instance.runtime_dir, config.state / "feat-agent")
            self.assertEqual(
                instance.endpoint,
                config.state
                / "feat-agent"
                / "tauri-agent"
                / "com.ducktape.app"
                / "endpoint.json",
            )
            self.assertEqual(instance.app, root / "repo" / "app")
            self.assertEqual(instance.token_file, config.tokens / "feat-agent")

    def test_instance_env_is_derived_from_the_instance_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=root / "repo",
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(root / "repo"), "feat/agent", "feat-agent")
            instance = fleet.instance_for(config, row, slot=2)

            env = fleet.instance_env(config, instance)

            self.assertEqual(env["HOME"], str(instance.home))
            self.assertEqual(env["DISPLAY"], instance.display)
            self.assertEqual(env["DUCKTAPE_TAURI_DEV_PORT"], str(instance.vite_port))
            self.assertEqual(env["XDG_RUNTIME_DIR"], str(instance.runtime_dir))
            self.assertEqual(env["DUCKTAPE_NODE_BIN"], str(config.node_bin))

    def test_write_tauri_dev_config_uses_instance_runtime_and_vite_port(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=root / "repo",
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(root / "repo"), "feat/agent", "feat-agent")
            instance = fleet.instance_for(config, row, slot=2)

            path = fleet.write_tauri_dev_config(instance)

            self.assertEqual(path, instance.runtime_dir / "no-before.json")
            self.assertEqual(
                path.read_text(encoding="utf-8"),
                '{ "build": { "beforeDevCommand": null, "devUrl": "http://localhost:1432" } }\n',
            )

    def test_agent_observe_contract_is_derived_from_instance_and_relative_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=root / "repo",
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(root / "repo"), "feat/agent", "feat-agent")
            instance = fleet.instance_for(config, row, slot=2)

            observe = fleet.agent_observe_contract(instance, ".claude/worktrees/feat+agent")

            self.assertEqual(
                observe,
                {
                    "protocol": "tauri-agent-observe-ndjson",
                    "cwd": ".claude/worktrees/feat+agent",
                    "env": {"XDG_RUNTIME_DIR": str(instance.runtime_dir)},
                    "argv": [
                        "app/scripts/tauri-agent",
                        "observe",
                        "--app",
                        "com.ducktape.app",
                        "--format",
                        "ndjson",
                    ],
                },
            )

    def test_up_one_orchestrates_instance_lifecycle_through_named_steps(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            app = root / "repo" / "app"
            app.mkdir(parents=True)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=root / "repo",
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(root / "repo"), "feat/agent", "feat-agent")
            port_checks = []
            background_calls = []
            run_to_log_calls = []
            vnc_calls = []

            def fake_port_up(_config, port):
                port_checks.append(port)
                return False

            def fake_background(args, log_path, *, cwd=None, env=None):
                background_calls.append((tuple(str(arg) for arg in args), log_path, cwd, env))

            def fake_run_to_log(args, log_path, *, cwd=None, env=None):
                run_to_log_calls.append((tuple(args), log_path, cwd, env))
                return 0

            def fake_run(args, **_kwargs):
                vnc_calls.append(tuple(str(arg) for arg in args))

                class Completed:
                    returncode = 0

                return Completed()

            with (
                patch.object(fleet, "ensure_node_bin", return_value=True),
                patch.object(fleet, "seed_workspace") as seed_workspace,
                patch.object(fleet, "process_exists", return_value=False),
                patch.object(fleet, "background", side_effect=fake_background),
                patch.object(fleet, "run_to_log", side_effect=fake_run_to_log),
                patch.object(fleet, "port_up", side_effect=fake_port_up),
                patch.object(fleet.subprocess, "run", side_effect=fake_run),
                patch.object(fleet.time, "sleep"),
            ):
                fleet.up_one(config, row)

            instance = fleet.resolve_instance(config, row)
            seed_workspace.assert_called_once_with(config, row.id, instance.home)
            self.assertEqual(port_checks, [instance.vite_port, instance.vnc_port])
            self.assertTrue(instance.runtime_dir.is_dir())
            self.assertEqual(instance.runtime_dir.stat().st_mode & 0o777, 0o700)
            self.assertEqual(
                (instance.runtime_dir / "no-before.json").read_text(encoding="utf-8"),
                '{ "build": { "beforeDevCommand": null, "devUrl": "http://localhost:1430" } }\n',
            )
            self.assertEqual(
                instance.token_file.read_text(encoding="utf-8"),
                "feat-agent: 127.0.0.1:5910\n",
            )
            self.assertEqual(
                run_to_log_calls[0][0],
                ("bun", "install"),
            )
            self.assertIn(("Xvfb", ":110", "-screen", "0", "1400x900x24", "-nolisten", "tcp"), [call[0] for call in background_calls])
            self.assertIn(("bun", "run", "dev"), [call[0] for call in background_calls])
            self.assertIn(
                (
                    "dbus-run-session",
                    "--",
                    "bunx",
                    "tauri",
                    "dev",
                    "--config",
                    str(instance.runtime_dir / "no-before.json"),
                    "--no-dev-server-wait",
                ),
                [call[0] for call in background_calls],
            )
            self.assertEqual(vnc_calls[0][0], str(config.x11vnc))
            self.assertIn("-rfbport", vnc_calls[0])
            self.assertIn(str(instance.vnc_port), vnc_calls[0])

    def test_fleet_node_for_row_builds_unslotted_down_node(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            main = root / "repo"
            feature = root / "wt" / "feat-agent"
            feature.mkdir(parents=True)
            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=root / "ops" / "fleet-console" / "dist",
                real_home=root / "home",
                prefix=root / "prefix",
                state=root / "prefix" / "fleet",
                tokens=root / "prefix" / "fleet" / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=main,
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="127.0.0.1",
            )
            row = fleet.FleetWorktree(str(feature), "feat/agent", "feat-agent")

            def sh(*args):
                cmd = tuple(str(arg) for arg in args)
                return {
                    ("git", "-C", str(feature), "rev-parse", "--short", "HEAD"): "def5678",
                    ("git", "-C", str(feature), "log", "-1", "--pretty=%s"): "add agent",
                    ("git", "-C", str(feature), "rev-list", "--count", "dev..HEAD"): "3",
                    ("git", "-C", str(feature), "rev-list", "--count", "HEAD..dev"): "1",
                    (
                        "git",
                        "-C",
                        str(feature),
                        "log",
                        "-4",
                        "--pretty=%h\x1f%s\x1f%cr",
                    ): "def5678\x1fadd agent\x1f2 minutes ago",
                    ("git", "-C", str(feature), "status", "--porcelain"): " M app/src/main.ts\n",
                }[cmd]

            node = fleet.fleet_node_for_row(
                config,
                row,
                slots={},
                sh=sh,
                port_open=lambda _port: False,
            )

            self.assertEqual(
                node,
                {
                    "id": "feat-agent",
                    "branch": "feat/agent",
                    "path": "../wt/feat-agent",
                    "head": {"sha": "def5678", "subject": "add agent"},
                    "parent": "dev",
                    "ahead": 3,
                    "behind": 1,
                    "activity": {
                        "dirty": 1,
                        "commits": [
                            {
                                "sha": "def5678",
                                "subject": "add agent",
                                "age": "2 minutes ago",
                            }
                        ],
                    },
                    "status": "down",
                },
            )

    def test_build_fleet_doc_preserves_agent_contract_and_status_gate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            main = root / "repo"
            dev = main
            feature = root / "wt" / "feat-agent"
            dist = root / "dist"
            state = root / "state"
            for path in (dev / "app", feature / "app"):
                path.mkdir(parents=True)
            endpoint = (
                state
                / "feat-agent"
                / "tauri-agent"
                / "com.ducktape.app"
                / "endpoint.json"
            )
            endpoint.parent.mkdir(parents=True)
            endpoint.write_text("{}", encoding="utf-8")

            raw = f"""worktree {dev}
HEAD abc
branch refs/heads/dev

worktree {feature}
HEAD def
branch refs/heads/feat/agent
"""

            def sh(*args):
                cmd = tuple(str(arg) for arg in args)
                if cmd == ("git", "-C", str(main), "worktree", "list", "--porcelain"):
                    return raw
                if cmd[0:3] == ("git", "-C", str(dev)):
                    return {
                        ("git", "-C", str(dev), "rev-parse", "--short", "HEAD"): "abc1234",
                        ("git", "-C", str(dev), "log", "-1", "--pretty=%s"): "dev tip",
                        ("git", "-C", str(dev), "rev-list", "--count", "dev..HEAD"): "0",
                        ("git", "-C", str(dev), "rev-list", "--count", "HEAD..dev"): "0",
                        ("git", "-C", str(dev), "log", "-4", "--pretty=%h\x1f%s\x1f%cr"): "",
                        ("git", "-C", str(dev), "status", "--porcelain"): "",
                    }[cmd]
                if cmd[0:3] == ("git", "-C", str(feature)):
                    return {
                        ("git", "-C", str(feature), "rev-parse", "--short", "HEAD"): "def5678",
                        ("git", "-C", str(feature), "log", "-1", "--pretty=%s"): "add agent",
                        ("git", "-C", str(feature), "rev-list", "--count", "dev..HEAD"): "3",
                        ("git", "-C", str(feature), "rev-list", "--count", "HEAD..dev"): "1",
                        (
                            "git",
                            "-C",
                            str(feature),
                            "log",
                            "-4",
                            "--pretty=%h\x1f%s\x1f%cr",
                        ): "def5678\x1fadd agent\x1f2 minutes ago",
                        ("git", "-C", str(feature), "status", "--porcelain"): " M app/src/main.ts\n",
                    }[cmd]
                raise AssertionError(f"unexpected command: {cmd}")

            config = fleet.FleetConfig(
                self_dir=root / "ops",
                console_dir=root / "ops" / "fleet-console",
                dist=dist,
                real_home=root / "home",
                prefix=root / "prefix",
                state=state,
                tokens=state / "tokens",
                node_bin=root / "prefix" / "bin" / "ducktape-node",
                x11vnc=root / "prefix" / "root" / "usr" / "bin" / "x11vnc",
                xdo=root / "prefix" / "root" / "usr" / "bin" / "xdotool",
                novnc=root / "prefix" / "noVNC",
                main_root=main,
                base_branch="dev",
                disp_base=110,
                vite_base=1430,
                vnc_base=5910,
                web_port=6090,
                screen="1400x900x24",
                tsip="100.64.0.1",
            )
            doc = fleet.build_fleet_doc(
                config,
                slots={"feat-agent": 1},
                sh=sh,
                port_open=lambda port: port == 5911,
                now=lambda: datetime.datetime(
                    2026, 7, 8, 12, 0, tzinfo=datetime.timezone.utc
                ),
            )

            self.assertEqual(doc["generatedAt"], "2026-07-08T12:00:00+00:00")
            self.assertEqual([node["branch"] for node in doc["worktrees"]], ["dev", "feat/agent"])

            feature_node = doc["worktrees"][1]
            self.assertEqual(feature_node["id"], "feat-agent")
            self.assertEqual(feature_node["path"], "../wt/feat-agent")
            self.assertEqual(feature_node["status"], "up")
            self.assertEqual(feature_node["agent"]["endpointPath"], str(endpoint))
            self.assertEqual(
                feature_node["agent"]["observe"],
                {
                    "protocol": "tauri-agent-observe-ndjson",
                    "cwd": "../wt/feat-agent",
                    "env": {"XDG_RUNTIME_DIR": str(state / "feat-agent")},
                    "argv": [
                        "app/scripts/tauri-agent",
                        "observe",
                        "--app",
                        "com.ducktape.app",
                        "--format",
                        "ndjson",
                    ],
                },
            )


if __name__ == "__main__":
    unittest.main()
