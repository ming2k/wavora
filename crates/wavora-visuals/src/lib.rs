//! Audio-reactive visual engines rendered with Flux.
//!
//! Presets share a feature contract and transition model, but each preset has
//! its own composition. Adding an effect does not require touching playback,
//! application state, or the UI toolkit.

use flux::{Canvas, GradientStop, rgba};
use iris::PaintHost;
use std::sync::{Arc, RwLock};
use wavora_audio_analysis::{AudioFeatures, SPECTRUM_BANDS};

/// Rendering strategy used by a visual preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualKind {
    ParticleVeil,
    PulseTunnel,
    OrbitalCore,
    SpectralVoid,
    VinylHalo,
    StarRiver,
}

/// Metadata and palette for one selectable visual engine.
#[derive(Debug, Clone, Copy)]
pub struct VisualPreset {
    pub kind: VisualKind,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
}

/// Built-in effects. Their compositions intentionally differ; these are not
/// colour variants of a shared waveform.
pub const PRESETS: [VisualPreset; 6] = [
    VisualPreset {
        kind: VisualKind::ParticleVeil,
        accent: [112, 246, 218],
        secondary: [108, 149, 255],
    },
    VisualPreset {
        kind: VisualKind::PulseTunnel,
        accent: [255, 92, 112],
        secondary: [244, 210, 138],
    },
    VisualPreset {
        kind: VisualKind::OrbitalCore,
        accent: [104, 205, 255],
        secondary: [139, 109, 255],
    },
    VisualPreset {
        kind: VisualKind::SpectralVoid,
        accent: [174, 111, 255],
        secondary: [72, 218, 255],
    },
    VisualPreset {
        kind: VisualKind::VinylHalo,
        accent: [244, 210, 138],
        secondary: [255, 126, 91],
    },
    VisualPreset {
        kind: VisualKind::StarRiver,
        accent: [125, 232, 203],
        secondary: [126, 151, 255],
    },
];

/// User-facing response controls shared by all compositions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualTuning {
    pub intensity: f32,
    pub motion: f32,
    pub depth: f32,
    pub glow: f32,
}

impl Default for VisualTuning {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            motion: 1.0,
            depth: 1.0,
            glow: 0.9,
        }
    }
}

