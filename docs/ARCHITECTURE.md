# Wavora Architecture

Wavora follows the boundary design proven in Termus, implemented with Rust
channels and the Optics desktop stack. The code is a Cargo workspace, and each
crate exposes only the APIs required by adjacent layers.

```text
Iris main thread
  ├─ Lens UI / input / app state
  ├─ wavora-visuals ──> Flux paint callback (visual state snapshot)
  ├─ commands ──> audio worker ──> Rodio/Symphonia decoder
  │                              ├─ wavora-audio-analysis
  │                              │    └─ PCM ──> 32 bands + pitch/loudness/onset
  │                              └─ GStreamer appsrc ──> native sound server
  └─ commands ──> library worker ──> filesystem + decoder validation
```

## Workspace and dependency direction

```text
wavora (binary + app)
  ├─ wavora-core
  ├─ wavora-i18n
  ├─ wavora-media ──> wavora-core
  │                └─ wavora-audio-analysis
  ├─ wavora-visuals ──> wavora-audio-analysis
  │                  └─ Optics Iris / Flux
  └─ Optics Iris / Lens / Flux

wavora-core            Track, PlaybackState, and pure formatting logic
wavora-audio-analysis  Backend-independent PCM feature frames: spectrum, pitch,
                       loudness, low/mid/high bands, and transients
wavora-i18n            System-locale resolution, language preferences, and typed
                       localized-copy tables
wavora-media           File URIs, asynchronous scanning, built-in decoding,
                       analysis scheduling, and native output
wavora-visuals         Six independent compositions, audio-response envelopes,
                       preset transitions, and Flux drawing
wavora                 Application state, configuration persistence, and UI
                       orchestration
```

- The UI does not own the decoder or scanner.
- The audio thread does not access Lens or Flux.
- The Flux paint callback reads only a lightweight visual snapshot and does
  not lock application state. The visual crate does not depend on the media
  layer.
- The audio-analysis crate does not depend on the decoder, GStreamer, or the
  UI. It clears transient history after a seek to prevent false beat events.
- File scanning uses cancellable streaming traversal. A worker thread verifies
  decoding support and reads the actual duration.
- Configuration is replaced atomically by writing a temporary file in the
  same directory and renaming it.
- Symphonia and Rodio decode audio into `f32` PCM. GStreamer handles only format
  conversion, resampling, volume, and native output, so common formats remain
  playable when GStreamer codec plugins are unavailable.
- Seek operations use the GStreamer timeline to call back into the decoder.
  End of stream is detected through both bus messages and an end-position
  guard.
- UI strings are accessed through `wavora-i18n::Key` or
  `visual_preset_text`. The visual-rendering crate stores only composition
  types and palettes, not user-facing copy. The default language preference
  is `System`, which resolves the system locale at startup.
- The UI calculates the visual stage's logical-pixel viewport and writes it to
  a lightweight visual snapshot. Flux callbacks draw in that local coordinate
  system, excluding the control rail from particle compositions. Visual
  adjustment parameters are persisted atomically with the main configuration.

## Tables and scrolling

The track table uses Lens's safe Rust API and virtualizes visible rows. Nested
clipping is applied to the header and body, and each cell is clipped
individually so long titles or artist names cannot overlap adjacent columns.
The scrollbar remains discoverable. Iris converts Wayland's physical scroll
axis to Lens's logical convention at the platform boundary, so scrolling down
behaves consistently across all Lens scroll controls.

## Optics boundary

Gaps in general-purpose graphics or UI capabilities belong in `../optics`.
Optics provides the safe Rust table API, virtualization callbacks, cell and
body clipping, nested-clip replay, and Wayland scroll-axis conversion. None of
these capabilities contain Wavora business logic. Wavora retains only
player-specific visual orchestration and product state.
