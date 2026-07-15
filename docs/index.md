# Wavora Documentation

Wavora is a local-first music player with durable library identities,
playlists, playback history, and audio-driven visuals.

## Start

- [README](../README.md) provides build, run, and verification commands.

## Explanations

- [System Architecture](explanation/system-architecture.md) explains the
  boundaries among interaction, media work, persistence, and rendering.
- [Track Identity](explanation/track-identity.md) explains why a track keeps
  its identity when its file location or metadata changes.
- [Visual Design](explanation/visual-design.md) explains the visual and
  interaction principles of the immersive stage.

## Reference

- [Lyrics Format](reference/lyrics-format.md) specifies synchronized
  local-lyrics sidecars and validation rules.
- [Track Identity Reference](reference/track-identity.md) lists the exact
  scanner, reconciliation, persistence, and playback behavior.
- [Visual Reference](reference/visuals.md) lists compositions, tuning ranges,
  and rendering behavior.
- [Workspace Reference](reference/workspace.md) lists crate responsibilities,
  dependency direction, and runtime boundaries.
