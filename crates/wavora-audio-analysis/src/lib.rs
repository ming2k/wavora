//! Realtime, playback-independent PCM feature extraction.
//!
//! The analyser deliberately has no dependency on the decoder, the audio
//! output backend, or the visual renderer. One feature frame is produced for
//! every interleaved PCM buffer and can be consumed by any visual frontend.

/// Number of logarithmic frequency bands exposed to visual engines.
pub const SPECTRUM_BANDS: usize = 32;

const BAND_FREQUENCIES: [f32; SPECTRUM_BANDS] = [
    45.0, 55.0, 68.0, 84.0, 103.0, 127.0, 157.0, 193.0, 238.0, 293.0, 361.0, 445.0, 548.0, 675.0,
    832.0, 1_025.0, 1_263.0, 1_556.0, 1_917.0, 2_362.0, 2_910.0, 3_585.0, 4_417.0, 5_441.0,
    6_704.0, 8_258.0, 10_172.0, 11_800.0, 13_100.0, 14_200.0, 15_100.0, 16_000.0,
];

/// A compact description of the sound in one PCM window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioFeatures {
    /// Log-normalized root-mean-square amplitude, in the range `0..=1`.
    pub energy: f32,
    /// Linear root-mean-square amplitude before perceptual normalization.
    pub rms: f32,
    /// Maximum absolute sample amplitude in this window.
    pub peak: f32,
    /// Window loudness in dBFS. Silence is clamped to `-120 dBFS`.
    pub loudness_db: f32,
    /// Aggregate low-frequency energy (roughly 45–193 Hz).
    pub bass: f32,
    /// Aggregate mid-frequency energy (roughly 238 Hz–2.4 kHz).
    pub mid: f32,
    /// Aggregate high-frequency energy (roughly 2.9–16 kHz).
    pub treble: f32,
    /// Spectral centre of mass, in hertz.
    pub spectral_centroid_hz: f32,
    /// Strongest sampled spectral frequency, in hertz.
    pub dominant_frequency_hz: f32,
    /// Fundamental-frequency estimate from normalized autocorrelation.
    pub pitch_hz: f32,
    /// Confidence of `pitch_hz`, in the range `0..=1`.
    pub pitch_confidence: f32,
    /// Positive spectral change from the preceding window.
    pub spectral_flux: f32,
    /// Short transient/beat strength, in the range `0..=1`.
    pub onset: f32,
    /// Logarithmic spectrum from 45 Hz to 16 kHz.
    pub spectrum: [f32; SPECTRUM_BANDS],
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            energy: 0.0,
            rms: 0.0,
            peak: 0.0,
            loudness_db: -120.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            spectral_centroid_hz: 0.0,
            dominant_frequency_hz: 0.0,
            pitch_hz: 0.0,
            pitch_confidence: 0.0,
            spectral_flux: 0.0,
            onset: 0.0,
            spectrum: [0.0; SPECTRUM_BANDS],
        }
    }
}

/// Stateful realtime analyser used by the audio worker.
#[derive(Debug, Clone)]
pub struct Analyzer {
    sample_rate: f32,
    channels: usize,
    previous_spectrum: [f32; SPECTRUM_BANDS],
    flux_average: f32,
    previous_bass: f32,
    previous_energy: f32,
}

