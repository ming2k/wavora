# Organize Music with Playlists

Open **Playlists** from the navigation rail, enter a name, and select
**Create**. The selected playlist keeps the existing track table and local
editing actions: add the playing track, remove a selected entry, reorder
entries, or delete the playlist.

Use **List** for a compact playlist selector or **Covers** for a visual
collection. Cover cards keep the playlist name centered beneath the artwork
and use a clear hover treatment without repeating local-only metadata.
The list and cover presentations are both collection-level views. Selecting a
playlist enters a separate detail level; its header shows a lightweight Unix-
style path such as `Playlists / Road trip`. Hovering the parent segment reveals
an accent underline and selecting it returns to the collection. The track table
and editing actions live only at the detail level. Creating a playlist enters
its detail directly, and deleting it returns to the collection. Playback
remains available from the detail table and the persistent player controls.

Wavora derives a playlist cover locally. It checks the playlist's first
available tracks in order and uses the first embedded front cover or supported
sidecar such as `cover.jpg` or `folder.png`. If none is present, the card uses
a theme-colored Wavora placeholder. No artwork or playlist metadata is fetched
from a network service.

The selected List/Covers presentation is saved as a local preference.
