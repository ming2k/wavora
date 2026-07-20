use flux::{Device, Format, Image};
use image::imageops::FilterType;
use image::{ImageReader, Limits};
use iris::{PaintHost, request_animation_frame};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::hash::Hash;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use wavora_media::load_artwork;

const ARTWORK_TEXTURE_SIZE: u32 = 320;
const GALLERY_ARTWORK_TEXTURE_SIZE: u32 = 256;
/// Combined upload + submit budget per gallery paint; keeps one frame from
/// monopolizing the GPU or the decode queue when a large collection opens.
const GALLERY_LOOKUPS_PER_FRAME: usize = 2;
const MAX_DECODE_DIMENSION: u32 = 8_192;
const MAX_DECODE_ALLOCATION: u64 = 96 * 1024 * 1024;

/// Background artwork decode worker.
///
/// Cover discovery plus JPEG/PNG decode plus the Lanczos3 resize cost tens
/// of milliseconds per cover (measured ~45 ms average, >140 ms worst case on
/// a real library) — far over the 16.7 ms frame budget, so running them in
/// the paint callback visibly stutters any animation on screen. Caches
/// submit jobs here and only perform the sub-millisecond GPU upload once
/// pixels are ready.
struct ArtworkJob {
    id: u64,
    uri: String,
    size: u32,
    cancel: Arc<AtomicBool>,
}

type DecodedArtwork = Option<(u32, u32, Vec<u8>)>;

struct ArtworkWorker {
    submit: std::sync::mpsc::Sender<ArtworkJob>,
    completed: Arc<Mutex<HashMap<u64, DecodedArtwork>>>,
    next_id: AtomicU64,
}

impl ArtworkWorker {
    fn submit(&self, uri: &str, size: u32, cancel: Arc<AtomicBool>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.submit.send(ArtworkJob {
            id,
            uri: uri.to_owned(),
            size,
            cancel,
        });
        id
    }

    /// Returns the finished decode for `id` (removing it), or `None` while
    /// the job is still queued or decoding.
    fn take(&self, id: u64) -> Option<DecodedArtwork> {
        self.completed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id)
    }
}

fn artwork_worker() -> &'static ArtworkWorker {
    static WORKER: OnceLock<ArtworkWorker> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (submit, receive) = std::sync::mpsc::channel::<ArtworkJob>();
        let completed = Arc::new(Mutex::new(HashMap::new()));
        let sink = Arc::clone(&completed);
        std::thread::Builder::new()
            .name("wavora-artwork".to_owned())
            .spawn(move || {
                while let Ok(job) = receive.recv() {
                    let decoded = load_artwork(&job.uri)
                        .and_then(|artwork| decode_artwork(&artwork.bytes, job.size));
                    // A cancelled job (selection changed mid-decode) drops
                    // its pixels instead of parking them in the map forever.
                    if !job.cancel.load(Ordering::Relaxed) {
                        sink.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .insert(job.id, decoded);
                    }
                }
            })
            .expect("spawn artwork worker");
        ArtworkWorker {
            submit,
            completed,
            next_id: AtomicU64::new(1),
        }
    })
}

/// Bridges cover discovery in the paint callback with Lens's next UI frame.
/// A failed lookup is cached for the current URI so missing artwork never
/// causes per-frame filesystem or metadata work.
pub struct ArtworkCache {
    desired_uri: Option<String>,
    loaded_uri: Option<String>,
    texture: Option<Image>,
    dirty: bool,
    pending: Option<(u64, String, Arc<AtomicBool>)>,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        Self {
            desired_uri: None,
            loaded_uri: None,
            texture: None,
            dirty: true,
            pending: None,
        }
    }
}

impl ArtworkCache {
    /// Selects the current track and returns its live Flux texture, if ready.
    pub fn select(&mut self, uri: Option<&str>) -> Option<*mut c_void> {
        if self.desired_uri.as_deref() != uri {
            self.desired_uri = uri.map(str::to_owned);
            self.dirty = true;
        }
        (self.loaded_uri == self.desired_uri)
            .then_some(self.texture.as_ref())
            .flatten()
            .map(|image| image.as_raw().cast())
    }

