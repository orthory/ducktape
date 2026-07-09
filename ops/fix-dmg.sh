#!/usr/bin/env bash
# make app (macOS) — post-fix the tauri-built DMG so .VolumeIcon.icns doesn't
# render in the installer window.
#
# tauri-bundler's bundle_dmg.sh installs the volume icon by copying it to
# /Volumes/<name>/.VolumeIcon.icns but never hides or positions it. Through
# macOS 15 Finder name-hid dotfiles, so nobody noticed; macOS 26 Finder renders
# every dotfile at a disk-image root — REGARDLESS of the UF_HIDDEN flag or
# FinderInfo invisible bit (verified empirically on 26.5 with a probe image:
# plain dotfile, chflags-hidden, SetFile -a V, HFS+ and APFS — all rendered).
# So the icon file shows up at Finder's default grid slot, overlapping the app
# icon. Upstream tauri (dev branch, checked 2026-07-10) has the same gap; the
# create-dmg lineage solves it by parking hidden items outside the window's
# visible rect (their REPOSITION_HIDDEN_FILES_CLAUSE), which tauri only emits
# when a DMG background image is configured.
#
# Fix, per built DMG: reopen it read-write, park .VolumeIcon.icns far outside
# the ~660x400 window rect via Finder scripting (position persists in the
# DMG's .DS_Store; Finder addresses the nobrowse mount by folder alias),
# belt-and-suspenders chflags hidden (honored by pre-26 Finder and again if
# Apple fixes 26), and re-compress in place. Finder automation is the same
# dependency tauri's bundler itself uses to lay out the window, so this adds
# no new build requirement. The mount stays -nobrowse throughout: a browsable
# read-write mount lets fseventsd flush a .fseventsd journal dir into the
# image on unmount — one more dotdir macOS 26 would render. If DMG
# signing/notarization is ever added, it must happen AFTER this step.
#
# The same pass also re-asserts the app/Applications icon positions. The
# positions the build wrote (bundler defaults {180,170}/{480,170} — centered
# for the 660x368 content area) render shifted by ~{+20,+45} on a fresh open,
# reading visibly low and off-center; positions rewritten through this Finder
# channel render exactly where set (all measured via the accessibility tree
# on 26.5). So writing the same centered coordinates here is what actually
# centers the pair.
set -euo pipefail

cd "$(dirname "$0")/.."

log() { printf '\033[36m[fix-dmg]\033[0m %s\n' "$*"; }

# transient "resource busy" on detach is the classic dmg-pipeline flake
# (Spotlight/quicklook poking the fresh volume); retry briefly before letting
# a final loud attempt fail the build.
detach_mnt() { # $1 = mountpoint
  local _try
  for _try in 1 2 3 4 5; do
    hdiutil detach -quiet "$1" 2>/dev/null && return 0
    sleep 1
  done
  hdiutil detach "$1"
}

# x well beyond the bundler's default 660px window width; Finder never scrolls
# an installer window there on its own.
PARK_X=1200
PARK_Y=100

# icon centers for the two real items: x symmetric about the 660-wide window's
# middle (330±150), y so the 128px icon plus its label block centers in the
# 368px content area.
APP_X=180
APPLICATIONS_X=480
ROW_Y=172

shopt -s nullglob
dmgs=(target/release/bundle/dmg/*.dmg)
if [ ${#dmgs[@]} -eq 0 ]; then
  log "no DMG under target/release/bundle/dmg — nothing to do"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for dmg in "${dmgs[@]}"; do
  work="$tmp/$(basename "$dmg" .dmg)"
  mkdir -p "$work/mnt"

  # UDZO is read-only; round-trip through UDRW to edit. The explicit
  # mountpoint keeps addressing unambiguous even if a same-named volume is
  # already mounted.
  hdiutil convert -quiet "$dmg" -format UDRW -o "$work/rw.dmg"
  hdiutil attach -quiet -nobrowse -noverify -noautoopen -mountpoint "$work/mnt" "$work/rw.dmg"

  changed=0
  if [ -f "$work/mnt/.VolumeIcon.icns" ]; then
    /usr/bin/osascript >/dev/null <<EOF
tell application "Finder"
  set root to folder (POSIX file "$work/mnt" as alias)
  set position of item ".VolumeIcon.icns" of root to {$PARK_X, $PARK_Y}
  set position of item "Ducktape.app" of root to {$APP_X, $ROW_Y}
  set position of item "Applications" of root to {$APPLICATIONS_X, $ROW_Y}
end tell
EOF
    # give Finder a beat to flush the positions into the volume's .DS_Store
    sleep 2
    sync
    chflags hidden "$work/mnt/.VolumeIcon.icns"
    changed=1
  fi
  if [ -d "$work/mnt/.fseventsd" ]; then
    rm -rf "$work/mnt/.fseventsd"
    changed=1
  fi

  detach_mnt "$work/mnt"
  if [ "$changed" -eq 1 ]; then
    hdiutil convert -quiet "$work/rw.dmg" -format UDZO -o "$work/out.dmg"
    mv "$work/out.dmg" "$dmg"
    log "parked+hid .VolumeIcon.icns in $(basename "$dmg")"
  else
    log "$(basename "$dmg") has no .VolumeIcon.icns — left as-is"
  fi
done
