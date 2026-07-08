#!/usr/bin/env python3
import importlib.util
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("dogfood_forge.py")
SPEC = importlib.util.spec_from_file_location("dogfood_forge", MODULE_PATH)
dogfood = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dogfood)


class DogfoodForgeTest(unittest.TestCase):
    def test_resolve_base_url_prefers_explicit_env_and_strips_trailing_slash(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(
                dogfood.resolve_base_url(
                    {"DUCKTAPE_DEV_FORGE_URL": "http://127.0.0.1:7777/"},
                    Path(tmp),
                ),
                "http://127.0.0.1:7777",
            )

    def test_resolve_base_url_reads_active_workspace_http_listen(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            workspace = home / ".ducktape" / "workspaces" / "alpha"
            workspace.mkdir(parents=True)
            (home / ".ducktape" / "registry.json").write_text(
                json.dumps({"active": "alpha"}),
                encoding="utf-8",
            )
            (workspace / "node.toml").write_text(
                'http_listen = "127.0.0.1:9222"\n',
                encoding="utf-8",
            )

            self.assertEqual(dogfood.resolve_base_url({}, home), "http://127.0.0.1:9222")

    def test_resolve_base_url_falls_back_to_legacy_dev_port(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(dogfood.resolve_base_url({}, Path(tmp)), "http://127.0.0.1:8844")

    def test_config_from_env_preserves_existing_env_knobs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = dogfood.config_from_env(
                root,
                {
                    "HOME": str(root / "home"),
                    "FORGE_REPO": "self-hosted",
                    "FORGE_REMOTE": "branch-node",
                    "SRC_REF": "feature/head",
                },
            )

            self.assertEqual(config.repo_root, root)
            self.assertEqual(config.home, root / "home")
            self.assertEqual(config.forge_repo, "self-hosted")
            self.assertEqual(config.forge_remote, "branch-node")
            self.assertEqual(config.src_ref, "feature/head")

    def test_run_dogfood_adds_missing_remote_and_pushes_requested_ref(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []

            def run(args, **kwargs):
                calls.append((tuple(args), kwargs))
                if args[:3] == ["git", "remote", "get-url"]:
                    return dogfood.Completed(2, "")
                return dogfood.Completed(0, "")

            config = dogfood.DogfoodConfig(
                repo_root=root,
                home=root / "home",
                forge_repo="ducktape",
                forge_remote="ducktape-dev",
                src_ref="HEAD",
            )

            with contextlib.redirect_stdout(io.StringIO()):
                rc = dogfood.run_dogfood(config, run=run, health_check=lambda _base: True)

            self.assertEqual(rc, 0)
            self.assertIn(
                (("git", "remote", "add", "ducktape-dev", "http://127.0.0.1:8844/forge/ducktape"), {"cwd": root}),
                calls,
            )
            self.assertIn(
                (("git", "push", "ducktape-dev", "HEAD:refs/heads/main"), {"cwd": root}),
                calls,
            )

    def test_run_dogfood_repoints_existing_remote_before_push(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []

            def run(args, **kwargs):
                calls.append((tuple(args), kwargs))
                if args[:3] == ["git", "remote", "get-url"]:
                    return dogfood.Completed(0, "http://old.example/forge/ducktape\n")
                return dogfood.Completed(0, "")

            config = dogfood.DogfoodConfig(
                repo_root=root,
                home=root / "home",
                forge_repo="ducktape",
                forge_remote="ducktape-dev",
                src_ref="dev",
            )

            with contextlib.redirect_stdout(io.StringIO()):
                rc = dogfood.run_dogfood(config, run=run, health_check=lambda _base: True)

            self.assertEqual(rc, 0)
            self.assertIn(
                (("git", "remote", "set-url", "ducktape-dev", "http://127.0.0.1:8844/forge/ducktape"), {"cwd": root}),
                calls,
            )
            self.assertIn(
                (("git", "push", "ducktape-dev", "dev:refs/heads/main"), {"cwd": root}),
                calls,
            )

    def test_run_dogfood_fails_before_git_mutation_when_node_is_down(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []
            config = dogfood.DogfoodConfig(
                repo_root=root,
                home=root / "home",
                forge_repo="ducktape",
                forge_remote="ducktape-dev",
                src_ref="HEAD",
            )

            with (
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(io.StringIO()),
            ):
                rc = dogfood.run_dogfood(
                    config,
                    run=lambda args, **kwargs: calls.append((tuple(args), kwargs)) or dogfood.Completed(0, ""),
                    health_check=lambda _base: False,
                )

            self.assertEqual(rc, 1)
            self.assertEqual(calls, [])

    def test_run_dogfood_relays_push_failure_stderr(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            def run(args, **_kwargs):
                if args[:3] == ["git", "remote", "get-url"]:
                    return dogfood.Completed(0, "http://127.0.0.1:8844/forge/ducktape\n")
                if args[:2] == ["git", "push"]:
                    class FailedPush:
                        returncode = 128
                        stdout = ""
                        stderr = "fatal: push rejected\n"

                    return FailedPush()
                return dogfood.Completed(0, "")

            config = dogfood.DogfoodConfig(
                repo_root=root,
                home=root / "home",
                forge_repo="ducktape",
                forge_remote="ducktape-dev",
                src_ref="HEAD",
            )

            with (
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(io.StringIO()) as stderr,
            ):
                rc = dogfood.run_dogfood(config, run=run, health_check=lambda _base: True)

            self.assertEqual(rc, 128)
            self.assertIn("fatal: push rejected", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
