//! Audio-reactive visual engines rendered with Flux.
//!
//! Subject effects share a feature contract and transition model, but each has
//! its own visual model. Adding an effect does not require touching playback,
//! application state, or the UI toolkit.

use flux::{Canvas, Device, Format, GradientStop, Image, rgba};
use iris::PaintHost;
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::sync::{Arc, RwLock};
use wavora_audio_analysis::{AudioFeatures, SPECTRUM_BANDS};

/// Maximum number of independently positioned light sources.
///
/// The bound keeps serialized scenes understandable and caps full-screen
/// overdraw on integrated GPUs while still allowing useful colour mixing.
pub const MAX_LIGHT_SOURCES: usize = 4;

/// Rendering strategy used by a visual preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualKind {
    ParticleVeil,
    PulseTunnel,
    OrbitalCore,
    SpectralVoid,
    VinylHalo,
    StarRiver,
    PrismRibbons,
    LuminousBloom,
    RippleField,
    ParticleTerrain,
    CoverRelief,
}

type SubjectRenderer = fn(&Canvas, &VisualState, VisualPreset);

/// Metadata, palette, and renderer for one selectable subject effect.
///
/// Keeping the renderer in the registry makes subject effects self-contained:
/// adding one does not require a second dispatch table in the stage painter.
#[derive(Debug, Clone, Copy)]
pub struct VisualPreset {
    pub kind: VisualKind,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
    renderer: SubjectRenderer,
}

impl VisualPreset {
    fn draw(self, canvas: &Canvas, state: &VisualState) {
        (self.renderer)(canvas, state, self);
    }
}

/// Built-in effects. Their visual models intentionally differ; these are not
/// colour variants of a shared waveform.
pub const SUBJECT_EFFECTS: [VisualPreset; 11] = [
    VisualPreset {
        kind: VisualKind::ParticleVeil,
        accent: [112, 246, 218],
        secondary: [108, 149, 255],
        renderer: draw_particle_veil,
    },
    VisualPreset {
        kind: VisualKind::PulseTunnel,
        accent: [255, 92, 112],
        secondary: [244, 210, 138],
        renderer: draw_pulse_tunnel,
    },
    VisualPreset {
        kind: VisualKind::OrbitalCore,
        accent: [104, 205, 255],
        secondary: [139, 109, 255],
        renderer: draw_orbital_core,
    },
    VisualPreset {
        kind: VisualKind::SpectralVoid,
        accent: [174, 111, 255],
        secondary: [72, 218, 255],
        renderer: draw_spectral_void,
    },
    VisualPreset {
        kind: VisualKind::VinylHalo,
        accent: [244, 210, 138],
        secondary: [255, 126, 91],
        renderer: draw_vinyl_halo,
    },
    VisualPreset {
        kind: VisualKind::StarRiver,
        accent: [125, 232, 203],
        secondary: [126, 151, 255],
        renderer: draw_star_river,
    },
    VisualPreset {
        kind: VisualKind::PrismRibbons,
        accent: [255, 111, 199],
        secondary: [89, 214, 255],
        renderer: draw_prism_ribbons,
    },
    VisualPreset {
        kind: VisualKind::LuminousBloom,
        accent: [255, 183, 92],
        secondary: [164, 116, 255],
        renderer: draw_luminous_bloom,
    },
    VisualPreset {
        kind: VisualKind::RippleField,
        accent: [155, 184, 207],
        secondary: [111, 231, 255],
        renderer: draw_ripple_field,
    },
    VisualPreset {
        kind: VisualKind::ParticleTerrain,
        accent: [244, 142, 75],
        secondary: [57, 128, 232],
        renderer: draw_particle_terrain,
    },
    VisualPreset {
        kind: VisualKind::CoverRelief,
        accent: [244, 210, 138],
        secondary: [104, 205, 255],
        renderer: draw_cover_relief,
    },
];

/// Backwards-compatible registry name used by the rest of the application.
pub const PRESETS: [VisualPreset; 11] = SUBJECT_EFFECTS;

/// User-facing response controls shared by all subject effects.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

/// The stage's primary, attention-carrying visual layer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubjectLayer {
    pub enabled: bool,
    pub effect: usize,
    pub tuning: VisualTuning,
}

impl Default for SubjectLayer {
    fn default() -> Self {
        Self {
            enabled: true,
            effect: 0,
            tuning: VisualTuning::default(),
        }
    }
}

impl SubjectLayer {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.effect %= PRESETS.len();
        self.tuning = self.tuning.normalized();
        self
    }
}

/// How a light source chooses its two gradient colours.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightPalette {
    /// Follow the active subject's accent and secondary colours.
    #[default]
    Preset,
    /// Derive a private two-colour ramp from the source hue and saturation.
    Custom,
}

/// Radial opacity profile for one light source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightFalloff {
    /// Wide, low-contrast light suited to off-window placement.
    #[default]
    Diffuse,
    /// Brighter centre with a soft edge.
    Focused,
    /// A restrained ring with a dim centre.
    Halo,
}

/// Geometric footprint of one light source.
///
/// Geometry is deliberately independent from [`LightFalloff`]: the
/// former controls where light exists, while the latter controls how it fades.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightSourceShape {
    /// A single isotropic light. This is the cheapest source to render.
    #[default]
    Circle,
    /// A soft elongated area light whose long axis follows `rotation`.
    Oval,
    /// A directional, capsule-like wash suited to edge and off-window light.
    Beam,
}

/// Audio feature that independently drives one light source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightAudioResponse {
    /// Keep the source entirely static apart from its optional drift.
    None,
    /// Follow broad perceptual energy.
    #[default]
    Energy,
    /// Follow low-frequency energy.
    Bass,
    /// Follow mid-frequency energy.
    Mid,
    /// Follow high-frequency energy.
    Treble,
    /// Follow short attacks and beat-like transients.
    Onset,
}

/// Optional procedural material field beneath the immediate light sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LightMaterialKind {
    /// No texture field; light sources render directly over the base.
    #[default]
    None,
    /// Soft, granulated pigment plumes guided by the configured sources.
    Watercolor,
    /// A refracted, line-like light field inspired by water caustics.
    Caustics,
    /// Broad vertical curtains with slow spectral folds.
    Aurora,
    /// Layered, cloud-like colour with granular depth.
    Nebula,
}

/// Configuration shared by the procedural ambient material fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightMaterial {
    pub kind: LightMaterialKind,
    pub intensity: f32,
    pub scale: f32,
    pub motion: f32,
    pub palette: LightPalette,
    pub hue: f32,
    pub saturation: f32,
    pub seed: u32,
}

impl Default for LightMaterial {
    fn default() -> Self {
        Self {
            kind: LightMaterialKind::None,
            intensity: 0.85,
            scale: 1.0,
            motion: 0.28,
            palette: LightPalette::Preset,
            hue: 0.54,
            saturation: 0.68,
            seed: 0x57A7_0A11,
        }
    }
}

impl LightMaterial {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.intensity = finite_or(self.intensity, defaults.intensity).clamp(0.0, 1.5);
        self.scale = finite_or(self.scale, defaults.scale).clamp(0.45, 2.2);
        self.motion = finite_or(self.motion, defaults.motion).clamp(0.0, 1.0);
        self.hue = finite_or(self.hue, defaults.hue).rem_euclid(1.0);
        self.saturation = finite_or(self.saturation, defaults.saturation).clamp(0.0, 1.0);
        self
    }
}

/// One independently positioned and animated ambient light.
///
/// Positions are normalized window coordinates. Values outside `0..=1` are
/// intentional: they place the source outside the window while its tail can
/// remain visible. Radius and drift are fractions of the window's effective
/// width. Radius is the circle radius or the short-axis radius of an elongated
/// source; aspect and rotation define its long axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightSource {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub shape: LightSourceShape,
    pub aspect: f32,
    pub rotation: f32,
    pub intensity: f32,
    pub palette: LightPalette,
    pub hue: f32,
    pub saturation: f32,
    pub falloff: LightFalloff,
    pub drift: f32,
    pub phase: f32,
    pub audio_response: LightAudioResponse,
    pub audio_intensity: f32,
    pub audio_scale: f32,
}

impl Default for LightSource {
    fn default() -> Self {
        Self {
            x: 0.56,
            y: 0.42,
            radius: 0.48,
            shape: LightSourceShape::Circle,
            aspect: 2.0,
            rotation: 0.0,
            intensity: 1.0,
            palette: LightPalette::Preset,
            hue: 0.56,
            saturation: 0.72,
            falloff: LightFalloff::Diffuse,
            drift: 0.0,
            phase: 0.0,
            audio_response: LightAudioResponse::Energy,
            audio_intensity: 0.0,
            audio_scale: 0.18,
        }
    }
}

impl LightSource {
    /// A visible, deliberately offset source suitable for an Add action.
    #[must_use]
    pub fn added(index: usize) -> Self {
        const POSITIONS: [(f32, f32); MAX_LIGHT_SOURCES] =
            [(0.56, 0.42), (-0.10, 0.66), (1.12, 0.22), (0.38, 1.10)];
        const HUES: [f32; MAX_LIGHT_SOURCES] = [0.56, 0.78, 0.08, 0.46];
        let slot = index.min(MAX_LIGHT_SOURCES - 1);
        let phase = f32::from(u16::try_from(slot).unwrap_or_default())
            / f32::from(u16::try_from(MAX_LIGHT_SOURCES).unwrap_or(1));
        Self {
            x: POSITIONS[slot].0,
            y: POSITIONS[slot].1,
            radius: if slot == 0 { 0.48 } else { 0.42 },
            shape: if slot == 0 {
                LightSourceShape::Circle
            } else if slot == 2 {
                LightSourceShape::Beam
            } else {
                LightSourceShape::Oval
            },
            aspect: if slot == 2 { 2.8 } else { 2.0 },
            rotation: [0.0, -0.08, 0.13, 0.22][slot],
            intensity: if slot == 0 { 1.0 } else { 0.78 },
            palette: if slot == 0 {
                LightPalette::Preset
            } else {
                LightPalette::Custom
            },
            hue: HUES[slot],
            saturation: 0.72,
            falloff: if slot == 2 {
                LightFalloff::Halo
            } else {
                LightFalloff::Diffuse
            },
            drift: 0.0,
            phase,
            audio_response: match slot {
                1 => LightAudioResponse::Bass,
                2 => LightAudioResponse::Treble,
                3 => LightAudioResponse::Onset,
                _ => LightAudioResponse::Energy,
            },
            audio_intensity: if slot == 0 { 0.0 } else { 0.22 },
            audio_scale: if slot == 0 { 0.18 } else { 0.12 },
        }
    }

