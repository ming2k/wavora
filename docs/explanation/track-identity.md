# Track Identity

Wavora treats the identity of a library track as separate from the place
where its audio file currently lives. This separation keeps playlists,
favorites, playback history, and restart state attached to the intended
track when a file is renamed, moved, or retagged.

For the exact matching order, tolerances, stored fields, and edge cases, use
the [Track Identity Reference](../reference/track-identity.md).

## Identity and Location

A **track identity** is the durable identity assigned to one local library
item. Wavora represents it with an immutable, random `TrackId`.

A **URI** is the current locator used to open that item's audio file. A URI
normally encodes the file's canonical path, so renaming or moving the file
changes the URI.

These values answer different questions:

```text
TrackId  Which library item is this?
URI      Where can its audio be opened now?
Index    Where is it currently displayed in a sorted list?
```

The catalog persists relationships against `TrackId`, while the playback
path opens the current URI. A changed URI is therefore evidence that the
location changed, not proof that the track identity changed.

## Rename and Metadata Behavior

During a scan, Wavora observes the current path, filesystem identity,
duration, metadata, and an exact signature of the decoded audio. The catalog
compares that observation with its existing records before accepting the new
candidate identity.

For an ordinary rename on the same filesystem, the URI changes while the
underlying filesystem identity and decoded audio remain stable. The catalog
recognizes the existing item, updates its URI, and preserves its `TrackId`.

A move across filesystems can change both the URI and filesystem identity.
The catalog can still reconnect the item when the old location is gone and
there is one unambiguous missing record with the same decoded-audio signature
and a near-equal duration.

Changing a file timestamp invalidates a signature-cache entry but does not
define a new identity. The scanner recomputes the signature, and the catalog
preserves the `TrackId` when the decoded audio still matches. Editing title,
artist, album, or artwork has the same result when the editor leaves the
decoded audio unchanged.

## Exact and Fuzzy Audio Evidence

The audio signature is conservative evidence for identity reconciliation.
It describes exact decoded audio rather than a fuzzy perception of the song.
Metadata-only edits and lossless container changes can retain the signature;
lossy re-encoding, resampling, volume changes, or inserted silence can change
it.

This exact evidence prevents a different audio stream written over an existing
path from inheriting the old track's playlists and history.

When exact matching fails, a fuzzy acoustic fingerprint answers a narrower
fallback question: whether a new observation probably contains the same
recording as one missing catalog item despite small audio differences. This
allows a re-encoded or volume-adjusted replacement to reconnect without making
similarity the primary definition of identity.

Fuzzy matching is deliberately asymmetric. It considers missing records, not
other available files, and it succeeds only for one strong, nearly full-length
match with a compatible duration. Ambiguous matches produce a new identity.
This keeps fuzzy evidence useful for repair without turning it into automatic
duplicate merging.

## Simultaneous Copies

An ordinary copy that exists alongside its source is a separate local library
item, even when both files decode to identical audio. Each item has its own
location, availability, metadata, and future lifecycle. Giving both files one
`TrackId` would leave a single catalog record with two competing current
locations.

The catalog interprets an exact or fuzzy match as a move or replacement only
when the previous location is unavailable and the match is unambiguous. It
does not use either form of evidence as a global content identifier.

If Wavora later needs to present duplicate files as one recording, that
grouping should be a separate concept layered above the per-item `TrackId`.
The files can then remain independently manageable while sharing a recording
or duplicate-group identity.

## Missing Files

A scan marks an unseen track unavailable instead of immediately deleting its
record. Playlist entries, favorites, playback history, and restart state keep
their existing `TrackId` references. The user can still see that the intended
item is missing rather than finding that a playlist silently points somewhere
else.

When the file returns at a new location, reconciliation can make the same
record available again. When several missing records are equally plausible,
the catalog avoids an automatic merge and creates a separate identity instead.

## Playback Boundary

The audio worker does not use `TrackId` to open a file. It receives the current
URI. The application uses `TrackId` around that operation to record playback,
restore the last selected track, resolve playlist entries, and preserve the
selection when a list is resorted or refreshed.

This boundary keeps a stable user-visible identity without forcing the audio
backend to understand catalog persistence.
