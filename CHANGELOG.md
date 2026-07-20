# Changelog

All notable changes to Wavora are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.5] - 2026-07-20

### Fixed

- Optics adds `--embed-dir` to embed SPIR-V shaders via C23 `#embed`
  (`libs/flux/meson.build:99`), which requires GCC 15+. The CI runner's
  default GCC 13 rejected the flag. Both workflows now install `gcc-15`
  from `ppa:ubuntu-toolchain-r/test` and set `CC=gcc-15` for the meson
  build.

## [0.0.4] - 2026-07-20

### Fixed

- CI built Optics with meson 1.3.2 (Ubuntu 24.04 apt) which rejected
  `import('wayland')` — stabilized only in meson 1.8.0. The workflows now
  `sudo pip3 install 'meson>=1.8.0'` to `/usr/local/bin`, shadowing the
  apt version via PATH ordering.

## [0.0.3] - 2026-07-20

### Fixed

- CI failed to build Optics: `fribidi` was missing (required by flux-text
  for bidirectional text shaping). Both workflows now install
  `libfribidi-dev`.

### Changed

- Simplified `packaging/`: removed `install.sh` (duplicated README build
  steps) and `wavora-launcher` (only set `LD_LIBRARY_PATH`, a no-op for
  static-linked releases). The binary now ships directly at `bin/wavora`.
- The release workflow injects the AppStream `<release>` version from
  the git tag, so version bumps only touch `Cargo.toml` + `CHANGELOG.md`.

## [0.0.2] - 2026-07-20

### Changed

- Rewrote README as a concise quick-start (~45 lines) with an inline
  demo video rendered from `docs/media/demo.mp4`.
- Added `README-zh.md` (Simplified Chinese) with a language toggle.

### Fixed

- CI failed to build Optics because `glslangValidator` was missing on
  the Ubuntu 24.04 runner. Both workflows now install `glslang-tools`,
  which provides the SPIR-V shader compiler Optics uses at build time.

## [0.0.1] - 2026-07-20

First public release. Wavora is a local-first, visually immersive music
player built on the Optics graphics stack (Iris / Lens / Flux) with native
Wayland + Vulkan rendering.

### Added

- Local audio playback for FLAC, MP3, M4A/AAC, Ogg Vorbis, and WAV via
  Symphonia + Rodio, routed through GStreamer to PipeWire, PulseAudio, or
  ALSA — no GStreamer codec plugins required for the common formats.
- Audio-reactive visual stage with eleven modular subject effects plus
  independent ambient materials, driven by a 32-band spectrum, pitch,
  loudness, low/mid/high bands, transients, and current-track artwork.
- Separate playback, media-scanning, and rendering paths to avoid
  blocking UI frames.
- Durable local library with atomic configuration writes and startup
  restoration of the most recent track, favorites, volume, and visual
  stage; corrupt configuration is backed up and recovered automatically.
- Durable local playlists backed by immutable track IDs; exact matching
  preserves moves and metadata edits, while guarded acoustic matching
  can reconnect one unambiguous re-encoded missing track.
- Queue-aware sequential, repeat-one, and shuffle playback; shuffle
  visits every queue entry before beginning another cycle.
- Synchronized multi-track lyrics with translations, transliterations,
  timed text segments, and optional media binding through validated
  `.wlyric.json` sidecars.
- Virtualized track table with a fixed header, visible scrollbar, and
  Wayland-conventional scrolling direction.
- System-language detection with settings to force English or
  Simplified Chinese; UI localized in both.
- Self-contained release pipeline: CI builds Optics statically and
  links libiris/liblens/libflux into a single binary; release tarball
  ships with a desktop entry, icon, AppStream metadata, and installer.
- Rust bindings (flux, lens, iris) consumed as git dependencies pinned
  to a specific Optics commit, so any consumer can build without a
  sibling Optics checkout.

[0.0.5]: https://github.com/ming2k/wavora/releases/tag/v0.0.5
[0.0.4]: https://github.com/ming2k/wavora/releases/tag/v0.0.4
[0.0.3]: https://github.com/ming2k/wavora/releases/tag/v0.0.3
[0.0.2]: https://github.com/ming2k/wavora/releases/tag/v0.0.2
[0.0.1]: https://github.com/ming2k/wavora/releases/tag/v0.0.1