    #[must_use]
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.x = finite_or(self.x, defaults.x).clamp(-4.0, 5.0);
        self.y = finite_or(self.y, defaults.y).clamp(-4.0, 5.0);
        self.radius = finite_or(self.radius, defaults.radius).clamp(0.05, 2.0);
        self.aspect = finite_or(self.aspect, defaults.aspect).clamp(1.0, 4.0);
        self.rotation = finite_or(self.rotation, defaults.rotation).clamp(-0.5, 0.5);
        self.intensity = finite_or(self.intensity, defaults.intensity).clamp(0.0, 2.0);
        self.hue = finite_or(self.hue, defaults.hue).rem_euclid(1.0);
        self.saturation = finite_or(self.saturation, defaults.saturation).clamp(0.0, 1.0);
        self.drift = finite_or(self.drift, defaults.drift).clamp(0.0, 0.18);
        self.phase = finite_or(self.phase, defaults.phase).rem_euclid(1.0);
        self.audio_intensity =
            finite_or(self.audio_intensity, defaults.audio_intensity).clamp(0.0, 1.5);
        self.audio_scale = finite_or(self.audio_scale, defaults.audio_scale).clamp(0.0, 0.8);
        self
    }
}

/// A composable light layer rendered independently from the subject effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientLayer {
    pub enabled: bool,
    pub field: LightMaterial,
    pub sources: Vec<LightSource>,
    #[serde(
        default,
        rename = "composition_visible",
        skip_serializing,
        skip_serializing_if = "Option::is_none"
    )]
    legacy_subject_enabled: Option<bool>,
}

impl Default for AmbientLayer {
    fn default() -> Self {
        Self {
            enabled: true,
            field: LightMaterial::default(),
            sources: vec![LightSource::default()],
            legacy_subject_enabled: None,
        }
    }
}

impl AmbientLayer {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.field = self.field.normalized();
        self.sources.truncate(MAX_LIGHT_SOURCES);
        self.sources = self
            .sources
            .into_iter()
            .map(LightSource::normalized)
            .collect();
        self
    }

    /// Adds a visible source when capacity permits.
    pub fn add_source(&mut self) -> bool {
        if self.sources.len() >= MAX_LIGHT_SOURCES {
            return false;
        }
        self.sources.push(LightSource::added(self.sources.len()));
        true
    }

    /// Removes one source without changing the remaining sources' parameters.
    pub fn remove_source(&mut self, index: usize) -> bool {
        if index >= self.sources.len() {
            return false;
        }
        self.sources.remove(index);
        true
    }

    fn has_motion(&self) -> bool {
        self.enabled
            && (self.sources.iter().any(|source| source.drift > 0.000_1)
                || (self.field.kind != LightMaterialKind::None && self.field.motion > 0.000_1))
    }

    fn take_legacy_subject_enabled(&mut self) -> Option<bool> {
        self.legacy_subject_enabled.take()
    }
}

/// Complete visual-stage configuration. Subject and ambient are sibling modules
/// and can be enabled, persisted, and evolved independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VisualStage {
    #[serde(alias = "focus")]
    pub subject: SubjectLayer,
    #[serde(alias = "lighting", alias = "light")]
    pub ambient: AmbientLayer,
}

impl VisualStage {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if let Some(enabled) = self.ambient.take_legacy_subject_enabled() {
            self.subject.enabled = enabled;
        }
        self.subject = self.subject.normalized();
        self.ambient = self.ambient.normalized();
        self
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[derive(Debug, Clone, Copy)]
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

/// Animated, normalized values used by the compact audio metric bars.
///
/// The order is loudness, pitch, spectral centroid, and onset. Keeping the
/// snapshot value-only lets the UI render it without taking ownership of the
/// animation clock or duplicating the feature mapping.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AudioMetricSnapshot {
    pub levels: [f32; 4],
    pub pulses: [f32; 4],
}

#[derive(Debug, Clone, Copy, Default)]
struct BallisticMetric {
    level: f32,
    fall_velocity: f32,
    pulse: f32,
}

impl BallisticMetric {
    fn update(&mut self, target: f32, impulse: f32, gravity: f32, pulse_decay: f32, dt: f32) {
        let target = target.clamp(0.0, 1.0);
        let rise = (target - self.level).max(0.0);
        if target >= self.level {
            self.level = target;
            self.fall_velocity = 0.0;
        } else {
            // A ballistic release starts gently, then accelerates until it
            // meets the live signal. A new upward sample immediately catches
            // the bar and resets the fall, like a peak meter with gravity.
            let fall_distance = self.fall_velocity.mul_add(dt, 0.5 * gravity * dt * dt);
            self.fall_velocity += gravity * dt;
            self.level = (self.level - fall_distance).max(target);
            if self.level <= target + f32::EPSILON {
                self.fall_velocity = 0.0;
            }
        }
        self.pulse = (self.pulse * (-pulse_decay * dt).exp())
            .max(impulse)
            .max(rise * 2.4)
            .clamp(0.0, 1.0);
    }

    fn is_settled(self) -> bool {
        self.level < 0.000_5 && self.pulse < 0.002 && self.fall_velocity < 0.002
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AudioMetricMotion {
    metrics: [BallisticMetric; 4],
}

impl AudioMetricMotion {
    fn update(&mut self, features: &AudioFeatures, dt: f32) {
        let targets = normalized_audio_metrics(features);
        let onset = features.onset.clamp(0.0, 1.0);
        let impulses = [onset * 0.34, onset * 0.10, onset * 0.16, onset];
        // Transients should clear quickly; frequency-bearing values stay
        // readable long enough for the eye to track them.
        let gravity = [3.0, 1.7, 2.1, 6.2];
        let pulse_decay = [6.8, 5.2, 5.8, 9.5];
        for (index, metric) in self.metrics.iter_mut().enumerate() {
            metric.update(
                targets[index],
                impulses[index],
                gravity[index],
                pulse_decay[index],
                dt,
            );
        }
    }

    fn snapshot(self) -> AudioMetricSnapshot {
        AudioMetricSnapshot {
            levels: self.metrics.map(|metric| metric.level),
            pulses: self.metrics.map(|metric| metric.pulse),
        }
    }

    fn is_settled(self) -> bool {
        self.metrics.into_iter().all(BallisticMetric::is_settled)
    }
}

fn normalized_audio_metrics(features: &AudioFeatures) -> [f32; 4] {
    let loudness = if features.loudness_db.is_finite() {
        ((features.loudness_db.clamp(-60.0, 0.0) + 60.0) / 60.0).powf(0.82)
    } else {
        0.0
    };
    let pitch = if features.pitch_confidence > 0.2 {
        normalize_log_frequency(features.pitch_hz, 50.0, 1_200.0)
    } else {
        0.0
    };
    let centroid = normalize_log_frequency(features.spectral_centroid_hz, 45.0, 16_000.0);
    let onset = features.onset.clamp(0.0, 1.0).sqrt();
    [loudness, pitch, centroid, onset]
}

fn normalize_log_frequency(value: f32, minimum: f32, maximum: f32) -> f32 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    ((value.clamp(minimum, maximum) / minimum).ln() / (maximum / minimum).ln()).clamp(0.0, 1.0)
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
    pub subject_enabled: bool,
    pub preset: usize,
    pub tuning: VisualTuning,
    ambient: Arc<AmbientLayer>,
    artwork: Option<usize>,
    viewport: Option<StageViewport>,
    transition: f32,
    metric_motion: AudioMetricMotion,
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
            subject_enabled: true,
            preset: 0,
            tuning: VisualTuning::default(),
            ambient: Arc::new(AmbientLayer::default()),
            artwork: None,
            viewport: None,
            transition: 0.0,
            metric_motion: AudioMetricMotion::default(),
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
        measured: AudioFeatures,
        stage: &VisualStage,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        self.subject_enabled = stage.subject.enabled;
        self.tuning = stage.subject.tuning.normalized();
        if self.ambient.as_ref() != &stage.ambient {
            self.ambient = Arc::new(stage.ambient.clone().normalized());
        }
        self.elapsed += dt * self.tuning.motion;
        self.position_ratio = position_ratio.clamp(0.0, 1.0);
        self.playing = playing;
        let preset = stage.subject.effect % PRESETS.len();
        if preset != self.preset {
            self.preset = preset;
            self.transition = 1.0;
        }
        self.transition = (self.transition - dt * 1.45).max(0.0);

        let measured = if playing {
            measured
        } else {
            AudioFeatures::default()
        };
        self.metric_motion.update(&measured, dt);
        let mut target = measured;
        apply_intensity(&mut target, self.tuning.intensity);
        smooth_features(&mut self.features, &target, dt);
    }

    /// Returns the current UI meter animation without exposing mutable motion
    /// state to the chrome layer.
    #[must_use]
    pub fn audio_metric_snapshot(&self) -> AudioMetricSnapshot {
        self.metric_motion.snapshot()
    }

    pub fn set_stage_viewport(&mut self, viewport: Option<(f32, f32, f32, f32)>) {
        self.viewport = viewport.map(|(x, y, width, height)| StageViewport {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        });
    }

    /// Whether host-owned motion still needs an active-rate follow-up frame.
    ///
    /// Playback continuously advances the scene. After playback or a preset
    /// transition stops, frames continue only until the visible envelopes
    /// have settled, allowing Iris to return to its low-power idle cadence.
    #[must_use]
    pub fn needs_animation_frame(&self) -> bool {
        self.playing
            || self.transition > 0.0
            || !features_are_settled(&self.features)
            || !self.metric_motion.is_settled()
            || self.ambient.has_motion()
    }
}

pub type SharedVisualState = Arc<RwLock<VisualState>>;

