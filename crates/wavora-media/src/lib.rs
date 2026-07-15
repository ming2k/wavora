//! Media services for Wavora.

mod artwork;
mod audio;
mod library;
mod lyrics;
mod uri;

pub use artwork::{ArtworkData, load_artwork};
pub use audio::{AudioController, AudioError, AudioEvent};
pub use library::{
    AcousticFingerprint, CachedAudioEvidence, FileIdentity, LibraryEvent, LibraryScanner,
    ScannedTrack, acoustic_fingerprints_match, is_supported_audio,
};
pub use lyrics::{LoadedLyrics, LyricsLoadError, load_sidecar_lyrics, lyric_sidecar_paths};
pub use uri::{file_uri_to_path, path_to_file_uri};
pub use wavora_audio_analysis::{AudioFeatures, SPECTRUM_BANDS};
