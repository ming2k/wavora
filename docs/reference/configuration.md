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
