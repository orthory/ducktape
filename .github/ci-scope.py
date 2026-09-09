#!/usr/bin/env python3
"""Only guest integration-test Rust changes can omit server/product gates."""
import re
import subprocess
import sys


def full_suite(paths):
    return not paths or any(
        re.fullmatch(rb"crates/views/[^/]+/tests/[^/]+\.rs", path) is None
        for path in paths
    )


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        test = b"crates/views/agents/tests/view.rs"
        assert not full_suite([test])
        assert not full_suite([test, b"crates/views/chat/tests/view.rs"])
        for path in (
            b"app/src/main.rs", b"Cargo.lock", b"Makefile",
            b"crates/views/agents/src/lib.rs", b"crates/views/Cargo.toml",
            b"crates/views/support/design/src/lib.rs",
            b"crates/views/agents/tests/data.json", b".github/workflows/pr.yml",
            b"crates/views/agents/tests/nested/helper.rs",
            b"crates/views/agents/tests/view.rs\nCargo.lock",
        ):
            assert full_suite([test, path]), path
        assert full_suite([])
        print("CI scope assertions passed")
    else:
        # No rename folding: moving production code into tests must include its
        # deleted source path and continue to select the full suite.
        changed = subprocess.check_output([
            "git", "diff", "--no-renames", "--name-only", "-z", sys.argv[1], "HEAD",
        ])
        paths = changed.rstrip(b"\0").split(b"\0") if changed else []
        print("full=" + str(full_suite(paths)).lower())
