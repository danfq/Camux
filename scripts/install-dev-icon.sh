#!/bin/sh
set -eu

if [ "$(uname -s)" != "Linux" ]; then
  exit 0
fi

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_directory=$(dirname -- "$script_directory")
data_directory=${XDG_DATA_HOME:-"${HOME}/.local/share"}
desktop_file="${data_directory}/applications/dev.danfq.camux.desktop"
icon_file="${data_directory}/icons/hicolor/512x512/apps/dev.danfq.camux.png"
legacy_icon_file="${data_directory}/icons/hicolor/1254x1254/apps/dev.danfq.camux.png"

refresh_desktop_cache() {
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${data_directory}/applications" >/dev/null 2>&1 || true
  fi

  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${data_directory}/icons/hicolor" >/dev/null 2>&1 || true
  fi

  if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
  elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental >/dev/null 2>&1 || true
  fi
}

if [ "${1:-}" = "--remove" ]; then
  rm -f -- "$desktop_file" "$icon_file" "$legacy_icon_file"
  refresh_desktop_cache
  exit 0
fi

install -Dm644 "${project_directory}/assets/linux.dev.desktop" "$desktop_file"
install -Dm644 "${project_directory}/assets/macos_icon_fallback.png" "$icon_file"
rm -f -- "$legacy_icon_file"
refresh_desktop_cache
