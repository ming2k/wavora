use crate::file_uri_to_path;
use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardVisualKey, Visual};
use symphonia::core::probe::Hint;

const MAX_ARTWORK_BYTES: u64 = 24 * 1024 * 1024;
const ARTWORK_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Encoded cover artwork discovered for one local audio file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkData {
    pub media_type: Option<String>,
    pub bytes: Box<[u8]>,
}

/// Loads the preferred artwork for a local track.
///
/// Embedded front-cover art wins. When it is absent, deterministic sidecar
/// names are considered, with track-specific files ahead of album-directory
/// conventions such as `cover.jpg` and `folder.png`.
#[must_use]
pub fn load_artwork(uri: &str) -> Option<ArtworkData> {
    let path = file_uri_to_path(uri)?;
    embedded_artwork(&path).or_else(|| sidecar_artwork(&path))
}

fn embedded_artwork(path: &Path) -> Option<ArtworkData> {
    let file = File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;

    if let Some(mut metadata) = probed.metadata.get()
        && let Some(revision) = metadata.skip_to_latest()
        && let Some(artwork) = artwork_from_revision(revision)
    {
        return Some(artwork);
    }
    probed
        .format
        .metadata()
        .skip_to_latest()
        .and_then(artwork_from_revision)
}

fn artwork_from_revision(revision: &MetadataRevision) -> Option<ArtworkData> {
    let visual = revision
        .visuals()
        .iter()
        .find(|visual| visual.usage == Some(StandardVisualKey::FrontCover))
        .or_else(|| revision.visuals().first())?;
    artwork_from_visual(visual)
}

fn artwork_from_visual(visual: &Visual) -> Option<ArtworkData> {
    (u64::try_from(visual.data.len()).ok()? <= MAX_ARTWORK_BYTES).then(|| ArtworkData {
        media_type: (!visual.media_type.trim().is_empty()).then(|| visual.media_type.clone()),
        bytes: visual.data.clone(),
    })
}

fn sidecar_artwork(audio_path: &Path) -> Option<ArtworkData> {
    let path = sidecar_candidates(audio_path).into_iter().next()?;
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ARTWORK_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?.into_boxed_slice();
    let media_type = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .and_then(|extension| match extension.as_str() {
            "jpg" | "jpeg" => Some("image/jpeg".to_owned()),
            "png" => Some("image/png".to_owned()),
            "webp" => Some("image/webp".to_owned()),
            _ => None,
        });
    Some(ArtworkData { media_type, bytes })
}

fn sidecar_candidates(audio_path: &Path) -> Vec<PathBuf> {
    let Some(parent) = audio_path.parent() else {
        return Vec::new();
    };
    let Some(track_stem) = audio_path.file_stem().and_then(|stem| stem.to_str()) else {
        return Vec::new();
    };
    let track_stem = track_stem.to_ascii_lowercase();
    let mut candidates = parent
        .read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            if !ARTWORK_EXTENSIONS.contains(&extension.as_str()) {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
            let rank = if stem == format!("{track_stem}.cover") {
                0
            } else if stem == track_stem {
                1
            } else {
                match stem.as_str() {
                    "cover" => 2,
                    "folder" => 3,
                    "front" => 4,
                    "album" => 5,
                    _ => return None,
                }
            };
            Some((rank, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates.into_iter().map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn track_specific_sidecar_precedes_folder_cover() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("wavora-artwork-{unique}"));
        std::fs::create_dir_all(&directory).expect("directory");
        let audio = directory.join("Track.FLAC");
        std::fs::write(directory.join("cover.png"), b"cover").expect("cover");
        std::fs::write(directory.join("track.cover.jpg"), b"track").expect("track cover");

        let candidates = sidecar_candidates(&audio);
        assert_eq!(
            candidates.first().and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("track.cover.jpg"))
        );

        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
