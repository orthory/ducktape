"""Offline checks for the dedicated lane guard; never contact Proxmox."""
import importlib.util
import json
import io
import tempfile
import subprocess
import tarfile
import tomllib
from types import SimpleNamespace
from unittest.mock import patch
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("lane", Path(__file__).with_name("proxmox-view-lane.py"))
lane = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lane)


class LaneGuardTests(unittest.TestCase):
    def test_only_three_distinct_owned_containers_can_pass(self):
        record = {
            "owner": "ducktape-view-lane-test", "host": "root@zk",
            "nodes": [
                {"id": 801, "name": "dt-view-a"},
                {"id": 802, "name": "dt-view-b"},
                {"id": 803, "name": "dt-view-c"},
            ],
        }
        configs = {
            node["id"]: {"hostname": node["name"], "description": record["owner"]}
            for node in record["nodes"]
        }
        lane.validate_record(record)
        for node in record["nodes"]:
            lane.validate_owner(record, node, configs[node["id"]])
        for wrong in [
            {"hostname": "another-task", "description": record["owner"]},
            {"hostname": "dt-view-a", "description": "another-owner"},
            {"hostname": "dt-view-a", "description": ""},
        ]:
            with self.assertRaisesRegex(ValueError, "ownership"):
                lane.validate_owner(record, record["nodes"][0], wrong)
        for ids in [[120, 802, 803], [100, 802, 803], [801, 801, 803], [801, 802]]:
            bad = dict(record, nodes=[{"id": i, "name": f"dt-view-{i}"} for i in ids])
            with self.assertRaises(ValueError):
                lane.validate_record(bad)

    def test_pct_encoded_description_keeps_exact_owner_check(self):
        record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "nodes": [
            {"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]
        ]}
        def remote(host, *command):
            if command[:2] == ("pct", "config"):
                return f"hostname: dt-view-{command[2]}\ndescription: {record['owner']}%0A\n"
            if "cat" in command:
                return record["owner"] + "\n"
            self.fail(f"unexpected command: {command}")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "lane.json"
            path.write_text(json.dumps(record))
            with patch("sys.argv", ["lane", "--record", str(path), "check"]), patch.object(lane, "remote", remote), patch("sys.stdout", io.StringIO()) as output:
                try:
                    lane.main()
                except ValueError as error:
                    self.fail(f"PVE's encoded trailing newline must preserve the exact owner: {error}")
                self.assertIn("all three ownership checks passed", output.getvalue())

    def test_reset_checks_every_inner_marker_before_stopping_any_node(self):
        record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "nodes": [
            {"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]
        ]}
        seen = []

        def remote(host, *command):
            seen.append(command)
            if command[:2] == ("pct", "config"):
                return f"hostname: dt-view-{command[2]}\ndescription: {record['owner']}\n"
            if "cat" in command:
                return "other-owner" if command[2] == 803 else record["owner"]
            self.fail(f"mutation attempted before full ownership verification: {command}")

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "lane.json"
            path.write_text(json.dumps(record))
            with patch("sys.argv", ["lane", "--record", str(path), "--reason", "test schema reset", "reset-network"]):
                with patch.object(lane, "remote", remote):
                    with self.assertRaisesRegex(ValueError, "inner ownership"):
                        lane.main()
        self.assertEqual(len(seen), 6)

    def test_owned_reset_stops_all_services_then_archives_network_state(self):
        record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "nodes": [
            {"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]
        ]}
        mutations = []

        def remote(host, *command):
            if command[:2] == ("pct", "config"):
                return f"hostname: dt-view-{command[2]}\ndescription: {record['owner']}\n"
            if "cat" in command:
                return record["owner"]
            mutations.append(command)
            return ""

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "lane.json"
            path.write_text(json.dumps(record))
            argv = ["lane", "--record", str(path), "--reason", "schema changed", "reset-network"]
            with patch("sys.argv", argv), patch.object(lane, "remote", remote), patch("sys.stdout", io.StringIO()):
                lane.main()
            events = [json.loads(line) for line in path.with_suffix(".events.jsonl").read_text().splitlines()]
            self.assertEqual([event["result"] for event in events], ["started", "node_archived", "node_archived", "node_archived", "network_data_archived"])
            backup = Path(events[0]["record_backup"])
            self.assertEqual(json.loads(backup.read_text()), record)
            self.assertEqual(events[0]["reason"], "schema changed")
            self.assertEqual(json.loads(path.read_text()), record)
        expected = [("pct", "exec", i, "--", "systemctl", "stop", "ducktape-view-lane.service")
                    for i in [801, 802, 803]]
        self.assertEqual(mutations[:3], expected)
        self.assertEqual(len(mutations), 6)
        for command in mutations[3:]:
            self.assertEqual(command[3:7], ("--", "sh", "-ec", command[-1]))
            self.assertNotIn("rm ", command[-1])
            self.assertIn("mv -T --", command[-1])

    def test_partial_archive_preserves_release_record_and_completed_paths(self):
        record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "release": {"revision": "old"},
                  "nodes": [{"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]]}
        def remote(host, *command):
            if command[:2] == ("pct", "config"):
                return f"hostname: dt-view-{command[2]}\ndescription: {record['owner']}\n"
            if "cat" in command:
                return record["owner"]
            if "sh" in command and command[2] == 802:
                raise RuntimeError("archive refused")
            return ""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "lane.json"
            path.write_text(json.dumps(record))
            with patch("sys.argv", ["lane", "--record", str(path), "--reason", "test", "reset-network"]), patch.object(lane, "remote", remote):
                with self.assertRaisesRegex(RuntimeError, "archive refused"):
                    lane.main()
            self.assertEqual(json.loads(path.read_text()), record)
            events = [json.loads(line) for line in path.with_suffix(".events.jsonl").read_text().splitlines()]
            self.assertEqual([event["result"] for event in events], ["started", "node_archived"])
            self.assertEqual(events[1]["container"], 801)
            self.assertEqual(json.loads(Path(events[0]["record_backup"]).read_text()), record)

    def test_archive_command_preserves_bytes_and_refuses_collision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            network = root / "network"
            network.mkdir()
            (network / "state").write_bytes(b"committed state")
            archive = root / "network-archive-test"
            command = lane.archive_network_command(str(root), str(archive))
            subprocess.run(["sh", "-ec", command], check=True)
            self.assertFalse(network.exists())
            self.assertEqual((archive / "state").read_bytes(), b"committed state")
            network.mkdir()
            (network / "state").write_bytes(b"new state")
            self.assertNotEqual(subprocess.run(["sh", "-ec", command]).returncode, 0)
            self.assertEqual((network / "state").read_bytes(), b"new state")
            self.assertEqual((archive / "state").read_bytes(), b"committed state")

    def test_inventory_excludes_existing_vms_and_containers(self):
        nodes = lane.choose_nodes([{"vmid": 200, "type": "qemu"}, {"vmid": 202, "type": "lxc"}],
                                  "ducktape-view-lane-test")
        self.assertEqual([node["id"] for node in nodes], [201, 203, 204])

    def test_actual_archive_commands_detect_corrupted_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "ducktape"
            binary.write_bytes(b"native binary")
            modules = root / "modules"
            modules.mkdir()
            (modules / "chat.component.wasm").write_bytes(b"component")
            archive = root / "release.tar"
            lane.package_release(binary, modules, "ab" * 20, "cd" * 20, archive)
            extracted = root / "extracted"
            extracted.mkdir()
            subprocess.run(["tar", "-xf", str(archive), "-C", str(extracted), "--no-same-owner"], check=True)
            command = ["sha256sum", "-c", "SHA256SUMS"]
            self.assertEqual(subprocess.run(command, cwd=extracted, capture_output=True).returncode, 0)
            (extracted / "modules/chat.component.wasm").write_bytes(b"corrupt")
            self.assertNotEqual(subprocess.run(command, cwd=extracted, capture_output=True).returncode, 0)

    def test_failed_provision_records_only_created_containers_and_never_adopts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            key = root / "id.pub"
            key.write_text("ssh-ed25519 public-test-key")
            args = SimpleNamespace(record=root / "lane.json", host="root@zk", template="store:template",
                                   storage="store", bridge="bridge", ssh_key=key)
            commands = []

            def remote(host, *command):
                commands.append(command)
                if command[0] == "mktemp":
                    return command[-1].replace("XXXXXX", "ABC123")
                if command[:3] == ("pct", "create", 203):
                    raise ValueError("ID allocated concurrently")
                return ""

            with patch.object(lane, "inventory", return_value=[{"vmid": 200}, {"vmid": 202}]), \
                 patch.object(lane, "remote", remote), patch.object(lane, "send_file"):
                with self.assertRaisesRegex(ValueError, "concurrently"):
                    lane.provision(args)
                record = json.loads(args.record.read_text())
                self.assertEqual(record["created"], [201])
                self.assertEqual([c[2] for c in commands if c[:2] == ("pct", "start")], [201])
                self.assertFalse(any("destroy" in c for c in commands))
                with self.assertRaisesRegex(ValueError, "record already exists"):
                    lane.provision(args)

    def test_changed_founding_files_require_reset_before_any_remote_rollout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "ducktape"
            binary.write_bytes(b"binary")
            modules = root / "modules"
            modules.mkdir()
            (modules / "chat.component.wasm").write_bytes(b"replacement genesis component")
            record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "nodes": [],
                      "release": {"files": {"modules/chat.component.wasm": "old-hash"}}}
            args = SimpleNamespace(host="root@zk", record=root / "lane.json", binary=binary,
                                   modules=modules, revision="ab" * 20, ui_revision="cd" * 20,
                                   reason="binary upgrade")
            with patch.object(lane, "live_addresses", return_value=["192.0.2.11", "192.0.2.12", "192.0.2.13"]), \
                 patch.object(lane, "remote_stage", return_value="/var/tmp/test-stage"), \
                 patch.object(lane, "remote"), patch.object(lane, "send_file") as upload:
                with self.assertRaisesRegex(ValueError, "founding files changed.*reset-network"):
                    lane.rollout(args, record)
                upload.assert_not_called()

    def test_partial_start_requires_reset_before_changed_founding_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "ducktape"
            binary.write_bytes(b"binary")
            modules = root / "modules"
            modules.mkdir()
            component = modules / "chat.component.wasm"
            component.write_bytes(b"founding A")
            args = SimpleNamespace(host="test", record=root / "lane.json", binary=binary,
                                   modules=modules, revision="ab" * 20, ui_revision="cd" * 20,
                                   reason="first rollout")
            record = {"owner": "ducktape-view-lane-test", "host": "test", "nodes": [
                {"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]
            ]}
            started = []

            def remote(host, *command):
                if "enable" in command:
                    if command[2] == 802:
                        raise ValueError("second node failed to start")
                    started.append(command[2])
                return ""

            with patch.object(lane, "live_addresses", return_value=["192.0.2.11", "192.0.2.12", "192.0.2.13"]), \
                 patch.object(lane, "remote_stage", return_value="/var/tmp/test-stage") as stage, \
                 patch.object(lane, "remote", remote), patch.object(lane, "send_file") as upload:
                with self.assertRaisesRegex(ValueError, "second node failed"):
                    lane.rollout(args, record)
                self.assertEqual(started, [801])
                persisted = json.loads(args.record.read_text())
                self.assertNotIn("release", persisted)
                self.assertIn("pending_release", persisted)
                component.write_bytes(b"different founding B")
                stage.reset_mock()
                upload.reset_mock()
                with patch.object(lane, "remote", return_value=""):
                    with self.assertRaisesRegex(ValueError, "founding files changed.*reset-network"):
                        lane.rollout(args, persisted)
                stage.assert_not_called()
                upload.assert_not_called()

    def test_three_node_rollout_validates_every_release_before_stopping(self):
        for fail_validation in [None, "checksum", "runtime"]:
            with self.subTest(fail_validation=fail_validation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = root / "ducktape"
                binary.write_bytes(b"binary bytes")
                modules = root / "modules"
                modules.mkdir()
                (modules / "governance.component.wasm").write_bytes(b"component bytes")
                args = SimpleNamespace(host="test", record=root / "lane.json", binary=binary,
                                       modules=modules, revision="ab" * 20, ui_revision="cd" * 20,
                                       reason="integrated revision")
                record = {"owner": "ducktape-view-lane-test", "host": "root@zk", "nodes": [
                    {"id": i, "name": f"dt-view-{i}"} for i in [801, 802, 803]
                ]}
                addresses = ["192.0.2.11", "192.0.2.12", "192.0.2.13"]
                commands, uploads = [], []

                def remote(host, *command):
                    commands.append(command)
                    if command[0] == "mktemp":
                        return command[-1].replace("XXXXXX", "ABC123")
                    if "ip" in command:
                        index = [801, 802, 803].index(command[2])
                        return json.dumps([{"addr_info": [{"scope": "global", "local": addresses[index]}]}])
                    if len(command) > 2 and command[2] == 802:
                        if fail_validation == "checksum" and "sha256sum -c" in str(command):
                            raise ValueError("injected stage failure: checksum mismatch")
                        if fail_validation == "runtime" and "./ducktape --version" in str(command):
                            raise ValueError("injected stage failure: missing ELF interpreter")
                    return ""

                with patch.object(lane, "remote", remote), patch.object(lane, "send_file", lambda *a: uploads.append(a)):
                    if fail_validation:
                        with self.assertRaisesRegex(ValueError, "stage failure"):
                            lane.rollout(args, record)
                        self.assertFalse(any("systemctl stop" in str(c) for c in commands),
                                         "bad staging must preserve every running node")
                        continue
                    lane.rollout(args, record)
                verified = [i for i, command in enumerate(commands) if "sha256sum -c" in str(command)]
                stopped = [i for i, command in enumerate(commands) if "systemctl stop" in str(command)]
                started = [i for i, command in enumerate(commands) if "enable" in command]
                self.assertEqual(len(verified), 3)
                self.assertEqual(len(stopped), 3)
                self.assertEqual(len(started), 3)
                self.assertLess(max(verified), min(stopped))
                self.assertLess(max(stopped), min(started))
                with tarfile.open(fileobj=io.BytesIO(uploads[0][2])) as archive:
                    self.assertEqual(archive.extractfile("modules/governance.component.wasm").read(), b"component bytes")
                    self.assertIn(b"ducktape", archive.extractfile("SHA256SUMS").read())
                self.assertEqual(record["release"]["revision"], "ab" * 20)
                for index in range(3):
                    config = tomllib.loads(lane.node_config(record, addresses, index))
                    self.assertEqual(config["peer_addrs"], [f"{ip}:26656" for ip in addresses])
                    self.assertEqual(config["wireguard_listen"], f"{addresses[index]}:51820")
                    self.assertEqual(config["http_listen"], "127.0.0.1:8844")
                    self.assertEqual(config["primary_coordinator"], "none")


if __name__ == "__main__":
    unittest.main()