impl Analyzer {
    /// Creates an analyser for interleaved PCM with a fixed stream format.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            channels: channels.max(1),
            previous_spectrum: [0.0; SPECTRUM_BANDS],
            flux_average: 0.02,
            previous_bass: 0.0,
            previous_energy: 0.0,
        }
    }

    /// Clears history used by transient detection, for example after seeking.
    pub fn reset(&mut self) {
        self.previous_spectrum = [0.0; SPECTRUM_BANDS];
        self.flux_average = 0.02;
        self.previous_bass = 0.0;
        self.previous_energy = 0.0;
    }

    /// Analyses one interleaved `f32` PCM buffer.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn analyze(&mut self, samples: &[f32]) -> AudioFeatures {
        let mono = strongest_channel(samples, self.channels);
        if mono.is_empty() || !self.sample_rate.is_finite() || self.sample_rate <= 0.0 {
            return AudioFeatures::default();
        }

        let mut finite_count = 0_usize;
        let mut square_sum = 0.0_f32;
        let mut peak = 0.0_f32;
        for sample in samples.iter().copied().filter(|sample| sample.is_finite()) {
            finite_count += 1;
            square_sum += sample * sample;
            peak = peak.max(sample.abs());
        }
        if finite_count == 0 {
            return AudioFeatures::default();
        }
        let mean_square = square_sum / finite_count as f32;
        let rms = mean_square.sqrt();
        let loudness_db = if rms <= 0.000_001 {
            -120.0
        } else {
            (20.0 * rms.log10()).max(-120.0)
        };
        let energy = normalize_db(loudness_db);
        let spectrum = goertzel_spectrum(&mono, self.sample_rate);
        let bass = band_average(&spectrum, 0, 8);
        let mid = band_average(&spectrum, 8, 21);
        let treble = band_average(&spectrum, 21, SPECTRUM_BANDS);

        let mut spectral_weight = 0.0;
        let mut spectral_total = 0.0;
        let mut dominant_index = 0;
        let mut dominant_value = 0.0;
        let mut flux = 0.0;
        for (index, (&magnitude, &previous)) in
            spectrum.iter().zip(&self.previous_spectrum).enumerate()
        {
            spectral_weight += BAND_FREQUENCIES[index] * magnitude;
            spectral_total += magnitude;
            if magnitude > dominant_value {
                dominant_value = magnitude;
                dominant_index = index;
            }
            flux += (magnitude - previous).max(0.0);
        }
        flux /= SPECTRUM_BANDS as f32;
        let spectral_centroid_hz = if spectral_total > f32::EPSILON {
            spectral_weight / spectral_total
        } else {
            0.0
        };
        let dominant_frequency_hz = if dominant_value > 0.015 {
            BAND_FREQUENCIES[dominant_index]
        } else {
            0.0
        };
        let (pitch_hz, pitch_confidence) = estimate_pitch(&mono, self.sample_rate, rms);

        let bass_rise = (bass - self.previous_bass).max(0.0);
        let energy_rise = (energy - self.previous_energy).max(0.0);
        let adaptive_threshold = self.flux_average.mul_add(1.7, 0.006);
        let flux_drive = ((flux - adaptive_threshold) / adaptive_threshold.max(0.01)).max(0.0);
        let onset =
            (flux_drive * 0.56 + bass_rise * 1.65 + energy * bass_rise * 0.7 + energy_rise * 0.78)
                .clamp(0.0, 1.0);

        self.previous_spectrum = spectrum;
        self.previous_bass = bass;
        self.previous_energy = energy;
        self.flux_average += (flux - self.flux_average)
            * if flux > self.flux_average {
                0.08
            } else {
                0.025
            };

        AudioFeatures {
            energy,
            rms,
            peak,
            loudness_db,
            bass,
            mid,
            treble,
            spectral_centroid_hz,
            dominant_frequency_hz,
            pitch_hz,
            pitch_confidence,
            spectral_flux: flux,
            onset,
            spectrum,
        }
    }
}

