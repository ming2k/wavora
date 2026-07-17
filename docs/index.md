# Wavora Documentation

Wavora is a local-first music player with durable library identities,
playlists, playback history, and audio-driven visuals.

## Start

- [README](../README.md) provides build, run, and verification commands.
- [Organize Music with Playlists](how-to/playlists.md) explains playlist list
  and cover views, local artwork selection, and editing actions.

## Explanations

- [System Architecture](explanation/system-architecture.md) explains the
  boundaries among interaction, media work, persistence, and rendering.
- [Track Identity](explanation/track-identity.md) explains why a track keeps
  its identity when its file location or metadata changes.
- [Visual Design](explanation/visual-design.md) explains the visual and
  interaction principles of the immersive stage.

## Reference

- [Configuration Reference](reference/configuration.md) lists persisted
  interface preferences and their accepted values.
- [Lyrics Format](reference/lyrics-format.md) specifies synchronized
  local-lyrics sidecars and validation rules.
- [Track Identity Reference](reference/track-identity.md) lists the exact
  scanner, reconciliation, persistence, and playback behavior.
- [Visual Reference](reference/visuals.md) lists subject effects, lighting modules, tuning ranges,
  and rendering behavior.
- [Workspace Reference](reference/workspace.md) lists crate responsibilities,
  dependency direction, and runtime boundaries.
