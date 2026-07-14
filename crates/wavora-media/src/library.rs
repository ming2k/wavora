use crate::path_to_file_uri;
use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, Source};
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use walkdir::WalkDir;
use wavora_core::Track;

const AUDIO_EXTENSIONS: &[&str] = &["aac", "flac", "m4a", "mp3", "oga", "ogg", "wav"];

#[derive(Debug)]
enum LibraryCommand {
    Scan(PathBuf),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum LibraryEvent {
    ScanStarted(PathBuf),
    Track(Track),
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
    let mut known = HashSet::new();
    while !shutdown.load(Ordering::Acquire) {
        let Ok(command) = commands.recv() else {
            break;
        };
        match command {
            LibraryCommand::Scan(root) => {
                let _ = events.send(LibraryEvent::ScanStarted(root.clone()));
                let (discovered, rejected) = scan_root(&root, &mut known, events, shutdown);
                let _ = events.send(LibraryEvent::ScanFinished {
                    root,
                    discovered,
                    rejected,
                });
            }
            LibraryCommand::Shutdown => break,
        }
    }
}

fn scan_root(
    root: &Path,
    known: &mut HashSet<PathBuf>,
    events: &Sender<LibraryEvent>,
    shutdown: &AtomicBool,
) -> (usize, usize) {
    if root.is_file() {
        return match scan_path(root.to_owned(), known, events, shutdown) {
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
        match scan_path(entry.into_path(), known, events, shutdown) {
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
    known: &mut HashSet<PathBuf>,
    events: &Sender<LibraryEvent>,
    shutdown: &AtomicBool,
) -> ScanResult {
    if shutdown.load(Ordering::Acquire) || !is_supported_audio(&path) {
        return ScanResult::Ignored;
    }
    let canonical = path.canonicalize().unwrap_or(path);
    if !known.insert(canonical.clone()) {
        return ScanResult::Ignored;
    }
    let Ok(file) = File::open(&canonical) else {
        return ScanResult::Rejected;
    };
    let Ok(decoder) = Decoder::try_from(file) else {
        return ScanResult::Rejected;
    };
    let uri = path_to_file_uri(&canonical);
    let mut track = Track::from_path(uri, &canonical);
    track.duration_ms = decoder
        .total_duration()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default();
    codec_label(&canonical).clone_into(&mut track.codec);
    if shutdown.load(Ordering::Acquire) {
        return ScanResult::Ignored;
    }
    if events.send(LibraryEvent::Track(track)).is_ok() {
        ScanResult::Added
    } else {
        ScanResult::Ignored
    }
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
}
