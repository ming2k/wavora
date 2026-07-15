use crate::path_to_file_uri;
use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, Source};
use rusty_chromaprint::{Configuration, Fingerprinter, match_fingerprints};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey, Tag, Value};
use symphonia::core::probe::Hint;
use walkdir::WalkDir;
use wavora_core::{Track, TrackId};

const AUDIO_EXTENSIONS: &[&str] = &["aac", "flac", "m4a", "mp3", "oga", "ogg", "wav"];
const SIGNATURE_SECONDS: u64 = 90;
const ACOUSTIC_FINGERPRINT_ALGORITHM: u32 = 1;
const FINGERPRINT_FRAMES_PER_CHUNK: usize = 4_096;
const MIN_FUZZY_MATCH_SECONDS: f32 = 20.0;
const MIN_FUZZY_MATCH_COVERAGE: f32 = 0.8;
const MAX_FUZZY_MATCH_SCORE: f64 = 6.0;

/// A versioned Chromaprint fingerprint used only for conservative fallback
/// reconciliation after exact identity evidence fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcousticFingerprint {
    algorithm: u32,
    items: Vec<u32>,
}

impl AcousticFingerprint {
    #[must_use]
    pub fn from_parts(algorithm: u32, items: Vec<u32>) -> Self {
        Self { algorithm, items }
    }

    fn current(items: Vec<u32>) -> Self {
        Self::from_parts(ACOUSTIC_FINGERPRINT_ALGORITHM, items)
    }

    #[must_use]
    pub const fn algorithm(&self) -> u32 {
        self.algorithm
    }

    #[must_use]
    pub fn items(&self) -> &[u32] {
        &self.items
    }
}

