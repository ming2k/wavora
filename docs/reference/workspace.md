# Workspace Reference

This page lists the current crate responsibilities, dependency direction, and
runtime boundaries. For the design model behind these boundaries, use
[System Architecture](../explanation/system-architecture.md).

## Dependency Direction

```text
wavora
  ├─ wavora-core
  ├─ wavora-i18n
  ├─ wavora-library ──> wavora-core
  │                  └─ wavora-media
  ├─ wavora-media ──> wavora-core
  │                └─ wavora-audio-analysis
  ├─ wavora-visuals ──> wavora-audio-analysis
  │                  └─ Optics Iris / Flux
  └─ Optics Iris / Lens / Flux
```

Dependencies point from orchestration toward domain and service crates. The
domain and analysis crates do not depend on the UI stack.

## Crates

| Crate | Responsibility |
|-------|----------------|
| `wavora` | Application state, configuration persistence, worker coordination, and Optics UI |
| `wavora-core` | Track identity, playback queue and mode semantics, lyrics schema and timing validation, playback state, and pure formatting logic |
| `wavora-audio-analysis` | Backend-independent PCM spectrum, pitch, loudness, band, and transient features |
| `wavora-i18n` | System-locale resolution, language preferences, keys, and localized copy tables |
| `wavora-library` | SQLite catalog, media identity, favorites, history, missing-file records, and playlists |
| `wavora-media` | File URIs, asynchronous scanning, decoding, bounded lyrics sidecar loading, analysis scheduling, and native output |
| `wavora-visuals` | Compositions, response envelopes, preset transitions, and Flux drawing |

## Runtime Boundaries

| Boundary | Owns | Must not access |
|----------|------|-----------------|
| Iris main thread | Lens UI, input, application state, and command dispatch | Decoder internals and filesystem traversal |
| Audio worker | Decoder, output pipeline, seek coordination, and audio-feature production | Lens, Flux, and application-state locks |
| Library worker | Streaming traversal, metadata, duration, and PCM-signature observations | Lens and Flux |
| Media catalog | Track identity, availability, playlists, favorites, history, and restart selection | UI layout and audio-output state |
| Flux paint callback | A lightweight visual-state snapshot | Application-state locks and the media layer |
| Audio analysis | PCM feature frames and transient history | Decoder, GStreamer, and UI types |

The application owns playback queue source and cursor state. `wavora-core`
owns deterministic queue transitions, while the audio worker receives only
load, play, pause, seek, and volume commands.

## Media and Catalog Behavior

- Scans are cancellable and stream results as they become available.
- Metadata uses the same Symphonia format stack as playback.
- Duration comes from the decoded media rather than a filename or extension.
- PCM signatures are cached and exclude mutable embedded tags.
- Catalog foreign keys are enabled, playlist ordering is transactional, and
  SQLite uses write-ahead logging.
- Catalog files use private filesystem permissions.
- Missing media remains in the catalog as unavailable records.
- JSON state is retained for preferences and one-time migration of legacy URI
  references.

The exact identity and matching rules are listed in the
[Track Identity Reference](track-identity.md).

## Interface Boundaries

| Capability | Owner |
|------------|-------|
| Table API, row virtualization, cell and body clipping | Optics Lens |
| Nested-clip replay and Flux drawing primitives | Optics |
| Wayland physical-to-logical scroll-axis conversion | Optics Iris |
| Player visual orchestration and visual-stage state | Wavora |
| Track-table content and product interaction state | Wavora |

The visual callback receives a logical-pixel viewport for the stage. The
viewport excludes the control rail and does not use the window device scale.

## Localization Boundary

Interface strings are resolved through `wavora-i18n::Key` or
`visual_preset_text`. The visual-rendering crate stores composition types and
palettes but no user-facing copy. The default language preference is `System`,
which resolves the system locale at startup.