    /// Returns the prepared current texture without changing the selection.
    #[must_use]
    pub fn texture(&self) -> Option<*mut c_void> {
        (self.loaded_uri == self.desired_uri)
            .then_some(self.texture.as_ref())
            .flatten()
            .map(|image| image.as_raw().cast())
    }

    /// Picks up the finished decode and uploads it, and queues a background
    /// decode when the selection changed. All expensive work (metadata probe,
    /// image decode, resize) happens on the artwork worker thread.
    #[allow(unsafe_code)]
    pub fn prepare(&mut self, host: &PaintHost) {
        let worker = artwork_worker();
        if let Some((id, uri, cancel)) = self.pending.take() {
            match worker.take(id) {
                Some(decoded) => {
                    if self.desired_uri.as_deref() == Some(uri.as_str())
                        && let Some((width, height, pixels)) = decoded
                    {
                        // SAFETY: Iris owns this device and guarantees it
                        // remains live for the entire paint callback. The
                        // borrowed wrapper never releases it.
                        let device = unsafe { Device::borrow_raw(host.device().cast()) };
                        self.texture = Image::from_bytes(
                            &device,
                            width,
                            height,
                            Format::FLUX_FORMAT_RGBA8_SRGB,
                            &pixels,
                        )
                        .ok();
                        if self.texture.is_some() {
                            request_animation_frame();
                        }
                    }
                }
                None => self.pending = Some((id, uri, cancel)),
            }
        }
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.texture = None;
        self.loaded_uri = self.desired_uri.clone();
        if let Some((_, _, cancel)) = self.pending.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        let Some(uri) = self.desired_uri.as_deref() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let id = worker.submit(uri, ARTWORK_TEXTURE_SIZE, Arc::clone(&cancel));
        self.pending = Some((id, uri.to_owned(), cancel));
    }
}

#[derive(Default)]
struct GalleryEntry {
    candidates: Vec<String>,
    texture: Option<Image>,
    next_candidate: usize,
    prepared: bool,
    pending: Option<(u64, usize, Arc<AtomicBool>)>,
}

/// Bounded-work texture cache for covers in lists and galleries.
///
/// Each entry can supply several ordered candidates, allowing a playlist card
/// to fall through to a later track when the first track has no artwork.
pub struct ArtworkGallery<Key> {
    entries: HashMap<Key, GalleryEntry>,
    active: bool,
}

impl<Key> Default for ArtworkGallery<Key> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            active: false,
        }
    }
}

