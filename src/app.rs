use crate::artwork::{ArtworkCache, ArtworkGallery};
use crate::config::{
    AppConfig, AppState, PersistenceError, PersistenceStore, PersistentApp, PlaylistDisplay,
    UserData,
};
use iris::{Application, Config, Input, TextBuf, request_animation_frame};
use std::cell::RefCell;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use thiserror::Error;
use wavora_core::{
    LyricsDocument, PlaybackMode, PlaybackQueue, PlaybackState, Playlist, PlaylistId, Track,
    TrackId,
};
use wavora_i18n::{Key, Language, LanguagePreference, text};
use wavora_library::{Catalog, CatalogError, PlaylistEntry};
use wavora_media::{
    AudioController, AudioEvent, AudioFeatures, LibraryEvent, LibraryScanner, file_uri_to_path,
    is_supported_audio, load_sidecar_lyrics, path_to_file_uri,
};
use wavora_visuals::{
    Atmosphere, AudioMetricSnapshot, PRESETS, SharedVisualState, VisualRenderer, VisualTuning,
    shared_state,
};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
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
    Playlists,
    Lyrics,
    Visuals,
    Settings,
}

/// Active page in the Visual Stage inspector.
///
/// This is transient editing state rather than a playback preference, so it
/// intentionally is not persisted with the visual scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VisualInspectorTab {
    #[default]
    Composition,
    Atmosphere,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackModeToast {
    pub mode: PlaybackMode,
    pub opacity: f32,
    pub offset_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientToast<'a> {
    pub message: &'a str,
    pub is_error: bool,
    pub opacity: f32,
    pub offset_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueSource {
    Library,
    Playlist(PlaylistId),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PlaylistLevel {
    #[default]
    Collection,
    Detail,
}

/// Cached, UI-facing metadata for one playlist card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistPreview {
    pub playlist_id: PlaylistId,
    pub cover_candidates: Vec<String>,
}

const TABLE_ACTIVATION_INTERVAL: Duration = Duration::from_millis(400);
const MODE_TOAST_ENTER: Duration = Duration::from_millis(180);
const MODE_TOAST_HOLD: Duration = Duration::from_millis(900);
const MODE_TOAST_EXIT: Duration = Duration::from_millis(420);
const TOAST_ENTER: Duration = Duration::from_millis(180);
const TOAST_SUCCESS_HOLD: Duration = Duration::from_millis(2_600);
const TOAST_ERROR_HOLD: Duration = Duration::from_millis(4_600);
const TOAST_EXIT: Duration = Duration::from_millis(260);
const QUEUE_PANEL_TRANSITION_SECONDS: f32 = 0.28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableActivationTarget {
    Library {
        favorites_only: bool,
        track_id: TrackId,
    },
    Playlist {
        playlist_id: PlaylistId,
        row: usize,
        track_id: TrackId,
    },
}

#[derive(Debug, Default)]
struct TableActivationTracker {
    pending: Option<(TableActivationTarget, Instant)>,
}

impl TableActivationTracker {
    fn register(&mut self, target: TableActivationTarget, now: Instant) -> bool {
        let activates = self.pending.is_some_and(|(pending, clicked_at)| {
            pending == target
                && now.saturating_duration_since(clicked_at) <= TABLE_ACTIVATION_INTERVAL
        });
        self.pending = (!activates).then_some((target, now));
        activates
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub playback_state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub seek_ratio: f32,
    pub volume: f32,
    pub playback_mode: PlaybackMode,
    pub preset: usize,
    pub visual_tuning: VisualTuning,
    pub atmosphere: Atmosphere,
    pub selected_atmosphere_source: usize,
    pub visual_inspector_tab: VisualInspectorTab,
    pub view: View,
    pub search: TextBuf,
    pub playlist_name: TextBuf,
    pub selected_playlist_row: Option<usize>,
    pub scanning: bool,
    language: Language,
    audio_features: AudioFeatures,
    active_scans: usize,
    config: AppConfig,
    state: AppState,
    user_data: UserData,
    persistence_store: PersistenceStore,
    catalog: Catalog,
    playlists: Vec<Playlist>,
    playlist_previews: Vec<PlaylistPreview>,
    selected_playlist_id: Option<PlaylistId>,
    playlist_level: PlaylistLevel,
    playlist_entries: Vec<PlaylistEntry>,
    playlist_tracks: Vec<Track>,
    pending_playlist_delete: Option<(PlaylistId, Instant)>,
    scanner: LibraryScanner,
    audio: AudioController,
    playback_queue: PlaybackQueue,
    queue_source: QueueSource,
    queue_panel_open: bool,
    queue_panel_progress: f32,
    table_activation: TableActivationTracker,
    lyrics: Option<LyricsDocument>,
    lyrics_path: Option<PathBuf>,
    visuals: SharedVisualState,
    loaded_uri: Option<String>,
    autoplay_uri: Option<String>,
    preset_override: Option<usize>,
    dirty_persistence: bool,
    persistence_save_due: Instant,
    last_frame: Instant,
    toast: Option<(String, Instant)>,
    toast_is_error: bool,
    playback_error: Option<String>,
    playback_mode_toast: Option<(PlaybackMode, Instant)>,
    last_audible_volume: f32,
}

impl App {
    fn new(
        mut config: AppConfig,
        mut state: AppState,
        mut user_data: UserData,
        persistence_store: PersistenceStore,
        paths: &[PathBuf],
        visuals: SharedVisualState,
    ) -> Result<Self, AppError> {
        config.normalize();
        state.normalize();
        user_data.normalize();
        let mut dirty_persistence = false;
        let mut autoplay_uri = None;
        for path in paths {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if canonical.is_dir() {
                if !config.library_roots.contains(&canonical) {
                    config.library_roots.push(canonical);
                    dirty_persistence = true;
                }
            } else if is_supported_audio(&canonical) {
                let uri = path_to_file_uri(&canonical);
                autoplay_uri.get_or_insert(uri);
            }
        }
        config.normalize();
        let catalog = Catalog::open(persistence_store.catalog_path())?;
        let tracks = catalog.available_tracks()?;
        let playlists = catalog.playlists()?;
        let playlist_previews = load_playlist_previews(&catalog, &playlists)?;
        let selected_playlist_id = playlists.first().map(|playlist| playlist.id);
        let playlist_entries = selected_playlist_id
            .map_or_else(|| Ok(Vec::new()), |id| catalog.playlist_entries(id))?;
        let playlist_tracks = selected_playlist_id
            .map_or_else(|| Ok(Vec::new()), |id| catalog.playlist_tracks(id))?;
        let last_track_id = catalog.last_track_id()?;
        let current_index =
            last_track_id.and_then(|id| tracks.iter().position(|track| track.id == id));
        let playback_queue =
            PlaybackQueue::new(tracks.iter().map(|track| track.id).collect(), current_index);
        let scanner = LibraryScanner::spawn()?;
        scanner.set_audio_evidence_cache(catalog.audio_evidence_cache()?);
        let mut scheduled = HashSet::new();
        for root in &config.library_roots {
            scheduled.insert(root.clone());
        }
        for path in paths.iter().filter(|path| path.is_file()) {
            scheduled.insert(path.canonicalize().unwrap_or_else(|_| path.clone()));
        }
        for uri in &state.recent_uris {
            if let Some(path) = file_uri_to_path(uri) {
                scheduled.insert(path);
            }
        }
        for track in &tracks {
            if let Some(path) = file_uri_to_path(&track.uri)
                && path.exists()
                && !config
                    .library_roots
                    .iter()
                    .any(|root| path.starts_with(root))
            {
                scheduled.insert(path);
            }
        }
        for path in scheduled {
            scanner.scan(path);
        }
        let volume = config.volume;
        let playback_mode = config.playback_mode;
        let preset = config.visual_preset;
        let visual_tuning = VisualTuning {
            intensity: config.visual_intensity,
            motion: config.visual_motion,
            depth: config.visual_depth,
            glow: config.visual_glow,
        }
        .normalized();
        let atmosphere = config.atmosphere.clone().normalized();
        let language = config.language.resolve();
        let view = if config.library_roots.is_empty() && tracks.is_empty() {
            View::Home
        } else {
            View::Library
        };
        let mut app = Self {
            tracks,
            current_index,
            playback_state: PlaybackState::Stopped,
            position_ms: 0,
            duration_ms: 0,
            seek_ratio: 0.0,
            volume,
            playback_mode,
            preset,
            visual_tuning,
            atmosphere,
            selected_atmosphere_source: 0,
            visual_inspector_tab: VisualInspectorTab::default(),
            view,
            search: TextBuf::new(256, ""),
            playlist_name: TextBuf::new(128, ""),
            selected_playlist_row: None,
            scanning: false,
            language,
            audio_features: AudioFeatures::default(),
            active_scans: 0,
            config,
            state,
            user_data,
            persistence_store,
            catalog,
            playlists,
            playlist_previews,
            selected_playlist_id,
            playlist_level: PlaylistLevel::Collection,
            playlist_entries,
            playlist_tracks,
            pending_playlist_delete: None,
            scanner,
            audio: AudioController::spawn(volume)?,
            playback_queue,
            queue_source: QueueSource::Library,
            queue_panel_open: false,
            queue_panel_progress: 0.0,
            table_activation: TableActivationTracker::default(),
            lyrics: None,
            lyrics_path: None,
            visuals,
            loaded_uri: None,
            autoplay_uri,
            preset_override: None,
            dirty_persistence,
            persistence_save_due: Instant::now() + Duration::from_millis(350),
            last_frame: Instant::now(),
            toast: None,
            toast_is_error: false,
            playback_error: None,
            playback_mode_toast: None,
            last_audible_volume: if volume > 0.02 { volume } else { 0.72 },
        };
        app.refresh_current_lyrics();
        Ok(app)
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
        let queue_animation_pending = self.update_queue_panel_animation(dt);
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
                &self.atmosphere,
            );
            visual.needs_animation_frame()
        } else {
            false
        };
        if animation_pending || queue_animation_pending {
            request_animation_frame();
        }
        if self.dirty_persistence && now >= self.persistence_save_due {
            self.save_persistence();
        }
        if let Some(started_at) = self.toast.as_ref().map(|(_, started_at)| *started_at) {
            if toast_animation(
                now.saturating_duration_since(started_at),
                self.toast_is_error,
            )
            .is_some()
            {
                request_animation_frame();
            } else {
                self.toast = None;
            }
        }
        if let Some((_, started_at)) = self.playback_mode_toast {
            if mode_toast_animation(now.saturating_duration_since(started_at)).is_some() {
                request_animation_frame();
            } else {
                self.playback_mode_toast = None;
            }
        }
    }

    fn handle_library_event(&mut self, event: LibraryEvent) {
        match event {
            LibraryEvent::ScanStarted(root) => {
                self.active_scans = self.active_scans.saturating_add(1);
                self.scanning = true;
                self.catalog.begin_scan(root);
            }
            LibraryEvent::Track(scanned) => {
                let track_uri = scanned.track.uri.clone();
                let legacy_favorite = self.user_data.favorite_uris.contains(&track_uri);
                let legacy_last = self.state.last_track_uri.as_deref() == Some(&track_uri);
                let legacy_recent = self
                    .state
                    .recent_uris
                    .iter()
                    .position(|uri| uri == &track_uri);
                let reconciled = (|| -> Result<Track, CatalogError> {
                    let mut track = self.catalog.reconcile(&scanned)?;
                    if legacy_favorite {
                        self.catalog.set_favorite(track.id, true)?;
                        track.favorite = true;
                    }
                    if legacy_last {
                        self.catalog.set_last_track(track.id)?;
                    }
                    if let Some(rank) = legacy_recent {
                        self.catalog.import_recent_track(track.id, rank)?;
                    }
                    Ok(track)
                })();
                let Ok(track) = reconciled else {
                    self.set_toast(
                        format!("Catalog update failed: {}", reconciled.unwrap_err()),
                        true,
                    );
                    return;
                };
                if legacy_favorite || legacy_recent.is_some() || legacy_last {
                    self.user_data.favorite_uris.retain(|uri| uri != &track_uri);
                    self.state.recent_uris.retain(|uri| uri != &track_uri);
                    if legacy_last {
                        self.state.last_track_uri = None;
                    }
                    self.mark_persistence_dirty();
                }
                self.upsert_track(track);
                self.refresh_selected_playlist();
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
                match self.catalog.finish_scan() {
                    Ok(_) => match self.catalog.available_tracks() {
                        Ok(tracks) => self.replace_tracks(tracks),
                        Err(error) => {
                            self.set_toast(format!("Catalog refresh failed: {error}"), true);
                        }
                    },
                    Err(CatalogError::NoActiveScan) => {}
                    Err(error) => self.set_toast(format!("Catalog scan failed: {error}"), true),
                }
                self.refresh_selected_playlist();
                self.refresh_playlist_previews();
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
                    && let Ok(Some(id)) = self.catalog.last_track_id()
                {
                    self.current_index = self.tracks.iter().position(|track| track.id == id);
                }
            }
            LibraryEvent::Error(error) => {
                self.catalog.abort_scan();
                self.active_scans = 0;
                self.scanning = false;
                self.set_toast(error, true);
            }
        }
    }

    fn upsert_track(&mut self, track: Track) {
        let current_id = self.current_track().map(|current| current.id);
        if let Some(index) = self
            .tracks
            .iter()
            .position(|existing| existing.id == track.id)
        {
            self.tracks[index] = track;
        } else {
            self.tracks.push(track);
        }
        sort_tracks(&mut self.tracks);
        self.current_index =
            current_id.and_then(|id| self.tracks.iter().position(|candidate| candidate.id == id));
        self.sync_library_queue();
    }

    fn replace_tracks(&mut self, mut tracks: Vec<Track>) {
        let current_id = self.current_track().map(|current| current.id);
        sort_tracks(&mut tracks);
        self.tracks = tracks;
        self.current_index =
            current_id.and_then(|id| self.tracks.iter().position(|candidate| candidate.id == id));
        self.sync_library_queue();
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
            AudioEvent::EndOfStream => self.advance_after_end(),
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
                    self.mark_persistence_dirty();
                }
                self.scanner.scan(canonical);
            } else {
                self.set_toast(text(self.language, Key::InvalidFolderPath).to_owned(), true);
            }
        }
    }

    pub fn play_index(&mut self, index: usize) {
        let Some(track_id) = self.tracks.get(index).map(|track| track.id) else {
            return;
        };
        self.queue_source = QueueSource::Library;
        self.playback_queue = PlaybackQueue::new(
            self.tracks.iter().map(|track| track.id).collect(),
            Some(index),
        );
        self.play_track(track_id);
    }

    pub fn click_library_table_row(&mut self, index: usize, favorites_only: bool) {
        let Some(track_id) = self.tracks.get(index).map(|track| track.id) else {
            return;
        };
        if self.table_activation.register(
            TableActivationTarget::Library {
                favorites_only,
                track_id,
            },
            Instant::now(),
        ) {
            self.play_index(index);
        }
    }

    pub fn play_queue_position(&mut self, position: usize) {
        if let Some(track_id) = self.playback_queue.select(position) {
            self.play_track(track_id);
        }
    }

    fn play_track(&mut self, track_id: TrackId) {
        let Some(index) = self
            .tracks
            .iter()
            .position(|candidate| candidate.id == track_id)
        else {
            return;
        };
        let track = &self.tracks[index];
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
        self.refresh_current_lyrics();
        if let Err(error) = self.catalog.record_played(track_id) {
            self.set_toast(format!("Could not update playback history: {error}"), true);
        }
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
        if let Some(track_id) = self.playback_queue.next(self.playback_mode) {
            self.play_track(track_id);
        }
    }

    pub fn previous(&mut self) {
        if self.position_ms > 4_000 {
            self.audio.seek(0);
            return;
        }
        if let Some(track_id) = self.playback_queue.previous(self.playback_mode) {
            self.play_track(track_id);
        } else {
            self.audio.seek(0);
        }
    }

    fn advance_after_end(&mut self) {
        if let Some(track_id) = self.playback_queue.on_end(self.playback_mode) {
            self.play_track(track_id);
        } else {
            self.playback_state = PlaybackState::Stopped;
            self.position_ms = self.duration_ms;
            self.seek_ratio = 1.0;
        }
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
        if self.volume > 0.02 {
            self.last_audible_volume = self.volume;
        }
        self.audio.set_volume(self.volume);
        self.config.volume = self.volume;
        self.mark_persistence_dirty();
    }

    pub fn apply_volume_with_feedback(&mut self) {
        self.apply_volume();
        // While a track is loaded, the music itself is immediate level
        // feedback. Opening a second output pipeline here can pre-empt the
        // primary stream on exclusive or fallback audio backends.
        if self.loaded_uri.is_none() {
            self.audio.preview_volume(self.volume);
        }
    }

    pub fn toggle_mute(&mut self) {
        if self.volume <= 0.001 {
            self.volume = self.last_audible_volume.max(0.05);
        } else {
            self.last_audible_volume = self.volume;
            self.volume = 0.0;
        }
        self.apply_volume_with_feedback();
    }

    pub fn cycle_playback_mode(&mut self) {
        self.set_playback_mode(self.playback_mode.next());
    }

    pub fn set_playback_mode(&mut self, mode: PlaybackMode) {
        if self.playback_mode != mode {
            self.playback_mode = mode;
            if mode == PlaybackMode::Shuffle {
                self.playback_queue.restart_shuffle_cycle();
            }
            self.config.playback_mode = mode;
            self.playback_mode_toast = Some((mode, Instant::now()));
            request_animation_frame();
            self.mark_persistence_dirty();
        }
    }

    #[must_use]
    pub fn playback_mode_toast(&self) -> Option<PlaybackModeToast> {
        let (mode, started_at) = self.playback_mode_toast?;
        let (opacity, offset_y) =
            mode_toast_animation(Instant::now().saturating_duration_since(started_at))?;
        Some(PlaybackModeToast {
            mode,
            opacity,
            offset_y,
        })
    }

    #[must_use]
    pub const fn queue_panel_open(&self) -> bool {
        self.queue_panel_open
    }

    #[must_use]
    pub fn queue_panel_progress(&self) -> f32 {
        smoothstep(self.queue_panel_progress)
    }

    pub fn toggle_queue_panel(&mut self) {
        self.set_queue_panel_open(!self.queue_panel_open);
    }

    pub fn set_queue_panel_open(&mut self, open: bool) {
        if self.queue_panel_open != open {
            self.queue_panel_open = open;
            request_animation_frame();
        }
    }

    fn update_queue_panel_animation(&mut self, dt: f32) -> bool {
        let (progress, pending) =
            advance_queue_panel(self.queue_panel_progress, self.queue_panel_open, dt);
        self.queue_panel_progress = progress;
        pending
    }

    pub fn set_preset(&mut self, preset: usize) {
        self.preset = preset % PRESETS.len();
        self.preset_override = None;
        self.config.visual_preset = self.preset;
        self.mark_persistence_dirty();
    }

    pub fn apply_visual_tuning(&mut self) {
        self.visual_tuning = self.visual_tuning.normalized();
        self.config.visual_intensity = self.visual_tuning.intensity;
        self.config.visual_motion = self.visual_tuning.motion;
        self.config.visual_depth = self.visual_tuning.depth;
        self.config.visual_glow = self.visual_tuning.glow;
        self.mark_persistence_dirty();
    }

    pub fn apply_atmosphere(&mut self) {
        self.atmosphere = std::mem::take(&mut self.atmosphere).normalized();
        self.selected_atmosphere_source = self
            .selected_atmosphere_source
            .min(self.atmosphere.sources.len().saturating_sub(1));
        self.config.atmosphere.clone_from(&self.atmosphere);
        self.mark_persistence_dirty();
        request_animation_frame();
    }

    pub fn add_atmosphere_source(&mut self) {
        if self.atmosphere.add_source() {
            self.selected_atmosphere_source = self.atmosphere.sources.len() - 1;
            self.apply_atmosphere();
        }
    }

    pub fn remove_selected_atmosphere_source(&mut self) {
        if self
            .atmosphere
            .remove_source(self.selected_atmosphere_source)
        {
            self.selected_atmosphere_source = self
                .selected_atmosphere_source
                .min(self.atmosphere.sources.len().saturating_sub(1));
            self.apply_atmosphere();
        }
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
        let track_id = track.id;
        let favorite = track.favorite;
        if let Err(error) = self.catalog.set_favorite(track_id, favorite) {
            track.favorite = !favorite;
            self.set_toast(format!("Could not update favorites: {error}"), true);
            return;
        }
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
    }

    #[must_use]
    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    #[must_use]
    pub fn playlist_previews(&self) -> &[PlaylistPreview] {
        &self.playlist_previews
    }

    #[must_use]
    pub const fn playlist_display(&self) -> PlaylistDisplay {
        self.config.playlist_display
    }

    pub fn set_playlist_display(&mut self, display: PlaylistDisplay) {
        if self.config.playlist_display != display {
            self.config.playlist_display = display;
            self.mark_persistence_dirty();
        }
    }

    #[must_use]
    pub fn selected_playlist_id(&self) -> Option<PlaylistId> {
        self.selected_playlist_id
    }

    #[must_use]
    pub const fn playlist_detail_open(&self) -> bool {
        matches!(self.playlist_level, PlaylistLevel::Detail)
    }

    pub fn open_playlist(&mut self, playlist_id: PlaylistId) {
        self.select_playlist(playlist_id);
        self.playlist_level = PlaylistLevel::Detail;
    }

    pub fn close_playlist_detail(&mut self) {
        self.playlist_level = PlaylistLevel::Collection;
        self.selected_playlist_row = None;
        self.pending_playlist_delete = None;
    }

    #[must_use]
    pub fn selected_playlist(&self) -> Option<&Playlist> {
        let selected = self.selected_playlist_id?;
        self.playlists
            .iter()
            .find(|playlist| playlist.id == selected)
    }

    #[must_use]
    pub fn selected_playlist_tracks(&self) -> &[Track] {
        &self.playlist_tracks
    }

    pub fn select_playlist(&mut self, playlist_id: PlaylistId) {
        if self.selected_playlist_id == Some(playlist_id) {
            return;
        }
        self.selected_playlist_id = Some(playlist_id);
        self.selected_playlist_row = None;
        self.pending_playlist_delete = None;
        self.refresh_selected_playlist();
    }

    pub fn create_playlist(&mut self) {
        let name = self.playlist_name.as_str().trim().to_owned();
        match self.catalog.create_playlist(&name) {
            Ok(playlist) => {
                self.selected_playlist_id = Some(playlist.id);
                self.playlist_level = PlaylistLevel::Detail;
                self.playlist_previews.push(PlaylistPreview {
                    playlist_id: playlist.id,
                    cover_candidates: Vec::new(),
                });
                self.playlists.push(playlist);
                self.playlists
                    .sort_by_key(|playlist| playlist.name.to_lowercase());
                self.playlist_name.set("");
                self.selected_playlist_row = None;
                self.refresh_selected_playlist();
            }
            Err(error) => self.set_toast(format!("Could not create playlist: {error}"), true),
        }
    }

    pub fn delete_selected_playlist(&mut self) {
        let Some(playlist_id) = self.selected_playlist_id else {
            return;
        };
        let now = Instant::now();
        let confirmed = self
            .pending_playlist_delete
            .is_some_and(|(pending, requested_at)| {
                pending == playlist_id && now.duration_since(requested_at) < Duration::from_secs(4)
            });
        if !confirmed {
            self.pending_playlist_delete = Some((playlist_id, now));
            self.set_toast(
                text(self.language, Key::ConfirmDeletePlaylist).to_owned(),
                true,
            );
            return;
        }
        self.pending_playlist_delete = None;
        match self.catalog.delete_playlist(playlist_id) {
            Ok(()) => {
                self.playlists.retain(|playlist| playlist.id != playlist_id);
                self.playlist_previews
                    .retain(|preview| preview.playlist_id != playlist_id);
                self.selected_playlist_id = self.playlists.first().map(|playlist| playlist.id);
                self.playlist_level = PlaylistLevel::Collection;
                self.selected_playlist_row = None;
                self.refresh_selected_playlist();
            }
            Err(error) => self.set_toast(format!("Could not delete playlist: {error}"), true),
        }
    }

    pub fn add_current_to_selected_playlist(&mut self) {
        let Some(playlist_id) = self.selected_playlist_id else {
            return;
        };
        let Some(track) = self.current_track() else {
            return;
        };
        let track_id = track.id;
        match self.catalog.add_to_playlist(playlist_id, track_id) {
            Ok(_) => {
                self.refresh_selected_playlist();
                self.selected_playlist_row = self.playlist_tracks.len().checked_sub(1);
            }
            Err(error) => self.set_toast(format!("Could not update playlist: {error}"), true),
        }
    }

    pub fn remove_selected_playlist_entry(&mut self) {
        let Some(row) = self.selected_playlist_row else {
            return;
        };
        let Some(entry) = self.playlist_entries.get(row).copied() else {
            return;
        };
        match self.catalog.remove_playlist_entry(entry.id) {
            Ok(()) => {
                self.refresh_selected_playlist();
                self.selected_playlist_row = row
                    .checked_sub(1)
                    .or_else(|| (!self.playlist_entries.is_empty()).then_some(0));
            }
            Err(error) => self.set_toast(format!("Could not update playlist: {error}"), true),
        }
    }

    pub fn move_selected_playlist_entry(&mut self, offset: isize) {
        let Some(row) = self.selected_playlist_row else {
            return;
        };
        let Some(entry) = self.playlist_entries.get(row).copied() else {
            return;
        };
        let target = row
            .saturating_add_signed(offset)
            .min(self.playlist_entries.len() - 1);
        if target == row {
            return;
        }
        match self.catalog.move_playlist_entry(entry.id, target) {
            Ok(()) => {
                self.refresh_selected_playlist();
                self.selected_playlist_row = Some(target);
            }
            Err(error) => self.set_toast(format!("Could not reorder playlist: {error}"), true),
        }
    }

    pub fn play_selected_playlist_row(&mut self, row: usize) {
        let Some(track) = self.playlist_tracks.get(row) else {
            return;
        };
        self.selected_playlist_row = Some(row);
        if !track.available {
            self.set_toast(
                "This playlist item is missing from the library".to_owned(),
                true,
            );
            return;
        }
        let track_id = track.id;
        let Some(playlist_id) = self.selected_playlist_id else {
            return;
        };
        let mut queue = Vec::new();
        let mut queue_position = None;
        for (playlist_row, candidate) in self.playlist_tracks.iter().enumerate() {
            if candidate.available
                && self
                    .tracks
                    .iter()
                    .any(|library_track| library_track.id == candidate.id)
            {
                if playlist_row == row {
                    queue_position = Some(queue.len());
                }
                queue.push(candidate.id);
            }
        }
        let Some(queue_position) = queue_position else {
            return;
        };
        self.queue_source = QueueSource::Playlist(playlist_id);
        self.playback_queue = PlaybackQueue::new(queue, Some(queue_position));
        self.play_track(track_id);
    }

    pub fn click_playlist_table_row(&mut self, row: usize) {
        let Some(track_id) = self.playlist_tracks.get(row).map(|track| track.id) else {
            return;
        };
        self.selected_playlist_row = Some(row);
        let Some(playlist_id) = self.selected_playlist_id else {
            return;
        };
        if self.table_activation.register(
            TableActivationTarget::Playlist {
                playlist_id,
                row,
                track_id,
            },
            Instant::now(),
        ) {
            self.play_selected_playlist_row(row);
        }
    }

    fn refresh_selected_playlist(&mut self) {
        let result: Result<(Vec<PlaylistEntry>, Vec<Track>), CatalogError> =
            self.selected_playlist_id.map_or_else(
                || Ok((Vec::new(), Vec::new())),
                |playlist_id| {
                    Ok((
                        self.catalog.playlist_entries(playlist_id)?,
                        self.catalog.playlist_tracks(playlist_id)?,
                    ))
                },
            );
        match result {
            Ok((entries, tracks)) => {
                if let Some(playlist_id) = self.selected_playlist_id {
                    let preview = playlist_preview(playlist_id, &tracks);
                    if let Some(existing) = self
                        .playlist_previews
                        .iter_mut()
                        .find(|candidate| candidate.playlist_id == playlist_id)
                    {
                        *existing = preview;
                    } else {
                        self.playlist_previews.push(preview);
                    }
                }
                self.playlist_entries = entries;
                self.playlist_tracks = tracks;
            }
            Err(error) => self.set_toast(format!("Could not read playlist: {error}"), true),
        }
    }

    fn refresh_playlist_previews(&mut self) {
        match load_playlist_previews(&self.catalog, &self.playlists) {
            Ok(previews) => self.playlist_previews = previews,
            Err(error) => {
                self.set_toast(format!("Could not read playlist covers: {error}"), true);
            }
        }
    }

    #[must_use]
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|index| self.tracks.get(index))
    }

    #[must_use]
    pub fn lyrics(&self) -> Option<&LyricsDocument> {
        self.lyrics.as_ref()
    }

    #[must_use]
    pub fn lyrics_path(&self) -> Option<&Path> {
        self.lyrics_path.as_deref()
    }

    #[must_use]
    pub fn active_lyric_cues(&self) -> Vec<usize> {
        self.lyrics.as_ref().map_or_else(Vec::new, |lyrics| {
            lyrics.active_cue_indices(self.position_ms)
        })
    }

    pub fn refresh_current_lyrics(&mut self) {
        self.lyrics = None;
        self.lyrics_path = None;
        let Some((track_id, uri, duration_ms)) = self
            .current_track()
            .map(|track| (track.id, track.uri.clone(), track.duration_ms))
        else {
            return;
        };
        match load_sidecar_lyrics(&uri) {
            Ok(Some(loaded)) => {
                let needs_signature = loaded.document.media.as_ref().is_some_and(|media| {
                    media
                        .fingerprints
                        .iter()
                        .any(|fingerprint| fingerprint.algorithm == "wavora-pcm-signature-v1")
                });
                let signature = if needs_signature {
                    match self.catalog.audio_signature(track_id) {
                        Ok(Some(signature)) => Some(hex_bytes(&signature)),
                        Ok(None) => None,
                        Err(error) => {
                            self.set_toast(
                                format!(
                                    "{}: could not verify media binding: {error}",
                                    text(self.language, Key::LyricsLoadFailed)
                                ),
                                true,
                            );
                            return;
                        }
                    }
                } else {
                    None
                };
                if let Err(error) = loaded
                    .document
                    .validate_media_binding(duration_ms, signature.as_deref())
                {
                    self.set_toast(
                        format!("{}: {error}", text(self.language, Key::LyricsLoadFailed)),
                        true,
                    );
                    return;
                }
                self.lyrics = Some(loaded.document);
                self.lyrics_path = Some(loaded.path);
            }
            Ok(None) => {}
            Err(error) => self.set_toast(
                format!("{}: {error}", text(self.language, Key::LyricsLoadFailed)),
                true,
            ),
        }
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
    pub fn audio_metric_snapshot(&self) -> AudioMetricSnapshot {
        self.visuals.read().map_or_else(
            |_| AudioMetricSnapshot::default(),
            |state| state.audio_metric_snapshot(),
        )
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
    pub fn queue_items(&self) -> Vec<(usize, usize)> {
        self.playback_queue
            .upcoming(self.playback_mode, 20)
            .into_iter()
            .filter_map(|(queue_position, track_id)| {
                self.tracks
                    .iter()
                    .position(|track| track.id == track_id)
                    .map(|track_index| (queue_position, track_index))
            })
            .collect()
    }

    #[must_use]
    pub fn playback_queue_position(&self) -> Option<usize> {
        self.playback_queue.current_position()
    }

    #[must_use]
    pub fn config_path(&self) -> &Path {
        self.persistence_store.config_path()
    }

    #[must_use]
    pub fn state_path(&self) -> &Path {
        self.persistence_store.state_path()
    }

    #[must_use]
    pub fn user_data_path(&self) -> &Path {
        self.persistence_store.user_data_path()
    }

    #[must_use]
    pub fn catalog_path(&self) -> &Path {
        self.catalog.path()
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
            self.mark_persistence_dirty();
        }
    }

    #[must_use]
    pub fn playback_error_message(&self) -> Option<&str> {
        self.playback_error.as_deref()
    }

    #[must_use]
    pub fn transient_toast(&self) -> Option<TransientToast<'_>> {
        let (message, started_at) = self.toast.as_ref()?;
        let (opacity, offset_y) = toast_animation(
            Instant::now().saturating_duration_since(*started_at),
            self.toast_is_error,
        )?;
        Some(TransientToast {
            message,
            is_error: self.toast_is_error,
            opacity,
            offset_y,
        })
    }

    fn set_toast(&mut self, message: String, is_error: bool) {
        self.toast = Some((message, Instant::now()));
        self.toast_is_error = is_error;
    }

    fn mark_persistence_dirty(&mut self) {
        self.dirty_persistence = true;
        self.persistence_save_due = Instant::now() + Duration::from_millis(350);
    }

    fn save_persistence(&mut self) {
        self.config.volume = self.volume;
        self.config.playback_mode = self.playback_mode;
        if self.preset_override.is_none() {
            self.config.visual_preset = self.preset;
        }
        self.config.visual_intensity = self.visual_tuning.intensity;
        self.config.visual_motion = self.visual_tuning.motion;
        self.config.visual_depth = self.visual_tuning.depth;
        self.config.visual_glow = self.visual_tuning.glow;
        self.config.atmosphere.clone_from(&self.atmosphere);
        self.user_data.normalize();
        let persistent = PersistentApp {
            config: self.config.clone(),
            state: self.state.clone(),
            user_data: self.user_data.clone(),
        };
        match self.persistence_store.save(&persistent) {
            Ok(()) => self.dirty_persistence = false,
            Err(error) => {
                self.set_toast(
                    format!(
                        "{}: {error}",
                        text(self.language, Key::SavePersistenceFailed)
                    ),
                    true,
                );
                self.persistence_save_due = Instant::now() + Duration::from_secs(5);
            }
        }
    }

    fn sync_library_queue(&mut self) {
        if self.queue_source != QueueSource::Library {
            return;
        }
        let preferred = self
            .playback_queue
            .current()
            .or_else(|| self.current_track().map(|track| track.id));
        self.playback_queue.replace(
            self.tracks.iter().map(|track| track.id).collect(),
            preferred,
        );
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_persistence();
    }
}

