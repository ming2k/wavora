# Wavora Lyrics Format 1.x

The **Wavora Lyrics Format** is Wavora's canonical JSON sidecar model for
synchronized local lyrics. It represents multiple simultaneous lyric tracks,
multiple language or reading variants, and optional timed text segments. Its
filename suffix is `.wlyric.json`.

The format is designed as an application-native model. Importers and exporters
should map LRC, WebVTT, TTML, or another interchange format into this model
instead of weakening the model to the limitations of one source format.

## File discovery

For `Signal.flac`, Wavora checks these files in order:

1. `Signal.flac.wlyric.json`
2. `Signal.wlyric.json`

The full-filename form is canonical because two audio files can share a stem.
The first existing sidecar wins. If it is invalid, Wavora reports the error
instead of silently selecting a different candidate.

Files must use UTF-8 without a byte-order mark and must not exceed 4 MiB.

## Complete example

The repository provides a
[complete multi-track example](../../examples/lyrics/Signal.flac.wlyric.json)
and a machine-readable [JSON Schema](wavora-lyrics.schema.json). The example
contains overlapping lead and background tracks, a translation, a
transliteration, and timed segments.

A minimal document is:

```json
{
  "format": "wavora-lyrics",
  "version": "1.0",
  "tracks": [
    { "id": "main", "role": "main", "language": "en" }
  ],
  "cues": [
    {
      "id": "cue-001",
      "track_id": "main",
      "start_ms": 1200,
      "texts": [
        { "kind": "original", "text": "First line" }
      ]
    }
  ]
}
```

## Versioning and feature negotiation

`version` uses `<major>.<minor>` syntax. A version 1 reader accepts any `1.x`
document when all entries in `required_features` are supported. Readers reject
an unknown major version or unknown required feature.

Readers ignore unknown fields. Writers must preserve their own extension data
when editing a document, and extensions should use a namespaced field such as
`org.example:confidence`. An extension that changes how known data must be
interpreted must also add its identifier to `required_features`.

Wavora 1.x recognizes these feature identifiers:

- `media-binding`
- `multi-track`
- `text-variants`
- `timed-segments`

Features defined by the base schema do not need to be listed. The list exists
to make semantic dependencies explicit for generated or extended documents.

## Top-level fields

| Field | Type | Required | Meaning |
|------|------|----------|---------|
| `$schema` | URI string | No | Location of the JSON Schema. |
| `format` | string | Yes | Exact value `wavora-lyrics`. |
| `version` | string | Yes | Compatible version such as `1.0`. |
| `required_features` | string array | No | Features a reader must understand. |
| `offset_ms` | integer | No | Global display offset; defaults to `0`. |
| `media` | object | No | Duration and audio fingerprint bindings. |
| `metadata` | object | No | Descriptive, source, contributor, and rights data. |
| `tracks` | array | Yes | Between 1 and 64 logical lyric tracks. |
| `cues` | array | Yes | Between 1 and 20,000 timed cues. |

A positive `offset_ms` displays lyrics later. A negative value displays them
earlier. Adjusted timestamps below zero clamp to zero.

All JSON integers must remain in the interoperable safe range from
`-9007199254740991` to `9007199254740991`. Timestamp fields are nonnegative.

## Media binding

`media.duration_ms` identifies the audio edit for which the timing was made.
Wavora rejects a sidecar when its declared duration differs from the decoded
track by more than 1,500 milliseconds.

`media.fingerprints` contains unique algorithm/value pairs. Wavora currently
verifies `wavora-pcm-signature-v1`, whose value is a 64-character hexadecimal
digest of tag-independent decoded PCM. A declared supported fingerprint must
match the selected audio file.

Media binding is optional for handwritten files and recommended for generated
or distributed files. A local catalog UUID must not be used as a portable
media identifier.

## Metadata

| Field | Type | Meaning |
|------|------|---------|
| `title` | string | Track title represented by the sidecar. |
| `artist` | string | Primary credited artist. |
| `album` | string | Release or collection title. |
| `contributors` | array | Objects containing `name` and a machine-readable `role`. |
| `source` | object | At least one of `name` or absolute `uri`. |
| `rights` | object | At least one of `copyright`, `license`, or `license_uri`. |

Common contributor roles include `transcriber`, `timer`, `translator`, and
`editor`. License identifiers should use SPDX identifiers when one applies.
Metadata does not override tags displayed from the audio file.