#[must_use]
pub fn shared_state(stage: &VisualStage) -> SharedVisualState {
    Arc::new(RwLock::new(VisualState {
        subject_enabled: stage.subject.enabled,
        preset: stage.subject.effect % PRESETS.len(),
        tuning: stage.subject.tuning.normalized(),
        ambient: Arc::new(stage.ambient.clone().normalized()),
        ..VisualState::default()
    }))
}

const MATERIAL_OVERSCAN: f32 = 0.10;
const MATERIAL_LONG_EDGE: u32 = 512;
const MATERIAL_SHORT_EDGE_MIN: u32 = 224;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn material_texture_dimensions(logical_width: f32, logical_height: f32) -> (u32, u32) {
    let aspect = (logical_width.max(1.0) / logical_height.max(1.0)).clamp(0.35, 2.85);
    if aspect >= 1.0 {
        let height = ((MATERIAL_LONG_EDGE as f32 / aspect).round() as u32)
            .clamp(MATERIAL_SHORT_EDGE_MIN, MATERIAL_LONG_EDGE);
        (MATERIAL_LONG_EDGE, height)
    } else {
        let width = ((MATERIAL_LONG_EDGE as f32 * aspect).round() as u32)
            .clamp(MATERIAL_SHORT_EDGE_MIN, MATERIAL_LONG_EDGE);
        (width, MATERIAL_LONG_EDGE)
    }
}

#[allow(clippy::cast_precision_loss)]
fn generate_material_texture(key: &MaterialTextureKey) -> Vec<u8> {
    let pixel_count = (key.width as usize).saturating_mul(key.height as usize);
    let mut pixels = vec![0_u8; pixel_count.saturating_mul(4)];
    if key.field.kind == LightMaterialKind::None {
        return pixels;
    }
    let inverse_width = 1.0 / key.width.max(1) as f32;
    let inverse_height = 1.0 / key.height.max(1) as f32;
    let span = 1.0 + MATERIAL_OVERSCAN * 2.0;
    for y in 0..key.height {
        let scene_y = (y as f32 + 0.5).mul_add(inverse_height * span, -MATERIAL_OVERSCAN);
        for x in 0..key.width {
            let scene_x = (x as f32 + 0.5).mul_add(inverse_width * span, -MATERIAL_OVERSCAN);
            let pixel = match key.field.kind {
                LightMaterialKind::None => [0, 0, 0, 0],
                LightMaterialKind::Watercolor => watercolor_pixel(key, scene_x, scene_y),
                LightMaterialKind::Caustics => caustics_pixel(key, scene_x, scene_y),
                LightMaterialKind::Aurora => aurora_pixel(key, scene_x, scene_y),
                LightMaterialKind::Nebula => nebula_pixel(key, scene_x, scene_y),
            };
            let offset = ((y as usize) * (key.width as usize) + x as usize) * 4;
            pixels[offset..offset + 4].copy_from_slice(&pixel);
        }
    }
    pixels
}

