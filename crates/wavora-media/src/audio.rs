use crate::file_uri_to_path;
use crossbeam_channel::{Receiver, Sender};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use rodio::{Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use wavora_core::PlaybackState;

pub const SPECTRUM_BANDS: usize = 16;
const ANALYSIS_FREQUENCIES: [f32; SPECTRUM_BANDS] = [
    55.0, 75.0, 100.0, 135.0, 180.0, 240.0, 320.0, 430.0, 575.0, 770.0, 1_030.0, 1_380.0, 1_850.0,
    2_480.0, 3_100.0, 3_700.0,
];
const FRAMES_PER_BUFFER: usize = 2_048;

#[derive(Debug)]
enum AudioCommand {
    Load(String),
    Play,
    Pause,
    Seek(u64),
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone, Error)]
pub enum AudioError {
    #[error("invalid local file URI")]
    InvalidUri,
    #[error("could not open {path}: {message}")]
    Open { path: PathBuf, message: String },
    #[error("this audio stream is not supported: {0}")]
    Decode(String),
    #[error("audio output is unavailable: {0}")]
    Output(String),
    #[error("seeking failed: {0}")]
    Seek(String),
    #[error("audio pipeline failed: {0}")]
    Pipeline(String),
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Position {
        position_ms: u64,
        duration_ms: u64,
    },
    State(PlaybackState),
    Analysis {
        energy: f32,
        bands: [f32; SPECTRUM_BANDS],
    },
    EndOfStream,
    Error(AudioError),
}

pub struct AudioController {
    commands: Sender<AudioCommand>,
    events: Receiver<AudioEvent>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AudioController {
    /// Starts the dedicated playback worker.
    ///
    /// # Errors
    ///
    /// Returns the operating-system thread creation error.
    pub fn spawn(initial_volume: f32) -> std::io::Result<Self> {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let worker = thread::Builder::new()
            .name("wavora-audio".to_owned())
            .spawn(move || audio_worker(&command_rx, &event_tx, initial_volume))?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            worker: Some(worker),
        })
    }

    pub fn load(&self, uri: impl Into<String>) {
        let _ = self.commands.send(AudioCommand::Load(uri.into()));
    }

    pub fn play(&self) {
        let _ = self.commands.send(AudioCommand::Play);
    }

    pub fn pause(&self) {
        let _ = self.commands.send(AudioCommand::Pause);
    }

    pub fn seek(&self, milliseconds: u64) {
        let _ = self.commands.send(AudioCommand::Seek(milliseconds));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.commands.send(AudioCommand::SetVolume(volume));
    }

    pub fn try_iter(&self) -> impl Iterator<Item = AudioEvent> + '_ {
        self.events.try_iter()
    }
}

