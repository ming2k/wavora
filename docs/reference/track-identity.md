# Track Identity Reference

This page describes the current `TrackId` lifecycle and reconciliation rules.
For the design model and rationale, use
[Track Identity](../explanation/track-identity.md).

## Identity Terms

| Term | Representation | Role |
|------|----------------|------|
| `TrackId` | Random UUID v4 | Immutable identity of one catalog track record |
| URI | Percent-encoded `file://` URI | Current playback locator derived from the canonical path |
| Path | Filesystem path | Current local location stored with the catalog record |
| File identity | Optional device and inode plus size and modification time | Scanner and reconciliation evidence |
| PCM signature | 32-byte BLAKE3 digest | Exact, tag-independent audio evidence |
| Acoustic fingerprint | Versioned Chromaprint sub-fingerprint sequence | Similarity-tolerant fallback evidence |
| Playlist entry ID | SQLite integer | Identity of one occurrence of a track in a playlist |

`TrackId` is serialized as a UUID string in SQLite and as a transparent UUID
value through Serde. It is not derived from a URI, path, file hash, PCM
signature, or acoustic fingerprint. Deleting the catalog and scanning again
creates new track IDs.

## URI Construction

The scanner attempts to canonicalize a path before constructing its URI.
Canonicalization resolves relative path components and normally resolves
symbolic links. If canonicalization fails, the scanner uses the input path.

URI construction adds the `file://` prefix and percent-encodes bytes other
than ASCII letters, digits, `/`, `-`, `_`, `.`, and `~`.

```text
/home/user/Music/Hello World.flac
file:///home/user/Music/Hello%20World.flac
```

Two observations have the same URI when their generated URI strings are
equal. URI equality alone is not sufficient to preserve a `TrackId`.

## Scanner Observation

For each supported and decodable file, the scanner produces:

- A temporary, newly generated `TrackId` candidate.
- The canonical path and corresponding file URI.
- Device, inode, byte size, and modification time when available.
- Decoded duration, codec label, and embedded title, artist, and album.
- An exact PCM signature and a versioned acoustic fingerprint.

The candidate `TrackId` becomes persistent only when reconciliation finds no
existing identity. A normal rescan therefore generates temporary UUIDs without
changing the UUIDs stored in the catalog.

## Audio Evidence

### PCM Signature

The scanner hashes at most the first 90 seconds of decoded PCM with BLAKE3.
The input contains:

- The domain marker `wavora-pcm-signature-v1`.
- The decoded sample rate.
- The decoded channel count.
- The bit representation of each decoded `f32` sample.

The signature is an exact digest, not a fuzzy acoustic fingerprint. Two
signatures match only when their 32-byte values are equal.

The scanner can reuse a cached signature when byte size and modification time
match and either the path matches or the non-empty device/inode identity
matches. A changed size or modification time causes recomputation; it does not
directly cause a new `TrackId`.

### Acoustic Fingerprint

The same PCM window feeds a pure-Rust Chromaprint-compatible implementation
using the test2 preset. The stored fingerprint has Wavora algorithm version
`1` and contains raw 32-bit sub-fingerprints. Fingerprints with different
algorithm versions never match.

An acoustic match must satisfy all of these conditions:

- Each fingerprint provides at least 20 seconds of comparable material.
- One aligned segment covers at least 80% of the shorter fingerprint.
- The segment's average 32-bit Hamming-distance score is at most `6.0`.
- The catalog duration difference is within 1% of the observed duration, with
  a minimum allowance of 3,000 milliseconds and a maximum of 10,000
  milliseconds.
- Exactly one missing or path-unavailable catalog record passes every check.
- No more than 32 missing, duration-compatible candidates require comparison;
  a larger candidate set disables fuzzy reconciliation for that observation.

The scanner cache stores both forms of audio evidence. A version 1 catalog has
no acoustic fingerprints; schema migration adds nullable columns, and the next
unchanged-file scan recomputes and backfills the missing values once.

## Reconciliation Order

The catalog applies the following rules in order. An exact duration match means
an absolute difference of no more than 1,500 milliseconds. A fuzzy duration
match uses the bounded 1% tolerance defined above.

| Priority | Candidate lookup | Additional requirements | Result |
|----------|------------------|-------------------------|--------|
| 1 | Available record with the same URI | Exact PCM signature and duration match | Reuse its `TrackId` |
| 2 | Record with the same device and inode | Exact PCM signature and duration match | Reuse its `TrackId` |
| 3 | Records with an exact PCM signature and duration match | Candidate is unavailable or its stored path no longer exists; exactly one candidate remains | Reuse its `TrackId` |
| 4 | Records within fuzzy duration tolerance | Candidate is unavailable or its stored path no longer exists; acoustic thresholds pass; exactly one candidate remains | Reuse its `TrackId` |
| 5 | No rule matches | None | Insert a new record with the scanner's candidate `TrackId` |