fn material_palette(key: &MaterialTextureKey) -> ([u8; 3], [u8; 3]) {
    match key.field.palette {
        LightPalette::Preset => (key.preset_accent, key.preset_secondary),
        LightPalette::Custom => custom_light_palette(key.field.hue, key.field.saturation),
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn watercolor_pixel(key: &MaterialTextureKey, scene_x: f32, scene_y: f32) -> [u8; 4] {
    let (primary, secondary) = material_palette(key);
    let aspect = key.width as f32 / key.height.max(1) as f32;
    let height_in_widths = 1.0 / aspect;
    let scale = key.field.scale;
    let warp_x = (fbm(scene_x * 2.1 * scale, scene_y * 2.1 * scale, key.field.seed) - 0.5) * 0.15;
    let warp_y = (fbm(
        scene_x * 2.3 * scale + 9.7,
        scene_y * 2.3 * scale - 4.1,
        key.field.seed ^ 0xA341_316C,
    ) - 0.5)
        * 0.15;

    let fallback_anchors = [
        MaterialAnchor::from(&LightSource::added(0)),
        MaterialAnchor::from(&LightSource::added(1)),
    ];
    let anchors: &[MaterialAnchor] = if key.anchors.is_empty() {
        &fallback_anchors
    } else {
        &key.anchors
    };
    let mut alpha_union = 0.0_f32;
    let mut colour_sum = [0.0_f32; 3];
    let mut colour_weight = 0.0_f32;
    for (index, anchor) in anchors.iter().enumerate() {
        let radius = anchor.radius * 0.92_f32.min(height_in_widths * 1.7);
        for pigment_index in 0_u32..2 {
            let pigment_phase = anchor.phase.mul_add(
                std::f32::consts::TAU,
                index as f32 * 0.73 + pigment_index as f32 * 2.71,
            );
            let centre_x = anchor.x + pigment_phase.cos() * radius * 0.16;
            let centre_y = anchor.y + pigment_phase.sin() * radius * 0.13 / height_in_widths;
            let dx = scene_x + warp_x - centre_x;
            let dy = (scene_y + warp_y - centre_y) * height_in_widths;
            let rotation = anchor.rotation * std::f32::consts::TAU;
            let local_x = rotation.cos().mul_add(dx, rotation.sin() * dy);
            let local_y = (-rotation.sin()).mul_add(dx, rotation.cos() * dy);
            let radius = radius.max(0.025);
            let distance = match anchor.shape {
                LightSourceShape::Circle => local_x.hypot(local_y) / radius,
                LightSourceShape::Oval => (local_x / anchor.aspect).hypot(local_y) / radius,
                LightSourceShape::Beam => {
                    let longitudinal = local_x.abs() / (radius * anchor.aspect);
                    let lateral = local_y.abs() / radius;
                    (longitudinal.powi(4) + lateral.powi(4)).powf(0.25)
                }
            };
            let pigment_slot = (index as u32).wrapping_mul(2) + pigment_index;
            let edge_noise = fbm(
                scene_x * 5.4 * scale + pigment_slot as f32 * 3.7,
                scene_y * 5.4 * scale - pigment_slot as f32 * 2.9,
                key.field.seed ^ pigment_slot.wrapping_mul(0x9E37_79B9),
            );
            let boundary = 1.02 - distance + (edge_noise - 0.5) * 0.58;
            let wash = smoothstep(0.0, 0.72, boundary);
            if wash <= f32::EPSILON {
                continue;
            }
            let granulation = 0.62
                + fbm(
                    scene_x * 23.0 * scale,
                    scene_y * 23.0 * scale,
                    key.field.seed ^ 0xC801_3EA4 ^ pigment_slot,
                ) * 0.38;
            let tide = scene_x
                .mul_add(17.0, scene_y * 11.0 + pigment_slot as f32 * 1.7)
                .sin()
                * 0.5
                + 0.5;
            let density = wash.powf(0.78) * granulation * (0.82 + tide * 0.18);
            let source_alpha = (density * 0.17 * key.field.intensity).clamp(0.0, 0.38);
            alpha_union = 1.0 - (1.0 - alpha_union) * (1.0 - source_alpha);
            let pigment = if pigment_index == 0 {
                mix_rgb(primary, secondary, edge_noise * 0.22)
            } else {
                mix_rgb(secondary, primary, edge_noise * 0.18)
            };
            for channel in 0..3 {
                colour_sum[channel] += f32::from(pigment[channel]) * density;
            }
            colour_weight += density;
        }
    }
    if colour_weight <= f32::EPSILON {
        return [0, 0, 0, 0];
    }
    let colour = colour_sum.map(|channel| (channel / colour_weight).round().clamp(0.0, 255.0));
    premultiplied_pixel(colour, alpha_union)
}

#[allow(clippy::cast_precision_loss)]
fn caustics_pixel(key: &MaterialTextureKey, scene_x: f32, scene_y: f32) -> [u8; 4] {
    let (primary, secondary) = material_palette(key);
    let aspect = key.width as f32 / key.height.max(1) as f32;
    let scale = key.field.scale;
    let x = scene_x * aspect * 7.5 * scale;
    let y = scene_y * 7.5 * scale;
    let warp = fbm(x * 0.24, y * 0.24, key.field.seed) - 0.5;
    let wave = (x * 1.37 + warp * 2.1).sin()
        + (y * 1.83 - warp * 1.7).sin()
        + ((x + y) * 0.91 + warp).sin();
    let cross_wave = (x.mul_add(0.73, -y * 1.19) + warp * 2.6).sin()
        + (x.mul_add(1.11, y * 0.58) - warp * 1.4).sin();
    let ridge = (-wave.abs() * 4.6).exp().powf(1.5);
    let cross_ridge = (-cross_wave.abs() * 5.8).exp().powf(1.8);
    let shimmer = (ridge * 0.72 + cross_ridge * 0.54).clamp(0.0, 1.0);
    let grain = 0.76
        + fbm(
            scene_x * 31.0 * scale,
            scene_y * 31.0 * scale,
            key.field.seed ^ 0xAD90_7A1D,
        ) * 0.24;
    let alpha = ((shimmer - 0.08).max(0.0) * grain * 0.23 * key.field.intensity).clamp(0.0, 0.38);
    let colour = mix_rgb(primary, secondary, (warp + 0.5).clamp(0.0, 1.0));
    premultiplied_pixel(
        colour.map(|channel| f32::from(channel.saturating_add(18))),
        alpha,
    )
}

#[allow(clippy::cast_precision_loss)]
fn aurora_pixel(key: &MaterialTextureKey, scene_x: f32, scene_y: f32) -> [u8; 4] {
    let (primary, secondary) = material_palette(key);
    let aspect = key.width as f32 / key.height.max(1) as f32;
    let scale = key.field.scale;
    let x = scene_x * aspect * scale;
    let y = scene_y * scale;
    let warp = fbm(x * 1.7, y * 0.72, key.field.seed) - 0.5;
    let fine_warp = fbm(x * 4.8 + 7.3, y * 1.4 - 2.1, key.field.seed ^ 0x6C8E_9CF5) - 0.5;
    let curtain = (x * 5.2 + warp * 2.8 + fine_warp * 0.9).sin();
    let second_curtain = (x * 3.4 - warp * 2.1 + 1.7).sin();
    let crest = 0.42 + curtain * 0.14 + second_curtain * 0.06;
    let distance = (y - crest).abs();
    let ribbon = (-distance * distance * 34.0).exp();
    let tail = ((y - crest) * 3.2).clamp(0.0, 1.0);
    let striation = 0.58 + (x * 28.0 + fine_warp * 5.0).sin().abs() * 0.42;
    let alpha = ribbon * (1.0 - tail * 0.44) * striation * 0.26 * key.field.intensity;
    let colour = mix_rgb(
        primary,
        secondary,
        (scene_x * 0.45 + warp + 0.5).rem_euclid(1.0),
    );
    premultiplied_pixel(colour.map(f32::from), alpha.clamp(0.0, 0.42))
}

fn nebula_pixel(key: &MaterialTextureKey, scene_x: f32, scene_y: f32) -> [u8; 4] {
    let (primary, secondary) = material_palette(key);
    let scale = key.field.scale;
    let broad = fbm(scene_x * 2.2 * scale, scene_y * 2.2 * scale, key.field.seed);
    let folds = fbm(
        scene_x * 5.1 * scale + broad * 1.6,
        scene_y * 4.7 * scale - broad * 1.2,
        key.field.seed ^ 0xB529_7A4D,
    );
    let grain = fbm(
        scene_x * 17.0 * scale,
        scene_y * 17.0 * scale,
        key.field.seed ^ 0x68E3_1DA4,
    );
    let density = smoothstep(0.43, 0.78, broad * 0.66 + folds * 0.44) * (0.72 + grain * 0.28);
    let alpha = density.powf(1.22) * 0.24 * key.field.intensity;
    let colour = mix_rgb(
        primary,
        secondary,
        (folds * 0.74 + grain * 0.26).clamp(0.0, 1.0),
    );
    premultiplied_pixel(colour.map(f32::from), alpha.clamp(0.0, 0.40))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn mix_rgb(first: [u8; 3], second: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    std::array::from_fn(|index| {
        f32::from(first[index])
            .mul_add(1.0 - amount, f32::from(second[index]) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn premultiplied_pixel(colour: [f32; 3], alpha: f32) -> [u8; 4] {
    let alpha = alpha.clamp(0.0, 1.0);
    let alpha_u8 = (alpha * 255.0).round() as u8;
    let channels = colour.map(|channel| (channel.clamp(0.0, 255.0) * alpha).round() as u8);
    [channels[0], channels[1], channels[2], alpha_u8]
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let normalized = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    normalized * normalized * (3.0 - 2.0 * normalized)
}

fn fbm(mut x: f32, mut y: f32, seed: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.54;
    let mut normalization = 0.0;
    for octave in 0_u32..4 {
        value += value_noise(x, y, seed ^ octave.wrapping_mul(0x68BC_21EB)) * amplitude;
        normalization += amplitude;
        x = x.mul_add(1.97, 0.37);
        y = y.mul_add(2.03, -0.29);
        amplitude *= 0.48;
    }
    value / normalization
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let sample = |px: i32, py: i32| {
        let mixed = (px as u32)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add((py as u32).wrapping_mul(0x85EB_CA6B))
            ^ seed;
        hash01(mixed)
    };
    let top = sample(x0, y0).mul_add(1.0 - sx, sample(x0 + 1, y0) * sx);
    let bottom = sample(x0, y0 + 1).mul_add(1.0 - sx, sample(x0 + 1, y0 + 1) * sx);
    top.mul_add(1.0 - sy, bottom * sy)
}

#[derive(Debug, Clone, PartialEq)]
struct MaterialAnchor {
    x: f32,
    y: f32,
    radius: f32,
    shape: LightSourceShape,
    aspect: f32,
    rotation: f32,
    phase: f32,
}

impl From<&LightSource> for MaterialAnchor {
    fn from(source: &LightSource) -> Self {
        Self {
            x: source.x,
            y: source.y,
            radius: source.radius,
            shape: source.shape,
            aspect: source.aspect,
            rotation: source.rotation,
            phase: source.phase,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MaterialTextureKey {
    width: u32,
    height: u32,
    field: LightMaterial,
    anchors: Vec<MaterialAnchor>,
    preset_accent: [u8; 3],
    preset_secondary: [u8; 3],
}

#[derive(Default)]
struct MaterialTextureCache {
    device: Option<*mut c_void>,
    key: Option<MaterialTextureKey>,
    image: Option<Image>,
}

impl MaterialTextureCache {
    #[allow(unsafe_code)]
    fn prepare(
        &mut self,
        device_raw: *mut c_void,
        ambient: &AmbientLayer,
        preset: VisualPreset,
        logical_width: f32,
        logical_height: f32,
    ) {
        if !ambient.enabled || ambient.field.kind == LightMaterialKind::None {
            self.clear();
            return;
        }
        let (width, height) = material_texture_dimensions(logical_width, logical_height);
        let mut texture_field = ambient.field.clone();
        // Motion only changes the destination transform and never invalidates
        // the generated pixels.
        texture_field.motion = 0.0;
        let (preset_accent, preset_secondary) = if texture_field.palette == LightPalette::Preset {
            (preset.accent, preset.secondary)
        } else {
            ([0; 3], [0; 3])
        };
        let key = MaterialTextureKey {
            width,
            height,
            field: texture_field,
            anchors: if matches!(ambient.field.kind, LightMaterialKind::Watercolor) {
                ambient.sources.iter().map(MaterialAnchor::from).collect()
            } else {
                Vec::new()
            },
            preset_accent,
            preset_secondary,
        };
        if self.device == Some(device_raw) && self.key.as_ref() == Some(&key) {
            return;
        }

        self.image = None;
        self.device = Some(device_raw);
        let pixels = generate_material_texture(&key);
        // SAFETY: Iris owns this device. Uploaded Images retain their own
        // device reference, matching the application's artwork cache model.
        let device = unsafe { Device::borrow_raw(device_raw.cast()) };
        self.image = Image::from_bytes(
            &device,
            width,
            height,
            Format::FLUX_FORMAT_RGBA8_SRGB,
            &pixels,
        )
        .ok();
        self.key = Some(key);
    }

    fn clear(&mut self) {
        self.image = None;
        self.key = None;
        self.device = None;
    }
}

/// Device-backed renderer state cached by the application's paint closure.
///
/// Material fields are generated and uploaded only when their configuration,
/// palette, geometry inputs, or target aspect ratio changes. Radial sources
/// remain immediate Canvas draws; only source-guided watercolor needs a new
/// texture after source geometry changes. Source audio response only affects
/// immediate light geometry, avoiding per-frame material regeneration.
#[derive(Default)]
pub struct VisualRenderer {
    shell_material: MaterialTextureCache,
    stage_material: MaterialTextureCache,
}

impl VisualRenderer {
    /// Paints the current visual snapshot into Iris's live Flux canvas.
    #[allow(unsafe_code, clippy::needless_pass_by_value)]
    pub fn paint(
        &mut self,
        host: PaintHost,
        state: &SharedVisualState,
        artwork: Option<*mut c_void>,
    ) {
        let mut snapshot = state
            .read()
            .map_or_else(|_| VisualState::default(), |state| state.clone());
        snapshot.artwork = artwork.map(|image| image as usize);
        let preset = PRESETS[snapshot.preset % PRESETS.len()];
        self.shell_material.prepare(
            host.device(),
            &snapshot.ambient,
            preset,
            snapshot.width,
            snapshot.height,
        );
        if let Some(viewport) = snapshot.viewport.filter(|viewport| viewport.is_visible()) {
            self.stage_material.prepare(
                host.device(),
                &snapshot.ambient,
                preset,
                viewport.width,
                viewport.height,
            );
        } else {
            self.stage_material.clear();
        }

        let scale = host.scale().max(1.0);
        let canvas = unsafe {
            // SAFETY: Iris owns this live canvas and keeps it valid throughout
            // the paint callback. The borrowed handle never destroys it.
            Canvas::borrow_raw(host.canvas().cast::<flux::sys::flux_canvas>())
        };
        canvas.save();
        canvas.scale(scale, scale);
        paint_scene_with_materials(
            &canvas,
            &snapshot,
            scale,
            self.shell_material.image.as_ref(),
            self.stage_material.image.as_ref(),
        );
        canvas.restore();
    }
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

#[cfg(test)]
fn paint_scene(canvas: &Canvas, state: &VisualState, device_scale: f32) {
    paint_scene_with_materials(canvas, state, device_scale, None, None);
}

fn paint_scene_with_materials(
    canvas: &Canvas,
    state: &VisualState,
    device_scale: f32,
    shell_material: Option<&Image>,
    stage_material: Option<&Image>,
) {
    let preset = PRESETS[state.preset % PRESETS.len()];
    if let Some(viewport) = state.viewport.filter(|viewport| viewport.is_visible()) {
        draw_app_backdrop_with_material(canvas, state, preset, shell_material);
        let mut local = state.clone();
        local.width = viewport.width;
        local.height = viewport.height;
        local.viewport = None;
        canvas.save();
        canvas.clip_rect(
            viewport.x * device_scale,
            viewport.y * device_scale,
            viewport.width * device_scale,
            viewport.height * device_scale,
        );
        canvas.translate(viewport.x, viewport.y);
        draw_stage_with_material(canvas, &local, preset, stage_material);
        canvas.restore();
    } else {
        draw_stage_with_material(canvas, state, preset, shell_material);
    }
}

#[cfg(test)]
fn draw_stage(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    draw_stage_with_material(canvas, state, preset, None);
}

fn draw_stage_with_material(
    canvas: &Canvas,
    state: &VisualState,
    preset: VisualPreset,
    material: Option<&Image>,
) {
    draw_backdrop_with_material(canvas, state, preset, material);
    if state.subject_enabled {
        preset.draw(canvas, state);
    }
    draw_transition(canvas, state, preset);
}

#[cfg(test)]
fn draw_app_backdrop(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    draw_app_backdrop_with_material(canvas, state, preset, None);
}

fn draw_app_backdrop_with_material(
    canvas: &Canvas,
    state: &VisualState,
    preset: VisualPreset,
    material: Option<&Image>,
) {
    // Keep the shell behind the isolated Visual Stage on the same diffuse
    // gradient as every other tab. Only the subject itself is clipped to
    // the stage viewport, so switching tabs no longer changes the page glow.
    draw_backdrop_with_material(canvas, state, preset, material);
}

#[cfg(test)]
fn draw_backdrop(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    draw_backdrop_with_material(canvas, state, preset, None);
}

fn draw_backdrop_with_material(
    canvas: &Canvas,
    state: &VisualState,
    preset: VisualPreset,
    material: Option<&Image>,
) {
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
    if let Some(material) = material {
        draw_material_field(canvas, state, material);
    }
    draw_ambient(canvas, state, preset);
}

fn draw_material_field(canvas: &Canvas, state: &VisualState, material: &Image) {
    let field = &state.ambient.field;
    let margin = MATERIAL_OVERSCAN;
    let phase = f32::from(u16::try_from(field.seed & 0xffff).unwrap_or_default())
        / f32::from(u16::MAX)
        * std::f32::consts::TAU;
    let speed = match field.kind {
        LightMaterialKind::Watercolor => 0.025,
        LightMaterialKind::Caustics => 0.070,
        LightMaterialKind::Aurora => 0.042,
        LightMaterialKind::Nebula => 0.018,
        LightMaterialKind::None => 0.0,
    };
    let travel = field.motion * margin * 0.34;
    let angle = state.elapsed.mul_add(speed, phase);
    let offset_x = angle.cos() * state.width * travel;
    let offset_y = angle.sin() * state.height * travel;
    canvas.draw_image(
        material,
        -state.width * margin + offset_x,
        -state.height * margin + offset_y,
        state.width * (1.0 + margin * 2.0),
        state.height * (1.0 + margin * 2.0),
    );
}

fn draw_ambient(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    if !state.ambient.enabled {
        return;
    }

    for source in &state.ambient.sources {
        draw_light_source(canvas, state, preset, source);
    }
}

fn draw_light_source(
    canvas: &Canvas,
    state: &VisualState,
    preset: VisualPreset,
    source: &LightSource,
) {
    let (width, height) = (state.width, state.height);
    let phase = source.phase * std::f32::consts::TAU;
    let drift_angle = state.elapsed.mul_add(0.07 + source.phase * 0.09, phase);
    let drift_radius = width.min(height) * source.drift;
    let center = (
        width.mul_add(source.x, drift_angle.cos() * drift_radius),
        height.mul_add(source.y, drift_angle.sin() * drift_radius),
    );
    let audio_drive = light_audio_drive(&state.features, source.audio_response);
    let radius = light_source_radius(width, height, source.radius)
        * audio_drive.mul_add(source.audio_scale, 1.0);
    if radius <= f32::EPSILON {
        return;
    }

    let (primary, secondary) = match source.palette {
        LightPalette::Preset => (preset.accent, preset.secondary),
        LightPalette::Custom => custom_light_palette(source.hue, source.saturation),
    };
    let strength =
        source.intensity * state.tuning.glow * audio_drive.mul_add(source.audio_intensity, 1.0);
    let rotation = source.rotation * std::f32::consts::TAU;
    let axis = (rotation.cos(), rotation.sin());
    let axis_span = radius * (source.aspect - 1.0);

    match source.shape {
        LightSourceShape::Circle => draw_light_lobe(
            canvas,
            (width, height),
            center,
            radius,
            strength,
            primary,
            secondary,
            source.falloff,
        ),
        LightSourceShape::Oval if axis_span <= radius * 0.01 => draw_light_lobe(
            canvas,
            (width, height),
            center,
            radius,
            strength,
            primary,
            secondary,
            source.falloff,
        ),
        LightSourceShape::Oval => {
            // A bounded chain of soft radial lobes approximates an anisotropic
            // area light without allocating a source texture or regenerating
            // pixels for audio-driven scale changes.
            for index in 0_u16..9 {
                let unit = f32::from(index) / 8.0;
                let position = unit.mul_add(1.84, -0.92);
                let cross_section = (1.0 - position * position).max(0.0).sqrt();
                let lobe_center = (
                    center.0 + axis.0 * axis_span * position,
                    center.1 + axis.1 * axis_span * position,
                );
                draw_light_lobe(
                    canvas,
                    (width, height),
                    lobe_center,
                    radius * cross_section.max(0.18),
                    strength * 0.28,
                    primary,
                    secondary,
                    source.falloff,
                );
            }
        }
        LightSourceShape::Beam => {
            // A capsule profile keeps directional light soft at both ends and
            // remains useful when its centre sits beyond a window edge.
            for index in 0_u16..9 {
                let position = (f32::from(index) / 8.0).mul_add(2.0, -1.0);
                let lobe_center = (
                    center.0 + axis.0 * axis_span * position,
                    center.1 + axis.1 * axis_span * position,
                );
                draw_light_lobe(
                    canvas,
                    (width, height),
                    lobe_center,
                    radius,
                    strength * 0.22,
                    primary,
                    secondary,
                    source.falloff,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_light_lobe(
    canvas: &Canvas,
    viewport: (f32, f32),
    center: (f32, f32),
    radius: f32,
    strength: f32,
    primary: [u8; 3],
    secondary: [u8; 3],
    falloff: LightFalloff,
) {
    let left = (center.0 - radius).max(0.0);
    let top = (center.1 - radius).max(0.0);
    let right = (center.0 + radius).min(viewport.0);
    let bottom = (center.1 + radius).min(viewport.1);
    if radius <= f32::EPSILON || strength <= f32::EPSILON || right <= left || bottom <= top {
        return;
    }

    let diffuse = [
        GradientStop::new(0.0, color(primary, alpha_u8(31.0 * strength))),
        GradientStop::new(0.38, color(secondary, alpha_u8(16.0 * strength))),
        GradientStop::new(1.0, color(primary, 0)),
    ];
    let focused = [
        GradientStop::new(0.0, color(primary, alpha_u8(52.0 * strength))),
        GradientStop::new(0.24, color(primary, alpha_u8(36.0 * strength))),
        GradientStop::new(0.58, color(secondary, alpha_u8(13.0 * strength))),
        GradientStop::new(1.0, color(primary, 0)),
    ];
    let halo = [
        GradientStop::new(0.0, color(primary, alpha_u8(4.0 * strength))),
        GradientStop::new(0.22, color(secondary, alpha_u8(12.0 * strength))),
        GradientStop::new(0.46, color(primary, alpha_u8(37.0 * strength))),
        GradientStop::new(0.72, color(secondary, alpha_u8(11.0 * strength))),
        GradientStop::new(1.0, color(primary, 0)),
    ];
    let stops: &[GradientStop] = match falloff {
        LightFalloff::Diffuse => &diffuse,
        LightFalloff::Focused => &focused,
        LightFalloff::Halo => &halo,
    };
    canvas.fill_rect_radial_gradient(
        (left, top, right - left, bottom - top),
        center,
        radius,
        stops,
    );
}

fn light_source_radius(width: f32, height: f32, source_radius: f32) -> f32 {
    width.min(height * 1.7) * source_radius
}

fn light_audio_drive(features: &AudioFeatures, response: LightAudioResponse) -> f32 {
    let value = match response {
        LightAudioResponse::None => 0.0,
        LightAudioResponse::Energy => features.energy,
        LightAudioResponse::Bass => features.bass,
        LightAudioResponse::Mid => features.mid,
        LightAudioResponse::Treble => features.treble,
        LightAudioResponse::Onset => features.onset,
    };
    finite_or(value, 0.0).clamp(0.0, 1.0)
}

fn custom_light_palette(hue: f32, saturation: f32) -> ([u8; 3], [u8; 3]) {
    (
        hsl_to_rgb(hue, saturation, 0.68),
        hsl_to_rgb((hue + 0.08).rem_euclid(1.0), saturation * 0.82, 0.52),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [u8; 3] {
    let hue = finite_or(hue, 0.0).rem_euclid(1.0);
    let saturation = finite_or(saturation, 0.0).clamp(0.0, 1.0);
    let lightness = finite_or(lightness, 0.0).clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue * 6.0;
    let secondary = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match sector.floor() as u8 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let match_value = lightness - chroma * 0.5;
    [red, green, blue]
        .map(|channel| ((channel + match_value) * 255.0).round().clamp(0.0, 255.0) as u8)
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

/// Draws a reusable sampled spectrum ribbon. Effects can vary geometry,
/// phase, palette, and density without duplicating the audio sampling rules.
#[allow(clippy::too_many_arguments)]
fn draw_spectrum_ribbon(
    canvas: &Canvas,
    state: &VisualState,
    tint: [u8; 3],
    baseline: f32,
    amplitude: f32,
    frequency: f32,
    phase: f32,
    thickness: f32,
    alpha: f32,
) {
    for index in 0_u16..128 {
        let u = f32::from(index) / 127.0;
        let band = spectrum_at(&state.features.spectrum, (u + phase * 0.07).fract());
        let envelope = (u * std::f32::consts::PI).sin().powf(0.52);
        let wave = (u * frequency + phase + state.elapsed * 0.46).sin();
        let harmonic = (u * frequency * 0.47 - phase * 1.7 + state.elapsed * 0.21).cos();
        let x = state.width * (0.08 + u * 0.84);
        let y = baseline + envelope * amplitude * (wave * 0.72 + harmonic * 0.28) * (0.45 + band);
        dot(
            canvas,
            x,
            y,
            thickness * (0.72 + band * 1.25),
            color(tint, alpha_u8(alpha * (0.45 + band * 0.9))),
        );
    }
}

fn draw_prism_ribbons(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center = (state.width * 0.52, state.height * 0.47);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        center,
        state.width.min(state.height) * 0.72,
        &[
            GradientStop::new(0.0, color(preset.accent, 24)),
            GradientStop::new(0.48, color(preset.secondary, 11)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );

    let spread = state.height * (0.045 + state.tuning.depth * 0.018);
    for layer in 0_u16..7 {
        let unit = f32::from(layer) / 6.0;
        let tint = mix_rgb(preset.accent, preset.secondary, unit);
        let drive = spectrum_at(&state.features.spectrum, unit);
        draw_spectrum_ribbon(
            canvas,
            state,
            tint,
            center.1 + (f32::from(layer) - 3.0) * spread,
            state.height * (0.045 + drive * 0.085),
            11.0 + f32::from(layer) * 1.7,
            f32::from(layer) * 0.82,
            0.85 + unit * 0.65,
            54.0 + drive * 92.0,
        );
    }

    for index in 0_u32..90 {
        let depth = hash01(index * 59 + 7);
        let x = hash01(index * 31 + 13) * state.width;
        let travel = (state.elapsed * (0.8 + depth * 2.4)).rem_euclid(state.height * 0.34);
        let y = (hash01(index * 83) * state.height + travel).rem_euclid(state.height);
        dot(
            canvas,
            x,
            y,
            0.55 + depth * 1.35,
            color(
                if index % 2 == 0 {
                    preset.accent
                } else {
                    preset.secondary
                },
                alpha_u8(12.0 + depth * 36.0),
            ),
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_luminous_bloom(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let center_x = state.width * 0.54;
    let center_y = state.height * 0.45;
    let base = state.width.min(state.height) * (0.075 + state.features.bass * 0.022);
    let bloom_radius = base * (3.4 + state.tuning.depth * 0.9);
    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (center_x, center_y),
        bloom_radius * 1.34,
        &[
            GradientStop::new(0.0, color(preset.accent, 44)),
            GradientStop::new(0.32, color(preset.secondary, 20)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );

    for petal in 0_u16..18 {
        let unit = f32::from(petal) / 18.0;
        let band = spectrum_at(&state.features.spectrum, unit);
        let angle = unit * std::f32::consts::TAU
            + state.elapsed * (0.075 + state.features.pitch_confidence * 0.09)
            + (state.elapsed * 0.19 + unit * 8.0).sin() * 0.035;
        let petal_length = bloom_radius * (0.58 + band * 0.56 + state.features.onset * 0.12);
        let tint = mix_rgb(preset.accent, preset.secondary, (unit * 2.0).fract());
        for sample in 0_u16..20 {
            let radial = f32::from(sample) / 19.0;
            let curl = (radial * std::f32::consts::PI).sin() * (0.06 + state.features.mid * 0.10);
            let sample_angle = angle + curl * if petal % 2 == 0 { 1.0 } else { -1.0 };
            let radius = base * 0.72 + petal_length * radial.powf(0.78);
            let x = center_x + sample_angle.cos() * radius;
            let y = center_y + sample_angle.sin() * radius * 0.72;
            let taper = (radial * std::f32::consts::PI).sin().powf(0.55);
            dot(
                canvas,
                x,
                y,
                0.7 + taper * (1.4 + band * 2.2),
                color(tint, alpha_u8(18.0 + taper * (54.0 + band * 104.0))),
            );
        }
    }

    circle(canvas, center_x, center_y, base, rgba(4, 5, 10, 236));
    ring(
        canvas,
        center_x,
        center_y,
        base * (1.02 + state.features.onset * 0.08),
        color(
            preset.accent,
            alpha_u8(110.0 + state.features.onset * 116.0),
        ),
        1.2 + state.features.onset * 2.8,
    );
    circle(
        canvas,
        center_x,
        center_y,
        base * 0.22,
        color(preset.secondary, 190),
    );
}

#[derive(Clone, Copy)]
struct PointFieldSample {
    height: f32,
    tint: [u8; 3],
    alpha: f32,
    size: f32,
}

/// Shared orthographic projection for point-field subjects. The effect owns
/// only its height and colour function; grid generation, depth projection,
/// sizing, and submission stay reusable.
fn draw_projected_point_field(
    canvas: &Canvas,
    state: &VisualState,
    columns: u16,
    rows: u16,
    mut sample: impl FnMut(f32, f32) -> PointFieldSample,
) {
    let center_x = state.width * 0.52;
    let center_y = state.height * 0.46;
    let span_x = state.width.min(state.height * 1.55) * 0.48;
    let span_y = state.height * 0.29;
    for row in 0..rows {
        let v = f32::from(row) / f32::from(rows.saturating_sub(1).max(1));
        for column in 0..columns {
            let u = f32::from(column) / f32::from(columns.saturating_sub(1).max(1));
            let point = sample(u, v);
            let world_x = u - 0.5;
            let world_z = v - 0.5;
            let x = center_x + (world_x - world_z) * span_x;
            let y = center_y + (world_x + world_z) * span_y
                - point.height * state.height * 0.085 * state.tuning.depth;
            let perspective = 0.72 + v * 0.42;
            dot(
                canvas,
                x,
                y,
                point.size * perspective,
                color(point.tint, alpha_u8(point.alpha * perspective)),
            );
        }
    }
}

fn draw_ripple_field(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    let ripple_origin = (
        0.5 + (state.elapsed * 0.17).sin() * 0.18,
        0.5 + (state.elapsed * 0.13).cos() * 0.16,
    );
    draw_projected_point_field(canvas, state, 48, 32, |u, v| {
        let center_distance = (u - 0.5).hypot(v - 0.5);
        let ripple_distance = (u - ripple_origin.0).hypot(v - ripple_origin.1);
        let base = (center_distance * 22.0 - state.elapsed * 2.0).sin() * 0.10;
        let envelope = (1.0 - ripple_distance * 2.2).clamp(0.0, 1.0);
        let ripple = (ripple_distance * 44.0 - state.elapsed * 7.0).cos()
            * envelope
            * (0.08 + state.features.onset * 0.34);
        let band = spectrum_at(&state.features.spectrum, u);
        let height = base + ripple + band * 0.18;
        PointFieldSample {
            height,
            tint: mix_rgb(
                preset.accent,
                preset.secondary,
                (height + 0.24).clamp(0.0, 0.48) / 0.48,
            ),
            alpha: 26.0 + band * 102.0 + envelope * state.features.onset * 86.0,
            size: 0.72 + band * 1.65 + envelope * state.features.onset * 1.4,
        }
    });
}

fn draw_particle_terrain(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    draw_projected_point_field(canvas, state, 52, 34, |u, v| {
        let x = (u - 0.5) * 10.0;
        let z = (v - 0.5) * 10.0;
        let time = state.elapsed;
        let wave = (x * 0.45 + time * 0.80).sin() * 0.34
            + (z * 0.50 - time * 0.65).sin() * 0.29
            + ((x + z) * 0.33 + time * 1.10).sin() * 0.16
            + ((x - z) * 0.70 - time * 1.40).sin() * 0.10;
        let band = spectrum_at(&state.features.spectrum, (u * 0.74 + v * 0.26).fract());
        let height = wave + band * 0.42 + state.features.bass * 0.11;
        let crest = ((height + 0.72) / 1.44).clamp(0.0, 1.0);
        let warm = mix_rgb(preset.secondary, preset.accent, crest);
        PointFieldSample {
            height,
            tint: mix_rgb(warm, [255, 247, 224], (crest - 0.62).max(0.0) * 1.8),
            alpha: 22.0 + crest * 88.0 + band * 56.0,
            size: 0.58 + crest * 1.46 + band * 0.78,
        }
    });
}

#[allow(unsafe_code)]
fn with_affine_rect(
    canvas: &Canvas,
    origin: (f32, f32),
    x_axis: (f32, f32),
    y_axis: (f32, f32),
    draw: impl FnOnce(&Canvas),
) {
    canvas.save();
    let transform = flux::sys::flux_mat3x2 {
        m: [x_axis.0, x_axis.1, y_axis.0, y_axis.1, origin.0, origin.1],
    };
    unsafe {
        flux::sys::flux_canvas_transform(canvas.as_raw(), transform);
    }
    draw(canvas);
    canvas.restore();
}

fn fill_affine_rect(
    canvas: &Canvas,
    origin: (f32, f32),
    x_axis: (f32, f32),
    y_axis: (f32, f32),
    fill: u32,
) {
    with_affine_rect(canvas, origin, x_axis, y_axis, |canvas| {
        canvas.fill_rect(0.0, 0.0, 1.0, 1.0, fill);
    });
}

#[allow(unsafe_code, clippy::too_many_arguments)]
fn draw_affine_image_sub(
    canvas: &Canvas,
    image: *mut flux::sys::flux_image,
    origin: (f32, f32),
    x_axis: (f32, f32),
    y_axis: (f32, f32),
    u: f32,
    v: f32,
    du: f32,
    dv: f32,
) {
    with_affine_rect(canvas, origin, x_axis, y_axis, |canvas| unsafe {
        flux::sys::flux_canvas_draw_image_sub(
            canvas.as_raw(),
            image,
            flux::sys::flux_rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            flux::sys::flux_rect {
                x: u,
                y: v,
                w: du,
                h: dv,
            },
        );
    });
}

#[allow(clippy::cast_precision_loss)]
fn draw_cover_relief(canvas: &Canvas, state: &VisualState, preset: VisualPreset) {
    const COLUMNS: u16 = 10;
    const ROWS: u16 = 10;
    let cell = state.width.min(state.height * 1.45) * 0.043;
    let x_axis = (cell * 0.84, cell * 0.38);
    let y_axis = (-cell * 0.84, cell * 0.38);
    let origin = (
        state.width * 0.52,
        state.height * 0.31 - f32::from(COLUMNS + ROWS) * cell * 0.19,
    );
    let artwork = state
        .artwork
        .map(|image| image as *mut flux::sys::flux_image);
    let base_depth = (cell * 0.34).max(2.0);

    canvas.fill_rect_radial_gradient(
        (0.0, 0.0, state.width, state.height),
        (state.width * 0.52, state.height * 0.52),
        state.width.min(state.height) * 0.55,
        &[
            GradientStop::new(0.0, color(preset.accent, 22)),
            GradientStop::new(0.55, color(preset.secondary, 8)),
            GradientStop::new(1.0, color(preset.secondary, 0)),
        ],
    );

    for diagonal in 0..(COLUMNS + ROWS - 1) {
        for row in 0..ROWS {
            if diagonal < row {
                continue;
            }
            let column = diagonal - row;
            if column >= COLUMNS {
                continue;
            }
            let u = f32::from(column) / f32::from(COLUMNS);
            let v = f32::from(row) / f32::from(ROWS);
            let center_u = (f32::from(column) + 0.5) / f32::from(COLUMNS);
            let center_v = (f32::from(row) + 0.5) / f32::from(ROWS);
            let band = spectrum_at(
                &state.features.spectrum,
                (center_u * 0.72 + center_v * 0.28).fract(),
            );
            let radial = (center_u - 0.5).hypot(center_v - 0.5);
            let pulse = (radial * 22.0 - state.elapsed * 4.0).cos() * state.features.onset;
            let lift =
                (band * 0.82 + state.features.bass * 0.18 + pulse.max(0.0) * 0.34) * cell * 2.4;
            let top = (
                origin.0 + f32::from(column) * x_axis.0 + f32::from(row) * y_axis.0,
                origin.1 + f32::from(column) * x_axis.1 + f32::from(row) * y_axis.1 - lift,
            );
            let extrusion = (0.0, base_depth + lift);
            let left_tint = color(
                mix_rgb(preset.secondary, [4, 6, 11], 0.62),
                alpha_u8(92.0 + band * 92.0),
            );
            let right_tint = color(
                mix_rgb(preset.accent, [5, 7, 12], 0.70),
                alpha_u8(76.0 + band * 82.0),
            );

            fill_affine_rect(
                canvas,
                (top.0 + y_axis.0, top.1 + y_axis.1),
                x_axis,
                extrusion,
                left_tint,
            );
            fill_affine_rect(
                canvas,
                (top.0 + x_axis.0, top.1 + x_axis.1),
                y_axis,
                extrusion,
                right_tint,
            );

            if let Some(image) = artwork {
                draw_affine_image_sub(
                    canvas,
                    image,
                    top,
                    x_axis,
                    y_axis,
                    u,
                    v,
                    1.0 / f32::from(COLUMNS),
                    1.0 / f32::from(ROWS),
                );
            } else {
                let fallback = mix_rgb(preset.accent, preset.secondary, (u + v) * 0.5);
                fill_affine_rect(
                    canvas,
                    top,
                    x_axis,
                    y_axis,
                    color(fallback, alpha_u8(132.0 + band * 92.0)),
                );
            }
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
            let mut state = VisualState {
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
            state.set_stage_viewport(Some((0.0, 0.0, 320.0, 180.0)));
            paint_scene(&canvas, &state, 1.0);
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
    fn every_preset_renders_a_distinct_full_screen_background() {
        let mut hashes = HashSet::new();
        for preset in 0..PRESETS.len() {
            let canvas = Canvas::new_cpu(160, 100, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            paint_scene(
                &canvas,
                &VisualState {
                    width: 160.0,
                    height: 100.0,
                    elapsed: 2.25,
                    preset,
                    ..VisualState::default()
                },
                1.0,
            );
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            let hash = pixels.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
            assert!(
                hashes.insert(hash),
                "preset {preset} duplicated another full-screen background"
            );
        }
    }

    #[test]
    fn isolated_stage_shell_uses_the_standard_diffuse_backdrop() {
        fn render(state: &VisualState, shell: bool) -> Vec<u8> {
            let canvas = Canvas::new_cpu(240, 140, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let preset = PRESETS[state.preset % PRESETS.len()];
            if shell {
                draw_app_backdrop(&canvas, state, preset);
            } else {
                draw_backdrop(&canvas, state, preset);
            }
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        let state = VisualState {
            width: 240.0,
            height: 140.0,
            features: AudioFeatures {
                energy: 0.72,
                ..AudioFeatures::default()
            },
            preset: 2,
            ..VisualState::default()
        };

        assert_eq!(render(&state, true), render(&state, false));
    }

    #[test]
    fn stage_rendering_is_clipped_to_its_owned_viewport() {
        const WIDTH: u16 = 160;
        const HEIGHT: u16 = 100;
        const SCALE: u16 = 2;
        const VIEWPORT: (u16, u16, u16, u16) = (32, 10, 96, 80);

        fn render(state: &VisualState, shell_only: bool) -> (usize, Vec<u8>) {
            let canvas = Canvas::new_cpu(
                u32::from(WIDTH) * u32::from(SCALE),
                u32::from(HEIGHT) * u32::from(SCALE),
                1.0,
            )
            .expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            canvas.scale(f32::from(SCALE), f32::from(SCALE));
            if shell_only {
                let preset = PRESETS[state.preset % PRESETS.len()];
                draw_app_backdrop(&canvas, state, preset);
            } else {
                paint_scene(&canvas, state, f32::from(SCALE));
            }
            canvas.end();
            let (_, _, stride, pixels) = canvas.read_pixels().expect("CPU pixels");
            (stride as usize, pixels.to_vec())
        }

        let shell = VisualState {
            width: f32::from(WIDTH),
            height: f32::from(HEIGHT),
            elapsed: 2.5,
            preset: 3,
            ..VisualState::default()
        };
        let mut staged = shell.clone();
        staged.set_stage_viewport(Some((
            f32::from(VIEWPORT.0),
            f32::from(VIEWPORT.1),
            f32::from(VIEWPORT.2),
            f32::from(VIEWPORT.3),
        )));

        let (shell_stride, shell_pixels) = render(&shell, true);
        let (staged_stride, staged_pixels) = render(&staged, false);
        assert_eq!(shell_stride, staged_stride);

        let mut changed_inside = false;
        for y in 0..HEIGHT * SCALE {
            for x in 0..WIDTH * SCALE {
                let offset = usize::from(y) * shell_stride + usize::from(x) * 4;
                let shell_pixel = &shell_pixels[offset..offset + 4];
                let staged_pixel = &staged_pixels[offset..offset + 4];
                let inside = x >= VIEWPORT.0 * SCALE
                    && x < (VIEWPORT.0 + VIEWPORT.2) * SCALE
                    && y >= VIEWPORT.1 * SCALE
                    && y < (VIEWPORT.1 + VIEWPORT.3) * SCALE;
                if inside {
                    changed_inside |= shell_pixel != staged_pixel;
                } else {
                    assert_eq!(shell_pixel, staged_pixel, "stage leaked at ({x}, {y})");
                }
            }
        }
        assert!(changed_inside, "stage did not render inside its viewport");
    }

    #[test]
    fn full_screen_background_matches_an_equal_sized_stage() {
        fn render(state: &VisualState) -> Vec<u8> {
            let canvas = Canvas::new_cpu(240, 140, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            paint_scene(&canvas, state, 1.0);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        let full_screen = VisualState {
            width: 240.0,
            height: 140.0,
            elapsed: 3.4,
            features: AudioFeatures {
                energy: 0.62,
                peak: 0.74,
                loudness_db: -12.0,
                bass: 0.70,
                mid: 0.52,
                treble: 0.44,
                spectral_centroid_hz: 3_800.0,
                dominant_frequency_hz: 440.0,
                pitch_hz: 440.0,
                pitch_confidence: 0.94,
                spectral_flux: 0.20,
                onset: 0.46,
                spectrum: [0.38; SPECTRUM_BANDS],
                ..AudioFeatures::default()
            },
            preset: 3,
            transition: 0.35,
            ..VisualState::default()
        };
        let mut staged = full_screen.clone();
        staged.set_stage_viewport(Some((0.0, 0.0, 240.0, 140.0)));

        assert_eq!(render(&full_screen), render(&staged));
    }

    #[test]
    fn ambient_radius_keeps_its_horizontal_spread_across_common_aspect_ratios() {
        let stage_radius = light_source_radius(790.0, 1_150.0, 0.48);
        let window_radius = light_source_radius(1_792.0, 1_440.0, 0.48);
        assert!((stage_radius / 790.0 - 0.48).abs() < f32::EPSILON);
        assert!((window_radius / 1_792.0 - 0.48).abs() < f32::EPSILON);

        let ultrawide_radius = light_source_radius(2_560.0, 1_080.0, 0.48);
        assert!((ultrawide_radius - 1_080.0 * 1.7 * 0.48).abs() < f32::EPSILON);
    }

    #[test]
    fn ambient_normalization_preserves_off_window_sources_and_caps_cost() {
        let mut source = LightSource {
            x: -1.25,
            y: 1.4,
            radius: f32::INFINITY,
            aspect: 12.0,
            rotation: f32::NEG_INFINITY,
            intensity: 9.0,
            hue: -0.2,
            saturation: 4.0,
            drift: 0.8,
            audio_intensity: 8.0,
            audio_scale: -4.0,
            ..LightSource::default()
        };
        let mut ambient = AmbientLayer {
            sources: vec![source.clone(); MAX_LIGHT_SOURCES + 2],
            ..AmbientLayer::default()
        }
        .normalized();

        assert_eq!(ambient.sources.len(), MAX_LIGHT_SOURCES);
        source = ambient.sources.remove(0);
        assert!((source.x + 1.25).abs() < f32::EPSILON);
        assert!((source.y - 1.4).abs() < f32::EPSILON);
        assert!((source.radius - LightSource::default().radius).abs() < f32::EPSILON);
        assert!((source.aspect - 4.0).abs() < f32::EPSILON);
        assert!((source.rotation - LightSource::default().rotation).abs() < f32::EPSILON);
        assert!((source.intensity - 2.0).abs() < f32::EPSILON);
        assert!((source.hue - 0.8).abs() < f32::EPSILON);
        assert!((source.saturation - 1.0).abs() < f32::EPSILON);
        assert!((source.drift - 0.18).abs() < f32::EPSILON);
        assert!((source.audio_intensity - 1.5).abs() < f32::EPSILON);
        assert!(source.audio_scale.abs() < f32::EPSILON);
    }

    #[test]
    fn ambient_audio_response_is_per_source_and_bounded() {
        let features = AudioFeatures {
            energy: 0.62,
            bass: 0.81,
            mid: 0.47,
            treble: 1.4,
            onset: f32::NAN,
            ..AudioFeatures::default()
        };

        assert!(light_audio_drive(&features, LightAudioResponse::None).abs() < f32::EPSILON);
        assert!(
            (light_audio_drive(&features, LightAudioResponse::Energy) - 0.62).abs() < f32::EPSILON
        );
        assert!(
            (light_audio_drive(&features, LightAudioResponse::Bass) - 0.81).abs() < f32::EPSILON
        );
        assert!(
            (light_audio_drive(&features, LightAudioResponse::Mid) - 0.47).abs() < f32::EPSILON
        );
        assert!(
            (light_audio_drive(&features, LightAudioResponse::Treble) - 1.0).abs() < f32::EPSILON
        );
        assert!(light_audio_drive(&features, LightAudioResponse::Onset).abs() < f32::EPSILON);
    }

    #[test]
    fn off_window_light_source_contributes_only_its_visible_tail() {
        fn render(ambient: AmbientLayer) -> Vec<u8> {
            let canvas = Canvas::new_cpu(180, 120, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let state = VisualState {
                width: 180.0,
                height: 120.0,
                ambient: Arc::new(ambient),
                ..VisualState::default()
            };
            draw_backdrop(&canvas, &state, PRESETS[0]);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        let dark = render(AmbientLayer {
            sources: Vec::new(),
            ..AmbientLayer::default()
        });
        let lit = render(AmbientLayer {
            sources: vec![LightSource {
                x: -0.18,
                y: 0.5,
                radius: 0.55,
                intensity: 1.5,
                palette: LightPalette::Custom,
                hue: 0.55,
                ..LightSource::default()
            }],
            ..AmbientLayer::default()
        });

        assert_ne!(dark, lit);
    }

    #[test]
    fn ambient_shapes_and_rotation_produce_distinct_footprints() {
        fn render(shape: LightSourceShape, rotation: f32) -> Vec<u8> {
            let canvas = Canvas::new_cpu(180, 120, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let state = VisualState {
                width: 180.0,
                height: 120.0,
                ambient: Arc::new(AmbientLayer {
                    sources: vec![LightSource {
                        x: 0.5,
                        y: 0.5,
                        radius: 0.22,
                        shape,
                        aspect: 2.8,
                        rotation,
                        intensity: 1.6,
                        palette: LightPalette::Custom,
                        hue: 0.58,
                        audio_response: LightAudioResponse::None,
                        ..LightSource::default()
                    }],
                    ..AmbientLayer::default()
                }),
                ..VisualState::default()
            };
            draw_backdrop(&canvas, &state, PRESETS[0]);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        let circle = render(LightSourceShape::Circle, 0.0);
        let oval = render(LightSourceShape::Oval, 0.0);
        let vertical_oval = render(LightSourceShape::Oval, 0.25);
        let beam = render(LightSourceShape::Beam, 0.0);

        assert_ne!(circle, oval);
        assert_ne!(oval, vertical_oval);
        assert_ne!(oval, beam);
    }

    #[test]
    fn disabled_subject_leaves_custom_ambient_independent_of_subject_effect() {
        fn render(preset: usize) -> Vec<u8> {
            let canvas = Canvas::new_cpu(180, 120, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let state = VisualState {
                width: 180.0,
                height: 120.0,
                preset,
                subject_enabled: false,
                ambient: Arc::new(AmbientLayer {
                    sources: vec![LightSource {
                        palette: LightPalette::Custom,
                        hue: 0.12,
                        ..LightSource::default()
                    }],
                    ..AmbientLayer::default()
                }),
                ..VisualState::default()
            };
            draw_stage(&canvas, &state, PRESETS[preset]);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        assert_eq!(render(0), render(4));
    }

    #[test]
    fn subject_and_ambient_layers_can_be_disabled_independently() {
        fn render(subject_enabled: bool, light_enabled: bool) -> Vec<u8> {
            let canvas = Canvas::new_cpu(180, 120, 1.0).expect("CPU canvas");
            canvas
                .begin_cpu(Some(rgba(0, 0, 0, 255)))
                .expect("begin frame");
            let state = VisualState {
                width: 180.0,
                height: 120.0,
                elapsed: 2.6,
                subject_enabled,
                preset: 6,
                features: AudioFeatures {
                    energy: 0.72,
                    bass: 0.63,
                    mid: 0.48,
                    treble: 0.57,
                    spectrum: [0.44; SPECTRUM_BANDS],
                    ..AudioFeatures::default()
                },
                ambient: Arc::new(AmbientLayer {
                    enabled: light_enabled,
                    sources: vec![LightSource {
                        palette: LightPalette::Custom,
                        hue: 0.15,
                        audio_response: LightAudioResponse::None,
                        ..LightSource::default()
                    }],
                    ..AmbientLayer::default()
                }),
                ..VisualState::default()
            };
            draw_stage(&canvas, &state, PRESETS[state.preset]);
            canvas.end();
            let (_, _, _, pixels) = canvas.read_pixels().expect("CPU pixels");
            pixels.to_vec()
        }

        let base = render(false, false);
        let subject_only = render(true, false);
        let ambient_only = render(false, true);
        let both = render(true, true);

        assert_ne!(base, subject_only);
        assert_ne!(base, ambient_only);
        assert_ne!(subject_only, ambient_only);
        assert_ne!(both, subject_only);
        assert_ne!(both, ambient_only);
    }

    #[test]
    fn hsl_conversion_has_stable_primary_colours() {
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), [255, 0, 0]);
        assert_eq!(hsl_to_rgb(1.0 / 3.0, 1.0, 0.5), [0, 255, 0]);
        assert_eq!(hsl_to_rgb(2.0 / 3.0, 1.0, 0.5), [0, 0, 255]);
    }

    fn material_key(kind: LightMaterialKind) -> MaterialTextureKey {
        MaterialTextureKey {
            width: 96,
            height: 64,
            field: LightMaterial {
                kind,
                ..LightMaterial::default()
            },
            anchors: AmbientLayer::default()
                .sources
                .iter()
                .map(MaterialAnchor::from)
                .collect(),
            preset_accent: PRESETS[2].accent,
            preset_secondary: PRESETS[2].secondary,
        }
    }

    #[test]
    fn material_textures_are_bounded_deterministic_and_premultiplied() {
        assert_eq!(material_texture_dimensions(2_560.0, 1_080.0), (512, 224));
        assert_eq!(material_texture_dimensions(800.0, 1_200.0), (341, 512));

        let watercolor_key = material_key(LightMaterialKind::Watercolor);
        let caustics_key = material_key(LightMaterialKind::Caustics);
        let aurora_key = material_key(LightMaterialKind::Aurora);
        let nebula_key = material_key(LightMaterialKind::Nebula);
        let watercolor = generate_material_texture(&watercolor_key);
        let repeated = generate_material_texture(&watercolor_key);
        let caustics = generate_material_texture(&caustics_key);
        let aurora = generate_material_texture(&aurora_key);
        let nebula = generate_material_texture(&nebula_key);

        assert_eq!(watercolor, repeated);
        assert_ne!(watercolor, caustics);
        assert_ne!(caustics, aurora);
        assert_ne!(aurora, nebula);
        for material in [&watercolor, &caustics, &aurora, &nebula] {
            assert!(material.chunks_exact(4).any(|pixel| pixel[3] > 0));
            for pixel in material.chunks_exact(4) {
                assert!(pixel[0] <= pixel[3]);
                assert!(pixel[1] <= pixel[3]);
                assert!(pixel[2] <= pixel[3]);
            }
        }
    }

    #[test]
    fn material_configuration_changes_regenerate_the_field() {
        let first = material_key(LightMaterialKind::Watercolor);
        let mut changed_seed = first.clone();
        changed_seed.field.seed ^= 0xFFFF_0000;
        let mut moved_source = first.clone();
        moved_source.anchors[0].x = -0.4;

        assert_ne!(
            generate_material_texture(&first),
            generate_material_texture(&changed_seed)
        );
        assert_ne!(
            generate_material_texture(&first),
            generate_material_texture(&moved_source)
        );
    }

    #[test]
    fn material_settings_are_normalized_to_safe_ranges() {
        let normalized = LightMaterial {
            intensity: 9.0,
            scale: 0.0,
            motion: f32::INFINITY,
            hue: -0.25,
            saturation: 3.0,
            ..LightMaterial::default()
        }
        .normalized();
        assert!((normalized.intensity - 1.5).abs() < f32::EPSILON);
        assert!((normalized.scale - 0.45).abs() < f32::EPSILON);
        assert!((normalized.motion - LightMaterial::default().motion).abs() < f32::EPSILON);
        assert!((normalized.hue - 0.75).abs() < f32::EPSILON);
        assert!((normalized.saturation - 1.0).abs() < f32::EPSILON);
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

        state.ambient = Arc::new(AmbientLayer {
            sources: vec![LightSource {
                drift: 0.02,
                ..LightSource::default()
            }],
            ..AmbientLayer::default()
        });
        assert!(state.needs_animation_frame());

        state.ambient = Arc::new(AmbientLayer {
            field: LightMaterial {
                kind: LightMaterialKind::Caustics,
                motion: 0.4,
                ..LightMaterial::default()
            },
            sources: Vec::new(),
            ..AmbientLayer::default()
        });
        assert!(state.needs_animation_frame());
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

    #[test]
    fn metric_ranges_match_the_feature_contract() {
        let metrics = normalized_audio_metrics(&AudioFeatures {
            loudness_db: -30.0,
            pitch_hz: 1_200.0,
            pitch_confidence: 0.9,
            spectral_centroid_hz: 45.0,
            onset: 0.25,
            ..AudioFeatures::default()
        });
        assert!((0.5..0.6).contains(&metrics[0]));
        assert!((metrics[1] - 1.0).abs() < f32::EPSILON);
        assert!(metrics[2].abs() < f32::EPSILON);
        assert!((metrics[3] - 0.5).abs() < f32::EPSILON);

        let uncertain_pitch = normalized_audio_metrics(&AudioFeatures {
            pitch_hz: 440.0,
            pitch_confidence: 0.2,
            ..AudioFeatures::default()
        });
        assert!(uncertain_pitch[1].abs() < f32::EPSILON);
    }

    #[test]
    fn metric_release_accelerates_under_gravity() {
        let mut metric = BallisticMetric::default();
        metric.update(1.0, 0.0, 3.0, 6.0, 1.0 / 60.0);
        metric.update(0.0, 0.0, 3.0, 6.0, 0.05);
        let first_drop = 1.0 - metric.level;
        let before_second_drop = metric.level;
        metric.update(0.0, 0.0, 3.0, 6.0, 0.05);
        let second_drop = before_second_drop - metric.level;
        assert!(second_drop > first_drop);
    }
}