## Tracks

A **track** is an independent stream of nonoverlapping cues. Separate tracks
represent lead vocals, duet parts, background vocals, or other simultaneous
content.

| Field | Type | Required | Constraint |
|------|------|----------|------------|
| `id` | identifier | Yes | Unique within the document. |
| `role` | identifier | Yes | For example `main`, `duet`, or `background`. |
| `label` | string | No | Human-readable speaker or part name. |
| `language` | BCP 47 tag | No | Default language for original text. |
| `direction` | enum | No | `auto`, `ltr`, or `rtl`. |

Track and cue identifiers contain 1 to 64 ASCII letters, digits, `.`, `:`,
`_`, or `-`. IDs are stable within an edited document and should not be
reassigned merely because timing or text changes.

## Cues and overlap

Each cue contains:

| Field | Type | Required | Constraint |
|------|------|----------|------------|
| `id` | identifier | Yes | Unique stable cue identity. |
| `track_id` | identifier | Yes | References an existing track. |
| `start_ms` | integer | Yes | Absolute decoded-audio timestamp. |
| `end_ms` | integer | No | Greater than `start_ms`. |
| `texts` | array | Yes | Between 1 and 16 text variants. |

The cue array is ordered by nondecreasing `start_ms`. Cues from different
tracks may start together or overlap. Cues on the same track must start
strictly later and must not overlap.

When `end_ms` is absent, the next cue on the same track is its implicit end.
The final cue on a track remains active until playback ends. Use an explicit
end to create a lyric-free gap.

At a playback position, Wavora selects at most one active cue from each track
and presents all active tracks together in stable document order.

## Text variants

Each cue has exactly one `original` text and may have additional variants:

| Field | Type | Required | Constraint |
|------|------|----------|------------|
| `kind` | identifier | Yes | Common values: `original`, `translation`, `transliteration`. |
| `language` | BCP 47 tag | Conditional | May inherit from the track only for `original`. |
| `direction` | enum | No | Overrides the track direction. |
| `text` | string | Yes | Nonempty, single-line Unicode text. |
| `segments` | array | No | Up to 2,000 timed text segments. |

The combination of `kind` and `language` must be unique within a cue.
Translations and transliterations always declare their language. `und` is the
appropriate tag when the language is genuinely undetermined.

## Timed segments

Segments replace a word-specific model. A segment may represent a word,
character, syllable, punctuation mark, whitespace, or phrase, which keeps the
format usable across writing systems.

Each segment contains absolute `start_ms`, `end_ms`, and `text`. Segments:

- start at or after their cue;
- end after they start and no later than an explicit cue end;
- appear in nonoverlapping chronological order;
- may contain whitespace-only text but never an empty string or control
  character.

Segment text should reconstruct its parent text when concatenated. A reader
does not require exact reconstruction because imported word-timing formats
often omit untimed whitespace and punctuation.

## Language tags and text direction

Language values use well-formed BCP 47 syntax, including script, region,
extension, private-use, and grandfathered forms. The validator checks syntax,
not whether a language or extension is registered.

Use a script subtag when it materially affects rendering or meaning, such as
`zh-Hans`, `zh-Hant`, or `ja-Latn`. Use `direction: rtl` for right-to-left text
when automatic direction detection is insufficient.

## Validation and security constraints

Wavora validates constraints that JSON Schema cannot express:

- unique track and cue IDs;
- supported required features;
- cue references to existing tracks;
- global cue ordering and per-track nonoverlap;
- unique text kind/language combinations;
- exactly one original text per cue;
- segment ordering and cue containment;
- duration and supported fingerprint bindings.

Text rejects empty values and Unicode control characters. The application
treats strings as text only; it does not interpret markup, HTML, or embedded
commands.

## Legacy draft compatibility

Wavora still reads the earlier line-based draft with numeric `version: 1`,
`lines`, `translation`, and `words`. The loader upgrades it in memory to one
`main` track, text variants, and timed segments. New files must use the 1.x
model documented here.

## Authoring guidance

- Derive timestamps from the decoded audio, not a different edit of the
  recording.
- Prefer the canonical full-filename sidecar and move it with the audio file.
- Create separate tracks for simultaneously active vocal parts.
- Preserve original punctuation and Unicode text without applying lossy
  normalization.
- Provide media binding, source, contributors, and rights information for
  generated or distributed files.
- Distribute lyric text only when its license or applicable law permits it.
