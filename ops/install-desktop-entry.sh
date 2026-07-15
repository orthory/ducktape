#!/usr/bin/env bash
# install (or refresh) the user-level freedesktop launcher for the ducktape
# desktop app: hicolor icons + ducktape.desktop. the linux `make install-app`
# calls this after placing the binary; takes the absolute path of the
# installed binary as its only argument.
set -euo pipefail
cd "$(dirname "$0")/.."

bin="${1:?usage: install-desktop-entry.sh /abs/path/to/ducktape}"
case "$bin" in
  /*) ;;
  *) echo "desktop executable must be an absolute path: $bin" >&2; exit 2 ;;
esac
case "$bin" in
  *$'\n'*|*$'\r'*) echo "desktop executable path contains a line break" >&2; exit 2 ;;
esac
[ -x "$bin" ] || { echo "desktop executable is not executable: $bin" >&2; exit 2; }

# Desktop Entry Exec quoting is not shell quoting. Inside double quotes these
# four characters need backslashes, and a literal percent is written as %%.
exec_path=${bin//\\/\\\\}
exec_path=${exec_path//\"/\\\"}
exec_path=${exec_path//\`/\\\`}
exec_path=${exec_path//\$/\\\$}
exec_path=${exec_path//%/%%}

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_home/applications"
icon_root="$data_home/icons/hicolor"

for size in 32x32 64x64 128x128; do
  install -Dm644 "app/src-iced/assets/icons/${size}.png" "$icon_root/${size}/apps/ducktape.png"
done
install -Dm644 "app/src-iced/assets/icons/128x128@2x.png" "$icon_root/256x256/apps/ducktape.png"
install -Dm644 "app/src-iced/assets/icons/icon.png" "$icon_root/512x512/apps/ducktape.png"

# the window's wayland app_id / x11 WM_CLASS is the binary name ("ducktape"),
# so the desktop file must be named ducktape.desktop and StartupWMClass must
# match it — that is what groups the running window onto the launcher icon.
mkdir -p "$apps_dir"
cat > "$apps_dir/ducktape.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Ducktape
Comment=Consensus-based workplace super-app
Exec="$exec_path"
Icon=ducktape
Terminal=false
Categories=Office;
StartupWMClass=ducktape
EOF

# cache refreshes are best-effort: the tools are often absent and desktops
# rescan on login anyway.
command -v update-desktop-database >/dev/null && update-desktop-database "$apps_dir" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -t "$icon_root" || true

echo "installed $apps_dir/ducktape.desktop"
