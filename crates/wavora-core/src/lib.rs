//! Domain types shared by Wavora's application and media layers.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const WAVORA_LYRICS_FORMAT: &str = "wavora-lyrics";
pub const WAVORA_LYRICS_VERSION: &str = "1.0";
pub const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
pub const MAX_LYRIC_TRACKS: usize = 64;
pub const MAX_LYRIC_CUES: usize = 20_000;
pub const MAX_LYRIC_TEXTS_PER_CUE: usize = 16;
pub const MAX_LYRIC_SEGMENTS_PER_TEXT: usize = 2_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct TrackId(Uuid);

impl TrackId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TrackId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for TrackId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct PlaylistId(Uuid);

impl PlaylistId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlaylistId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PlaylistId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for PlaylistId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub system_kind: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub id: TrackId,
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub codec: String,
    pub favorite: bool,
    pub available: bool,
}

impl Track {
    #[must_use]
    pub fn from_path(id: TrackId, uri: impl Into<String>, path: &Path) -> Self {
        let uri = uri.into();
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Unknown track");
        let (artist, title) = stem
            .split_once(" - ")
            .map_or(("Unknown artist", stem), |(artist, title)| (artist, title));
        Self {
            id,
            uri,
            title: title.trim().to_owned(),
            artist: artist.trim().to_owned(),
            album: "Local music".to_owned(),
            duration_ms: 0,
            codec: "Audio".to_owned(),
            favorite: false,
            available: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Buffering,
    Paused,
    Playing,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlaybackMode {
    #[default]
    Sequential,
    RepeatOne,
    Shuffle,
}

impl PlaybackMode {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Sequential => Self::RepeatOne,
            Self::RepeatOne => Self::Shuffle,
            Self::Shuffle => Self::Sequential,
        }
    }
}

/// An ordered playback context with repeat-one and shuffle-cycle semantics.
///
/// Shuffle visits every other queue position before starting another cycle.
/// Its history is position-based, so duplicate tracks in a playlist remain
/// distinct entries and Previous follows the order the listener heard.
#[derive(Debug, Clone)]
pub struct PlaybackQueue {
    entries: Vec<TrackId>,
    cursor: Option<usize>,
    shuffle_remaining: Vec<usize>,
    shuffle_history: Vec<usize>,
    random_state: u64,
}

impl PlaybackQueue {
    #[must_use]
    pub fn new(entries: Vec<TrackId>, cursor: Option<usize>) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x9e37_79b9_7f4a_7c15, |duration| {
                u64::try_from(duration.as_nanos()).unwrap_or(duration.as_secs())
            });
        Self::with_seed(entries, cursor, seed)
    }

    #[must_use]
    pub fn with_seed(entries: Vec<TrackId>, cursor: Option<usize>, seed: u64) -> Self {
        let mut queue = Self {
            entries,
            cursor: None,
            shuffle_remaining: Vec::new(),
            shuffle_history: Vec::new(),
            random_state: seed.max(1),
        };
        queue.select(cursor.unwrap_or_default());
        queue
    }

    #[must_use]
    pub fn entries(&self) -> &[TrackId] {
        &self.entries
    }

    #[must_use]
    pub fn current_position(&self) -> Option<usize> {
        self.cursor
    }

    #[must_use]
    pub fn current(&self) -> Option<TrackId> {
        self.cursor
            .and_then(|index| self.entries.get(index).copied())
    }

    pub fn replace(&mut self, entries: Vec<TrackId>, preferred: Option<TrackId>) {
        let cursor = preferred.and_then(|id| entries.iter().position(|candidate| *candidate == id));
        self.entries = entries;
        self.select(cursor.unwrap_or_default());
    }

    pub fn select(&mut self, position: usize) -> Option<TrackId> {
        self.cursor = (position < self.entries.len()).then_some(position);
        self.reset_shuffle_cycle();
        self.current()
    }

    pub fn restart_shuffle_cycle(&mut self) {
        self.reset_shuffle_cycle();
    }

    /// Advances after a natural end of stream.
    pub fn on_end(&mut self, mode: PlaybackMode) -> Option<TrackId> {
        if mode == PlaybackMode::RepeatOne {
            self.current()
        } else {
            self.next(mode)
        }
    }

    /// Advances after an explicit Next action.
    pub fn next(&mut self, mode: PlaybackMode) -> Option<TrackId> {
        if self.entries.is_empty() {
            self.cursor = None;
            return None;
        }
        if mode == PlaybackMode::Shuffle {
            return self.next_shuffled();
        }
        let next = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        if next >= self.entries.len() {
            return None;
        }
        self.cursor = Some(next);
        self.current()
    }

    pub fn previous(&mut self, mode: PlaybackMode) -> Option<TrackId> {
        if mode == PlaybackMode::Shuffle {
            let previous = self.shuffle_history.pop()?;
            if let Some(current) = self.cursor
                && !self.shuffle_remaining.contains(&current)
            {
                self.shuffle_remaining.push(current);
            }
            self.shuffle_remaining
                .retain(|position| *position != previous);
            self.cursor = Some(previous);
            return self.current();
        }
        let previous = self.cursor?.checked_sub(1)?;
        self.cursor = Some(previous);
        self.current()
    }

    /// Returns the current entry followed by the known upcoming cycle.
    #[must_use]
    pub fn upcoming(&self, mode: PlaybackMode, limit: usize) -> Vec<(usize, TrackId)> {
        let Some(cursor) = self.cursor else {
            return Vec::new();
        };
        let positions: Vec<usize> = match mode {
            PlaybackMode::Sequential => (cursor..self.entries.len()).collect(),
            PlaybackMode::RepeatOne => vec![cursor],
            PlaybackMode::Shuffle => std::iter::once(cursor)
                .chain(self.shuffle_remaining.iter().rev().copied())
                .collect(),
        };
        positions
            .into_iter()
            .take(limit)
            .filter_map(|position| self.entries.get(position).copied().map(|id| (position, id)))
            .collect()
    }

    fn next_shuffled(&mut self) -> Option<TrackId> {
        if self.entries.len() == 1 {
            self.cursor = Some(0);
            return self.current();
        }
        if self.shuffle_remaining.is_empty() {
            self.fill_shuffle_remaining();
        }
        if let Some(current) = self.cursor {
            self.shuffle_remaining
                .retain(|position| *position != current);
        }
        let next = self.shuffle_remaining.pop()?;
        if let Some(current) = self.cursor {
            self.shuffle_history.push(current);
        }
        self.cursor = Some(next);
        self.current()
    }

    fn reset_shuffle_cycle(&mut self) {
        self.shuffle_history.clear();
        self.fill_shuffle_remaining();
    }

    fn fill_shuffle_remaining(&mut self) {
        self.shuffle_remaining.clear();
        self.shuffle_remaining
            .extend((0..self.entries.len()).filter(|position| Some(*position) != self.cursor));
        for index in (1..self.shuffle_remaining.len()).rev() {
            let swap_with = usize::try_from(self.next_random()).unwrap_or_default() % (index + 1);
            self.shuffle_remaining.swap(index, swap_with);
        }
    }

    fn next_random(&mut self) -> u64 {
        let mut value = self.random_state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.random_state = value;
        value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsDocument {
    #[serde(
        rename = "$schema",
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub schema: Option<String>,
    pub format: String,
    pub version: String,
    #[serde(default)]
    pub required_features: Vec<String>,
    #[serde(default)]
    pub offset_ms: i64,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub media: Option<LyricsMedia>,
    #[serde(default)]
    pub metadata: LyricsMetadata,
    pub tracks: Vec<LyricTrack>,
    pub cues: Vec<LyricCue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LyricsMetadata {
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub artist: Option<String>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub album: Option<String>,
    pub contributors: Vec<LyricsContributor>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub source: Option<LyricsSource>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub rights: Option<LyricsRights>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsContributor {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LyricsSource {
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LyricsRights {
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub copyright: Option<String>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub license: Option<String>,
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub license_uri: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LyricsMedia {
    #[serde(
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub duration_ms: Option<u64>,
    pub fingerprints: Vec<LyricsFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsFingerprint {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricTrack {
    pub id: String,
    pub role: String,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub language: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricCue {
    pub id: String,
    pub track_id: String,
    pub start_ms: u64,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_ms: Option<u64>,
    pub texts: Vec<LyricText>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricText {
    pub kind: String,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub language: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub direction: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<LyricSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

impl LyricCue {
    #[must_use]
    pub fn original_text(&self) -> Option<&LyricText> {
        self.texts.iter().find(|text| text.kind == "original")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsValidationError(pub String);

impl std::fmt::Display for LyricsValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LyricsValidationError {}

impl LyricsDocument {
    /// Validates the v1 format, identifiers, feature negotiation, and timelines.
    ///
    /// # Errors
    ///
    /// Returns a field-specific error when the document cannot be interpreted
    /// deterministically by this reader.
    pub fn validate(&self) -> Result<(), LyricsValidationError> {
        if self.format != WAVORA_LYRICS_FORMAT {
            return Err(lyrics_error(format!(
                "format must be {WAVORA_LYRICS_FORMAT:?}"
            )));
        }
        validate_version(&self.version)?;
        validate_required_features(&self.required_features)?;
        if let Some(schema) = &self.schema {
            validate_absolute_uri(schema, "$schema")?;
        }
        if self.offset_ms.unsigned_abs() > JSON_SAFE_INTEGER_MAX {
            return Err(lyrics_error(
                "offset_ms exceeds the JSON safe-integer range",
            ));
        }
        validate_metadata(&self.metadata)?;
        if let Some(media) = &self.media {
            validate_media(media)?;
        }
        let tracks = validate_tracks(&self.tracks)?;
        validate_cues(self, &tracks)
    }

    /// Verifies optional duration and exact-fingerprint bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when the sidecar declares a supported binding that
    /// does not match the selected audio file.
    pub fn validate_media_binding(
        &self,
        actual_duration_ms: u64,
        actual_pcm_signature: Option<&str>,
    ) -> Result<(), LyricsValidationError> {
        let Some(media) = &self.media else {
            return Ok(());
        };
        if let Some(expected) = media.duration_ms
            && actual_duration_ms > 0
            && expected.abs_diff(actual_duration_ms) > 1_500
        {
            return Err(lyrics_error(format!(
                "media.duration_ms differs from the audio by more than 1500 ms ({expected} vs {actual_duration_ms})"
            )));
        }
        if let Some(expected) = media
            .fingerprints
            .iter()
            .find(|fingerprint| fingerprint.algorithm == "wavora-pcm-signature-v1")
        {
            let Some(actual) = actual_pcm_signature else {
                return Err(lyrics_error(
                    "the selected audio has no verifiable PCM signature",
                ));
            };
            if !expected.value.eq_ignore_ascii_case(actual) {
                return Err(lyrics_error(
                    "media fingerprint does not match the selected audio",
                ));
            }
        }
        Ok(())
    }

    /// Returns one active cue per track in stable document order.
    #[must_use]
    pub fn active_cue_indices(&self, position_ms: u64) -> Vec<usize> {
        let cutoff = self.cues.partition_point(|cue| {
            cue.start_ms.saturating_add_signed(self.offset_ms) <= position_ms
        });
        let mut seen_tracks = HashSet::new();
        let mut active = Vec::new();
        for index in (0..cutoff).rev() {
            let cue = &self.cues[index];
            if seen_tracks.insert(cue.track_id.as_str())
                && cue
                    .end_ms
                    .map(|end| end.saturating_add_signed(self.offset_ms))
                    .is_none_or(|end| position_ms < end)
            {
                active.push(index);
            }
        }
        active.sort_unstable();
        active
    }
}

fn validate_version(version: &str) -> Result<(), LyricsValidationError> {
    let Some((major, minor)) = version.split_once('.') else {
        return Err(lyrics_error("version must use the <major>.<minor> form"));
    };
    if major != "1" || minor.is_empty() || !minor.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(lyrics_error(format!(
            "unsupported lyrics version {version:?}"
        )));
    }
    Ok(())
}

fn validate_required_features(features: &[String]) -> Result<(), LyricsValidationError> {
    const SUPPORTED: &[&str] = &[
        "media-binding",
        "multi-track",
        "text-variants",
        "timed-segments",
    ];
    let mut seen = HashSet::new();
    for (index, feature) in features.iter().enumerate() {
        validate_identifier(feature, &format!("required_features[{index}]"))?;
        if !seen.insert(feature.as_str()) {
            return Err(lyrics_error(format!(
                "required_features contains duplicate {feature:?}"
            )));
        }
        if !SUPPORTED.contains(&feature.as_str()) {
            return Err(lyrics_error(format!(
                "required feature {feature:?} is not supported"
            )));
        }
    }
    Ok(())
}

fn validate_metadata(metadata: &LyricsMetadata) -> Result<(), LyricsValidationError> {
    for (name, value) in [
        ("title", &metadata.title),
        ("artist", &metadata.artist),
        ("album", &metadata.album),
    ] {
        if let Some(value) = value {
            validate_text(value, &format!("metadata.{name}"), false)?;
        }
    }
    for (index, contributor) in metadata.contributors.iter().enumerate() {
        validate_text(
            &contributor.name,
            &format!("metadata.contributors[{index}].name"),
            false,
        )?;
        validate_identifier(
            &contributor.role,
            &format!("metadata.contributors[{index}].role"),
        )?;
    }
    if let Some(source) = &metadata.source {
        if source.name.is_none() && source.uri.is_none() {
            return Err(lyrics_error("metadata.source must contain name or uri"));
        }
        validate_optional_text(source.name.as_deref(), "metadata.source.name")?;
        if let Some(uri) = &source.uri {
            validate_absolute_uri(uri, "metadata.source.uri")?;
        }
    }
    if let Some(rights) = &metadata.rights {
        if rights.copyright.is_none() && rights.license.is_none() && rights.license_uri.is_none() {
            return Err(lyrics_error(
                "metadata.rights must contain copyright, license, or license_uri",
            ));
        }
        validate_optional_text(rights.copyright.as_deref(), "metadata.rights.copyright")?;
        validate_optional_text(rights.license.as_deref(), "metadata.rights.license")?;
        if let Some(uri) = &rights.license_uri {
            validate_absolute_uri(uri, "metadata.rights.license_uri")?;
        }
    }
    Ok(())
}

fn validate_media(media: &LyricsMedia) -> Result<(), LyricsValidationError> {
    if media
        .duration_ms
        .is_some_and(|duration| duration > JSON_SAFE_INTEGER_MAX)
    {
        return Err(lyrics_error(
            "media.duration_ms exceeds the JSON safe-integer range",
        ));
    }
    if media.fingerprints.len() > 16 {
        return Err(lyrics_error(
            "media.fingerprints must not exceed 16 entries",
        ));
    }
    let mut algorithms = HashSet::new();
    for (index, fingerprint) in media.fingerprints.iter().enumerate() {
        validate_identifier(
            &fingerprint.algorithm,
            &format!("media.fingerprints[{index}].algorithm"),
        )?;
        validate_text(
            &fingerprint.value,
            &format!("media.fingerprints[{index}].value"),
            false,
        )?;
        if !algorithms.insert(fingerprint.algorithm.as_str()) {
            return Err(lyrics_error(format!(
                "media.fingerprints contains duplicate algorithm {:?}",
                fingerprint.algorithm
            )));
        }
        if fingerprint.algorithm == "wavora-pcm-signature-v1"
            && (fingerprint.value.len() != 64
                || !fingerprint
                    .value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(lyrics_error(
                "wavora-pcm-signature-v1 must be exactly 64 hexadecimal characters",
            ));
        }
    }
    Ok(())
}

fn validate_tracks(
    tracks: &[LyricTrack],
) -> Result<HashMap<&str, &LyricTrack>, LyricsValidationError> {
    if tracks.is_empty() || tracks.len() > MAX_LYRIC_TRACKS {
        return Err(lyrics_error(format!(
            "tracks must contain between 1 and {MAX_LYRIC_TRACKS} entries"
        )));
    }
    let mut result = HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        validate_identifier(&track.id, &format!("tracks[{index}].id"))?;
        validate_identifier(&track.role, &format!("tracks[{index}].role"))?;
        validate_optional_text(track.label.as_deref(), &format!("tracks[{index}].label"))?;
        validate_optional_language(
            track.language.as_deref(),
            &format!("tracks[{index}].language"),
        )?;
        validate_optional_direction(
            track.direction.as_deref(),
            &format!("tracks[{index}].direction"),
        )?;
        if result.insert(track.id.as_str(), track).is_some() {
            return Err(lyrics_error(format!(
                "tracks contains duplicate id {:?}",
                track.id
            )));
        }
    }
    Ok(result)
}

fn validate_cues(
    document: &LyricsDocument,
    tracks: &HashMap<&str, &LyricTrack>,
) -> Result<(), LyricsValidationError> {
    if document.cues.is_empty() || document.cues.len() > MAX_LYRIC_CUES {
        return Err(lyrics_error(format!(
            "cues must contain between 1 and {MAX_LYRIC_CUES} entries"
        )));
    }
    let mut cue_ids = HashSet::new();
    let mut previous_by_track: HashMap<&str, (usize, u64, Option<u64>)> = HashMap::new();
    let mut previous_start = None;
    for (index, cue) in document.cues.iter().enumerate() {
        validate_identifier(&cue.id, &format!("cues[{index}].id"))?;
        if !cue_ids.insert(cue.id.as_str()) {
            return Err(lyrics_error(format!(
                "cues contains duplicate id {:?}",
                cue.id
            )));
        }
        let Some(track) = tracks.get(cue.track_id.as_str()) else {
            return Err(lyrics_error(format!(
                "cues[{index}].track_id references unknown track {:?}",
                cue.track_id
            )));
        };
        validate_timestamp(cue.start_ms, &format!("cues[{index}].start_ms"))?;
        if let Some(end) = cue.end_ms {
            validate_timestamp(end, &format!("cues[{index}].end_ms"))?;
            if end <= cue.start_ms {
                return Err(lyrics_error(format!(
                    "cues[{index}].end_ms must be greater than start_ms"
                )));
            }
        }
        if previous_start.is_some_and(|previous| previous > cue.start_ms) {
            return Err(lyrics_error(format!(
                "cues[{index}].start_ms must not be earlier than the previous cue"
            )));
        }
        previous_start = Some(cue.start_ms);
        if let Some((previous_index, previous_track_start, previous_end)) =
            previous_by_track.insert(cue.track_id.as_str(), (index, cue.start_ms, cue.end_ms))
        {
            if previous_track_start >= cue.start_ms {
                return Err(lyrics_error(format!(
                    "cues[{index}] must start after the previous cue on track {:?}",
                    cue.track_id
                )));
            }
            if previous_end.is_some_and(|end| end > cue.start_ms) {
                return Err(lyrics_error(format!(
                    "cues[{previous_index}] overlaps cues[{index}] on track {:?}",
                    cue.track_id
                )));
            }
            if document.cues[previous_index]
                .texts
                .iter()
                .flat_map(|text| &text.segments)
                .any(|segment| segment.end_ms > cue.start_ms)
            {
                return Err(lyrics_error(format!(
                    "a segment in cues[{previous_index}] extends past its implicit end"
                )));
            }
        }
        if let Some(duration) = document.media.as_ref().and_then(|media| media.duration_ms)
            && (cue.start_ms > duration || cue.end_ms.is_some_and(|end| end > duration))
        {
            return Err(lyrics_error(format!(
                "cues[{index}] extends past media.duration_ms"
            )));
        }
        validate_cue_texts(
            cue,
            track,
            index,
            document.media.as_ref().and_then(|media| media.duration_ms),
        )?;
    }
    Ok(())
}

fn validate_cue_texts(
    cue: &LyricCue,
    track: &LyricTrack,
    cue_index: usize,
    media_duration: Option<u64>,
) -> Result<(), LyricsValidationError> {
    if cue.texts.is_empty() || cue.texts.len() > MAX_LYRIC_TEXTS_PER_CUE {
        return Err(lyrics_error(format!(
            "cues[{cue_index}].texts must contain between 1 and {MAX_LYRIC_TEXTS_PER_CUE} entries"
        )));
    }
    let mut variants = HashSet::new();
    let mut original_count = 0;
    for (text_index, text) in cue.texts.iter().enumerate() {
        let field = format!("cues[{cue_index}].texts[{text_index}]");
        validate_identifier(&text.kind, &format!("{field}.kind"))?;
        validate_optional_language(text.language.as_deref(), &format!("{field}.language"))?;
        validate_optional_direction(text.direction.as_deref(), &format!("{field}.direction"))?;
        validate_text(&text.text, &format!("{field}.text"), false)?;
        if text.kind == "original" {
            original_count += 1;
            if text.language.is_none() && track.language.is_none() {
                return Err(lyrics_error(format!(
                    "{field} needs language because its track has no language"
                )));
            }
        } else if text.language.is_none() {
            return Err(lyrics_error(format!(
                "{field}.language is required for non-original text"
            )));
        }
        let variant = (text.kind.as_str(), text.language.as_deref());
        if !variants.insert(variant) {
            return Err(lyrics_error(format!(
                "{field} duplicates a kind/language variant"
            )));
        }
        validate_segments(text, cue, media_duration, &field)?;
    }
    if original_count != 1 {
        return Err(lyrics_error(format!(
            "cues[{cue_index}] must contain exactly one original text"
        )));
    }
    Ok(())
}

fn validate_segments(
    text: &LyricText,
    cue: &LyricCue,
    media_duration: Option<u64>,
    field: &str,
) -> Result<(), LyricsValidationError> {
    if text.segments.len() > MAX_LYRIC_SEGMENTS_PER_TEXT {
        return Err(lyrics_error(format!(
            "{field}.segments exceeds {MAX_LYRIC_SEGMENTS_PER_TEXT} entries"
        )));
    }
    for (index, segment) in text.segments.iter().enumerate() {
        validate_text(
            &segment.text,
            &format!("{field}.segments[{index}].text"),
            true,
        )?;
        validate_timestamp(
            segment.start_ms,
            &format!("{field}.segments[{index}].start_ms"),
        )?;
        validate_timestamp(segment.end_ms, &format!("{field}.segments[{index}].end_ms"))?;
        if segment.start_ms < cue.start_ms || segment.end_ms <= segment.start_ms {
            return Err(lyrics_error(format!(
                "{field}.segments[{index}] has invalid timing"
            )));
        }
        if cue
            .end_ms
            .or(media_duration)
            .is_some_and(|end| segment.end_ms > end)
        {
            return Err(lyrics_error(format!(
                "{field}.segments[{index}] extends past its cue"
            )));
        }
        if index > 0 && text.segments[index - 1].end_ms > segment.start_ms {
            return Err(lyrics_error(format!(
                "{field}.segments[{index}] overlaps the previous segment"
            )));
        }
    }
    Ok(())
}

fn validate_timestamp(value: u64, field: &str) -> Result<(), LyricsValidationError> {
    if value > JSON_SAFE_INTEGER_MAX {
        Err(lyrics_error(format!(
            "{field} exceeds the JSON safe-integer range"
        )))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, field: &str) -> Result<(), LyricsValidationError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(lyrics_error(format!(
            "{field} must be 1-64 ASCII letters, digits, '.', ':', '_' or '-'"
        )));
    }
    Ok(())
}

fn validate_absolute_uri(value: &str, field: &str) -> Result<(), LyricsValidationError> {
    let Some((scheme, remainder)) = value.split_once(':') else {
        return Err(lyrics_error(format!("{field} must be an absolute URI")));
    };
    if scheme.is_empty()
        || remainder.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        return Err(lyrics_error(format!("{field} must be an absolute URI")));
    }
    Ok(())
}

fn validate_optional_language(
    value: Option<&str>,
    field: &str,
) -> Result<(), LyricsValidationError> {
    if value.is_some_and(|tag| !is_language_tag(tag)) {
        Err(lyrics_error(format!(
            "{field} must be a well-formed BCP 47 language tag"
        )))
    } else {
        Ok(())
    }
}

fn validate_optional_direction(
    value: Option<&str>,
    field: &str,
) -> Result<(), LyricsValidationError> {
    if value.is_some_and(|direction| !matches!(direction, "auto" | "ltr" | "rtl")) {
        Err(lyrics_error(format!(
            "{field} must be 'auto', 'ltr', or 'rtl'"
        )))
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn is_language_tag(value: &str) -> bool {
    const GRANDFATHERED: &[&str] = &[
        "art-lojban",
        "cel-gaulish",
        "en-gb-oed",
        "i-ami",
        "i-bnn",
        "i-default",
        "i-enochian",
        "i-hak",
        "i-klingon",
        "i-lux",
        "i-mingo",
        "i-navajo",
        "i-pwn",
        "i-tao",
        "i-tay",
        "i-tsu",
        "no-bok",
        "no-nyn",
        "sgn-be-fr",
        "sgn-be-nl",
        "sgn-ch-de",
        "zh-guoyu",
        "zh-hakka",
        "zh-min",
        "zh-min-nan",
        "zh-xiang",
    ];
    if value.len() > 255 {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if GRANDFATHERED.contains(&lower.as_str()) {
        return true;
    }
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("x"))
    {
        return parts.len() > 1
            && parts[1..].iter().all(|part| {
                (1..=8).contains(&part.len())
                    && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            });
    }
    let Some(language) = parts.first() else {
        return false;
    };
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return false;
    }
    let mut index = 1;
    if language.len() <= 3 {
        let mut extlangs = 0;
        while extlangs < 3
            && parts.get(index).is_some_and(|part| {
                part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_alphabetic())
            })
        {
            index += 1;
            extlangs += 1;
        }
    }
    if parts
        .get(index)
        .is_some_and(|part| part.len() == 4 && part.bytes().all(|byte| byte.is_ascii_alphabetic()))
    {
        index += 1;
    }
    if parts.get(index).is_some_and(|part| {
        (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_alphabetic()))
            || (part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_digit()))
    }) {
        index += 1;
    }
    let mut variants = HashSet::new();
    while parts.get(index).is_some_and(|part| {
        ((5..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
            || (part.len() == 4
                && part.as_bytes()[0].is_ascii_digit()
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    }) {
        if !variants.insert(parts[index].to_ascii_lowercase()) {
            return false;
        }
        index += 1;
    }
    let mut singletons = HashSet::new();
    while parts.get(index).is_some_and(|part| {
        part.len() == 1
            && part.as_bytes()[0].is_ascii_alphanumeric()
            && !part.eq_ignore_ascii_case("x")
    }) {
        let singleton = parts[index].to_ascii_lowercase();
        if !singletons.insert(singleton) {
            return false;
        }
        index += 1;
        let start = index;
        while parts.get(index).is_some_and(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    if parts
        .get(index)
        .is_some_and(|part| part.eq_ignore_ascii_case("x"))
    {
        index += 1;
        let start = index;
        while parts.get(index).is_some_and(|part| {
            (1..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    index == parts.len()
}

fn validate_optional_text(value: Option<&str>, field: &str) -> Result<(), LyricsValidationError> {
    if let Some(value) = value {
        validate_text(value, field, false)?;
    }
    Ok(())
}

fn validate_text(
    value: &str,
    field: &str,
    allow_whitespace_only: bool,
) -> Result<(), LyricsValidationError> {
    if value.is_empty() || (!allow_whitespace_only && value.trim().is_empty()) {
        return Err(lyrics_error(format!("{field} must not be empty")));
    }
    if value.chars().any(char::is_control) {
        return Err(lyrics_error(format!(
            "{field} must not contain control characters"
        )));
    }
    Ok(())
}

fn lyrics_error(message: impl Into<String>) -> LyricsValidationError {
    LyricsValidationError(message.into())
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl PlaybackState {
    #[must_use]
    pub const fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }
}

#[must_use]
pub fn format_duration(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes >= 60 {
        format!("{}:{:02}:{seconds:02}", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_short_and_long_durations() {
        assert_eq!(format_duration(65_000), "1:05");
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn derives_artist_and_title_from_filename() {
        let track = Track::from_path(
            TrackId::new(),
            "file:///music/Porter%20Robinson%20-%20Look%20at%20the%20Sky.flac",
            Path::new("/music/Porter Robinson - Look at the Sky.flac"),
        );
        assert_eq!(track.artist, "Porter Robinson");
        assert_eq!(track.title, "Look at the Sky");
    }

    #[test]
    fn sequential_and_repeat_one_have_distinct_end_behavior() {
        let first = TrackId::new();
        let second = TrackId::new();
        let mut queue = PlaybackQueue::with_seed(vec![first, second], Some(0), 7);

        assert_eq!(queue.on_end(PlaybackMode::RepeatOne), Some(first));
        assert_eq!(queue.next(PlaybackMode::RepeatOne), Some(second));
        assert_eq!(queue.on_end(PlaybackMode::Sequential), None);
    }

    #[test]
    fn shuffle_visits_every_position_before_repeating() {
        let entries = vec![TrackId::new(), TrackId::new(), TrackId::new()];
        let mut queue = PlaybackQueue::with_seed(entries.clone(), Some(0), 19);
        let mut heard = vec![queue.current().expect("current track")];
        heard.push(queue.next(PlaybackMode::Shuffle).expect("second track"));
        heard.push(queue.next(PlaybackMode::Shuffle).expect("third track"));

        heard.sort_by_key(ToString::to_string);
        let mut expected = entries;
        expected.sort_by_key(ToString::to_string);
        assert_eq!(heard, expected);
    }

    #[test]
    fn starting_shuffle_after_sequential_progress_excludes_the_current_entry() {
        let entries = vec![TrackId::new(), TrackId::new(), TrackId::new()];
        let mut queue = PlaybackQueue::with_seed(entries, Some(0), 23);
        queue.next(PlaybackMode::Sequential).expect("second track");
        queue.restart_shuffle_cycle();

        let upcoming = queue.upcoming(PlaybackMode::Shuffle, 10);
        assert_eq!(upcoming.len(), 3);
        assert_eq!(
            upcoming
                .iter()
                .filter(|(position, _)| Some(*position) == queue.current_position())
                .count(),
            1
        );
    }

    #[test]
    fn queue_positions_preserve_duplicate_playlist_entries() {
        let duplicate = TrackId::new();
        let last = TrackId::new();
        let mut queue = PlaybackQueue::with_seed(vec![duplicate, duplicate, last], Some(1), 3);

        assert_eq!(
            queue.upcoming(PlaybackMode::Sequential, 10),
            vec![(1, duplicate), (2, last)]
        );
        assert_eq!(queue.previous(PlaybackMode::Sequential), Some(duplicate));
        assert_eq!(queue.current_position(), Some(0));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn lyrics_validate_multi_track_overlap_and_respect_offset() {
        let lyrics = LyricsDocument {
            schema: None,
            format: WAVORA_LYRICS_FORMAT.to_owned(),
            version: WAVORA_LYRICS_VERSION.to_owned(),
            required_features: vec!["multi-track".to_owned(), "text-variants".to_owned()],
            offset_ms: 100,
            media: Some(LyricsMedia {
                duration_ms: Some(5_000),
                fingerprints: Vec::new(),
            }),
            metadata: LyricsMetadata {
                title: Some("Test".to_owned()),
                ..LyricsMetadata::default()
            },
            tracks: vec![
                LyricTrack {
                    id: "main".to_owned(),
                    role: "main".to_owned(),
                    label: None,
                    language: Some("en".to_owned()),
                    direction: None,
                },
                LyricTrack {
                    id: "backing".to_owned(),
                    role: "background".to_owned(),
                    label: None,
                    language: Some("zh-Hans".to_owned()),
                    direction: None,
                },
            ],
            cues: vec![
                LyricCue {
                    id: "cue-1".to_owned(),
                    track_id: "main".to_owned(),
                    start_ms: 1_000,
                    end_ms: Some(2_000),
                    texts: vec![LyricText {
                        kind: "original".to_owned(),
                        language: None,
                        direction: None,
                        text: "First".to_owned(),
                        segments: Vec::new(),
                    }],
                },
                LyricCue {
                    id: "cue-2".to_owned(),
                    track_id: "backing".to_owned(),
                    start_ms: 1_500,
                    end_ms: Some(2_500),
                    texts: vec![LyricText {
                        kind: "original".to_owned(),
                        language: None,
                        direction: None,
                        text: "背景".to_owned(),
                        segments: Vec::new(),
                    }],
                },
                LyricCue {
                    id: "cue-3".to_owned(),
                    track_id: "main".to_owned(),
                    start_ms: 3_000,
                    end_ms: None,
                    texts: vec![
                        LyricText {
                            kind: "original".to_owned(),
                            language: None,
                            direction: None,
                            text: "Second".to_owned(),
                            segments: Vec::new(),
                        },
                        LyricText {
                            kind: "translation".to_owned(),
                            language: Some("zh-Hans".to_owned()),
                            direction: None,
                            text: "第二句".to_owned(),
                            segments: Vec::new(),
                        },
                    ],
                },
            ],
        };

        lyrics.validate().expect("valid lyrics");
        assert!(lyrics.active_cue_indices(1_099).is_empty());
        assert_eq!(lyrics.active_cue_indices(1_100), vec![0]);
        assert_eq!(lyrics.active_cue_indices(1_600), vec![0, 1]);
        assert_eq!(lyrics.active_cue_indices(2_200), vec![1]);
        assert_eq!(lyrics.active_cue_indices(3_100), vec![2]);

        let mut same_track_overlap = lyrics.clone();
        same_track_overlap.cues[1].track_id = "main".to_owned();
        assert!(same_track_overlap.validate().is_err());

        let mut unknown_feature = lyrics.clone();
        unknown_feature.required_features = vec!["example:unknown".to_owned()];
        assert!(unknown_feature.validate().is_err());

        let mut fingerprint_bound = lyrics;
        fingerprint_bound
            .media
            .as_mut()
            .expect("media binding")
            .fingerprints
            .push(LyricsFingerprint {
                algorithm: "wavora-pcm-signature-v1".to_owned(),
                value: "ab".repeat(32),
            });
        assert!(
            fingerprint_bound
                .validate_media_binding(5_000, Some(&"ab".repeat(32)))
                .is_ok()
        );
        assert!(
            fingerprint_bound
                .validate_media_binding(5_000, Some(&"cd".repeat(32)))
                .is_err()
        );
        assert!(
            fingerprint_bound
                .validate_media_binding(8_000, Some(&"ab".repeat(32)))
                .is_err()
        );
    }

    #[test]
    fn language_tags_cover_scripts_extensions_private_use_and_grandfathered_tags() {
        assert!(is_language_tag("zh-Hans-CN"));
        assert!(is_language_tag("zh-cmn-Hans-CN"));
        assert!(is_language_tag("en-US-u-ca-gregory"));
        assert!(is_language_tag("x-wavora-test"));
        assert!(is_language_tag("i-klingon"));
        assert!(!is_language_tag("en_US"));
        assert!(!is_language_tag("en-u"));
        assert!(!is_language_tag("sl-rozaj-rozaj"));
    }
}