fn sort_tracks(tracks: &mut [Track]) {
    tracks.sort_by(|a, b| {
        a.artist
            .to_lowercase()
            .cmp(&b.artist.to_lowercase())
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
    });
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Starts Wavora and runs the Iris event loop until the window closes.
///
/// # Errors
///
/// Returns an error when configuration storage, worker creation, or the
/// platform window/event loop cannot be initialized.
pub fn run() -> Result<(), AppError> {
    let store = PersistenceStore::discover()?;
    let (persistent, recovered_files) = store.load_resilient()?;
    let paths = command_line_paths();
    let requested_view = command_line_view();
    let requested_preset = command_line_preset();
    let visuals = shared_state(persistent.config.visual_preset);
    let paint_visuals = visuals.clone();
    let mut app = App::new(
        persistent.config,
        persistent.state,
        persistent.user_data,
        store,
        &paths,
        visuals,
    )?;
    if let Some(view) = requested_view {
        app.view = view;
    }
    if let Some(preset) = requested_preset {
        app.preset = preset;
        app.preset_override = Some(preset);
    }
    if !recovered_files.is_empty() {
        let backups = recovered_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        app.set_toast(
            format!(
                "{} {}",
                text(app.language, Key::RestorePersistence),
                backups
            ),
            true,
        );
    }
    let window = Config::new("Wavora — local audio space")?
        .app_id("io.github.ming2k.Wavora")?
        .size(1380, 860)
        .force_dark();
    let artwork = Rc::new(RefCell::new(ArtworkCache::default()));
    let build_artwork = Rc::clone(&artwork);
    let paint_artwork = Rc::clone(&artwork);
    let playlist_artwork = Rc::new(RefCell::new(ArtworkGallery::default()));
    let build_playlist_artwork = Rc::clone(&playlist_artwork);
    let paint_playlist_artwork = Rc::clone(&playlist_artwork);
    let queue_artwork = Rc::new(RefCell::new(ArtworkGallery::default()));
    let build_queue_artwork = Rc::clone(&queue_artwork);
    let paint_queue_artwork = Rc::clone(&queue_artwork);
    let mut visual_renderer = VisualRenderer::default();
    Application::run(
        window,
        move |frame, input| {
            app.tick(input);
            let size = input.as_raw().display_size;
            let artwork = build_artwork
                .borrow_mut()
                .select(app.current_track().map(|track| track.uri.as_str()));
            let playlist_artwork = {
                let mut gallery = build_playlist_artwork.borrow_mut();
                gallery.synchronize(
                    app.playlist_previews()
                        .iter()
                        .map(|preview| (preview.playlist_id, preview.cover_candidates.as_slice())),
                );
                gallery.set_active(
                    app.view == View::Playlists
                        && !app.playlist_detail_open()
                        && app.playlist_display() == PlaylistDisplay::Covers,
                );
                app.playlist_previews()
                    .iter()
                    .filter_map(|preview| {
                        gallery
                            .texture(preview.playlist_id)
                            .map(|texture| (preview.playlist_id, texture))
                    })
                    .collect::<Vec<_>>()
            };
            let queue_artwork = {
                let queue_items = app.queue_items();
                let mut gallery = build_queue_artwork.borrow_mut();
                gallery.synchronize(queue_items.iter().filter_map(|(_, track_index)| {
                    app.tracks
                        .get(*track_index)
                        .map(|track| (track.id, std::slice::from_ref(&track.uri)))
                }));
                gallery.set_active(app.queue_panel_progress() > f32::EPSILON);
                queue_items
                    .iter()
                    .filter_map(|(_, track_index)| {
                        let track_id = app.tracks.get(*track_index)?.id;
                        gallery.texture(track_id).map(|texture| (track_id, texture))
                    })
                    .collect::<Vec<_>>()
            };
            crate::ui::build(
                &mut app,
                frame,
                size.x,
                size.y,
                artwork,
                &playlist_artwork,
                &queue_artwork,
            );
        },
        Some(move |host| {
            paint_artwork.borrow_mut().prepare(&host);
            paint_playlist_artwork.borrow_mut().prepare(&host);
            paint_queue_artwork.borrow_mut().prepare(&host);
            visual_renderer.paint(host, &paint_visuals);
        }),
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
        "--playlists" => Some(View::Playlists),
        "--lyrics" => Some(View::Lyrics),
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

fn load_playlist_previews(
    catalog: &Catalog,
    playlists: &[Playlist],
) -> Result<Vec<PlaylistPreview>, CatalogError> {
    playlists
        .iter()
        .map(|playlist| {
            catalog
                .playlist_tracks(playlist.id)
                .map(|tracks| playlist_preview(playlist.id, &tracks))
        })
        .collect()
}

fn playlist_preview(playlist_id: PlaylistId, tracks: &[Track]) -> PlaylistPreview {
    const COVER_CANDIDATE_LIMIT: usize = 12;

    let mut seen = HashSet::new();
    let cover_candidates = tracks
        .iter()
        .filter(|track| track.available)
        .filter(|track| seen.insert(track.uri.as_str()))
        .map(|track| track.uri.clone())
        .take(COVER_CANDIDATE_LIMIT)
        .collect();
    PlaylistPreview {
        playlist_id,
        cover_candidates,
    }
}

fn mode_toast_animation(elapsed: Duration) -> Option<(f32, f32)> {
    if elapsed < MODE_TOAST_ENTER {
        let progress = elapsed.as_secs_f32() / MODE_TOAST_ENTER.as_secs_f32();
        let eased = 1.0 - (1.0 - progress).powi(3);
        return Some((eased, 8.0 * (1.0 - eased)));
    }
    if elapsed < MODE_TOAST_ENTER + MODE_TOAST_HOLD {
        return Some((1.0, 0.0));
    }
    let exit_elapsed = elapsed.saturating_sub(MODE_TOAST_ENTER + MODE_TOAST_HOLD);
    if exit_elapsed >= MODE_TOAST_EXIT {
        return None;
    }
    let progress = exit_elapsed.as_secs_f32() / MODE_TOAST_EXIT.as_secs_f32();
    let eased = progress * progress * (3.0 - 2.0 * progress);
    Some((1.0 - eased, -4.0 * eased))
}

fn toast_animation(elapsed: Duration, is_error: bool) -> Option<(f32, f32)> {
    if elapsed < TOAST_ENTER {
        let progress = elapsed.as_secs_f32() / TOAST_ENTER.as_secs_f32();
        let eased = 1.0 - (1.0 - progress).powi(3);
        return Some((eased, -10.0 * (1.0 - eased)));
    }
    let hold = if is_error {
        TOAST_ERROR_HOLD
    } else {
        TOAST_SUCCESS_HOLD
    };
    if elapsed < TOAST_ENTER + hold {
        return Some((1.0, 0.0));
    }
    let exit_elapsed = elapsed.saturating_sub(TOAST_ENTER + hold);
    if exit_elapsed >= TOAST_EXIT {
        return None;
    }
    let progress = exit_elapsed.as_secs_f32() / TOAST_EXIT.as_secs_f32();
    let eased = progress * progress * (3.0 - 2.0 * progress);
    Some((1.0 - eased, -6.0 * eased))
}

fn advance_queue_panel(progress: f32, open: bool, dt: f32) -> (f32, bool) {
    let target = if open { 1.0 } else { 0.0 };
    let step = (dt.max(0.0) / QUEUE_PANEL_TRANSITION_SECONDS).clamp(0.0, 1.0);
    let progress = if progress < target {
        (progress + step).min(target)
    } else {
        (progress - step).max(target)
    };
    (progress, (progress - target).abs() > f32::EPSILON)
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview_track(uri: &str, available: bool) -> Track {
        Track {
            id: TrackId::new(),
            uri: uri.to_owned(),
            title: "Track".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration_ms: 1_000,
            codec: "FLAC".to_owned(),
            favorite: false,
            available,
        }
    }

    fn library_target(track_id: TrackId) -> TableActivationTarget {
        TableActivationTarget::Library {
            favorites_only: false,
            track_id,
        }
    }

    #[test]
    fn table_activation_requires_two_quick_clicks_on_same_target() {
        let mut tracker = TableActivationTracker::default();
        let now = Instant::now();
        let first = library_target(TrackId::new());
        let second = library_target(TrackId::new());

        assert!(!tracker.register(first, now));
        assert!(!tracker.register(second, now + Duration::from_millis(100)));
        assert!(tracker.register(
            second,
            now + Duration::from_millis(100) + TABLE_ACTIVATION_INTERVAL
        ));
    }

    #[test]
    fn expired_table_click_starts_a_new_activation_pair() {
        let mut tracker = TableActivationTracker::default();
        let now = Instant::now();
        let target = library_target(TrackId::new());

        assert!(!tracker.register(target, now));
        assert!(!tracker.register(
            target,
            now + TABLE_ACTIVATION_INTERVAL + Duration::from_millis(1)
        ));
        assert!(tracker.register(
            target,
            now + TABLE_ACTIVATION_INTERVAL + Duration::from_millis(100)
        ));
    }

    #[test]
    fn successful_table_activation_consumes_the_click_pair() {
        let mut tracker = TableActivationTracker::default();
        let now = Instant::now();
        let target = library_target(TrackId::new());

        assert!(!tracker.register(target, now));
        assert!(tracker.register(target, now + Duration::from_millis(100)));
        assert!(!tracker.register(target, now + Duration::from_millis(200)));
    }

    #[test]
    fn desktop_file_uri_argument_becomes_local_path() {
        assert_eq!(
            argument_to_path("file:///tmp/Hello%20World.flac".into()),
            Path::new("/tmp/Hello World.flac")
        );
    }

    #[test]
    fn playback_mode_toast_enters_holds_and_fades_out() {
        let entering = mode_toast_animation(Duration::from_millis(60)).expect("entering");
        assert!(entering.0 > 0.0 && entering.0 < 1.0);
        assert!(entering.1 > 0.0);

        assert_eq!(
            mode_toast_animation(MODE_TOAST_ENTER + Duration::from_millis(200)),
            Some((1.0, 0.0))
        );

        let exiting =
            mode_toast_animation(MODE_TOAST_ENTER + MODE_TOAST_HOLD + Duration::from_millis(200))
                .expect("exiting");
        assert!(exiting.0 > 0.0 && exiting.0 < 1.0);
        assert!(exiting.1 < 0.0);
        assert_eq!(
            mode_toast_animation(MODE_TOAST_ENTER + MODE_TOAST_HOLD + MODE_TOAST_EXIT),
            None
        );
    }

    #[test]
    fn transient_toast_slides_down_and_keeps_errors_visible_longer() {
        let entering = toast_animation(Duration::from_millis(60), false).expect("entering");
        assert!(entering.0 > 0.0 && entering.0 < 1.0);
        assert!(entering.1 < 0.0);

        assert_eq!(
            toast_animation(TOAST_ENTER + Duration::from_millis(200), false),
            Some((1.0, 0.0))
        );

        let exiting = toast_animation(
            TOAST_ENTER + TOAST_SUCCESS_HOLD + Duration::from_millis(100),
            false,
        )
        .expect("exiting");
        assert!(exiting.0 > 0.0 && exiting.0 < 1.0);
        assert!(exiting.1 < 0.0);
        assert_eq!(
            toast_animation(TOAST_ENTER + TOAST_SUCCESS_HOLD + TOAST_EXIT, false),
            None
        );
        assert!(toast_animation(TOAST_ENTER + TOAST_SUCCESS_HOLD + TOAST_EXIT, true).is_some());
        assert_eq!(
            toast_animation(TOAST_ENTER + TOAST_ERROR_HOLD + TOAST_EXIT, true),
            None
        );
    }

    #[test]
    fn queue_panel_animation_opens_and_closes_without_jumping() {
        let (opening, pending) = advance_queue_panel(0.0, true, 0.07);
        assert!(opening > 0.0 && opening < 1.0);
        assert!(pending);

        let (reversing, pending) = advance_queue_panel(opening, false, 0.035);
        assert!(reversing > 0.0 && reversing < opening);
        assert!(pending);

        assert_eq!(advance_queue_panel(reversing, false, 1.0), (0.0, false));
        assert_eq!(advance_queue_panel(opening, true, 1.0), (1.0, false));
    }

    #[test]
    fn playlist_preview_keeps_local_available_candidates_in_order() {
        let playlist_id = PlaylistId::new();
        let tracks = [
            preview_track("file:///music/a.flac", true),
            preview_track("file:///music/a.flac", true),
            preview_track("file:///music/missing.flac", false),
            preview_track("file:///music/b.flac", true),
        ];

        let preview = playlist_preview(playlist_id, &tracks);

        assert_eq!(
            preview.cover_candidates,
            ["file:///music/a.flac", "file:///music/b.flac"]
        );
    }
}
