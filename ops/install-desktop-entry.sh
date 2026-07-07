#!/usr/bin/env bash
# install (or refresh) the user-level freedesktop launcher for the ducktape
# desktop app: hicolor icons + ducktape.desktop. the linux `make install-app`
# calls this after placing the binary; takes the absolute path of the
# installed binary as its only argument.
set -euo pipefail
cd "$(dirname "$0")/.."

bin="${1:?usage: install-desktop-entry.sh /abs/path/to/ducktape}"

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_home/applications"
icon_root="$data_home/icons/hicolor"

for size in 32x32 64x64 128x128; do
  install -Dm644 "app/src-tauri/icons/${size}.png" "$icon_root/${size}/apps/ducktape.png"
done
install -Dm644 "app/src-tauri/icons/128x128@2x.png" "$icon_root/256x256/apps/ducktape.png"
install -Dm644 "app/src-tauri/icons/icon.png" "$icon_root/512x512/apps/ducktape.png"

# the window's wayland app_id / x11 WM_CLASS is the binary name ("ducktape"),
# so the desktop file must be named ducktape.desktop and StartupWMClass must
# match it — that is what groups the running window onto the launcher icon.
mkdir -p "$apps_dir"
cat > "$apps_dir/ducktape.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Ducktape
Comment=Consensus-based workplace super-app
Exec=$bin
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