impl VisualTuning {
    #[must_use]
    pub fn normalized(self) -> Self {
        Self {
            intensity: self.intensity.clamp(0.45, 1.75),
            motion: self.motion.clamp(0.35, 1.65),
            depth: self.depth.clamp(0.50, 1.50),
            glow: self.glow.clamp(0.25, 1.50),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StageViewport {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl StageViewport {
    fn is_visible(self) -> bool {
        self.width >= 80.0 && self.height >= 80.0
    }
}

/// Render-thread snapshot. Audio fields are smoothed here rather than in the
/// decoder so every visual backend observes the same attack/release behavior.
#[derive(Debug, Clone)]
pub struct VisualState {
    pub width: f32,
    pub height: f32,
    pub elapsed: f32,
    pub position_ratio: f32,
    pub playing: bool,
    pub features: AudioFeatures,
    pub preset: usize,
    pub tuning: VisualTuning,
    pub stage_active: bool,
    viewport: StageViewport,
    transition: f32,
}

impl Default for VisualState {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 800.0,
            elapsed: 0.0,
            position_ratio: 0.0,
            playing: false,
            features: AudioFeatures::default(),
            preset: 0,
            tuning: VisualTuning::default(),
            stage_active: false,
            viewport: StageViewport::default(),
            transition: 0.0,
        }
    }
}

impl VisualState {
    /// Advances animation, transitions, and feature envelopes.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt: f32,
        width: f32,
        height: f32,
        playing: bool,
        position_ratio: f32,
        preset: usize,
        measured: AudioFeatures,
        tuning: VisualTuning,
        stage_active: bool,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        self.tuning = tuning.normalized();
        self.elapsed += dt * self.tuning.motion;
        self.position_ratio = position_ratio.clamp(0.0, 1.0);
        self.playing = playing;
        self.stage_active = stage_active;
        let preset = preset % PRESETS.len();
        if preset != self.preset {
            self.preset = preset;
            self.transition = 1.0;
        }
        self.transition = (self.transition - dt * 1.45).max(0.0);

        let mut target = if playing {
            measured
        } else {
            AudioFeatures::default()
        };
        apply_intensity(&mut target, self.tuning.intensity);
        smooth_features(&mut self.features, &target, dt);
    }

    pub fn set_stage_viewport(&mut self, viewport: Option<(f32, f32, f32, f32)>) {
        self.viewport = viewport.map_or_else(StageViewport::default, |(x, y, width, height)| {
            StageViewport {
                x,
                y,
                width: width.max(0.0),
                height: height.max(0.0),
            }
        });
    }

    /// Whether host-owned motion still needs an active-rate follow-up frame.
    ///
    /// Playback continuously advances the scene. After playback or a preset
    /// transition stops, frames continue only until the visible envelopes
    /// have settled, allowing Iris to return to its low-power idle cadence.
    #[must_use]
    pub fn needs_animation_frame(&self) -> bool {
        self.playing || self.transition > 0.0 || !features_are_settled(&self.features)
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

/// Paints the current visual snapshot into Iris's live Flux canvas.
#[allow(unsafe_code, clippy::needless_pass_by_value)]
pub fn paint(host: PaintHost, state: &SharedVisualState) {
    let snapshot = state
        .read()
        .map_or_else(|_| VisualState::default(), |state| state.clone());
    let scale = host.scale().max(1.0);
    let canvas = unsafe {
        // SAFETY: Iris owns this live canvas and keeps it valid throughout the
        // paint callback. The borrowed Flux handle never destroys it.
        Canvas::borrow_raw(host.canvas().cast::<flux::sys::flux_canvas>())
    };
    canvas.save();
    canvas.scale(scale, scale);
    paint_scene(&canvas, &snapshot);
    canvas.restore();
}

fn smooth_features(visible: &mut AudioFeatures, target: &AudioFeatures, dt: f32) {
    visible.energy = envelope(visible.energy, target.energy, 14.0, 4.0, dt);
    visible.rms = envelope(visible.rms, target.rms, 14.0, 4.0, dt);
    visible.peak = envelope(visible.peak, target.peak, 18.0, 5.0, dt);
    visible.loudness_db = envelope(visible.loudness_db, target.loudness_db, 10.0, 3.0, dt);
    visible.bass = envelope(visible.bass, target.bass, 16.0, 4.0, dt);
    visible.mid = envelope(visible.mid, target.mid, 12.0, 3.5, dt);
    visible.treble = envelope(visible.treble, target.treble, 18.0, 5.0, dt);
    visible.spectral_centroid_hz = envelope(
        visible.spectral_centroid_hz,
        target.spectral_centroid_hz,
        8.0,
        3.0,
        dt,
    );
    visible.dominant_frequency_hz = envelope(
        visible.dominant_frequency_hz,
        target.dominant_frequency_hz,
        10.0,
        4.0,
        dt,
    );
    visible.pitch_hz = envelope(visible.pitch_hz, target.pitch_hz, 9.0, 3.0, dt);
    visible.pitch_confidence = envelope(
        visible.pitch_confidence,
        target.pitch_confidence,
        12.0,
        4.0,
        dt,
    );
    visible.spectral_flux = envelope(visible.spectral_flux, target.spectral_flux, 20.0, 6.0, dt);
    visible.onset = envelope(visible.onset, target.onset, 32.0, 7.0, dt);
    for (visible, target) in visible.spectrum.iter_mut().zip(target.spectrum) {
        *visible = envelope(*visible, target, 18.0, 4.5, dt);
    }
}

fn apply_intensity(features: &mut AudioFeatures, intensity: f32) {
    let scale = |value: &mut f32| *value = (*value * intensity).clamp(0.0, 1.0);
    scale(&mut features.energy);
    scale(&mut features.rms);
    scale(&mut features.peak);
    scale(&mut features.bass);
    scale(&mut features.mid);
    scale(&mut features.treble);
    scale(&mut features.spectral_flux);
    scale(&mut features.onset);
    for band in &mut features.spectrum {
        scale(band);
    }
}

fn envelope(current: f32, target: f32, attack: f32, release: f32, dt: f32) -> f32 {
    let speed = if target > current { attack } else { release };
    let blend = 1.0 - (-speed * dt).exp();
    current + (target - current) * blend
}

fn features_are_settled(features: &AudioFeatures) -> bool {
    const LEVEL_EPSILON: f32 = 0.000_5;
    const FREQUENCY_EPSILON_HZ: f32 = 0.5;
    const LOUDNESS_EPSILON_DB: f32 = 0.05;

    (features.loudness_db + 120.0).abs() < LOUDNESS_EPSILON_DB
        && [
            features.energy,
            features.rms,
            features.peak,
            features.bass,
            features.mid,
            features.treble,
            features.pitch_confidence,
            features.spectral_flux,
            features.onset,
        ]
        .into_iter()
        .all(|value| value.abs() < LEVEL_EPSILON)
        && [
            features.spectral_centroid_hz,
            features.dominant_frequency_hz,
            features.pitch_hz,
        ]
        .into_iter()
        .all(|value| value.abs() < FREQUENCY_EPSILON_HZ)
        && features
            .spectrum
            .iter()
            .all(|value| value.abs() < LEVEL_EPSILON)
}

fn paint_scene(canvas: &Canvas, state: &VisualState) {
    let preset = PRESETS[state.preset % PRESETS.len()];
    draw_app_backdrop(canvas, state, preset);
    if state.stage_active && state.viewport.is_visible() {
        let viewport = state.viewport;
        let mut local = state.clone();
        local.width = viewport.width;
        local.height = viewport.height;
        local.viewport = StageViewport::default();
        canvas.save();
        canvas.translate(viewport.x, viewport.y);
        draw_stage(canvas, &local, preset);
        canvas.restore();
    } else {
        draw_stage(canvas, state, preset);
    }
}

fn draw_stage(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    draw_backdrop(canvas, state, preset);
    match preset.kind {
        VisualKind::ParticleVeil => draw_particle_veil(canvas, state, preset),
        VisualKind::PulseTunnel => draw_pulse_tunnel(canvas, state, preset),
        VisualKind::OrbitalCore => draw_orbital_core(canvas, state, preset),
        VisualKind::SpectralVoid => draw_spectral_void(canvas, state, preset),
        VisualKind::VinylHalo => draw_vinyl_halo(canvas, state, preset),
        VisualKind::StarRiver => draw_star_river(canvas, state, preset),
    }
    draw_transition(canvas, state, preset);
}

fn draw_app_backdrop(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    canvas.fill_rect_linear_gradient(
        (0.0, 0.0, state.width, state.height),
        (0.0, 0.0),
        (state.width, state.height),
        &[
            GradientStop::new(0.0, rgba(2, 4, 7, 255)),
            GradientStop::new(0.58, rgba(4, 6, 11, 255)),
            GradientStop::new(1.0, rgba(1, 2, 5, 255)),
        ],
    );
    let glow = state.tuning.glow;
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (state.width * 0.55, state.height * 0.42),
        state.width.min(state.height) * 0.72,
        &[
            GradientStop::new(0.0, color(preset.accent, alpha_u8(9.0 * glow))),
            GradientStop::new(0.42, color(preset.secondary, alpha_u8(5.0 * glow))),
            GradientStop::new(1.0, color(preset.accent, 0)),
        ],
    );
    for index in 0_u32..72 {
        let x = hash01(index * 47 + 5) * state.width;
        let y = hash01(index * 83 + 17) * state.height;
        let shimmer = (state.elapsed * 0.42 + hash01(index) * 8.0).sin() * 0.5 + 0.5;
        dot(
            canvas,
            x,
            y,
            0.55 + shimmer * 0.8,
            rgba(210, 226, 240, alpha_u8(5.0 + shimmer * 21.0)),
        );
    }
}

fn draw_backdrop(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let (width, height) = (state.width, state.height);
    canvas.fill_rect_linear_gradient(
        (0.0, 0.0, width, height),
        (0.0, 0.0),
        (width, height),
        &[
            GradientStop::new(0.0, rgba(2, 4, 7, 255)),
            GradientStop::new(0.52, rgba(6, 8, 13, 255)),
            GradientStop::new(1.0, rgba(1, 2, 5, 255)),
        ],
    );
    let center = match preset.kind {
        VisualKind::PulseTunnel => (width * 0.55, height * 0.45),
        VisualKind::SpectralVoid => (width * 0.53, height * 0.43),
        VisualKind::VinylHalo => (width * 0.56, height * 0.44),
        _ => (width * 0.56, height * 0.42),
    };
    let radius = width.min(height) * (0.48 + state.features.energy * 0.09);
    let glow = state.tuning.glow;
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, width, height),
        center,
        radius,
        &[
            GradientStop::new(0.0, color(preset.accent, alpha_u8(31.0 * glow))),
            GradientStop::new(0.38, color(preset.secondary, alpha_u8(16.0 * glow))),
            GradientStop::new(1.0, color(preset.accent, 0)),
        ],
    );
}

