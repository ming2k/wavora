use flux::{Canvas, GradientStop, rgba};
use iris::PaintHost;
use std::sync::{Arc, RwLock};

use wavora_media::SPECTRUM_BANDS;

#[derive(Debug, Clone, Copy)]
pub struct VisualPreset {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
}

pub const PRESETS: [VisualPreset; 4] = [
    VisualPreset {
        name: "Aurora",
        subtitle: "薄荷极光",
        accent: [0, 245, 212],
        secondary: [115, 167, 255],
    },
    VisualPreset {
        name: "Ember",
        subtitle: "余烬脉冲",
        accent: [255, 83, 103],
        secondary: [244, 210, 138],
    },
    VisualPreset {
        name: "Nocturne",
        subtitle: "深海夜曲",
        accent: [127, 216, 255],
        secondary: [102, 92, 255],
    },
    VisualPreset {
        name: "Champagne",
        subtitle: "香槟星尘",
        accent: [244, 210, 138],
        secondary: [156, 255, 223],
    },
];

#[derive(Debug, Clone)]
pub struct VisualState {
    pub width: f32,
    pub height: f32,
    pub elapsed: f32,
    pub position_ratio: f32,
    pub playing: bool,
    pub energy: f32,
    pub spectrum: [f32; SPECTRUM_BANDS],
    pub preset: usize,
}

impl Default for VisualState {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 800.0,
            elapsed: 0.0,
            position_ratio: 0.0,
            playing: false,
            energy: 0.0,
            spectrum: [0.0; SPECTRUM_BANDS],
            preset: 0,
        }
    }
}

pub type SharedVisualState = Arc<RwLock<VisualState>>;

#[must_use]
pub fn shared_state(preset: usize) -> SharedVisualState {
    Arc::new(RwLock::new(VisualState {
        preset: preset % PRESETS.len(),
        ..VisualState::default()
    }))
}

#[allow(unsafe_code, clippy::needless_pass_by_value)]
pub fn paint(host: PaintHost, state: &SharedVisualState) {
    let snapshot = state
        .read()
        .map_or_else(|_| VisualState::default(), |state| state.clone());
    let preset = PRESETS[snapshot.preset % PRESETS.len()];
    let scale = host.scale().max(1.0);
    let canvas = unsafe {
        // SAFETY: iris provides a live canvas for the duration of this callback.
        Canvas::borrow_raw(host.canvas().cast::<flux::sys::flux_canvas>())
    };
    canvas.save();
    canvas.scale(scale, scale);

    let width = snapshot.width.max(1.0);
    let height = snapshot.height.max(1.0);
    canvas.fill_rect_linear_gradient(
        (0.0, 0.0, width, height),
        (0.0, 0.0),
        (width, height),
        &[
            GradientStop::new(0.0, rgba(3, 5, 8, 255)),
            GradientStop::new(0.55, rgba(6, 9, 14, 255)),
            GradientStop::new(1.0, rgba(2, 4, 7, 255)),
        ],
    );

    draw_ambient_orbs(&canvas, width, height, &snapshot, preset);
    draw_star_field(&canvas, width, height, &snapshot, preset);
    draw_wave_field(&canvas, width, height, &snapshot, preset);

    canvas.restore();
}

