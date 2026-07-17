# System Architecture

Wavora adapts the boundary design used by Termus to Rust channels and the
Optics desktop stack. It separates user interaction, media work, persistence,
and rendering so that slow I/O and audio processing do not block the
interface. The application coordinates these parts through commands and small
state snapshots rather than allowing each subsystem to reach into the others.

For the exact crate responsibilities and dependency direction, use the
[Workspace Reference](../reference/workspace.md).

## Boundary Model

The interface runs on the Iris main thread. It owns application state and
sends commands to dedicated audio and library workers. Those workers return
events and observations that the application can incorporate on the next UI
frame.

```text
user input ──> application state ──> audio worker ──> native audio output
                    │                    │
                    │                    └─> audio features
                    │                              │
                    ├─> library worker             └─> visual snapshot
                    │         │                              │
                    │         └─> media catalog              └─> paint callback
                    │
                    └─> preferences
```

This arrangement keeps ownership clear. The interface does not own a decoder
or scanner, the audio worker does not access Lens or Flux, and the visual layer
does not depend on the media layer.

## Playback and Analysis

The audio worker decodes media into floating-point pulse-code modulation
(PCM). One branch extracts spectrum, pitch, loudness, frequency bands, and
transients. The other branch sends the decoded stream through GStreamer for
format conversion, resampling, volume control, and native output.

Decoding remains inside the application, so supported formats do not depend on
GStreamer codec plugins. GStreamer still owns the output timeline. A seek uses
that timeline to reposition the decoder, and end-of-stream handling combines
output events with a position guard.

Analysis is backend-independent. A seek clears transient history before new
PCM is analyzed, which prevents the discontinuity from appearing as a false
beat.

The playback queue stores stable track identities and playlist positions, not
paths. This preserves duplicate playlist entries as distinct positions. A
sequential queue ends at its boundary, repeat-one restarts only after a natural
end, and shuffle consumes every other position before beginning another
randomized cycle.

Synchronized lyrics remain outside both the audio stream and media catalog.
The media boundary resolves a bounded UTF-8 sidecar next to the current audio
file. The domain layer validates version and feature negotiation, multi-track
cue timing, language variants, timed segments, and optional media bindings
before the interface can display it. This keeps malformed or mismatched text
away from playback state and lets a sidecar move with its audio file.

## Library and Identity

The library worker traverses the filesystem as a cancellable stream. It
validates media with the playback decoder, reads embedded metadata and actual
duration, and derives exact and fuzzy audio evidence for catalog
reconciliation.

Paths, filenames, and embedded tags are observations that can change. Durable
relationships such as playlists, favorites, history, and restart state use a
stable track identity instead. This allows a file move or metadata edit to
update the current observation without silently redirecting the user's saved
relationships. [Track Identity](track-identity.md) explains the reconciliation
model.

Missing files remain as unavailable catalog records. Keeping the record makes
the missing relationship visible and allows a later scan to repair it when the
match is unambiguous.

## Rendering and Interface

The application publishes a lightweight visual snapshot containing audio
features, subject/lighting state, tuning, artwork handle, and the logical-pixel stage viewport. The Flux
paint callback reads only that snapshot, so it does not lock application state
while drawing.

The local viewport excludes the control rail from subject effects and
keeps stage coordinates independent of device scale. Table rendering follows
the same separation: Lens virtualizes visible rows and clips the header, body,
and individual cells, while Iris normalizes the platform scroll axis at the
Wayland boundary.

General-purpose graphics and interface capabilities belong to Optics. Wavora
combines them through `wavora-ui`, an internal product-level layer containing
design tokens, theme recipes, and stateless component compositions. The
application retains page orchestration, presentation state, and product
behavior. This prevents application requirements from leaking into reusable
Optics primitives while keeping Wavora's repeated visual rules consistent.

## Persistence

The media catalog stores durable relationships in SQLite with foreign-key
enforcement, transactional ordering, and write-ahead logging. Preferences use
JSON so they remain independent of the library catalog and can also migrate
legacy URI-based state.

Preference updates replace the configuration atomically through a temporary
file in the same directory. Catalog and preference storage therefore use
different mechanisms while preserving the same rule: an interrupted write
must not expose partially updated user state.

Volume and playback mode are preferences. Playlist contents and track
relationships remain catalog data.
