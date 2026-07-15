use crate::file_uri_to_path;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;
use wavora_core::{
    LyricCue, LyricSegment, LyricText, LyricTrack, LyricsContributor, LyricsDocument,
    LyricsMetadata, LyricsRights, LyricsSource, LyricsValidationError, WAVORA_LYRICS_FORMAT,
    WAVORA_LYRICS_VERSION,
};

pub const MAX_LYRICS_FILE_BYTES: u64 = 4 * 1_024 * 1_024;

#[derive(Debug, Clone)]
pub struct LoadedLyrics {
    pub document: LyricsDocument,
    pub path: PathBuf,
}

#[derive(Debug, Error)]
pub enum LyricsLoadError {
    #[error("lyrics are only supported for local file URIs")]
    InvalidUri,
    #[error("could not read lyrics sidecar {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("lyrics sidecar {path} exceeds the {MAX_LYRICS_FILE_BYTES}-byte limit")]
    TooLarge { path: PathBuf },
    #[error("lyrics sidecar {path} is not UTF-8: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::str::Utf8Error,
    },
    #[error("lyrics sidecar {path} is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("lyrics sidecar {path} violates the format constraints: {source}")]
    Validation {
        path: PathBuf,
        source: LyricsValidationError,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLyricsDocument {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    format: String,
    version: u32,
    #[serde(default)]
    offset_ms: i64,
    #[serde(default)]
    metadata: LegacyLyricsMetadata,
    lines: Vec<LegacyLyricLine>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyLyricsMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    language: Option<String>,
    author: Option<String>,
    source: Option<String>,
    copyright: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLyricLine {
    start_ms: u64,
    #[serde(default)]
    end_ms: Option<u64>,
    text: String,
    #[serde(default)]
    translation: Option<String>,
    #[serde(default)]
    words: Vec<LegacyLyricWord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLyricWord {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

impl LegacyLyricsDocument {
    fn upgrade(self) -> Result<LyricsDocument, LyricsValidationError> {
        if self.format != WAVORA_LYRICS_FORMAT || self.version != 1 {
            return Err(LyricsValidationError(
                "legacy lyrics must use format \"wavora-lyrics\" and version 1".to_owned(),
            ));
        }
        let language = self.metadata.language.unwrap_or_else(|| "und".to_owned());
        let contributors = self
            .metadata
            .author
            .into_iter()
            .map(|name| LyricsContributor {
                name,
                role: "transcriber".to_owned(),
            })
            .collect();
        let source = self.metadata.source.map(|name| LyricsSource {
            name: Some(name),
            uri: None,
        });
        let rights = self.metadata.copyright.map(|copyright| LyricsRights {
            copyright: Some(copyright),
            license: None,
            license_uri: None,
        });
        let cues = self
            .lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let mut texts = vec![LyricText {
                    kind: "original".to_owned(),
                    language: None,
                    direction: None,
                    text: line.text,
                    segments: line
                        .words
                        .into_iter()
                        .map(|word| LyricSegment {
                            start_ms: word.start_ms,
                            end_ms: word.end_ms,
                            text: word.text,
                        })
                        .collect(),
                }];
                if let Some(translation) = line.translation {
                    texts.push(LyricText {
                        kind: "translation".to_owned(),
                        language: Some("und".to_owned()),
                        direction: None,
                        text: translation,
                        segments: Vec::new(),
                    });
                }
                LyricCue {
                    id: format!("legacy-{index:05}"),
                    track_id: "main".to_owned(),
                    start_ms: line.start_ms,
                    end_ms: line.end_ms,
                    texts,
                }
            })
            .collect();
        Ok(LyricsDocument {
            schema: None,
            format: WAVORA_LYRICS_FORMAT.to_owned(),
            version: WAVORA_LYRICS_VERSION.to_owned(),
            required_features: Vec::new(),
            offset_ms: self.offset_ms,
            media: None,
            metadata: LyricsMetadata {
                title: self.metadata.title,
                artist: self.metadata.artist,
                album: self.metadata.album,
                contributors,
                source,
                rights,
            },
            tracks: vec![LyricTrack {
                id: "main".to_owned(),
                role: "main".to_owned(),
                label: None,
                language: Some(language),
                direction: None,
            }],
            cues,
        })
    }
}

/// Loads the first existing lyrics sidecar using the documented precedence.
///
/// The collision-safe `<audio-filename>.wlyric.json` name wins over the
/// convenient legacy `<stem>.wlyric.json` name.
///
/// # Errors
///
/// Returns an error when the URI is not local or the first existing sidecar
/// cannot be read, decoded as UTF-8 and JSON, or validated as format version 1.
pub fn load_sidecar_lyrics(uri: &str) -> Result<Option<LoadedLyrics>, LyricsLoadError> {
    let audio_path = file_uri_to_path(uri).ok_or(LyricsLoadError::InvalidUri)?;
    let Some(path) = lyric_sidecar_paths(&audio_path)
        .into_iter()
        .find(|candidate| candidate.is_file())
    else {
        return Ok(None);
    };
    let mut file = File::open(&path).map_err(|source| LyricsLoadError::Io {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_LYRICS_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| LyricsLoadError::Io {
            path: path.clone(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LYRICS_FILE_BYTES {
        return Err(LyricsLoadError::TooLarge { path });
    }
    let json = std::str::from_utf8(&bytes).map_err(|source| LyricsLoadError::Utf8 {
        path: path.clone(),
        source,
    })?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|source| LyricsLoadError::Json {
            path: path.clone(),
            source,
        })?;
    let document = if value.get("lines").is_some() {
        serde_json::from_value::<LegacyLyricsDocument>(value)
            .map_err(|source| LyricsLoadError::Json {
                path: path.clone(),
                source,
            })?
            .upgrade()
            .map_err(|source| LyricsLoadError::Validation {
                path: path.clone(),
                source,
            })?
    } else {
        serde_json::from_value::<LyricsDocument>(value).map_err(|source| LyricsLoadError::Json {
            path: path.clone(),
            source,
        })?
    };
    document
        .validate()
        .map_err(|source| LyricsLoadError::Validation {
            path: path.clone(),
            source,
        })?;
    Ok(Some(LoadedLyrics { document, path }))
}

#[must_use]
pub fn lyric_sidecar_paths(audio_path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::with_capacity(2);
    if let Some(filename) = audio_path.file_name() {
        let mut sidecar_name = filename.to_os_string();
        sidecar_name.push(".wlyric.json");
        candidates.push(audio_path.with_file_name(sidecar_name));
    }
    let fallback = audio_path.with_extension("wlyric.json");
    if !candidates.contains(&fallback) {
        candidates.push(fallback);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_to_file_uri;
    use std::fs;

    #[test]
    fn sidecar_names_prioritize_the_full_audio_filename() {
        assert_eq!(
            lyric_sidecar_paths(Path::new("/music/Signal.flac")),
            vec![
                PathBuf::from("/music/Signal.flac.wlyric.json"),
                PathBuf::from("/music/Signal.wlyric.json"),
            ]
        );
    }

    #[test]
    fn example_document_and_schema_are_valid_json() {
        let document: LyricsDocument = serde_json::from_str(include_str!(
            "../../../examples/lyrics/Signal.flac.wlyric.json"
        ))
        .expect("parse example lyrics");
        document.validate().expect("validate example lyrics");
        let _: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/reference/wavora-lyrics.schema.json"
        ))
        .expect("parse lyrics schema");
    }

    #[test]
    fn unknown_optional_fields_are_forward_compatible() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/lyrics/Signal.flac.wlyric.json"
        ))
        .expect("parse example JSON");
        value["org.example:quality"] = serde_json::json!(0.98);
        value["tracks"][0]["org.example:voice"] = serde_json::json!("alto");

        let document: LyricsDocument = serde_json::from_value(value).expect("ignore extensions");
        document.validate().expect("validate extended document");
    }

    #[test]
    fn legacy_line_documents_upgrade_to_tracks_texts_and_segments() {
        let root =
            std::env::temp_dir().join(format!("wavora-lyrics-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create lyrics test directory");
        let audio_path = root.join("Legacy.flac");
        let sidecar = lyric_sidecar_paths(&audio_path).remove(0);
        fs::write(
            &sidecar,
            r#"{
              "format":"wavora-lyrics",
              "version":1,
              "metadata":{"language":"en","author":"A"},
              "lines":[{
                "start_ms":1000,
                "end_ms":2000,
                "text":"Hello",
                "translation":"你好",
                "words":[{"start_ms":1000,"end_ms":1900,"text":"Hello"}]
              }]
            }"#,
        )
        .expect("write legacy sidecar");

        let loaded = load_sidecar_lyrics(&path_to_file_uri(&audio_path))
            .expect("load legacy lyrics")
            .expect("legacy sidecar exists");
        assert_eq!(loaded.document.version, WAVORA_LYRICS_VERSION);
        assert_eq!(loaded.document.tracks.len(), 1);
        assert_eq!(loaded.document.cues[0].texts.len(), 2);
        assert_eq!(loaded.document.cues[0].texts[0].segments.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_invalid_canonical_sidecar_is_not_hidden_by_the_fallback() {
        let root =
            std::env::temp_dir().join(format!("wavora-lyrics-precedence-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create lyrics test directory");
        let audio_path = root.join("Signal.flac");
        let candidates = lyric_sidecar_paths(&audio_path);
        fs::write(&candidates[0], b"not json").expect("write canonical sidecar");
        fs::write(
            &candidates[1],
            include_bytes!("../../../examples/lyrics/Signal.flac.wlyric.json"),
        )
        .expect("write fallback sidecar");

        let uri = path_to_file_uri(&audio_path);
        assert!(matches!(
            load_sidecar_lyrics(&uri),
            Err(LyricsLoadError::Json { .. })
        ));
        fs::remove_file(&candidates[0]).expect("remove canonical sidecar");
        assert!(load_sidecar_lyrics(&uri).expect("load fallback").is_some());
        let _ = fs::remove_dir_all(root);
    }
}
