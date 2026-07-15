use flux::{Device, Format, Image};
use image::imageops::FilterType;
use image::{ImageReader, Limits};
use iris::{PaintHost, request_animation_frame};
use std::ffi::c_void;
use std::io::Cursor;
use wavora_media::load_artwork;

const ARTWORK_TEXTURE_SIZE: u32 = 320;
const MAX_DECODE_DIMENSION: u32 = 8_192;
const MAX_DECODE_ALLOCATION: u64 = 96 * 1024 * 1024;

/// Bridges cover discovery in the paint callback with Lens's next UI frame.
/// A failed lookup is cached for the current URI so missing artwork never
/// causes per-frame filesystem or metadata work.
pub struct ArtworkCache {
    desired_uri: Option<String>,
    loaded_uri: Option<String>,
    texture: Option<Image>,
    dirty: bool,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        Self {
            desired_uri: None,
            loaded_uri: None,
            texture: None,
            dirty: true,
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

    /// Resolves and uploads a changed cover while Iris exposes its GPU device.
    #[allow(unsafe_code)]
    pub fn prepare(&mut self, host: &PaintHost) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.texture = None;
        self.loaded_uri = self.desired_uri.clone();
        let Some(uri) = self.desired_uri.as_deref() else {
            return;
        };
        let Some(artwork) = load_artwork(uri) else {
            return;
        };
        let Some((width, height, pixels)) = decode_artwork(&artwork.bytes) else {
            return;
        };
        // SAFETY: Iris owns this device and guarantees it remains live for the
        // entire paint callback. The borrowed wrapper never releases it.
        let device = unsafe { Device::borrow_raw(host.device().cast()) };
        let Ok(texture) = Image::from_bytes(
            &device,
            width,
            height,
            Format::FLUX_FORMAT_RGBA8_SRGB,
            &pixels,
        ) else {
            return;
        };
        self.texture = Some(texture);
        request_animation_frame();
    }
}

fn decode_artwork(encoded: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut reader = ImageReader::new(Cursor::new(encoded))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIMENSION);
    limits.max_image_height = Some(MAX_DECODE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOCATION);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let cover = decoded.resize_to_fill(
        ARTWORK_TEXTURE_SIZE,
        ARTWORK_TEXTURE_SIZE,
        FilterType::Lanczos3,
    );
    let rgba = cover.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some((width, height, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbaImage};

    #[test]
    fn decoded_artwork_is_square_and_bounded() {
        let source = DynamicImage::ImageRgba8(RgbaImage::new(640, 320));
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode png");
        let (width, height, pixels) = decode_artwork(encoded.get_ref()).expect("decode artwork");
        assert_eq!(
            (width, height),
            (ARTWORK_TEXTURE_SIZE, ARTWORK_TEXTURE_SIZE)
        );
        assert_eq!(pixels.len(), (width * height * 4) as usize);
    }
}