fn draw_particle_veil(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let width = state.width;
    let height = state.height;
    let center_x = width * 0.50;
    let center_y = height * 0.47;
    let field_w = width * 0.82;
    let field_h = height * 0.56;
    let cols = 34_u16;
    let rows = 22_u16;
    let pitch = (state.features.pitch_hz / 880.0).clamp(0.0, 1.0);
    let centroid = (state.features.spectral_centroid_hz / 8_000.0).clamp(0.0, 1.0);
    let depth = state.tuning.depth;
    let yaw = -0.24 + (state.elapsed * 0.13).sin() * 0.07 + centroid * 0.15;
    let yaw_cos = yaw.cos();
    let yaw_sin = yaw.sin();

    // A quiet depth field makes the folded surface read as a volume rather
    // than a flat equalizer without stealing attention from the UI.
    for index in 0_u32..150 {
        let orbit = hash01(index * 53 + 7);
        let phase = hash01(index * 89 + 19) * std::f32::consts::TAU
            + state.elapsed * (0.025 + orbit * 0.045);
        let radius_x = width * (0.16 + orbit * 0.37);
        let radius_y = height * (0.13 + hash01(index * 31) * 0.27);
        let x = center_x + phase.cos() * radius_x;
        let y = center_y + phase.sin() * radius_y;
        let shimmer = (state.elapsed * 0.8 + hash01(index) * 11.0).sin() * 0.5 + 0.5;
        dot(
            canvas,
            x,
            y,
            0.55 + shimmer * 1.15,
            color(
                if index % 4 == 0 {
                    preset.secondary
                } else {
                    preset.accent
                },
                alpha_u8(9.0 + shimmer * 38.0),
            ),
        );
    }

    for row in 0..rows {
        let v = f32::from(row) / f32::from(rows - 1);
        for col in 0..cols {
            let u = f32::from(col) / f32::from(cols - 1);
            let band = spectrum_at(&state.features.spectrum, u);
            let phase = state.elapsed * (0.42 + pitch * 0.82) + u * 8.6 + v * 4.8;
            let edge = (v * std::f32::consts::PI).sin().powf(0.58);
            let fold = phase.sin() * (0.16 + state.features.mid * 0.58) * edge
                + ((u + v) * 11.0 - state.elapsed * 0.34).cos()
                    * (0.05 + state.features.bass * 0.22);
            let local_x = (u - 0.5) * field_w;
            let local_y = (v - 0.5) * field_h
                + (u * std::f32::consts::TAU).cos() * state.features.bass * height * 0.035;
            let local_z = fold * field_w * 0.24 * depth;
            let rotated_x = local_x * yaw_cos + local_z * yaw_sin;
            let rotated_z = -local_x * yaw_sin + local_z * yaw_cos;
            let perspective = (1.0 / (1.0 - rotated_z / (field_w * 2.2))).clamp(0.76, 1.28);
            let x = center_x + rotated_x * perspective;
            let y = center_y + (local_y + rotated_z * 0.13) * perspective;
            let shimmer =
                (state.elapsed * 1.2 + hash01(u32::from(row) * 97 + u32::from(col))).sin() * 0.5
                    + 0.5;
            let size =
                (0.72 + band * 2.9 + state.features.onset * 1.45 + shimmer * 0.62) * perspective;
            let tint = if fold + centroid * 0.18 > 0.08 {
                preset.accent
            } else {
                preset.secondary
            };
            let alpha =
                alpha_u8(24.0 + band * 137.0 + shimmer * 38.0 + perspective.max(1.0) * 12.0);
            dot(canvas, x, y, size, color(tint, alpha));
        }
    }

    // Three translucent frequency ribbons tie the particle sculpture to the
    // live spectrum and echo the flowing visual language of the player shell.
    for layer in 0_u16..3 {
        let drive = [
            state.features.bass,
            state.features.mid,
            state.features.treble,
        ][usize::from(layer)];
        for index in 0_u16..96 {
            let u = f32::from(index) / 95.0;
            let band = spectrum_at(
                &state.features.spectrum,
                (u + f32::from(layer) * 0.17).fract(),
            );
            let x = width * (0.08 + u * 0.84);
            let baseline = height * (0.78 + f32::from(layer) * 0.035);
            let y = baseline
                + (u * (8.0 + f32::from(layer) * 2.1)
                    + state.elapsed * (0.36 + f32::from(layer) * 0.11))
                    .sin()
                    * (3.0 + drive * 12.0 + band * 6.0);
            dot(
                canvas,
                x,
                y,
                0.65 + band * 1.35,
                color(
                    if layer == 1 {
                        preset.secondary
                    } else {
                        preset.accent
                    },
                    alpha_u8(13.0 + band * 48.0 + drive * 25.0),
                ),
            );
        }
    }
}

