# Wavora

Wavora is a local, immersive multimedia music player. Its application layer
is built in Rust, and its native Wayland and Vulkan UI is rendered with the
Iris, Lens, and Flux graphics stack from [Optics](../optics). Rodio and
Symphonia decode audio in the application. The decoded PCM is then passed to
GStreamer and the system's PipeWire, PulseAudio, or ALSA output, so common
formats do not depend on GStreamer codec plugins.

English is the project's working language. Use American English for project
documentation and English for developer collaboration. The user interface
remains localized in English and Simplified Chinese.

Current product direction:

- Local-first, with no dependency on online music accounts or proprietary
  music-source APIs
- A dark, restrained music stage with a strong sense of spatial depth
- Six visual engines with distinct compositions, driven by a 32-band
  spectrum, pitch, loudness, low/mid/high bands, and transients
- Separate playback, media-scanning, and rendering paths to avoid blocking UI
  frames
- System-language detection, with settings to force English or Simplified
  Chinese
- A virtualized track table with a fixed header, visible scrollbar, and
  Wayland-conventional scrolling direction
- Atomic configuration writes and startup restoration of the most recent
  track, favorites, volume, and visual preset; corrupt configuration is backed
  up and recovered automatically

Built-in decoding supports FLAC, MP3, M4A/AAC, Ogg Vorbis, and WAV. During
scanning, Wavora opens each file and reads its actual duration. Files that
cannot be decoded are skipped and included in a summary notice.

## Run

Build the adjacent Optics repository first:

```bash
meson setup ../optics/build ../optics -Dexamples=true
meson compile -C ../optics/build
```

Then run Wavora directly with Cargo:

```bash
cargo run --release
```

You can also pass music files or directories:

```bash
cargo run --release -- ~/Music
cargo run --release -- ~/Music/example.flac
cargo run --release -- --visuals --preset=0
```

Use `--visuals` or `--library` to open the visual stage or music library at
startup. Use `--preset=0..5` to preview a visual preset without overwriting the
saved selection.

A local installation defaults to `~/.local` and includes the desktop entry,
icon, AppStream metadata, and Optics runtime libraries:

```bash
./packaging/install.sh
```

Set `PREFIX=/custom/prefix ./packaging/install.sh` to change the installation
prefix. GStreamer base libraries, native audio-output plugins, and the Vulkan
and Wayland drivers must still be provided by the system.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release --locked
cargo audit --no-fetch
```

Runtime dependencies include Vulkan 1.3, Wayland, GStreamer 1.20+, `appsrc`,
`audioconvert`, `audioresample`, `volume`, and one of the PipeWire, PulseAudio,
automatic audio, or ALSA output plugins. GStreamer's FLAC, MP3, and AAC decoder
plugins are not required.

The workspace is divided by responsibility into `wavora-core` (domain model),
`wavora-audio-analysis` (pure PCM feature extraction), `wavora-i18n` (language
resolution and localized copy), `wavora-media` (scanning, decoding, and
output), `wavora-visuals` (presets, response envelopes, transitions, and Flux
drawing), and the root application (state, persistence, and Optics UI). See
[Architecture](docs/ARCHITECTURE.md) and [Design Direction](docs/DESIGN.md) for
the architectural and design constraints.