fn strongest_channel(samples: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    let mut channel_energy = vec![0.0_f32; channels];
    for frame in samples.chunks_exact(channels) {
        for (energy, sample) in channel_energy.iter_mut().zip(frame) {
            if sample.is_finite() {
                *energy += sample * sample;
            }
        }
    }
    let selected = channel_energy
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map_or(0, |(index, _)| index);
    samples
        .chunks_exact(channels)
        .map(|frame| {
            let sample = frame[selected];
            if sample.is_finite() { sample } else { 0.0 }
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn goertzel_spectrum(samples: &[f32], sample_rate: f32) -> [f32; SPECTRUM_BANDS] {
    let mut bands = [0.0; SPECTRUM_BANDS];
    let frame_count = samples.len() as f32;
    let nyquist = sample_rate * 0.5;
    let windowed = samples
        .iter()
        .copied()
        .enumerate()
        .map(|(index, sample)| {
            let window = if samples.len() > 1 {
                0.5 - 0.5
                    * (std::f32::consts::TAU * index as f32 / (samples.len() - 1) as f32).cos()
            } else {
                1.0
            };
            sample * window
        })
        .collect::<Vec<_>>();
    for (band, frequency) in bands.iter_mut().zip(BAND_FREQUENCIES) {
        if frequency >= nyquist * 0.98 {
            continue;
        }
        let omega = std::f32::consts::TAU * frequency / sample_rate;
        let coefficient = 2.0 * omega.cos();
        let mut previous = 0.0_f32;
        let mut before_previous = 0.0_f32;
        for sample in &windowed {
            let current = coefficient.mul_add(previous, *sample) - before_previous;
            before_previous = previous;
            previous = current;
        }
        let power = previous.mul_add(
            previous,
            before_previous * before_previous - coefficient * previous * before_previous,
        );
        let magnitude = power.max(0.0).sqrt() * 2.0 / frame_count;
        *band = normalize_db(20.0 * magnitude.max(0.000_001).log10());
    }
    bands
}

fn normalize_db(decibels: f32) -> f32 {
    ((decibels + 60.0) / 54.0).clamp(0.0, 1.0)
}

#[allow(clippy::cast_precision_loss)]
fn band_average(spectrum: &[f32; SPECTRUM_BANDS], start: usize, end: usize) -> f32 {
    let slice = &spectrum[start..end];
    slice.iter().sum::<f32>() / slice.len() as f32
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn estimate_pitch(samples: &[f32], sample_rate: f32, rms: f32) -> (f32, f32) {
    if rms < 0.008 || samples.len() < 96 {
        return (0.0, 0.0);
    }
    // Autocorrelation needs only enough bandwidth for the 50–1,200 Hz pitch
    // range. Decimating high-rate PCM cuts the dominant O(n²) work by roughly
    // an order of magnitude without reducing the reported pitch range.
    let stride = (sample_rate / 12_000.0).ceil().max(1.0) as usize;
    let sample_count = samples.len().div_ceil(stride);
    let pitch_sample_rate = sample_rate / stride as f32;
    let min_lag = (pitch_sample_rate / 1_200.0).ceil().max(2.0) as usize;
    let max_lag = ((pitch_sample_rate / 50.0).ceil() as usize).min(sample_count / 2);
    if min_lag >= max_lag {
        return (0.0, 0.0);
    }

    let mean = (0..sample_count)
        .map(|index| samples[index * stride])
        .sum::<f32>()
        / sample_count as f32;
    let mut best_lag = min_lag;
    let mut best_correlation = 0.0_f32;
    for lag in min_lag..=max_lag {
        let mut cross = 0.0;
        let mut left_energy = 0.0;
        let mut right_energy = 0.0;
        for index in 0..sample_count - lag {
            let left = samples[index * stride] - mean;
            let right = samples[(index + lag) * stride] - mean;
            cross += left * right;
            left_energy += left * left;
            right_energy += right * right;
        }
        let correlation = cross / (left_energy * right_energy).sqrt().max(0.000_001);
        // Periodic signals have nearly identical peaks at integer multiples
        // of the fundamental. Require a meaningful improvement before moving
        // to a longer lag so tiny floating-point differences do not report a
        // subharmonic (for example 110 Hz for a clean 440 Hz tone).
        if correlation > best_correlation + 0.01 {
            best_correlation = correlation;
            best_lag = lag;
        }
    }
    let confidence = if best_lag == min_lag || best_lag == max_lag {
        0.0
    } else {
        ((best_correlation - 0.35) / 0.6).clamp(0.0, 1.0)
    };
    if confidence <= 0.0 {
        (0.0, 0.0)
    } else {
        (pitch_sample_rate / best_lag as f32, confidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn extracts_frequency_pitch_and_loudness() {
        const SAMPLE_RATE: u32 = 48_000;
        let samples = (0..4_096)
            .map(|index| {
                (std::f32::consts::TAU * 440.0 * index as f32 / SAMPLE_RATE as f32).sin() * 0.5
            })
            .collect::<Vec<_>>();
        let mut analyzer = Analyzer::new(SAMPLE_RATE, 1);
        let features = analyzer.analyze(&samples);

        assert!(features.energy > 0.7);
        assert!((-12.0..=-6.0).contains(&features.loudness_db));
        assert!((400.0..=500.0).contains(&features.dominant_frequency_hz));
        assert!(
            (430.0..=450.0).contains(&features.pitch_hz),
            "features: {features:?}"
        );
        assert!(features.pitch_confidence > 0.8);
        assert!(features.mid > features.bass);
    }

    #[test]
    fn silence_has_no_false_pitch_or_onset() {
        let mut analyzer = Analyzer::new(48_000, 2);
        let features = analyzer.analyze(&vec![0.0; 4_096]);
        assert_eq!(features, AudioFeatures::default());
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn stereo_antiphase_does_not_cancel_visual_features() {
        const SAMPLE_RATE: u32 = 48_000;
        let samples = (0..2_048)
            .flat_map(|index| {
                let sample =
                    (std::f32::consts::TAU * 220.0 * index as f32 / SAMPLE_RATE as f32).sin() * 0.4;
                [sample, -sample]
            })
            .collect::<Vec<_>>();
        let features = Analyzer::new(SAMPLE_RATE, 2).analyze(&samples);
        assert!(features.energy > 0.6);
        assert!(
            (210.0..=230.0).contains(&features.pitch_hz),
            "features: {features:?}"
        );
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn detects_a_transient_after_silence() {
        let mut analyzer = Analyzer::new(8_000, 1);
        let _ = analyzer.analyze(&vec![0.0; 2_048]);
        let impulse = (0..2_048)
            .map(|index| {
                if index < 64 {
                    0.9 - index as f32 / 80.0
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        let features = analyzer.analyze(&impulse);
        assert!(features.onset > 0.25, "features: {features:?}");
    }
}
