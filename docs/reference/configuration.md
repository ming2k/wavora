# Configuration Reference

Wavora stores preferences in the platform configuration directory as JSON.
The settings interface displays the exact path for the current system. The
file is replaced atomically and uses private user-only permissions on Unix.

## Playlist presentation

`playlist_display` controls the selector shown on the Playlists page.

| Value | Behavior |
|-------|----------|
| `list` | Compact dropdown selector and full track table |
| `covers` | Local-artwork card selector and full track table |

The default is `list`. Existing configurations that predate this field receive
that default during normalization. Changing the selector in the interface
updates the preference automatically.

## Visual stage

The stage persists two independent modules. `subject` selects and tunes the
attention-carrying effect; `ambient` owns its procedural material and light
sources. Either `enabled` value can change without resetting the other module.

```json
{
  "visual_stage": {
    "subject": {
      "enabled": true,
      "effect": 0,
      "tuning": {
        "intensity": 1.0,
        "motion": 1.0,
        "depth": 1.0,
        "glow": 0.9
      }
    },
    "ambient": {
      "enabled": true,
      "field": {
        "kind": "none"
      },
      "sources": []
    }
  }
}
```

See the [Visual Reference](visuals.md) for complete ranges, material kinds,
source properties, and migration from pre-version-9 visual keys.
