# Wavora

[![CI](https://github.com/ming2k/wavora/actions/workflows/ci.yml/badge.svg)](https://github.com/ming2k/wavora/actions/workflows/ci.yml)
[![Release](https://github.com/ming2k/wavora/actions/workflows/release.yml/badge.svg)](https://github.com/ming2k/wavora/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A local-first, visually immersive music player. Native Wayland + Vulkan UI
rendered through the [Optics](https://github.com/ming2k/optics) graphics
stack (Iris / Lens / Flux), with on-device audio decoding and an
audio-reactive visual stage.

**Demo video: <https://vimeo.com/1211277546>**

---

## Highlights

- **Local-first.** No online account, no streaming service, no proprietary
  metadata API. Point Wavora at a folder and play.
- **Audio-reactive stage.** Eleven modular subject effects plus independent
  ambient materials, driven by a 32-band spectrum, pitch, loudness,
  transients, and current-track artwork.
- **Native rendering.** A dark, restrained Wayland surface drawn directly
  through Vulkan — no Electron, no toolkit web view.
- **Decoupled playback.** Scanning, decoding, and rendering run on separate
  threads so frames never wait on I/O.
- **Durable library.** Atomic configuration writes; favorites, playlists,
  volume, and the current queue survive restarts. Corrupt state is backed
  up and recovered automatically.
- **Synchronized lyrics.** Multi-track lyrics with translations,
  transliterations, and timed segments via validated `.wlyric.json`
  sidecars.
- **Localized.** English and Simplified Chinese, auto-detected from the
  system locale.

Built-in decoding covers FLAC, MP3, M4A/AAC, Ogg Vorbis, and WAV.
Decoding is handled by Symphonia + Rodio; the PCM is then handed to
GStreamer and the system's PipeWire, PulseAudio, or ALSA output, so
common formats do **not** depend on GStreamer codec plugins.

## Download a prebuilt binary

Prebuilt, self-contained Linux binaries are published on the
[Releases page](https://github.com/ming2k/wavora/releases). Each release
tarball statically links the Optics graphics libraries (libiris, liblens,
libflux, and the flux-text / flux-scene-graph siblings) into the `wavora`
binary, so you do not need to build Optics yourself. Vulkan, Wayland, and
GStreamer still come from your system.

To install a downloaded tarball:

```bash
mkdir -p /tmp/wavora-extract
tar -xzf wavora-*-x86_64-linux.tar.gz -C /tmp/wavora-extract
/tmp/wavora-extract/wavora-*-x86_64-linux/install.sh
# or, to choose a prefix:
PREFIX=$HOME/.local /tmp/wavora-extract/wavora-*-x86_64-linux/install.sh
```

## Build from source

### 1. Install system prerequisites

You need a recent Linux distribution with:

| Dependency            | Version   | Debian / Ubuntu                    | Fedora                              |
| --------------------- | --------- | ---------------------------------- | ----------------------------------- |
| Rust toolchain        | ≥ 1.92    | `rustup` recommended               | `rustup` recommended                |
| Meson + Ninja         | ≥ 1.0     | `meson ninja-build pkg-config`     | `meson ninja-build pkg-config`      |
| C compiler (gcc/clang)| C2x      | `gcc`                              | `gcc`                               |
| Vulkan SDK / headers  | ≥ 1.3     | `libvulkan-dev vulkan-tools`       | `vulkan-headers vulkan-tools`       |
| Wayland client        | any       | `libwayland-dev libxkbcommon-dev`  | `wayland-devel libxkbcommon-devel`  |
| GStreamer             | ≥ 1.20    | `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev` | `gstreamer1-devel gstreamer1-plugins-base-devel` |
| PipeWire / PulseAudio | any       | `gstreamer1.0-pipewire` (or `-pulseaudio`, `-alsa`) | `pipewire-devel` (or `pulseaudio-libs-devel`) |

A Vulkan-capable GPU and driver (Mesa RADV, NVIDIA proprietary, or AMDPRO) is required at runtime.

### 2. Build the Optics C libraries

The Rust bindings (`iris`, `lens`, `flux`) are pulled in automatically as
git dependencies, but the underlying native C libraries must be built and
installed once so `pkg-config` can find them:

```bash
git clone https://github.com/ming2k/optics.git
cd optics
meson setup build --prefix=/opt/optics -Ddefault_library=static -Dexamples=false
meson compile -C build
sudo meson install -C build      # or: meson install -C build --destdir /tmp/optics-pkg
```

`-Ddefault_library=static` produces `libiris.a`, `liblens.a`, `libflux.a`,
and the flux-text / flux-scene-graph static archives, so Wavora can
static-link them into a single self-contained binary. Drop the option if
you prefer shared libraries.

If you install to a non-system prefix (e.g. `/opt/optics`), expose it to
`pkg-config`:

```bash
export PKG_CONFIG_PATH=/opt/optics/lib/x86_64-linux-gnu/pkgconfig:/opt/optics/lib/pkgconfig:$PKG_CONFIG_PATH
# Static-link the Optics libraries into the binary:
export PKG_CONFIG_ALL_STATIC=1
# Tell the -sys build scripts to use the installed .pc files (not a build tree):
export FLUX_USE_INSTALLED=1 LENS_USE_INSTALLED=1 IRIS_USE_INSTALLED=1
```

### 3. Build Wavora

```bash
git clone https://github.com/ming2k/wavora.git
cd wavora
cargo build --release
```

The binary is at `target/release/wavora`.

### Local development with a sibling Optics checkout

If you hack on both Wavora and Optics, keep them as sibling directories
and let the build scripts auto-discover the meson build tree — no install
step or `PKG_CONFIG_PATH` needed:

```
projects/
├── optics/      # meson setup build && meson compile -C build
└── wavora/      # cargo run --release
```

```bash
# From optics/
meson setup build -Dexamples=false && meson compile -C build

# From wavora/
cargo run --release
```

To point Wavora at a local Optics checkout that is **not** a sibling,
override the git dependency with a `[patch]` entry in `Cargo.toml`:

```toml
[patch."https://github.com/ming2k/optics.git"]
flux = { path = "../optics/bindings/flux-rs/crates/flux" }
iris = { path = "../optics/bindings/iris-rs/crates/iris" }
lens = { path = "../optics/bindings/lens-rs/crates/lens" }
```

…and set `FLUX_BUILD_DIR`, `LENS_BUILD_DIR`, `IRIS_BUILD_DIR` to your
meson build tree if it is not `../optics/build`.

## Run

```bash
cargo run --release
cargo run --release -- ~/Music
cargo run --release -- ~/Music/example.flac
cargo run --release -- --visuals --preset=0
cargo run --release -- --lyrics
```

| Flag                     | Effect                                                     |
| ------------------------ | ---------------------------------------------------------- |
| `--visuals`              | Open the visual stage at startup.                          |
| `--library`              | Open the track library.                                    |
| `--playlists`            | Open the playlist browser.                                 |
| `--lyrics`               | Open the lyrics pane.                                      |
| `--preset=0..10`         | Preview a subject effect without overwriting the saved one.|
| _positional args_        | Files or directories to scan and queue.                    |

## Install

A local installation defaults to `~/.local` and includes the desktop
entry, icon, AppStream metadata, and the Optics runtime libraries:

```bash
./packaging/install.sh
# or:
PREFIX=/custom/prefix ./packaging/install.sh
```

GStreamer base libraries, native audio-output plugins, and the Vulkan
and Wayland drivers must still be provided by the system.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release --locked
cargo audit --no-fetch
```

Runtime dependencies: Vulkan 1.3, Wayland, GStreamer 1.20+ with
`appsrc`, `audiotestsrc`, `audioconvert`, `audioresample`, `volume`, and
one of the PipeWire, PulseAudio, automatic audio, or ALSA output
plugins. GStreamer's FLAC, MP3, and AAC decoder plugins are **not**
required.

## Architecture

The workspace is split by responsibility:

| Crate                     | Responsibility                                                        |
| ------------------------- | --------------------------------------------------------------------- |
| `wavora` (root)           | Application state, persistence, and Optics UI shell.                  |
| `wavora-core`             | Domain model.                                                         |
| `wavora-audio-analysis`   | Pure PCM feature extraction (spectrum, pitch, loudness, transients).  |
| `wavora-i18n`             | Language resolution and localized copy.                               |
| `wavora-library`          | Durable track identity and playlists.                                 |
| `wavora-media`            | Scanning, decoding, and audio output.                                 |
| `wavora-visuals`          | Subject and ambient modules, response envelopes, transitions, Flux.   |
| `wavora-ui`               | Design tokens and composable Iris recipes.                            |

Deeper docs:

- [System Architecture](docs/explanation/system-architecture.md)
- [Track Identity](docs/explanation/track-identity.md)
- [Visual Design](docs/explanation/visual-design.md)
- [Lyrics Format](docs/reference/lyrics-format.md)
- [Configuration Reference](docs/reference/configuration.md)
- [Visual Reference](docs/reference/visuals.md)
- [Workspace Reference](docs/reference/workspace.md)
- [Documentation Index](docs/index.md)

## Contributing

English is the project's working language. Use American English for
documentation and English for developer collaboration. The user
interface is localized in English and Simplified Chinese.

Pull requests should pass `cargo fmt`, `cargo clippy -- -D warnings`,
and `cargo test` before submission. See the
[CI workflow](.github/workflows/ci.yml) for the exact commands.

## License

[MIT](LICENSE). The Optics graphics stack is also MIT-licensed; see
[optics](https://github.com/ming2k/optics).
