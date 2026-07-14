#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
OPTICS="$(cd -- "$ROOT/../optics" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

if [[ ! -f "$OPTICS/build/build.ninja" ]]; then
    meson setup "$OPTICS/build" "$OPTICS" -Dexamples=false
fi
meson compile -C "$OPTICS/build"

export IRIS_SOURCE_DIR="$OPTICS"
export IRIS_BUILD_DIR="$OPTICS/build"
export LENS_SOURCE_DIR="$OPTICS"
export LENS_BUILD_DIR="$OPTICS/build"
export FLUX_SOURCE_DIR="$OPTICS"
export FLUX_BUILD_DIR="$OPTICS/build"
cargo build --manifest-path "$ROOT/Cargo.toml" --release --locked

install -Dm755 "$ROOT/target/release/wavora" "$PREFIX/libexec/wavora/wavora"
install -Dm755 "$ROOT/packaging/wavora-launcher" "$PREFIX/bin/wavora"
install -d "$PREFIX/lib/wavora"
for component in iris lens flux; do
    for library in "$OPTICS/build/libs/$component"/lib*.so*; do
        if [[ -f "$library" || -L "$library" ]]; then
            cp -a "$library" "$PREFIX/lib/wavora/"
        fi
    done
done
for library in "$OPTICS/build/libs/flux/text"/lib*.so* \
               "$OPTICS/build/libs/flux/scene_graph"/lib*.so*; do
    if [[ -f "$library" || -L "$library" ]]; then
        cp -a "$library" "$PREFIX/lib/wavora/"
    fi
done
install -Dm644 "$ROOT/packaging/io.github.ming2k.Wavora.desktop" \
    "$PREFIX/share/applications/io.github.ming2k.Wavora.desktop"
install -Dm644 "$ROOT/packaging/io.github.ming2k.Wavora.svg" \
    "$PREFIX/share/icons/hicolor/scalable/apps/io.github.ming2k.Wavora.svg"
install -Dm644 "$ROOT/packaging/io.github.ming2k.Wavora.metainfo.xml" \
    "$PREFIX/share/metainfo/io.github.ming2k.Wavora.metainfo.xml"

printf 'Installed Wavora to %s\n' "$PREFIX"