fn draw_ambient_orbs(
    canvas: &Canvas,
    width: f32,
    height: f32,
    state: &VisualState,
    preset: VisualPreset,
) {
    let pulse = if state.playing {
        (state.elapsed * 1.7).sin().mul_add(0.12, 0.18) + state.energy * 0.7
    } else {
        0.25
    };
    let radius = width.min(height) * (0.43 + pulse * 0.015);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, width, height),
        (width * 0.56, height * 0.43),
        radius,
        &[
            GradientStop::new(
                0.0,
                rgba(preset.accent[0], preset.accent[1], preset.accent[2], 30),
            ),
            GradientStop::new(
                0.34,
                rgba(preset.accent[0], preset.accent[1], preset.accent[2], 17),
            ),
            GradientStop::new(
                0.72,
                rgba(
                    preset.secondary[0],
                    preset.secondary[1],
                    preset.secondary[2],
                    5,
                ),
            ),
            GradientStop::new(
                1.0,
                rgba(preset.accent[0], preset.accent[1], preset.accent[2], 0),
            ),
        ],
    );

    let satellite = width.min(height) * 0.26;
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, width, height),
        (width * 0.84, height * 0.14),
        satellite,
        &[
            GradientStop::new(
                0.0,
                rgba(
                    preset.secondary[0],
                    preset.secondary[1],
                    preset.secondary[2],
                    15,
                ),
            ),
            GradientStop::new(
                1.0,
                rgba(
                    preset.secondary[0],
                    preset.secondary[1],
                    preset.secondary[2],
                    0,
                ),
            ),
        ],
    );
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn draw_star_field(
    canvas: &Canvas,
    width: f32,
    height: f32,
    state: &VisualState,
    preset: VisualPreset,
) {
    let drift = if state.playing {
        state.elapsed * (3.5 + state.energy * 5.5)
    } else {
        0.0
    };
    for index in 0_u32..180 {
        let x = hash01(index.wrapping_mul(0x9E37_79B9)) * width;
        let base_y = hash01(index.wrapping_mul(0x85EB_CA6B).wrapping_add(17)) * height;
        let y = (base_y + drift * (0.08 + hash01(index + 91) * 0.28)) % height;
        let shimmer = (state.elapsed * (0.45 + hash01(index + 7)) + hash01(index) * 8.0)
            .sin()
            .mul_add(0.5, 0.5);
        let size = 0.8 + hash01(index + 31) * 2.2;
        let alpha = (18.0 + shimmer * 76.0) as u8;
        let tint = if index % 5 == 0 {
            preset.accent
        } else {
            [214, 232, 238]
        };
        canvas.fill_rrect(
            x,
            y,
            size,
            size,
            size * 0.5,
            rgba(tint[0], tint[1], tint[2], alpha),
        );
    }
}

fn draw_wave_field(
    canvas: &Canvas,
    width: f32,
    height: f32,
    state: &VisualState,
    preset: VisualPreset,
) {
    let baseline = height * 0.73;
    let energy = if state.playing { 1.0 } else { 0.24 };
    let bars = 84_u16;
    let gap = 3.0;
    let bar_width = ((width * 0.62) / f32::from(bars) - gap).max(1.0);
    let start_x = width * 0.25;
    for index in 0..bars {
        let normalized = f32::from(index) / f32::from(bars);
        let envelope = (normalized * std::f32::consts::PI).sin().powf(0.72);
        let phase = state.elapsed * 2.8 + normalized * 22.0 + state.position_ratio * 9.0;
        let synthetic = (phase.sin() * 0.5 + (phase * 0.37).cos() * 0.5).abs();
        let spectrum_scaled = usize::from(index) * (SPECTRUM_BANDS - 1);
        let spectrum_denominator = usize::from(bars - 1);
        let lower = spectrum_scaled / spectrum_denominator;
        let upper = (lower + 1).min(SPECTRUM_BANDS - 1);
        let remainder = u16::try_from(spectrum_scaled % spectrum_denominator).unwrap_or_default();
        let blend = f32::from(remainder) / f32::from(bars - 1);
        let measured = state.spectrum[lower] * (1.0 - blend) + state.spectrum[upper] * blend;
        let amplitude = if state.playing {
            measured.mul_add(0.86, synthetic * 0.14)
        } else {
            synthetic * 0.24
        };
        let bar_height = 2.0 + envelope * amplitude * 72.0 * energy;
        canvas.fill_rrect(
            start_x + f32::from(index) * (bar_width + gap),
            baseline - bar_height * 0.5,
            bar_width,
            bar_height,
            bar_width * 0.5,
            rgba(preset.accent[0], preset.accent[1], preset.accent[2], 30),
        );
    }
}

fn hash01(mut value: u32) -> f32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    let high = u16::try_from(value >> 16).unwrap_or_default();
    f32::from(high) / f32::from(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic_and_normalized() {
        for seed in [0, 1, 42, u32::MAX] {
            let first = hash01(seed);
            assert_eq!(first.to_bits(), hash01(seed).to_bits());
            assert!((0.0..=1.0).contains(&first));
        }
    }
}