impl<Key> ArtworkGallery<Key>
where
    Key: Copy + Eq + Hash,
{
    /// Reconciles the active artwork requests and caches failed lookups.
    pub fn synchronize<'a>(&mut self, requests: impl IntoIterator<Item = (Key, &'a [String])>) {
        let requests = requests.into_iter().collect::<Vec<_>>();
        let desired = requests.iter().map(|(key, _)| *key).collect::<HashSet<_>>();
        self.entries.retain(|key, entry| {
            let keep = desired.contains(key);
            if !keep && let Some((_, _, cancel)) = entry.pending.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            keep
        });
        for (key, candidates) in requests {
            let entry = self.entries.entry(key).or_default();
            if entry.candidates != candidates {
                entry.candidates = candidates.to_vec();
                entry.texture = None;
                entry.next_candidate = 0;
                entry.prepared = false;
                if let Some((_, _, cancel)) = entry.pending.take() {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    pub const fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    #[must_use]
    pub fn texture(&self, key: Key) -> Option<*mut c_void> {
        self.entries
            .get(&key)
            .and_then(|entry| entry.texture.as_ref())
            .map(|image| image.as_raw().cast())
    }

    /// Uploads finished decodes and submits new background decodes, both
    /// bounded per paint callback so a large collection opening cannot
    /// monopolize a frame.
    #[allow(unsafe_code)]
    pub fn prepare(&mut self, host: &PaintHost) {
        if !self.active {
            return;
        }
        let worker = artwork_worker();
        // SAFETY: Iris owns this device and keeps it live throughout paint.
        let device = unsafe { Device::borrow_raw(host.device().cast()) };
        let mut steps = 0;
        for entry in self.entries.values_mut() {
            if steps == GALLERY_LOOKUPS_PER_FRAME {
                break;
            }
            let Some((id, candidate_index, _)) = entry.pending else {
                continue;
            };
            let Some(decoded) = worker.take(id) else {
                continue;
            };
            entry.pending = None;
            entry.texture = decoded.and_then(|(width, height, pixels)| {
                Image::from_bytes(
                    &device,
                    width,
                    height,
                    Format::FLUX_FORMAT_RGBA8_SRGB,
                    &pixels,
                )
                .ok()
            });
            entry.next_candidate = candidate_index + 1;
            entry.prepared =
                entry.texture.is_some() || entry.next_candidate == entry.candidates.len();
            steps += 1;
        }
        for entry in self
            .entries
            .values_mut()
            .filter(|entry| !entry.prepared && entry.pending.is_none())
        {
            if steps == GALLERY_LOOKUPS_PER_FRAME {
                break;
            }
            let Some(uri) = entry.candidates.get(entry.next_candidate) else {
                entry.prepared = true;
                continue;
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let id = worker.submit(uri, GALLERY_ARTWORK_TEXTURE_SIZE, Arc::clone(&cancel));
            entry.pending = Some((id, entry.next_candidate, cancel));
            steps += 1;
        }
        if steps > 0 || self.entries.values().any(|entry| entry.pending.is_some()) {
            request_animation_frame();
        }
    }
}

fn decode_artwork(encoded: &[u8], size: u32) -> Option<(u32, u32, Vec<u8>)> {
    let mut reader = ImageReader::new(Cursor::new(encoded))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIMENSION);
    limits.max_image_height = Some(MAX_DECODE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOCATION);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let cover = decoded.resize_to_fill(size, size, FilterType::Lanczos3);
    let rgba = cover.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some((width, height, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn decoded_artwork_is_square_and_bounded() {
        let source = DynamicImage::ImageRgba8(RgbaImage::new(640, 320));
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode png");
        let (width, height, pixels) =
            decode_artwork(encoded.get_ref(), ARTWORK_TEXTURE_SIZE).expect("decode artwork");
        assert_eq!(
            (width, height),
            (ARTWORK_TEXTURE_SIZE, ARTWORK_TEXTURE_SIZE)
        );
        assert_eq!(pixels.len(), (width * height * 4) as usize);
    }

    #[test]
    fn artwork_worker_decodes_submitted_image_in_background() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wavora-artwork-job-{unique}.png"));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(48, 48, image::Rgba([9, 8, 7, 255])))
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode png");
        std::fs::write(&path, encoded.into_inner()).expect("write png fixture");
        let uri = wavora_media::path_to_file_uri(&path);
        let cancel = Arc::new(AtomicBool::new(false));
        let id = artwork_worker().submit(&uri, 64, cancel);
        let deadline = Instant::now() + Duration::from_secs(5);
        let decoded = loop {
            if let Some(decoded) = artwork_worker().take(id) {
                break decoded;
            }
            assert!(Instant::now() < deadline, "worker never finished the job");
            std::thread::sleep(Duration::from_millis(5));
        };
        let _ = std::fs::remove_file(path);
        let (width, height, pixels) = decoded.expect("decode fixture");
        assert_eq!((width, height), (64, 64));
        assert_eq!(pixels.len(), 64 * 64 * 4);
        assert_eq!(&pixels[0..4], &[9, 8, 7, 255]);
    }
}
