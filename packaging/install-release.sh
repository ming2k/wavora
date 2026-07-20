#!/usr/bin/env bash
# Self-contained Wavora installer for the prebuilt release tarball.
#
# The release binary statically links the Optics graphics libraries
# (libiris, liblens, libflux, flux-text, flux-scene-graph), so this
# installer only copies files. System Vulkan, Wayland, and GStreamer
# must still be provided by your distribution.
#
# Usage:
#   ./install.sh                       # installs to ~/.local
#   PREFIX=/opt/wavora ./install.sh    # installs to /opt/wavora
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-${WAVORA_PREFIX:-$HOME/.local}}"

install -Dm755 "$ROOT/libexec/wavora/wavora" "$PREFIX/libexec/wavora/wavora"
install -Dm755 "$ROOT/bin/wavora" "$PREFIX/bin/wavora"
install -Dm644 "$ROOT/share/applications/io.github.ming2k.Wavora.desktop" \
    "$PREFIX/share/applications/io.github.ming2k.Wavora.desktop"
install -Dm644 "$ROOT/share/icons/hicolor/scalable/apps/io.github.ming2k.Wavora.svg" \
    "$PREFIX/share/icons/hicolor/scalable/apps/io.github.ming2k.Wavora.svg"
install -Dm644 "$ROOT/share/metainfo/io.github.ming2k.Wavora.metainfo.xml" \
    "$PREFIX/share/metainfo/io.github.ming2k.Wavora.metainfo.xml"

printf 'Installed Wavora to %s\nRun with: %s/bin/wavora\n' "$PREFIX" "$PREFIX"