When an available same-URI record fails its signature or duration check, the
catalog marks that record unavailable before continuing with the remaining
rules. This prevents new audio written over the same pathname from taking over
old playlist references.

After a match, the catalog updates the existing record with the newly observed
URI, path, metadata, file identity, exact signature, acoustic fingerprint, and
availability. The `TrackId`, creation time, and existing foreign-key references
remain unchanged.

## File Operation Outcomes

| Operation | Expected identity result | Notes |
|-----------|--------------------------|-------|
| Rename on the same filesystem | Preserve `TrackId` | Device/inode and PCM signature normally match |
| Move across filesystems | Preserve `TrackId` when unambiguous | Exact evidence is preferred; fuzzy evidence can repair one missing match |
| Change only modification time | Preserve `TrackId` | Signature cache misses and PCM is decoded again |
| Edit embedded tags or artwork | Preserve `TrackId` when PCM and duration still match | URI can remain unchanged |
| Losslessly rewrite a container | Preserve `TrackId` when decoded PCM and duration still match | File bytes, size, and modification time may change |
| Lossy re-encode or normalize | Preserve `TrackId` only for one strong missing fuzzy match | Exact decoded samples can change |
| Resample, trim, or insert silence | Preserve `TrackId` only when every fuzzy threshold passes | Large duration or alignment changes create a new ID |
| Replace audio at the same path with another recording | Create a new `TrackId` | Old record becomes unavailable and fuzzy evidence must not match |
| Create an ordinary copy while the source still exists | Create a new `TrackId` | Both paths are independent available items |
| Delete or stop scanning a file under the active root | Preserve record as unavailable | The record is not automatically garbage-collected |
| Restore one uniquely matching missing file | Reuse the missing record's `TrackId` | Exact evidence is preferred; guarded fuzzy evidence is the fallback |
| Restore a file matching several missing records | Create a new `TrackId` | Ambiguous candidates are not merged |

On a filesystem that reports the same device and inode for hard-linked paths,
the device/inode rule treats those paths as the same filesystem object. The
most recently reconciled path becomes the record's current locator. This is
different from an ordinary copy, which normally has a different inode.

If an ordinary copy is scanned while its source still exists, it receives a
new `TrackId`. Deleting the source later does not merge the two existing IDs.

## Scan Completion

Each active scan records the track IDs observed under its root. At scan
completion, an available record under that root becomes unavailable when its
`TrackId` was not observed. The catalog retains the row as a missing-file
tombstone.

Only available tracks populate the main library list. Playlist queries include
unavailable tracks so missing entries remain visible and repairable.

## Persistence References

| Storage | Track relationship |
|---------|--------------------|
| `tracks.id` | Text primary key containing `TrackId` |
| `tracks.acoustic_fingerprint_algorithm` | Nullable integer identifying the fingerprint profile |
| `tracks.acoustic_fingerprint` | Nullable little-endian sequence of 32-bit sub-fingerprints |
| `playlist_items.track_id` | Foreign key to `tracks.id` with deletion restricted |
| Favorites | System playlist whose entries use `playlist_items.track_id` |
| `playback_history.track_id` | Primary key and foreign key used for last-played time and play count |
| `catalog_state.last_track_id` | UUID string used to restore the last selected track |

Manual playlists may contain the same `TrackId` more than once. Each occurrence
has its own playlist entry ID, position, and addition time. Operations that
remove or reorder one occurrence use the playlist entry ID rather than
`TrackId`.

## Application and Playback Use

| Operation | Use of `TrackId` | Use of URI |
|-----------|------------------|------------|
| Load audio | Not sent to the audio worker | Sent to the audio worker as the source locator |
| Record a play | Updates history and `last_track_id` | Not used as the history key |
| Restore startup selection | Matches `last_track_id` against available tracks | Not used for the match |
| Preserve selection after sorting or scanning | Resolves the previous `TrackId` to its new list index | Updated when reconciliation finds a moved file |
| Play a playlist row | Resolves the row's `TrackId` to an available library item | Loaded after resolution |

Starting a playlist snapshots its available entries into the playback queue.
Each duplicate playlist occurrence remains a separate queue position. Queue
transitions continue to use `TrackId`; the current URI is resolved only when
an entry is loaded.

`TrackId` does not identify an audio decoder instance, playback session, queue
position, or globally recognized recording. Acoustic fingerprints repair one
missing identity; they do not group ordinary available duplicate files under a
shared recording identity.