/// Returns whether two version-compatible fingerprints contain one strong,
/// nearly full-length alignment.
#[must_use]
pub fn acoustic_fingerprints_match(
    left: &AcousticFingerprint,
    right: &AcousticFingerprint,
) -> bool {
    if left.algorithm != right.algorithm || left.algorithm != ACOUSTIC_FINGERPRINT_ALGORITHM {
        return false;
    }
    let configuration = Configuration::preset_test2();
    let comparable_items =
        u16::try_from(left.items.len().min(right.items.len())).unwrap_or(u16::MAX);
    let comparable_seconds = configuration.item_duration_in_seconds() * f32::from(comparable_items);
    if comparable_seconds < MIN_FUZZY_MATCH_SECONDS {
        return false;
    }
    let required_seconds = comparable_seconds * MIN_FUZZY_MATCH_COVERAGE;
    match_fingerprints(&left.items, &right.items, &configuration).is_ok_and(|segments| {
        segments.iter().any(|segment| {
            segment.score <= MAX_FUZZY_MATCH_SCORE
                && segment.duration(&configuration) >= required_seconds
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub device: Option<u64>,
    pub inode: Option<u64>,
    pub size_bytes: u64,
    pub modified_ns: i64,
}

#[derive(Debug, Clone)]
pub struct ScannedTrack {
    pub track: Track,
    pub path: PathBuf,
    pub file: FileIdentity,
    /// A tag-independent digest of the first decoded PCM window.
    ///
    /// It is intentionally an identity-reconciliation hint rather than the
    /// persisted track ID. Exact matches survive metadata edits and moves;
    /// fuzzy matching is a separate fallback and never becomes the ID.
    pub audio_signature: [u8; 32],
    /// A similarity-tolerant fingerprint used only after exact matching fails
    /// and only against missing catalog records.
    pub acoustic_fingerprint: AcousticFingerprint,
}

#[derive(Debug, Clone)]
pub struct CachedAudioEvidence {
    pub path: PathBuf,
    pub file: FileIdentity,
    pub audio_signature: [u8; 32],
    /// `None` marks a row created before acoustic fingerprints were added; the
    /// next scan recomputes and backfills both forms of audio evidence.
    pub acoustic_fingerprint: Option<AcousticFingerprint>,
}

#[derive(Debug)]
enum LibraryCommand {
    Scan(PathBuf),
    SetAudioEvidenceCache(Vec<CachedAudioEvidence>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum LibraryEvent {
    ScanStarted(PathBuf),
    Track(Box<ScannedTrack>),
    ScanFinished {
        root: PathBuf,
        discovered: usize,
        rejected: usize,
    },
    Error(String),
}

pub struct LibraryScanner {
    commands: Sender<LibraryCommand>,
    events: Receiver<LibraryEvent>,
    shutdown: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl LibraryScanner {
    /// Starts the dedicated filesystem and metadata worker.
    ///
    /// # Errors
    ///
    /// Returns the operating-system thread creation error.
    pub fn spawn() -> std::io::Result<Self> {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let worker = thread::Builder::new()
            .name("wavora-library".to_owned())
            .spawn(move || library_worker(&command_rx, &event_tx, &worker_shutdown))?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            shutdown,
            worker: Some(worker),
        })
    }

    pub fn scan(&self, path: impl Into<PathBuf>) {
        let _ = self.commands.send(LibraryCommand::Scan(path.into()));
    }

    pub fn set_audio_evidence_cache(&self, cache: Vec<CachedAudioEvidence>) {
        let _ = self
            .commands
            .send(LibraryCommand::SetAudioEvidenceCache(cache));
    }

    pub fn try_iter(&self) -> impl Iterator<Item = LibraryEvent> + '_ {
        self.events.try_iter()
    }
}

impl Drop for LibraryScanner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = self.commands.send(LibraryCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn library_worker(
    commands: &Receiver<LibraryCommand>,
    events: &Sender<LibraryEvent>,
    shutdown: &AtomicBool,
) {
    let mut audio_evidence_cache = Vec::new();
    while !shutdown.load(Ordering::Acquire) {
        let Ok(command) = commands.recv() else {
            break;
        };
        match command {
            LibraryCommand::Scan(root) => {
                let _ = events.send(LibraryEvent::ScanStarted(root.clone()));
                let (discovered, rejected) =
                    scan_root(&root, &mut audio_evidence_cache, events, shutdown);
                let _ = events.send(LibraryEvent::ScanFinished {
                    root,
                    discovered,
                    rejected,
                });
            }
            LibraryCommand::SetAudioEvidenceCache(cache) => audio_evidence_cache = cache,
            LibraryCommand::Shutdown => break,
        }
    }
}

fn scan_root(
    root: &Path,
    audio_evidence_cache: &mut Vec<CachedAudioEvidence>,
    events: &Sender<LibraryEvent>,
    shutdown: &AtomicBool,
) -> (usize, usize) {
    if root.is_file() {
        return match scan_path(root.to_owned(), audio_evidence_cache, events, shutdown) {
            ScanResult::Added => (1, 0),
            ScanResult::Rejected => (0, 1),
            ScanResult::Ignored => (0, 0),
        };
    }
    if !root.is_dir() {
        let _ = events.send(LibraryEvent::Error(format!(
            "music location does not exist: {}",
            root.display()
        )));
        return (0, 0);
    }

    let mut discovered = 0;
    let mut rejected = 0;
    for entry in WalkDir::new(root).follow_links(false) {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() || !is_supported_audio(entry.path()) {
            continue;
        }
        match scan_path(entry.into_path(), audio_evidence_cache, events, shutdown) {
            ScanResult::Added => discovered += 1,
            ScanResult::Rejected => rejected += 1,
            ScanResult::Ignored => {}
        }
    }
    (discovered, rejected)
}

enum ScanResult {
    Added,
    Rejected,
    Ignored,
}

fn scan_path(
    path: PathBuf,
    audio_evidence_cache: &mut Vec<CachedAudioEvidence>,
    events: &Sender<LibraryEvent>,
    shutdown: &AtomicBool,
) -> ScanResult {
    if shutdown.load(Ordering::Acquire) || !is_supported_audio(&path) {
        return ScanResult::Ignored;
    }
    let canonical = path.canonicalize().unwrap_or(path);
    let Ok(metadata) = canonical.metadata() else {
        return ScanResult::Rejected;
    };
    let identity = file_identity(&metadata);
    let Ok(file) = File::open(&canonical) else {
        return ScanResult::Rejected;
    };
    let Ok(mut decoder) = Decoder::try_from(file) else {
        return ScanResult::Rejected;
    };
    let uri = path_to_file_uri(&canonical);
    let mut track = Track::from_path(TrackId::new(), uri, &canonical);
    track.duration_ms = decoder
        .total_duration()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default();
    codec_label(&canonical).clone_into(&mut track.codec);
    apply_embedded_metadata(&canonical, &mut track);
    let (audio_signature, acoustic_fingerprint) =
        cached_audio_evidence(audio_evidence_cache, &canonical, identity)
            .unwrap_or_else(|| decoded_audio_evidence(&mut decoder));
    audio_evidence_cache.retain(|cached| cached.path != canonical);
    audio_evidence_cache.push(CachedAudioEvidence {
        path: canonical.clone(),
        file: identity,
        audio_signature,
        acoustic_fingerprint: Some(acoustic_fingerprint.clone()),
    });
    let scanned = ScannedTrack {
        track,
        path: canonical,
        file: identity,
        audio_signature,
        acoustic_fingerprint,
    };
    if shutdown.load(Ordering::Acquire) {
        return ScanResult::Ignored;
    }
    if events.send(LibraryEvent::Track(Box::new(scanned))).is_ok() {
        ScanResult::Added
    } else {
        ScanResult::Ignored
    }
}

fn cached_audio_evidence(
    cache: &[CachedAudioEvidence],
    path: &Path,
    identity: FileIdentity,
) -> Option<([u8; 32], AcousticFingerprint)> {
    cache
        .iter()
        .find(|cached| {
            cached.file.size_bytes == identity.size_bytes
                && cached.file.modified_ns == identity.modified_ns
                && (cached.path == path
                    || (cached.file.device.is_some()
                        && cached.file.device == identity.device
                        && cached.file.inode == identity.inode))
        })
        .and_then(|cached| {
            cached
                .acoustic_fingerprint
                .as_ref()
                .filter(|fingerprint| fingerprint.algorithm == ACOUSTIC_FINGERPRINT_ALGORITHM)
                .cloned()
                .map(|fingerprint| (cached.audio_signature, fingerprint))
        })
}

fn apply_embedded_metadata(path: &Path, track: &mut Track) {
    let Ok(file) = File::open(path) else {
        return;
    };
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let Ok(mut probed) = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return;
    };

    let mut embedded = EmbeddedMetadata::default();
    if let Some(mut metadata) = probed.metadata.get()
        && let Some(revision) = metadata.skip_to_latest()
    {
        embedded.absorb(revision);
    }
    if let Some(revision) = probed.format.metadata().skip_to_latest() {
        embedded.absorb(revision);
    }
    if let Some(title) = embedded.title {
        track.title = title;
    }
    if let Some(artist) = embedded.artist {
        track.artist = artist;
    }
    if let Some(album) = embedded.album {
        track.album = album;
    }
}

#[derive(Default)]
struct EmbeddedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl EmbeddedMetadata {
    fn absorb(&mut self, revision: &MetadataRevision) {
        for tag in revision.tags() {
            let Some(value) = metadata_tag_text(tag) else {
                continue;
            };
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) if self.title.is_none() => {
                    self.title = Some(value);
                }
                Some(StandardTagKey::Artist) if self.artist.is_none() => {
                    self.artist = Some(value);
                }
                Some(StandardTagKey::Album) if self.album.is_none() => {
                    self.album = Some(value);
                }
                _ => self.absorb_fallback_key(&tag.key, value),
            }
        }
    }

    fn absorb_fallback_key(&mut self, key: &str, value: String) {
        match key.trim().to_ascii_lowercase().as_str() {
            "title" | "inam" if self.title.is_none() => self.title = Some(value),
            "artist" | "iart" if self.artist.is_none() => self.artist = Some(value),
            "album" | "iprd" if self.album.is_none() => self.album = Some(value),
            _ => {}
        }
    }
}

fn metadata_tag_text(tag: &Tag) -> Option<String> {
    match &tag.value {
        Value::String(value) => non_empty_tag(value),
        Value::Binary(_)
        | Value::Boolean(_)
        | Value::Flag
        | Value::Float(_)
        | Value::SignedInt(_)
        | Value::UnsignedInt(_) => None,
    }
}

fn non_empty_tag(value: &str) -> Option<String> {
    let value =
        value.trim_matches(|character: char| character.is_whitespace() || character == '\0');
    (!value.is_empty()).then(|| value.to_owned())
}

fn decoded_audio_evidence<R>(decoder: &mut Decoder<R>) -> ([u8; 32], AcousticFingerprint)
where
    R: std::io::Read + std::io::Seek,
{
    let sample_rate = decoder.sample_rate().get();
    let channels = decoder.channels().get();
    let sample_limit = u64::from(sample_rate)
        .saturating_mul(u64::from(channels))
        .saturating_mul(SIGNATURE_SECONDS);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"wavora-pcm-signature-v1");
    hasher.update(&sample_rate.to_le_bytes());
    hasher.update(&channels.to_le_bytes());
    let configuration = Configuration::preset_test2();
    let mut fingerprinter = Fingerprinter::new(&configuration);
    let fingerprinting = fingerprinter
        .start(sample_rate, u32::from(channels))
        .is_ok();
    let chunk_samples = FINGERPRINT_FRAMES_PER_CHUNK.saturating_mul(usize::from(channels));
    let mut fingerprint_buffer = Vec::with_capacity(chunk_samples);
    for sample in decoder
        .by_ref()
        .take(usize::try_from(sample_limit).unwrap_or(usize::MAX))
    {
        hasher.update(&sample.to_bits().to_le_bytes());
        if fingerprinting {
            fingerprint_buffer.push(pcm_i16(sample));
            if fingerprint_buffer.len() == chunk_samples {
                fingerprinter.consume(&fingerprint_buffer);
                fingerprint_buffer.clear();
            }
        }
    }
    let fingerprint = if fingerprinting {
        if !fingerprint_buffer.is_empty() {
            fingerprinter.consume(&fingerprint_buffer);
        }
        fingerprinter.finish();
        fingerprinter.fingerprint().to_vec()
    } else {
        Vec::new()
    };
    (
        *hasher.finalize().as_bytes(),
        AcousticFingerprint::current(fingerprint),
    )
}

#[allow(clippy::cast_possible_truncation)]
fn pcm_i16(sample: f32) -> i16 {
    let scaled = sample.clamp(-1.0, 1.0) * f32::from(i16::MAX);
    scaled.round() as i16
}

fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or_default();
    let (device, inode) = unix_file_identity(metadata);
    FileIdentity {
        device,
        inode,
        size_bytes: metadata.len(),
        modified_ns,
    }
}

