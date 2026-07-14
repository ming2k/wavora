//! Domain types shared by Wavora's application and media layers.

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub id: u64,
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub codec: String,
    pub favorite: bool,
}

impl Track {
    #[must_use]
    pub fn from_path(uri: impl Into<String>, path: &Path) -> Self {
        let uri = uri.into();
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Unknown track");
        let (artist, title) = stem
            .split_once(" - ")
            .map_or(("Unknown artist", stem), |(artist, title)| (artist, title));
        Self {
            id: stable_id(&uri),
            uri,
            title: title.trim().to_owned(),
            artist: artist.trim().to_owned(),
            album: "Local music".to_owned(),
            duration_ms: 0,
            codec: "Audio".to_owned(),
            favorite: false,
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

impl PlaybackState {
    #[must_use]
    pub const fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }
}

#[must_use]
pub fn stable_id(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
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
            "file:///music/Porter%20Robinson%20-%20Look%20at%20the%20Sky.flac",
            Path::new("/music/Porter Robinson - Look at the Sky.flac"),
        );
        assert_eq!(track.artist, "Porter Robinson");
        assert_eq!(track.title, "Look at the Sky");
    }
}