fn draw_pulse_tunnel(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center_x = state.width * 0.55;
    let center_y = state.height * 0.44;
    let max_radius = state.width.min(state.height) * 0.47;
    let speed = 0.14 + state.features.energy * 0.18 + state.features.bass * 0.22;
    let twist = (state.features.spectral_centroid_hz / 16_000.0).clamp(0.0, 1.0);
    for index in 0_u16..16 {
        let depth = (f32::from(index) / 16.0 + state.elapsed * speed).fract();
        let eased = depth * depth;
        let radius = 10.0 + eased * max_radius * (1.0 - state.features.bass * 0.08);
        let alpha = alpha_u8((1.0 - depth) * 28.0 + depth * 86.0 + state.features.onset * 90.0);
        canvas.save();
        canvas.translate(center_x, center_y);
        canvas.rotate(state.elapsed * 0.05 + depth * twist * 0.22);
        canvas.scale(1.0, 0.58 + depth * 0.10);
        canvas.stroke_rrect(
            -radius,
            -radius,
            radius * 2.0,
            radius * 2.0,
            radius,
            color(
                if index % 3 == 0 {
                    preset.secondary
                } else {
                    preset.accent
                },
                alpha,
            ),
            0.7 + depth * 2.2 + state.features.onset * 1.6,
        );
        canvas.restore();
    }
    for index in 0_u32..120 {
        let phase = hash01(index * 29) * std::f32::consts::TAU;
        let depth = (hash01(index * 71 + 3) + state.elapsed * speed * 0.8).fract();
        let radius = depth * depth * max_radius;
        let x = center_x + phase.cos() * radius;
        let y = center_y + phase.sin() * radius * 0.62;
        let size = 0.7 + depth * 2.6 + state.features.bass * 1.8;
        dot(
            canvas,
            x,
            y,
            size,
            color(preset.accent, alpha_u8(18.0 + depth * 95.0)),
        );
    }
}

