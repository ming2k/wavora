# Wavora

**English** | [简体中文](README-zh.md)

[![CI](https://github.com/ming2k/wavora/actions/workflows/ci.yml/badge.svg)](https://github.com/ming2k/wavora/actions/workflows/ci.yml)
[![Release](https://github.com/ming2k/wavora/actions/workflows/release.yml/badge.svg)](https://github.com/ming2k/wavora/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A local-first, visually immersive music player. Native Wayland + Vulkan UI
rendered through the [Optics](https://github.com/ming2k/optics) graphics
stack, with on-device audio decoding and an audio-reactive visual stage.

## Demo

https://github.com/ming2k/wavora/raw/main/docs/media/demo.mp4

## Download

Prebuilt binaries are on the [Releases](https://github.com/ming2k/wavora/releases) page.
Each tarball statically links Optics and ships an `install.sh`.

## Build

Requires Rust ≥ 1.92, meson, Vulkan 1.3, Wayland, and GStreamer 1.20+.

```bash
# 1. Build and install Optics (one-time)
git clone https://github.com/ming2k/optics.git && cd optics
meson setup build --prefix=/usr/local -Ddefault_library=static -Dexamples=false
meson compile -C build && sudo meson install -C build

# 2. Build Wavora
cd .. && git clone https://github.com/ming2k/wavora.git && cd wavora
PKG_CONFIG_ALL_STATIC=1 cargo build --release
```

## Run

```bash
cargo run --release -- ~/Music
cargo run --release -- --visuals --preset=0
```

Flags: `--visuals` `--library` `--playlists` `--lyrics` `--preset=0..10`

## License

[MIT](LICENSE)
