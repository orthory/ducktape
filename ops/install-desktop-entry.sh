#!/usr/bin/env bash
set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SELF/install_desktop_entry.py" "$@"