fn draw_orbital_core(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center_x = state.width * 0.56;
    let center_y = state.height * 0.43;
    let base = state.width.min(state.height) * (0.095 + state.features.bass * 0.018);
    let pitch_drive = (state.features.dominant_frequency_hz / 4_000.0).clamp(0.0, 1.0);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (center_x, center_y),
        base * 2.8,
        &[
            GradientStop::new(0.0, rgba(230, 247, 255, 210)),
            GradientStop::new(0.16, color(preset.accent, 190)),
            GradientStop::new(0.48, color(preset.secondary, 64)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );
    circle(canvas, center_x, center_y, base, rgba(8, 14, 24, 235));
    ring(
        canvas,
        center_x,
        center_y,
        base * 1.04,
        color(preset.accent, 150),
        1.4,
    );

    for orbit in 0_u16..3 {
        let radius = base * (1.75 + f32::from(orbit) * 0.75);
        let rotation = -0.48 + f32::from(orbit) * 0.52 + state.elapsed * 0.025;
        canvas.save();
        canvas.translate(center_x, center_y);
        canvas.rotate(rotation);
        canvas.scale(1.0, 0.33 + f32::from(orbit) * 0.055);
        canvas.stroke_rrect(
            -radius,
            -radius,
            radius * 2.0,
            radius * 2.0,
            radius,
            color(
                if orbit == 1 {
                    preset.secondary
                } else {
                    preset.accent
                },
                58,
            ),
            1.0,
        );
        canvas.restore();

        let count = 24_u16 + orbit * 8;
        for index in 0..count {
            let angle = f32::from(index) / f32::from(count) * std::f32::consts::TAU
                + state.elapsed * (0.16 + pitch_drive * 0.65) * (1.0 + f32::from(orbit) * 0.22);
            let local_x = angle.cos() * radius;
            let local_y = angle.sin() * radius * (0.33 + f32::from(orbit) * 0.055);
            let cos_r = rotation.cos();
            let sin_r = rotation.sin();
            let x = center_x + local_x * cos_r - local_y * sin_r;
            let y = center_y + local_x * sin_r + local_y * cos_r;
            let band = state.features.spectrum
                [(usize::from(index) + usize::from(orbit) * 7) % SPECTRUM_BANDS];
            let size = 0.8 + band * 3.0 + state.features.treble * 1.4;
            dot(
                canvas,
                x,
                y,
                size,
                color(preset.accent, alpha_u8(32.0 + band * 142.0)),
            );
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_spectral_void(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center_x = state.width * 0.54;
    let center_y = state.height * 0.43;
    let base = state.width.min(state.height) * (0.085 + state.features.peak * 0.018);
    let halo = base * (2.4 + state.features.onset * 0.9);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (center_x, center_y),
        halo,
        &[
            GradientStop::new(0.0, color(preset.accent, 0)),
            GradientStop::new(0.38, color(preset.accent, 20)),
            GradientStop::new(0.68, color(preset.secondary, 70)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );
    for (index, band) in state.features.spectrum.iter().copied().enumerate() {
        let angle =
            index as f32 / SPECTRUM_BANDS as f32 * std::f32::consts::TAU + state.elapsed * 0.035;
        let inner = base * 1.18;
        let length = 4.0 + band * base * 1.35 + state.features.onset * 8.0;
        canvas.save();
        canvas.translate(center_x, center_y);
        canvas.rotate(angle);
        canvas.fill_rrect(
            inner,
            -0.8,
            length,
            1.6,
            0.8,
            color(
                if index % 4 == 0 {
                    preset.secondary
                } else {
                    preset.accent
                },
                alpha_u8(24.0 + band * 135.0),
            ),
        );
        canvas.restore();
    }
    circle(canvas, center_x, center_y, base, rgba(0, 1, 4, 250));
    ring(
        canvas,
        center_x,
        center_y,
        base * (1.02 + state.features.onset * 0.05),
        color(
            preset.secondary,
            alpha_u8(90.0 + state.features.onset * 140.0),
        ),
        1.2 + state.features.onset * 3.0,
    );
    for index in 0_u32..150 {
        let x = hash01(index * 47 + 5) * state.width;
        let y = hash01(index * 83 + 17) * state.height;
        let dx = x - center_x;
        let dy = y - center_y;
        if dx * dx + dy * dy < halo * halo * 0.58 {
            continue;
        }
        let shimmer = (state.elapsed * 0.7 + hash01(index) * 9.0).sin() * 0.5 + 0.5;
        dot(
            canvas,
            x,
            y,
            0.7 + shimmer * 1.4,
            rgba(215, 225, 246, alpha_u8(12.0 + shimmer * 56.0)),
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_vinyl_halo(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center_x = state.width * 0.56;
    let center_y = state.height * 0.44;
    let radius = state.width.min(state.height) * (0.19 + state.features.bass * 0.012);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (center_x, center_y),
        radius * 1.5,
        &[
            GradientStop::new(0.0, color(preset.accent, 36)),
            GradientStop::new(0.68, color(preset.secondary, 22)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );
    circle(canvas, center_x, center_y, radius, rgba(7, 8, 11, 245));
    for groove in 0_u16..12 {
        let groove_radius = radius * (0.25 + f32::from(groove) * 0.061);
        ring(
            canvas,
            center_x,
            center_y,
            groove_radius,
            rgba(222, 226, 230, if groove % 3 == 0 { 34 } else { 16 }),
            if groove % 4 == 0 { 1.0 } else { 0.55 },
        );
    }
    let spin = state.elapsed * (0.38 + state.features.bass * 0.16);
    for (index, band) in state.features.spectrum.iter().copied().enumerate() {
        let angle = index as f32 / SPECTRUM_BANDS as f32 * std::f32::consts::TAU + spin;
        let length = 2.0 + band * radius * 0.20;
        canvas.save();
        canvas.translate(center_x, center_y);
        canvas.rotate(angle);
        canvas.fill_rrect(
            radius * 0.82,
            -1.0,
            length,
            2.0,
            1.0,
            color(
                if index % 2 == 0 {
                    preset.accent
                } else {
                    preset.secondary
                },
                alpha_u8(38.0 + band * 150.0),
            ),
        );
        canvas.restore();
    }
    circle(
        canvas,
        center_x,
        center_y,
        radius * 0.235,
        color(preset.accent, 210),
    );
    circle(
        canvas,
        center_x,
        center_y,
        radius * 0.072,
        rgba(8, 9, 12, 255),
    );
    ring(
        canvas,
        center_x,
        center_y,
        radius,
        color(preset.accent, 100),
        1.2,
    );
}

fn draw_star_river(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    for index in 0_u32..220 {
        let drift = state.elapsed * (2.0 + state.features.energy * 5.0);
        let x = (hash01(index * 31) * state.width + drift * (0.08 + hash01(index + 9) * 0.4))
            % state.width;
        let y = hash01(index * 73 + 11) * state.height;
        let shimmer =
            (state.elapsed * (0.35 + hash01(index + 5)) + hash01(index) * 7.0).sin() * 0.5 + 0.5;
        dot(
            canvas,
            x,
            y,
            0.7 + shimmer * 1.8,
            rgba(220, 237, 240, alpha_u8(15.0 + shimmer * 72.0)),
        );
    }

    let start_x = state.width * 0.12;
    let span = state.width * 0.82;
    let baseline = state.height * 0.48;
    let drives = [
        state.features.bass,
        state.features.mid,
        state.features.treble,
    ];
    for layer in 0_u16..3 {
        let drive = drives[usize::from(layer)];
        let tint = if layer == 1 {
            preset.secondary
        } else {
            preset.accent
        };
        for index in 0_u16..150 {
            let u = f32::from(index) / 149.0;
            let band = spectrum_at(
                &state.features.spectrum,
                (u + f32::from(layer) * 0.13).fract(),
            );
            let envelope = (u * std::f32::consts::PI).sin().powf(0.65);
            let wave = (u * (9.0 + f32::from(layer) * 3.0)
                + state.elapsed * (0.55 + f32::from(layer) * 0.18))
                .sin();
            let y = baseline
                + (f32::from(layer) - 1.0) * 34.0
                + wave * envelope * (12.0 + drive * 54.0 + band * 20.0);
            let x = start_x + u * span;
            let size = 0.9 + band * 2.4 + drive * 1.2;
            dot(
                canvas,
                x,
                y,
                size,
                color(tint, alpha_u8(28.0 + band * 105.0 + drive * 54.0)),
            );
        }
    }
}

fn draw_transition(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    if state.transition <= 0.0 {
        return;
    }
    let wave = (state.transition * std::f32::consts::PI).sin().max(0.0);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (state.width * 0.55, state.height * 0.43),
        state.width.min(state.height) * (0.18 + wave * 0.55),
        &[
            GradientStop::new(0.0, color(preset.accent, alpha_u8(wave * 52.0))),
            GradientStop::new(0.45, color(preset.secondary, alpha_u8(wave * 22.0))),
            GradientStop::new(1.0, color(preset.accent, 0)),
        ],
    );
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn spectrum_at(spectrum: &[f32; SPECTRUM_BANDS], normalized: f32) -> f32 {
    let scaled = normalized.clamp(0.0, 1.0) * (SPECTRUM_BANDS - 1) as f32;
    let lower = scaled.floor() as usize;
    let upper = (lower + 1).min(SPECTRUM_BANDS - 1);
    let blend = scaled - lower as f32;
    spectrum[lower] * (1.0 - blend) + spectrum[upper] * blend
}

fn color(rgb: [u8; 3], alpha: u8) -> u32 {
    rgba(rgb[0], rgb[1], rgb[2], alpha)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn alpha_u8(alpha: f32) -> u8 {
    alpha.round().clamp(0.0, 255.0) as u8
}

fn dot(canvas: &Canvas, x: f32, y: f32, size: f32, color: u32) {
    canvas.fill_rrect(
        x - size * 0.5,
        y - size * 0.5,
        size,
        size,
        size * 0.5,
        color,
    );
}

fn circle(canvas: &Canvas, x: f32, y: f32, radius: f32, color: u32) {
    canvas.fill_rrect(
        x - radius,
        y - radius,
        radius * 2.0,
        radius * 2.0,
        radius,
        color,
    );
}

fn ring(canvas: &Canvas, x: f32, y: f32, radius: f32, color: u32, width: f32) {
    canvas.stroke_rrect(
        x - radius,
        y - radius,
        radius * 2.0,
        radius * 2.0,
        radius,
        color,
        width,
    );
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
    use std::collections::HashSet;

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn every_preset_renders_a_distinct_frame() {
        let mut hashes = HashSet::new();
        for preset in 0..PRESETS.len() {
            let canvas = Canvas::new_cpu(320, 180, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let mut spectrum = [0.0; SPECTRUM_BANDS];
            for (index, band) in spectrum.iter_mut().enumerate() {
                *band = 0.15 + (index % 7) as f32 * 0.09;
            }
            let state = VisualState {
                width: 320.0,
                height: 180.0,
                elapsed: 3.75,
                position_ratio: 0.42,
                playing: true,
                features: AudioFeatures {
                    energy: 0.64,
                    rms: 0.22,
                    peak: 0.72,
                    loudness_db: -13.2,
                    bass: 0.71,
                    mid: 0.48,
                    treble: 0.37,
                    spectral_centroid_hz: 3_400.0,
                    dominant_frequency_hz: 445.0,
                    pitch_hz: 440.0,
                    pitch_confidence: 0.92,
                    spectral_flux: 0.14,
                    onset: 0.58,
                    spectrum,
                },
                preset,
                transition: 0.0,
                ..VisualState::default()
            };
            paint_scene(&canvas, &state);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            let hash = pixels.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
            assert!(
                hashes.insert(hash),
                "preset {preset} duplicated another frame"
            );
        }
    }

    #[test]
    fn feature_smoothing_has_fast_attack_and_slower_release() {
        let mut visible = AudioFeatures::default();
        let loud = AudioFeatures {
            energy: 1.0,
            bass: 1.0,
            ..AudioFeatures::default()
        };
        smooth_features(&mut visible, &loud, 1.0 / 60.0);
        let attacked = visible.energy;
        smooth_features(&mut visible, &AudioFeatures::default(), 1.0 / 60.0);
        assert!(attacked > 0.15);
        assert!(visible.energy > attacked * 0.85);
    }

    #[test]
    fn animation_frames_stop_only_after_motion_settles() {
        let mut state = VisualState::default();
        assert!(!state.needs_animation_frame());

        state.playing = true;
        assert!(state.needs_animation_frame());

        state.playing = false;
        state.features.energy = 0.02;
        assert!(state.needs_animation_frame());

        state.features = AudioFeatures::default();
        state.transition = 0.25;
        assert!(state.needs_animation_frame());

        state.transition = 0.0;
        assert!(!state.needs_animation_frame());
    }

    #[test]
    fn exponential_envelope_is_frame_rate_independent() {
        let mut at_30_fps = 0.0;
        let mut at_60_fps = 0.0;
        for _ in 0..30 {
            at_30_fps = envelope(at_30_fps, 1.0, 8.0, 3.0, 1.0 / 30.0);
        }
        for _ in 0..60 {
            at_60_fps = envelope(at_60_fps, 1.0, 8.0, 3.0, 1.0 / 60.0);
        }
        assert!((at_30_fps - at_60_fps).abs() < 0.000_01);
    }

    #[test]
    fn tuning_is_normalized_to_safe_rendering_ranges() {
        let tuning = VisualTuning {
            intensity: 9.0,
            motion: -2.0,
            depth: 3.0,
            glow: 0.0,
        }
        .normalized();
        assert!((tuning.intensity - 1.75).abs() < f32::EPSILON);
        assert!((tuning.motion - 0.35).abs() < f32::EPSILON);
        assert!((tuning.depth - 1.50).abs() < f32::EPSILON);
        assert!((tuning.glow - 0.25).abs() < f32::EPSILON);
    }
}