impl Drop for AudioController {
    fn drop(&mut self) {
        let _ = self.commands.send(AudioCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn audio_worker(
    commands: &Receiver<AudioCommand>,
    events: &Sender<AudioEvent>,
    initial_volume: f32,
) {
    if let Err(error) = gst::init() {
        let _ = events.send(AudioEvent::Error(AudioError::Output(error.to_string())));
        return;
    }
    let mut playback: Option<Playback> = None;
    let mut volume = initial_volume.clamp(0.0, 1.0);
    let mut requested_state = PlaybackState::Stopped;
    let mut eos_reported = false;
    let mut tick = 0_u8;

    loop {
        match commands.recv_timeout(Duration::from_millis(16)) {
            Ok(AudioCommand::Load(uri)) => {
                let _ = events.send(AudioEvent::State(PlaybackState::Buffering));
                match Playback::open(&uri, volume, events) {
                    Ok(new_playback) => {
                        playback = Some(new_playback);
                        requested_state = PlaybackState::Playing;
                        eos_reported = false;
                        let _ = events.send(AudioEvent::State(PlaybackState::Playing));
                    }
                    Err(error) => {
                        playback = None;
                        requested_state = PlaybackState::Stopped;
                        let _ = events.send(AudioEvent::Error(error));
                    }
                }
            }
            Ok(AudioCommand::Play) => {
                if let Some(active) = playback.as_ref() {
                    if let Err(error) = active.set_state(gst::State::Playing) {
                        let _ = events.send(AudioEvent::Error(error));
                    } else {
                        requested_state = PlaybackState::Playing;
                        let _ = events.send(AudioEvent::State(PlaybackState::Playing));
                    }
                }
            }
            Ok(AudioCommand::Pause) => {
                if let Some(active) = playback.as_ref() {
                    if let Err(error) = active.set_state(gst::State::Paused) {
                        let _ = events.send(AudioEvent::Error(error));
                    } else {
                        requested_state = PlaybackState::Paused;
                        let _ = events.send(AudioEvent::State(PlaybackState::Paused));
                    }
                }
            }
            Ok(AudioCommand::Seek(milliseconds)) => {
                if let Some(active) = playback.as_ref()
                    && let Err(error) = active.seek(milliseconds)
                {
                    let _ = events.send(AudioEvent::Error(error));
                }
                eos_reported = false;
            }
            Ok(AudioCommand::SetVolume(value)) => {
                volume = value.clamp(0.0, 1.0);
                if let Some(active) = playback.as_ref() {
                    active.volume.set_property("volume", f64::from(volume));
                }
            }
            Ok(AudioCommand::Shutdown) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        if let Some(active) = playback.as_ref() {
            while let Some(message) = active.bus.timed_pop_filtered(
                gst::ClockTime::ZERO,
                &[gst::MessageType::Eos, gst::MessageType::Error],
            ) {
                match message.view() {
                    gst::MessageView::Eos(_) => {
                        if !eos_reported {
                            eos_reported = true;
                            requested_state = PlaybackState::Stopped;
                            let _ = events.send(AudioEvent::EndOfStream);
                        }
                    }
                    gst::MessageView::Error(error) => {
                        requested_state = PlaybackState::Stopped;
                        let detail = error.debug().map_or_else(
                            || error.error().to_string(),
                            |debug| format!("{} ({debug})", error.error()),
                        );
                        let _ = events.send(AudioEvent::Error(AudioError::Pipeline(detail)));
                    }
                    _ => {}
                }
            }
            tick = tick.wrapping_add(1);
            if tick.is_multiple_of(6) {
                let position_ms = active.position_ms();
                let _ = events.send(AudioEvent::Position {
                    position_ms,
                    duration_ms: active.duration_ms,
                });
                if !eos_reported
                    && requested_state == PlaybackState::Playing
                    && active.duration_ms > 0
                    && position_ms >= active.duration_ms.saturating_sub(20)
                {
                    eos_reported = true;
                    let _ = events.send(AudioEvent::EndOfStream);
                }
            }
        }
    }
    drop(playback);
}

type FileDecoder = Decoder<BufReader<File>>;

struct DecodeState {
    decoder: FileDecoder,
    channels: usize,
    sample_rate: u32,
    next_frame: u64,
    eos: bool,
}

struct Playback {
    pipeline: gst::Pipeline,
    bus: gst::Bus,
    volume: gst::Element,
    duration_ms: u64,
}

impl Playback {
    fn open(uri: &str, volume: f32, events: &Sender<AudioEvent>) -> Result<Self, AudioError> {
        let path = file_uri_to_path(uri).ok_or(AudioError::InvalidUri)?;
        let decoder = open_decoder(&path)?;
        let channels = usize::from(decoder.channels().get());
        let sample_rate = decoder.sample_rate().get();
        let duration_ms = decoder
            .total_duration()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .unwrap_or_default();
        let state = Arc::new(Mutex::new(DecodeState {
            decoder,
            channels,
            sample_rate,
            next_frame: 0,
            eos: false,
        }));

        let appsrc = gst::ElementFactory::make("appsrc")
            .name("wavora-decoded-audio")
            .build()
            .map_err(|error| AudioError::Output(error.to_string()))?
            .downcast::<gst_app::AppSrc>()
            .map_err(|_| AudioError::Output("appsrc has the wrong runtime type".to_owned()))?;
        let format = if cfg!(target_endian = "little") {
            "F32LE"
        } else {
            "F32BE"
        };
        let channels_i32 = i32::try_from(channels)
            .map_err(|_| AudioError::Decode("too many audio channels".to_owned()))?;
        let rate_i32 = i32::try_from(sample_rate)
            .map_err(|_| AudioError::Decode("sample rate is too large".to_owned()))?;
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", format)
            .field("layout", "interleaved")
            .field("channels", channels_i32)
            .field("rate", rate_i32)
            .build();
        appsrc.set_caps(Some(&caps));
        appsrc.set_format(gst::Format::Time);
        appsrc.set_stream_type(gst_app::AppStreamType::Seekable);
        appsrc.set_block(true);
        appsrc.set_max_bytes(u64::from(sample_rate) * u64::try_from(channels).unwrap_or(2) * 4 / 4);
        if duration_ms > 0 {
            appsrc.set_duration(gst::ClockTime::from_mseconds(duration_ms));
        }

        let analysis_events = events.clone();
        let need_state = state.clone();
        let seek_state = state;
        appsrc.set_callbacks(
            gst_app::AppSrcCallbacks::builder()
                .need_data(move |source, _| {
                    push_decoded_buffer(source, &need_state, &analysis_events);
                })
                .seek_data(move |_, offset| seek_decoder(&seek_state, offset))
                .build(),
        );

        let convert = make_element("audioconvert", "wavora-convert")?;
        let resample = make_element("audioresample", "wavora-resample")?;
        let volume_element = make_element("volume", "wavora-volume")?;
        volume_element.set_property("volume", f64::from(volume));
        let sink = build_audio_sink()?;
        let pipeline = gst::Pipeline::with_name("wavora-playback");
        pipeline
            .add_many([
                appsrc.upcast_ref(),
                &convert,
                &resample,
                &volume_element,
                &sink,
            ])
            .map_err(|error| AudioError::Pipeline(error.to_string()))?;
        gst::Element::link_many([
            appsrc.upcast_ref(),
            &convert,
            &resample,
            &volume_element,
            &sink,
        ])
        .map_err(|error| AudioError::Pipeline(error.to_string()))?;
        let bus = pipeline
            .bus()
            .ok_or_else(|| AudioError::Pipeline("pipeline has no message bus".to_owned()))?;
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| AudioError::Output(error.to_string()))?;
        Ok(Self {
            pipeline,
            bus,
            volume: volume_element,
            duration_ms,
        })
    }

    fn set_state(&self, state: gst::State) -> Result<(), AudioError> {
        self.pipeline
            .set_state(state)
            .map(|_| ())
            .map_err(|error| AudioError::Output(error.to_string()))
    }

    fn seek(&self, milliseconds: u64) -> Result<(), AudioError> {
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::ClockTime::from_mseconds(milliseconds),
            )
            .map_err(|error| AudioError::Seek(error.to_string()))
    }

    fn position_ms(&self) -> u64 {
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map_or(0, gst::ClockTime::mseconds)
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn open_decoder(path: &Path) -> Result<FileDecoder, AudioError> {
    let file = File::open(path).map_err(|error| AudioError::Open {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    Decoder::try_from(file).map_err(|error| AudioError::Decode(error.to_string()))
}

fn make_element(factory: &str, name: &str) -> Result<gst::Element, AudioError> {
    gst::ElementFactory::make(factory)
        .name(name)
        .build()
        .map_err(|error| AudioError::Output(format!("{factory}: {error}")))
}

fn build_audio_sink() -> Result<gst::Element, AudioError> {
    #[cfg(test)]
    {
        let sink = make_element("fakesink", "wavora-test-sink")?;
        sink.set_property("sync", false);
        sink.set_property("async", false);
        Ok(sink)
    }
    #[cfg(not(test))]
    {
        ["pipewiresink", "pulsesink", "autoaudiosink", "alsasink"]
            .into_iter()
            .find_map(|factory| gst::ElementFactory::make(factory).build().ok())
            .ok_or_else(|| {
                AudioError::Output(
                    "no PipeWire, PulseAudio, automatic, or ALSA sink is installed".to_owned(),
                )
            })
    }
}

#[allow(clippy::cast_precision_loss)]
fn push_decoded_buffer(
    appsrc: &gst_app::AppSrc,
    state: &Mutex<DecodeState>,
    events: &Sender<AudioEvent>,
) {
    let Ok(mut state) = state.lock() else {
        let _ = appsrc.end_of_stream();
        return;
    };
    if state.eos {
        return;
    }
    let sample_capacity = FRAMES_PER_BUFFER.saturating_mul(state.channels);
    let mut samples = Vec::with_capacity(sample_capacity);
    for _ in 0..sample_capacity {
        let Some(sample) = state.decoder.next() else {
            break;
        };
        samples.push(sample);
    }
    if samples.is_empty() {
        state.eos = true;
        drop(state);
        let _ = appsrc.end_of_stream();
        return;
    }
    let complete_samples = samples.len() - samples.len() % state.channels;
    samples.truncate(complete_samples);
    let frame_count = samples.len() / state.channels;
    let start_frame = state.next_frame;
    state.next_frame = state
        .next_frame
        .saturating_add(u64::try_from(frame_count).unwrap_or_default());
    let channels = state.channels;
    let sample_rate = state.sample_rate;
    drop(state);

    let (energy, bands) = analyse_samples(&samples, channels, sample_rate as f32);
    let _ = events.try_send(AudioEvent::Analysis { energy, bands });
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_ne_bytes());
    }
    let mut buffer = gst::Buffer::from_mut_slice(bytes);
    if let Some(buffer) = buffer.get_mut() {
        let pts_ns = start_frame.saturating_mul(1_000_000_000) / u64::from(sample_rate);
        let duration_ns = u64::try_from(frame_count)
            .unwrap_or_default()
            .saturating_mul(1_000_000_000)
            / u64::from(sample_rate);
        buffer.set_pts(gst::ClockTime::from_nseconds(pts_ns));
        buffer.set_duration(gst::ClockTime::from_nseconds(duration_ns));
    }
    let _ = appsrc.push_buffer(buffer);
}

fn seek_decoder(state: &Mutex<DecodeState>, offset_ns: u64) -> bool {
    let Ok(mut state) = state.lock() else {
        return false;
    };
    match state.decoder.try_seek(Duration::from_nanos(offset_ns)) {
        Ok(()) => {
            state.next_frame =
                offset_ns.saturating_mul(u64::from(state.sample_rate)) / 1_000_000_000;
            state.eos = false;
            true
        }
        Err(_) => false,
    }
}

#[allow(clippy::cast_precision_loss)]
fn analyse_samples(
    samples: &[f32],
    channels: usize,
    sample_rate: f32,
) -> (f32, [f32; SPECTRUM_BANDS]) {
    let channels = channels.max(1);
    let frame_count = samples.len() / channels;
    if frame_count == 0 || !sample_rate.is_finite() || sample_rate <= 0.0 {
        return (0.0, [0.0; SPECTRUM_BANDS]);
    }
    let mean_square = samples
        .iter()
        .copied()
        .filter(|sample| sample.is_finite())
        .map(|sample| sample * sample)
        .sum::<f32>()
        / samples.len() as f32;
    let energy = normalize_magnitude(mean_square.sqrt());
    let mut bands = [0.0; SPECTRUM_BANDS];
    for (band, frequency) in bands.iter_mut().zip(ANALYSIS_FREQUENCIES) {
        let omega = std::f32::consts::TAU * frequency / sample_rate;
        let coefficient = 2.0 * omega.cos();
        let mut previous = 0.0_f32;
        let mut before_previous = 0.0_f32;
        for sample in samples.iter().step_by(channels).copied() {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            let current = coefficient.mul_add(previous, sample) - before_previous;
            before_previous = previous;
            previous = current;
        }
        let power = previous.mul_add(
            previous,
            before_previous * before_previous - coefficient * previous * before_previous,
        );
        let magnitude = power.max(0.0).sqrt() / frame_count as f32;
        *band = normalize_magnitude(magnitude * 2.0);
    }
    (energy, bands)
}

fn normalize_magnitude(magnitude: f32) -> f32 {
    let decibels = 20.0 * magnitude.max(0.000_01).log10();
    ((decibels + 60.0) / 54.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_to_file_uri;
    use std::time::Instant;

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn pcm_analysis_distinguishes_silence_and_signal() {
        let silence = vec![0.0_f32; 8_000];
        let (silent_energy, silent_bands) = analyse_samples(&silence, 1, 8_000.0);
        assert!(silent_energy.abs() < f32::EPSILON);
        assert!(silent_bands.iter().all(|band| band.abs() < f32::EPSILON));

        let tone = (0..8_000)
            .map(|index| (std::f32::consts::TAU * 430.0 * index as f32 / 8_000.0).sin() * 0.5)
            .collect::<Vec<_>>();
        let (tone_energy, tone_bands) = analyse_samples(&tone, 1, 8_000.0);
        assert!(tone_energy > 0.7);
        let strongest = tone_bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index);
        assert_eq!(strongest, Some(7));
    }

    #[test]
    fn controller_decodes_plays_and_analyses_wav_without_gstreamer_decoder_plugins() {
        let path = std::env::temp_dir().join(format!(
            "wavora-audio-controller-{}.wav",
            std::process::id()
        ));
        write_test_wav(&path);
        let controller = AudioController::spawn(0.0).expect("start audio controller");
        controller.load(path_to_file_uri(&path));

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut playing = false;
        let mut analysed = false;
        let mut reached_eos = false;
        let mut seen = Vec::new();
        while Instant::now() < deadline && !reached_eos {
            for event in controller.try_iter() {
                seen.push(format!("{event:?}"));
                match event {
                    AudioEvent::State(PlaybackState::Playing) => playing = true,
                    AudioEvent::Analysis { energy, .. } if energy > 0.4 => analysed = true,
                    AudioEvent::EndOfStream => reached_eos = true,
                    AudioEvent::Error(error) => panic!("playback failed: {error}"),
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(controller);
        let _ = std::fs::remove_file(path);
        assert!(playing, "controller never played; events: {seen:?}");
        assert!(
            analysed,
            "controller emitted no PCM analysis; events: {seen:?}"
        );
        assert!(
            reached_eos,
            "controller did not reach EOS; events: {seen:?}"
        );
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn write_test_wav(path: &Path) {
        const SAMPLE_RATE: u32 = 8_000;
        const SAMPLE_COUNT: u32 = 4_000;
        let data_len = SAMPLE_COUNT * 2;
        let mut bytes = Vec::with_capacity(44 + usize::try_from(data_len).unwrap_or_default());
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for index in 0..SAMPLE_COUNT {
            let phase = std::f32::consts::TAU * 430.0 * index as f32 / SAMPLE_RATE as f32;
            let sample = (phase.sin() * 16_000.0) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).expect("write WAV fixture");
    }
}
