use crate::config::{AppConfig, ConfigError, ConfigStore};
use iris::{Application, Config, Input, TextBuf, request_animation_frame};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;
use wavora_core::{PlaybackState, Track};
use wavora_i18n::{Key, Language, LanguagePreference, text};
use wavora_media::{
    AudioController, AudioEvent, AudioFeatures, LibraryEvent, LibraryScanner, file_uri_to_path,
    is_supported_audio, path_to_file_uri,
};
use wavora_visuals::{PRESETS, SharedVisualState, VisualTuning, shared_state};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Window(#[from] iris::RunError),
    #[error("failed to start a background worker: {0}")]
    Worker(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    Library,
    Favorites,
    Visuals,
    Settings,
}

pub struct App {
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub playback_state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub seek_ratio: f32,
    pub volume: f32,
    pub preset: usize,
    pub visual_tuning: VisualTuning,
    pub view: View,
    pub search: TextBuf,
    pub scanning: bool,
    language: Language,
    audio_features: AudioFeatures,
    active_scans: usize,
    config: AppConfig,
    config_store: ConfigStore,
    scanner: LibraryScanner,
    audio: AudioController,
    visuals: SharedVisualState,
    loaded_uri: Option<String>,
    autoplay_uri: Option<String>,
    preset_override: Option<usize>,
    dirty_config: bool,
    config_save_due: Instant,
    last_frame: Instant,
    toast: Option<(String, Instant)>,
    toast_is_error: bool,
    playback_error: Option<String>,
}

impl App {
    fn new(
        mut config: AppConfig,
        config_store: ConfigStore,
        paths: &[PathBuf],
        visuals: SharedVisualState,
    ) -> Result<Self, AppError> {
        config.normalize();
        let mut dirty_config = false;
        let mut autoplay_uri = None;
        for path in paths {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if canonical.is_dir() {
                if !config.library_roots.contains(&canonical) {
                    config.library_roots.push(canonical);
                    dirty_config = true;
                }
            } else if is_supported_audio(&canonical) {
                let uri = path_to_file_uri(&canonical);
                config.remember_uri(&uri);
                autoplay_uri.get_or_insert(uri);
                dirty_config = true;
            }
        }
        config.normalize();
        let scanner = LibraryScanner::spawn()?;
        let mut scheduled = HashSet::new();
        for root in &config.library_roots {
            scheduled.insert(root.clone());
        }
        for path in paths.iter().filter(|path| path.is_file()) {
            scheduled.insert(path.canonicalize().unwrap_or_else(|_| path.clone()));
        }
        for uri in &config.recent_uris {
            if let Some(path) = file_uri_to_path(uri) {
                scheduled.insert(path);
            }
        }
        for path in scheduled {
            scanner.scan(path);
        }
        let volume = config.volume;
        let preset = config.visual_preset;
        let visual_tuning = VisualTuning {
            intensity: config.visual_intensity,
            motion: config.visual_motion,
            depth: config.visual_depth,
            glow: config.visual_glow,
        }
        .normalized();
        let language = config.language.resolve();
        let view = if config.library_roots.is_empty() {
            View::Home
        } else {
            View::Library
        };
        Ok(Self {
            tracks: Vec::new(),
            current_index: None,
            playback_state: PlaybackState::Stopped,
            position_ms: 0,
            duration_ms: 0,
            seek_ratio: 0.0,
            volume,
            preset,
            visual_tuning,
            view,
            search: TextBuf::new(256, ""),
            scanning: false,
            language,
            audio_features: AudioFeatures::default(),
            active_scans: 0,
            config,
            config_store,
            scanner,
            audio: AudioController::spawn(volume)?,
            visuals,
            loaded_uri: None,
            autoplay_uri,
            preset_override: None,
            dirty_config,
            config_save_due: Instant::now() + Duration::from_millis(350),
            last_frame: Instant::now(),
            toast: None,
            toast_is_error: false,
            playback_error: None,
        })
    }

    pub fn tick(&mut self, input: &Input) {
        let library_events: Vec<_> = self.scanner.try_iter().collect();
        for event in library_events {
            self.handle_library_event(event);
        }
        let audio_events: Vec<_> = self.audio.try_iter().collect();
        for event in audio_events {
            self.handle_audio_event(event);
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let raw = input.as_raw();
        let animation_pending = if let Ok(mut visual) = self.visuals.write() {
            visual.update(
                dt,
                raw.display_size.x,
                raw.display_size.y,
                self.playback_state.is_playing(),
                self.seek_ratio,
                self.preset,
                self.audio_features,
                self.visual_tuning,
                self.view == View::Visuals,
            );
            visual.needs_animation_frame()
        } else {
            false
        };
        if animation_pending {
            request_animation_frame();
        }
        if self.dirty_config && now >= self.config_save_due {
            self.save_config();
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| now.duration_since(*at).as_secs() > 4)
        {
            self.toast = None;
        }
    }

    fn handle_library_event(&mut self, event: LibraryEvent) {
        match event {
            LibraryEvent::ScanStarted(_) => {
                self.active_scans = self.active_scans.saturating_add(1);
                self.scanning = true;
            }
            LibraryEvent::Track(mut track) => {
                track.favorite = self.config.favorite_uris.contains(&track.uri);
                let track_uri = track.uri.clone();
                let current_uri = self.current_track().map(|current| current.uri.clone());
                if self.tracks.iter().all(|existing| existing.uri != track.uri) {
                    self.tracks.push(track);
                    self.tracks.sort_by(|a, b| {
                        a.artist
                            .to_lowercase()
                            .cmp(&b.artist.to_lowercase())
                            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                    });
                }
                if let Some(current_uri) = current_uri {
                    self.current_index = self
                        .tracks
                        .iter()
                        .position(|candidate| candidate.uri == current_uri);
                }
                if self.autoplay_uri.as_deref() == Some(track_uri.as_str()) {
                    self.autoplay_uri = None;
                    if let Some(index) = self
                        .tracks
                        .iter()
                        .position(|candidate| candidate.uri == track_uri)
                    {
                        self.play_index(index);
                    }
                }
            }
            LibraryEvent::ScanFinished {
                discovered,
                rejected,
                ..
            } => {
                self.active_scans = self.active_scans.saturating_sub(1);
                self.scanning = self.active_scans > 0;
                if rejected > 0 {
                    self.set_toast(
                        format!("{rejected} {}", text(self.language, Key::ScanSummary)),
                        true,
                    );
                } else if discovered > 0 {
                    self.set_toast(
                        format!("{discovered} {}", text(self.language, Key::AddedTracks)),
                        false,
                    );
                }
                if self.current_index.is_none()
                    && let Some(uri) = self.config.last_track_uri.clone()
                {
                    self.current_index = self.tracks.iter().position(|track| track.uri == uri);
                }
            }
            LibraryEvent::Error(error) => {
                self.active_scans = 0;
                self.scanning = false;
                self.set_toast(error, true);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn handle_audio_event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::Position {
                position_ms,
                duration_ms,
            } => {
                self.position_ms = position_ms;
                if duration_ms > 0 {
                    self.duration_ms = duration_ms;
                }
                self.seek_ratio = if self.duration_ms == 0 {
                    0.0
                } else {
                    (self.position_ms as f64 / self.duration_ms as f64) as f32
                };
            }
            AudioEvent::State(state) => self.playback_state = state,
            AudioEvent::Analysis(features) => self.audio_features = features,
            AudioEvent::EndOfStream => self.next(),
            AudioEvent::Error(error) => {
                self.playback_state = PlaybackState::Stopped;
                self.loaded_uri = None;
                self.playback_error = Some(format!(
                    "{}: {error}",
                    text(self.language, Key::PlaybackFailed)
                ));
            }
        }
    }

    pub fn pick_music_file(&mut self) {
        if let Some(uri) = iris::pick_file(Some(text(self.language, Key::AddFile))) {
            if let Some(path) = file_uri_to_path(&uri) {
                if !is_supported_audio(&path) {
                    self.set_toast(text(self.language, Key::UnsupportedFile).to_owned(), true);
                    return;
                }
                self.scanner.scan(path);
                self.config.remember_uri(&uri);
                self.mark_config_dirty();
            } else {
                self.set_toast(text(self.language, Key::InvalidFilePath).to_owned(), true);
            }
        }
    }

    pub fn pick_music_folder(&mut self) {
        if let Some(uri) = iris::pick_folder(Some(text(self.language, Key::AddFolder))) {
            if let Some(path) = file_uri_to_path(&uri) {
                let canonical = path.canonicalize().unwrap_or(path);
                if !canonical.is_dir() {
                    self.set_toast(text(self.language, Key::InvalidFolder).to_owned(), true);
                    return;
                }
                if !self.config.library_roots.contains(&canonical) {
                    self.config.library_roots.push(canonical.clone());
                    self.config.normalize();
                    self.mark_config_dirty();
                }
                self.scanner.scan(canonical);
            } else {
                self.set_toast(text(self.language, Key::InvalidFolderPath).to_owned(), true);
            }
        }
    }

    pub fn play_index(&mut self, index: usize) {
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let uri = track.uri.clone();
        let duration_ms = track.duration_ms;
        self.current_index = Some(index);
        self.position_ms = 0;
        self.duration_ms = duration_ms;
        self.seek_ratio = 0.0;
        self.playback_state = PlaybackState::Buffering;
        self.playback_error = None;
        self.audio.load(uri.clone());
        self.loaded_uri = Some(uri.clone());
        self.config.remember_uri(&uri);
        self.mark_config_dirty();
    }

    pub fn toggle_playback(&mut self) {
        let Some(index) = self.current_index else {
            if !self.tracks.is_empty() {
                self.play_index(0);
            }
            return;
        };
        let needs_load = self
            .tracks
            .get(index)
            .is_some_and(|track| self.loaded_uri.as_deref() != Some(track.uri.as_str()));
        if needs_load {
            self.play_index(index);
            return;
        }
        if self.playback_state.is_playing() {
            self.audio.pause();
        } else {
            self.audio.play();
        }
    }

    pub fn next(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        let next = self
            .current_index
            .map_or(0, |index| (index + 1) % self.tracks.len());
        self.play_index(next);
    }

    pub fn previous(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        if self.position_ms > 4_000 {
            self.audio.seek(0);
            return;
        }
        let previous = self.current_index.map_or(0, |index| {
            if index == 0 {
                self.tracks.len() - 1
            } else {
                index - 1
            }
        });
        self.play_index(previous);
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub fn commit_seek(&self) {
        if self.duration_ms > 0 {
            let position = (f64::from(self.seek_ratio) * self.duration_ms as f64) as u64;
            self.audio.seek(position);
        }
    }

    pub fn apply_volume(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.0);
        self.audio.set_volume(self.volume);
        self.config.volume = self.volume;
        self.mark_config_dirty();
    }

    pub fn set_preset(&mut self, preset: usize) {
        self.preset = preset % PRESETS.len();
        self.preset_override = None;
        self.config.visual_preset = self.preset;
        self.mark_config_dirty();
    }

    pub fn apply_visual_tuning(&mut self) {
        self.visual_tuning = self.visual_tuning.normalized();
        self.config.visual_intensity = self.visual_tuning.intensity;
        self.config.visual_motion = self.visual_tuning.motion;
        self.config.visual_depth = self.visual_tuning.depth;
        self.config.visual_glow = self.visual_tuning.glow;
        self.mark_config_dirty();
    }

    pub fn set_visual_viewport(&self, viewport: Option<(f32, f32, f32, f32)>) {
        if let Ok(mut visual) = self.visuals.write() {
            visual.set_stage_viewport(viewport);
        }
    }

    pub fn toggle_current_favorite(&mut self) {
        let Some(index) = self.current_index else {
            return;
        };
        let Some(track) = self.tracks.get_mut(index) else {
            return;
        };
        track.favorite = !track.favorite;
        let message = if track.favorite {
            format!("{} · {}", text(self.language, Key::Favorited), track.title)
        } else {
            format!(
                "{} · {}",
                text(self.language, Key::Unfavorited),
                track.title
            )
        };
        self.set_toast(message, false);
        self.mark_config_dirty();
    }

    #[must_use]
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|index| self.tracks.get(index))
    }

    #[must_use]
    pub fn favorite_count(&self) -> usize {
        self.tracks.iter().filter(|track| track.favorite).count()
    }

    #[must_use]
    pub fn live_audio_features(&self) -> AudioFeatures {
        if self.playback_state.is_playing() {
            self.audio_features
        } else {
            AudioFeatures::default()
        }
    }

    #[must_use]
    pub fn visible_track_count(&self, favorites_only: bool) -> usize {
        self.visible_track_indices(favorites_only).len()
    }

    #[must_use]
    pub fn visible_track_indices(&self, favorites_only: bool) -> Vec<usize> {
        let query = self.search.as_str().to_lowercase();
        self.tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !favorites_only || track.favorite)
            .filter(|(_, track)| {
                query.is_empty()
                    || track.title.to_lowercase().contains(&query)
                    || track.artist.to_lowercase().contains(&query)
                    || track.album.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    #[must_use]
    pub fn queue_indices(&self) -> Vec<usize> {
        if self.tracks.is_empty() {
            return Vec::new();
        }
        let start = self.current_index.unwrap_or(0);
        (0..self.tracks.len().min(20))
            .map(|offset| (start + offset) % self.tracks.len())
            .collect()
    }

    #[must_use]
    pub fn config_path(&self) -> &Path {
        self.config_store.path()
    }

    #[must_use]
    pub fn library_roots(&self) -> &[PathBuf] {
        &self.config.library_roots
    }

    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    #[must_use]
    pub const fn language_preference(&self) -> LanguagePreference {
        self.config.language
    }

    pub fn set_language_preference(&mut self, preference: LanguagePreference) {
        if self.config.language != preference {
            self.config.language = preference;
            self.language = preference.resolve();
            self.mark_config_dirty();
        }
    }

    #[must_use]
    pub fn toast_message(&self) -> Option<&str> {
        self.playback_error
            .as_deref()
            .or_else(|| self.toast.as_ref().map(|(message, _)| message.as_str()))
    }

    #[must_use]
    pub fn status_is_error(&self) -> bool {
        self.playback_error.is_some() || (self.toast.is_some() && self.toast_is_error)
    }

    fn set_toast(&mut self, message: String, is_error: bool) {
        self.toast = Some((message, Instant::now()));
        self.toast_is_error = is_error;
    }

    fn mark_config_dirty(&mut self) {
        self.dirty_config = true;
        self.config_save_due = Instant::now() + Duration::from_millis(350);
    }

    fn save_config(&mut self) {
        self.config.volume = self.volume;
        if self.preset_override.is_none() {
            self.config.visual_preset = self.preset;
        }
        self.config.visual_intensity = self.visual_tuning.intensity;
        self.config.visual_motion = self.visual_tuning.motion;
        self.config.visual_depth = self.visual_tuning.depth;
        self.config.visual_glow = self.visual_tuning.glow;
        let discovered = self
            .tracks
            .iter()
            .map(|track| track.uri.clone())
            .collect::<HashSet<_>>();
        self.config
            .favorite_uris
            .retain(|uri| !discovered.contains(uri));
        self.config.favorite_uris.extend(
            self.tracks
                .iter()
                .filter(|track| track.favorite)
                .map(|track| track.uri.clone()),
        );
        self.config.favorite_uris.sort();
        self.config.favorite_uris.dedup();
        match self.config_store.save(&self.config) {
            Ok(()) => self.dirty_config = false,
            Err(error) => {
                self.set_toast(
                    format!("{}: {error}", text(self.language, Key::SaveSettingsFailed)),
                    true,
                );
                self.config_save_due = Instant::now() + Duration::from_secs(5);
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_config();
    }
}

/// Starts Wavora and runs the Iris event loop until the window closes.
///
/// # Errors
///
/// Returns an error when configuration storage, worker creation, or the
/// platform window/event loop cannot be initialized.
pub fn run() -> Result<(), AppError> {
    let store = ConfigStore::discover()?;
    let (config, recovered_config) = store.load_resilient()?;
    let paths = command_line_paths();
    let requested_view = command_line_view();
    let requested_preset = command_line_preset();
    let visuals = shared_state(config.visual_preset);
    let paint_visuals = visuals.clone();
    let mut app = App::new(config, store, &paths, visuals)?;
    if let Some(view) = requested_view {
        app.view = view;
    }
    if let Some(preset) = requested_preset {
        app.preset = preset;
        app.preset_override = Some(preset);
    }
    if let Some(backup) = recovered_config {
        app.set_toast(
            format!(
                "{} {}",
                text(app.language, Key::RestoreConfig),
                backup.display()
            ),
            true,
        );
    }
    let window = Config::new("Wavora — local audio space")?
        .app_id("io.github.ming2k.Wavora")?
        .size(1380, 860)
        .force_dark();
    Application::run(
        window,
        move |frame, input| {
            app.tick(input);
            let size = input.as_raw().display_size;
            crate::ui::build(&mut app, frame, size.x, size.y);
        },
        Some(move |host| wavora_visuals::paint(host, &paint_visuals)),
    )?;
    Ok(())
}

fn command_line_paths() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(argument_to_path)
        .filter(|path| path.exists())
        .collect()
}

fn command_line_view() -> Option<View> {
    std::env::args().find_map(|argument| match argument.as_str() {
        "--visuals" => Some(View::Visuals),
        "--library" => Some(View::Library),
        _ => None,
    })
}

fn command_line_preset() -> Option<usize> {
    std::env::args()
        .find_map(|argument| argument.strip_prefix("--preset=")?.parse::<usize>().ok())
        .map(|preset| preset % PRESETS.len())
}

fn argument_to_path(argument: std::ffi::OsString) -> PathBuf {
    let uri_path = argument.to_str().and_then(file_uri_to_path);
    uri_path.unwrap_or_else(|| PathBuf::from(argument))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_file_uri_argument_becomes_local_path() {
        assert_eq!(
            argument_to_path("file:///tmp/Hello%20World.flac".into()),
            Path::new("/tmp/Hello World.flac")
        );
    }
}