#[cfg(unix)]
fn unix_file_identity(metadata: &std::fs::Metadata) -> (Option<u64>, Option<u64>) {
    use std::os::unix::fs::MetadataExt;
    (Some(metadata.dev()), Some(metadata.ino()))
}

#[cfg(not(unix))]
fn unix_file_identity(_metadata: &std::fs::Metadata) -> (Option<u64>, Option<u64>) {
    (None, None)
}

fn codec_label(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("flac") => "FLAC",
        Some("mp3") => "MP3",
        Some("m4a" | "aac") => "AAC",
        Some("ogg" | "oga") => "Ogg Vorbis",
        Some("wav") => "WAV",
        _ => "Audio",
    }
}

#[must_use]
pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_extensions_case_insensitively() {
        assert!(is_supported_audio(Path::new("track.FLAC")));
        assert!(!is_supported_audio(Path::new("cover.png")));
        assert!(!is_supported_audio(Path::new("unsupported.opus")));
    }

    #[test]
    fn fuzzy_matching_requires_version_coverage_and_a_strong_alignment() {
        let mut state = 17_u32;
        let source = (0..800)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                state
            })
            .collect::<Vec<_>>();
        let transcoded = source
            .iter()
            .enumerate()
            .map(|(index, item)| item ^ u32::from(index.is_multiple_of(3)))
            .collect::<Vec<_>>();
        let original = AcousticFingerprint::from_parts(1, source.clone());
        let similar = AcousticFingerprint::from_parts(1, transcoded);
        let short = AcousticFingerprint::from_parts(1, source[..100].to_vec());
        let wrong_version = AcousticFingerprint::from_parts(2, source);

        assert!(acoustic_fingerprints_match(&original, &similar));
        assert!(!acoustic_fingerprints_match(&original, &short));
        assert!(!acoustic_fingerprints_match(&original, &wrong_version));
    }

    #[test]
    fn decoded_fingerprints_tolerate_amplitude_changes() {
        let mut original_decoder = Decoder::try_from(std::io::Cursor::new(synthetic_wav(12_000)))
            .expect("original decoder");
        let mut normalized_decoder = Decoder::try_from(std::io::Cursor::new(synthetic_wav(8_000)))
            .expect("normalized decoder");

        let (original_signature, original_fingerprint) =
            decoded_audio_evidence(&mut original_decoder);
        let (normalized_signature, normalized_fingerprint) =
            decoded_audio_evidence(&mut normalized_decoder);

        assert_ne!(original_signature, normalized_signature);
        assert!(!original_fingerprint.items().is_empty());
        assert!(acoustic_fingerprints_match(
            &original_fingerprint,
            &normalized_fingerprint
        ));
    }

    #[test]
    fn reuses_signature_only_for_an_unchanged_file_observation() {
        let identity = FileIdentity {
            device: Some(1),
            inode: Some(2),
            size_bytes: 3,
            modified_ns: 4,
        };
        let cache = [CachedAudioEvidence {
            path: PathBuf::from("/music/old.flac"),
            file: identity,
            audio_signature: [5; 32],
            acoustic_fingerprint: Some(AcousticFingerprint::from_parts(1, vec![7; 200])),
        }];

        assert_eq!(
            cached_audio_evidence(&cache, Path::new("/music/renamed.flac"), identity),
            Some(([5; 32], AcousticFingerprint::from_parts(1, vec![7; 200])))
        );
        assert_eq!(
            cached_audio_evidence(
                &cache,
                Path::new("/music/old.flac"),
                FileIdentity {
                    modified_ns: 6,
                    ..identity
                }
            ),
            None
        );

        let legacy_cache = [CachedAudioEvidence {
            acoustic_fingerprint: None,
            ..cache[0].clone()
        }];
        assert_eq!(
            cached_audio_evidence(&legacy_cache, Path::new("/music/old.flac"), identity),
            None
        );
        let stale_cache = [CachedAudioEvidence {
            acoustic_fingerprint: Some(AcousticFingerprint::from_parts(2, vec![7; 200])),
            ..cache[0].clone()
        }];
        assert_eq!(
            cached_audio_evidence(&stale_cache, Path::new("/music/old.flac"), identity),
            None
        );
    }

    #[test]
    fn maps_standardized_metadata_fields() {
        let mut builder = symphonia::core::meta::MetadataBuilder::new();
        builder
            .add_tag(Tag::new(
                Some(StandardTagKey::TrackTitle),
                "",
                Value::from("  Title  "),
            ))
            .add_tag(Tag::new(
                Some(StandardTagKey::Artist),
                "",
                Value::from("Artist"),
            ))
            .add_tag(Tag::new(
                Some(StandardTagKey::Album),
                "",
                Value::from("Album"),
            ));
        let mut metadata = EmbeddedMetadata::default();
        metadata.absorb(&builder.metadata());

        assert_eq!(metadata.title.as_deref(), Some("Title"));
        assert_eq!(metadata.artist.as_deref(), Some("Artist"));
        assert_eq!(metadata.album.as_deref(), Some("Album"));
    }

    #[test]
    fn reads_riff_info_metadata_with_symphonia() {
        let root = std::env::temp_dir().join(format!(
            "wavora-symphonia-metadata-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let path = root.join("fallback.wav");
        std::fs::write(&path, tagged_wav()).expect("tagged wav");
        let mut track = Track::from_path(TrackId::new(), path_to_file_uri(&path), &path);

        apply_embedded_metadata(&path, &mut track);

        assert_eq!(track.title, "Symphonia title");
        assert_eq!(track.artist, "Symphonia artist");
        assert_eq!(track.album, "Symphonia album");
        let _ = std::fs::remove_dir_all(root);
    }

    fn tagged_wav() -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        let mut format = Vec::new();
        format.extend_from_slice(&1_u16.to_le_bytes());
        format.extend_from_slice(&1_u16.to_le_bytes());
        format.extend_from_slice(&8_000_u32.to_le_bytes());
        format.extend_from_slice(&16_000_u32.to_le_bytes());
        format.extend_from_slice(&2_u16.to_le_bytes());
        format.extend_from_slice(&16_u16.to_le_bytes());
        append_riff_chunk(&mut body, *b"fmt ", &format);

        let mut info = b"INFO".to_vec();
        append_riff_chunk(&mut info, *b"INAM", b"Symphonia title\0");
        append_riff_chunk(&mut info, *b"IART", b"Symphonia artist\0");
        append_riff_chunk(&mut info, *b"IPRD", b"Symphonia album\0");
        append_riff_chunk(&mut body, *b"LIST", &info);
        append_riff_chunk(&mut body, *b"data", &[0; 16]);

        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes());
        wav.extend_from_slice(&body);
        wav
    }

    fn synthetic_wav(amplitude: i16) -> Vec<u8> {
        const SAMPLE_RATE: u32 = 8_000;
        const SECONDS: usize = 30;
        const PERIODS: [usize; 8] = [31, 29, 25, 23, 21, 19, 17, 15];
        let sample_count = usize::try_from(SAMPLE_RATE).expect("sample rate") * SECONDS;
        let mut data = Vec::with_capacity(sample_count * 2);
        for index in 0..sample_count {
            let second = index / usize::try_from(SAMPLE_RATE).expect("sample rate");
            let period = PERIODS[second % PERIODS.len()];
            let primary = if index % period < period / 2 {
                amplitude
            } else {
                -amplitude
            };
            let secondary_period = period + 11;
            let secondary = if index % secondary_period < secondary_period / 2 {
                amplitude / 3
            } else {
                -amplitude / 3
            };
            data.extend_from_slice(&primary.saturating_add(secondary).to_le_bytes());
        }

        let mut body = b"WAVE".to_vec();
        let mut format = Vec::new();
        format.extend_from_slice(&1_u16.to_le_bytes());
        format.extend_from_slice(&1_u16.to_le_bytes());
        format.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        format.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
        format.extend_from_slice(&2_u16.to_le_bytes());
        format.extend_from_slice(&16_u16.to_le_bytes());
        append_riff_chunk(&mut body, *b"fmt ", &format);
        append_riff_chunk(&mut body, *b"data", &data);

        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes());
        wav.extend_from_slice(&body);
        wav
    }

    fn append_riff_chunk(output: &mut Vec<u8>, id: [u8; 4], data: &[u8]) {
        output.extend_from_slice(&id);
        output.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_le_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0);
        }
    }
}
