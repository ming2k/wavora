use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use wavora_core::PlaybackMode;
use wavora_i18n::LanguagePreference;
use wavora_visuals::Atmosphere;

const CONFIG_VERSION: u32 = 8;
const STATE_VERSION: u32 = 1;
const USER_DATA_VERSION: u32 = 1;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub library_roots: Vec<PathBuf>,
    pub volume: f32,
    pub playback_mode: PlaybackMode,
    pub visual_preset: usize,
    pub visual_intensity: f32,
    pub visual_motion: f32,
    pub visual_depth: f32,
    pub visual_glow: f32,
    pub atmosphere: Atmosphere,
    pub language: LanguagePreference,
    pub playlist_display: PlaylistDisplay,
    #[serde(rename = "recent_uris", skip_serializing)]
    legacy_recent_uris: Vec<String>,
    #[serde(rename = "favorite_uris", skip_serializing)]
    legacy_favorite_uris: Vec<String>,
    #[serde(rename = "last_track_uri", skip_serializing)]
    legacy_last_track_uri: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            library_roots: Vec::new(),
            volume: 0.72,
            playback_mode: PlaybackMode::Sequential,
            visual_preset: 0,
            visual_intensity: 1.0,
            visual_motion: 1.0,
            visual_depth: 1.0,
            visual_glow: 0.9,
            atmosphere: Atmosphere::default(),
            language: LanguagePreference::System,
            playlist_display: PlaylistDisplay::List,
            legacy_recent_uris: Vec::new(),
            legacy_favorite_uris: Vec::new(),
            legacy_last_track_uri: None,
        }
    }
}

/// Preferred presentation for the playlist collection selector.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaylistDisplay {
    #[default]
    List,
    Covers,
}

impl AppConfig {
    pub fn normalize(&mut self) {
        self.version = CONFIG_VERSION;
        self.volume = self.volume.clamp(0.0, 1.0);
        self.visual_preset %= wavora_visuals::PRESETS.len();
        self.visual_intensity = self.visual_intensity.clamp(0.45, 1.75);
        self.visual_motion = self.visual_motion.clamp(0.35, 1.65);
        self.visual_depth = self.visual_depth.clamp(0.50, 1.50);
        self.visual_glow = self.visual_glow.clamp(0.25, 1.50);
        self.atmosphere = std::mem::take(&mut self.atmosphere).normalized();
        self.library_roots.sort();
        self.library_roots.dedup();
    }

    fn take_legacy_persistence(&mut self) -> (AppState, UserData) {
        let mut state = AppState {
            recent_uris: std::mem::take(&mut self.legacy_recent_uris),
            last_track_uri: self.legacy_last_track_uri.take(),
            ..AppState::default()
        };
        let mut user_data = UserData {
            favorite_uris: std::mem::take(&mut self.legacy_favorite_uris),
            ..UserData::default()
        };
        state.normalize();
        user_data.normalize();
        (state, user_data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub version: u32,
    pub recent_uris: Vec<String>,
    pub last_track_uri: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            recent_uris: Vec::new(),
            last_track_uri: None,
        }
    }
}

impl AppState {
    pub fn normalize(&mut self) {
        self.version = STATE_VERSION;
        let mut seen_recent = HashSet::new();
        self.recent_uris
            .retain(|uri| seen_recent.insert(uri.clone()));
        self.recent_uris.truncate(40);
    }

    pub fn remember_uri(&mut self, uri: &str) {
        self.recent_uris.retain(|item| item != uri);
        self.recent_uris.insert(0, uri.to_owned());
        self.recent_uris.truncate(40);
        self.last_track_uri = Some(uri.to_owned());
    }

    fn is_empty(&self) -> bool {
        self.recent_uris.is_empty() && self.last_track_uri.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserData {
    pub version: u32,
    pub favorite_uris: Vec<String>,
}

impl Default for UserData {
    fn default() -> Self {
        Self {
            version: USER_DATA_VERSION,
            favorite_uris: Vec::new(),
        }
    }
}

impl UserData {
    pub fn normalize(&mut self) {
        self.version = USER_DATA_VERSION;
        self.favorite_uris.sort();
        self.favorite_uris.dedup();
    }

    fn is_empty(&self) -> bool {
        self.favorite_uris.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct PersistentApp {
    pub config: AppConfig,
    pub state: AppState,
    pub user_data: UserData,
}

impl PersistentApp {
    fn normalize(&mut self) {
        self.config.normalize();
        self.state.normalize();
        self.user_data.normalize();
    }
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("the operating system did not provide application storage directories")]
    NoStorageDirectory,
    #[error("persistence I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("stored data is invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct PersistenceStore {
    config: PathBuf,
    state: PathBuf,
    user_data: PathBuf,
}

impl PersistenceStore {
    /// Finds the platform-native config, state, and user-data locations.
    ///
    /// On Linux these honor `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and
    /// `XDG_DATA_HOME`, including the defaults prescribed by the XDG Base
    /// Directory Specification.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform provides no usable storage directory.
    pub fn discover() -> Result<Self, PersistenceError> {
        let dirs = ProjectDirs::from("io.github", "ming2k", "Wavora")
            .ok_or(PersistenceError::NoStorageDirectory)?;
        let state_dir = dirs
            .state_dir()
            .ok_or(PersistenceError::NoStorageDirectory)?;
        Ok(Self {
            config: dirs.config_dir().join("config.json"),
            state: state_dir.join("state.json"),
            user_data: dirs.data_dir().join("favorites.json"),
        })
    }

    #[must_use]
    pub fn at(
        config_path: impl Into<PathBuf>,
        state_path: impl Into<PathBuf>,
        user_data_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            config: config_path.into(),
            state: state_path.into(),
            user_data: user_data_path.into(),
        }
    }

    #[must_use]
    pub fn config_path(&self) -> &Path {
        &self.config
    }

    #[must_use]
    pub fn state_path(&self) -> &Path {
        &self.state
    }

    #[must_use]
    pub fn user_data_path(&self) -> &Path {
        &self.user_data
    }

    #[must_use]
    pub fn catalog_path(&self) -> PathBuf {
        self.user_data.with_file_name("library.sqlite3")
    }

    /// Loads each persistence class independently and migrates the former
    /// monolithic configuration format without losing recent tracks or favorites.
    ///
    /// Invalid JSON files are moved aside individually. Their paths are returned
    /// so the UI can tell the user exactly what was recovered.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when a file cannot be read or backed up.
    pub fn load_resilient(&self) -> Result<(PersistentApp, Vec<PathBuf>), PersistenceError> {
        let (mut config, config_outcome) = load_file_resilient::<AppConfig>(&self.config)?;
        let (legacy_state, legacy_user_data) = config.take_legacy_persistence();
        let (mut state, state_outcome) = load_file_resilient::<AppState>(&self.state)?;
        let (mut user_data, user_data_outcome) = load_file_resilient::<UserData>(&self.user_data)?;

        let mut migrated = false;
        if !state_outcome.was_loaded() && !legacy_state.is_empty() {
            state = legacy_state;
            migrated = true;
        }
        if !user_data_outcome.was_loaded() && !legacy_user_data.is_empty() {
            user_data = legacy_user_data;
            migrated = true;
        }

        let mut persistent = PersistentApp {
            config,
            state,
            user_data,
        };
        persistent.normalize();

        let recovered = [config_outcome, state_outcome, user_data_outcome]
            .into_iter()
            .filter_map(LoadOutcome::recovered_path)
            .collect();

        if migrated {
            self.save(&persistent)?;
        }

        Ok((persistent, recovered))
    }

    /// Atomically saves configuration, restart state, and user data in their
    /// respective platform-native directories.
    ///
    /// Newly-created directories use mode `0700` and files use `0600` on Unix.
    /// Each file and containing directory are synchronized before returning.
    ///
    /// # Errors
    ///
    /// Returns an I/O or JSON serialization error.
    pub fn save(&self, persistent: &PersistentApp) -> Result<(), PersistenceError> {
        save_json(&self.config, &persistent.config)?;
        save_json(&self.state, &persistent.state)?;
        save_json(&self.user_data, &persistent.user_data)?;
        Ok(())
    }
}

enum LoadOutcome {
    Loaded,
    Missing,
    Recovered(PathBuf),
}

impl LoadOutcome {
    const fn was_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    fn recovered_path(self) -> Option<PathBuf> {
        match self {
            Self::Recovered(path) => Some(path),
            Self::Loaded | Self::Missing => None,
        }
    }
}

fn load_file_resilient<T>(path: &Path) -> Result<(T, LoadOutcome), PersistenceError>
where
    T: Default + DeserializeOwned,
{
    match fs::read(path) {
        Ok(bytes) => {
            if let Ok(value) = serde_json::from_slice(&bytes) {
                Ok((value, LoadOutcome::Loaded))
            } else {
                let backup = available_invalid_backup(path);
                fs::rename(path, &backup)?;
                Ok((T::default(), LoadOutcome::Recovered(backup)))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok((T::default(), LoadOutcome::Missing))
        }
        Err(error) => Err(error.into()),
    }
}

fn available_invalid_backup(path: &Path) -> PathBuf {
    let first = path.with_extension("json.invalid");
    if !first.exists() {
        return first;
    }
    for suffix in 1_u32.. {
        let candidate = path.with_extension(format!("json.invalid.{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("the backup suffix space is finite but cannot be exhausted in practice")
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PersistenceError> {
    let parent = path.parent().ok_or(PersistenceError::NoStorageDirectory)?;
    create_private_directory(parent)?;

    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let (temporary, mut file) = create_private_temporary_file(path)?;
    let write_result = (|| -> io::Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        sync_directory(parent)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result.map_err(Into::into)
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn create_private_temporary_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("wavora-storage");
    loop {
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_file_name(format!(
            ".{file_name}.tmp.{}.{}",
            std::process::id(),
            sequence
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(name: &str) -> (PathBuf, PersistenceStore) {
        let root =
            std::env::temp_dir().join(format!("wavora-persistence-{name}-{}", std::process::id()));
        let store = PersistenceStore::at(
            root.join("config/wavora/config.json"),
            root.join("state/wavora/state.json"),
            root.join("data/wavora/favorites.json"),
        );
        (root, store)
    }

    #[test]
    fn round_trips_separate_persistence_classes_atomically() {
        let (root, store) = test_store("round-trip");
        let _ = fs::remove_dir_all(&root);
        let mut atmosphere = Atmosphere::default();
        assert!(atmosphere.add_source());
        atmosphere.sources[1].x = -0.35;
        atmosphere.sources[1].palette = wavora_visuals::AtmospherePalette::Custom;
        atmosphere.sources[1].shape = wavora_visuals::AtmosphereSourceShape::Beam;
        atmosphere.sources[1].audio_response = wavora_visuals::AtmosphereAudioResponse::Bass;
        atmosphere.field.kind = wavora_visuals::AtmosphereFieldKind::Watercolor;
        let mut persistent = PersistentApp {
            config: AppConfig {
                volume: 0.41,
                playback_mode: PlaybackMode::Shuffle,
                atmosphere: atmosphere.clone(),
                playlist_display: PlaylistDisplay::Covers,
                ..AppConfig::default()
            },
            state: AppState::default(),
            user_data: UserData {
                favorite_uris: vec!["file:///music/favorite.flac".to_owned()],
                ..UserData::default()
            },
        };
        persistent.state.remember_uri("file:///music/test.flac");

        store.save(&persistent).expect("save persistence");
        let (loaded, recovered) = store.load_resilient().expect("load persistence");

        assert!(recovered.is_empty());
        assert!((loaded.config.volume - 0.41).abs() < f32::EPSILON);
        assert_eq!(loaded.config.playback_mode, PlaybackMode::Shuffle);
        assert_eq!(loaded.config.atmosphere, atmosphere);
        assert_eq!(loaded.config.playlist_display, PlaylistDisplay::Covers);
        assert_eq!(
            loaded.state.last_track_uri.as_deref(),
            Some("file:///music/test.flac")
        );
        assert_eq!(
            loaded.user_data.favorite_uris,
            ["file:///music/favorite.flac"]
        );
        let config = fs::read_to_string(store.config_path()).expect("read config");
        assert!(!config.contains("recent_uris"));
        assert!(!config.contains("favorite_uris"));
        assert!(!config.contains("last_track_uri"));
        assert!(!root.join("config/wavora/.config.json.tmp").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migrates_the_monolithic_v3_configuration() {
        let (root, store) = test_store("migration");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(
            store
                .config_path()
                .parent()
                .expect("config parent directory"),
        )
        .expect("create config directory");
        fs::write(
            store.config_path(),
            br#"{
  "version": 3,
  "library_roots": ["/music"],
  "recent_uris": ["file:///music/recent.flac"],
  "favorite_uris": ["file:///music/favorite.flac"],
  "last_track_uri": "file:///music/recent.flac",
  "volume": 0.55,
  "visual_preset": 2,
  "visual_intensity": 1.0,
  "visual_motion": 1.0,
  "visual_depth": 1.0,
  "visual_glow": 0.9,
  "language": "system"
}"#,
        )
        .expect("write legacy configuration");

        let (loaded, recovered) = store.load_resilient().expect("migrate persistence");

        assert!(recovered.is_empty());
        assert_eq!(loaded.config.version, CONFIG_VERSION);
        assert_eq!(loaded.config.atmosphere, Atmosphere::default());
        assert_eq!(loaded.config.playlist_display, PlaylistDisplay::List);
        assert_eq!(loaded.state.recent_uris, ["file:///music/recent.flac"]);
        assert_eq!(
            loaded.user_data.favorite_uris,
            ["file:///music/favorite.flac"]
        );
        assert!(store.state_path().exists());
        assert!(store.user_data_path().exists());
        let config = fs::read_to_string(store.config_path()).expect("read migrated config");
        assert!(!config.contains("recent_uris"));
        assert!(!config.contains("favorite_uris"));
        assert!(!config.contains("last_track_uri"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migrates_existing_atmosphere_sources_to_geometry_and_audio_defaults() {
        let mut config: AppConfig = serde_json::from_str(
            r#"{
  "version": 6,
  "atmosphere": {
    "enabled": true,
    "composition_visible": true,
    "sources": [{
      "x": -0.25,
      "y": 0.6,
      "radius": 0.4,
      "intensity": 1.2,
      "palette": "preset",
      "falloff": "diffuse"
    }]
  }
}"#,
        )
        .expect("deserialize version 6 atmosphere");

        config.normalize();

        let source = &config.atmosphere.sources[0];
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(source.shape, wavora_visuals::AtmosphereSourceShape::Circle);
        assert_eq!(
            source.audio_response,
            wavora_visuals::AtmosphereAudioResponse::Energy
        );
        assert!((source.aspect - 2.0).abs() < f32::EPSILON);
        assert!((source.audio_scale - 0.18).abs() < f32::EPSILON);
    }

    #[test]
    fn recovers_each_invalid_file_independently() {
        let (root, store) = test_store("recovery");
        let _ = fs::remove_dir_all(&root);
        let persistent = PersistentApp {
            config: AppConfig {
                volume: 0.41,
                ..AppConfig::default()
            },
            state: AppState::default(),
            user_data: UserData::default(),
        };
        store.save(&persistent).expect("save persistence");
        fs::write(store.state_path(), b"{ definitely not json").expect("corrupt state");

        let (loaded, recovered) = store.load_resilient().expect("recover persistence");

        assert!((loaded.config.volume - 0.41).abs() < f32::EPSILON);
        assert!(loaded.state.recent_uris.is_empty());
        assert_eq!(recovered.len(), 1);
        assert_eq!(
            fs::read(&recovered[0]).expect("read backup"),
            b"{ definitely not json"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn creates_private_directories_and_files() {
        use std::os::unix::fs::PermissionsExt;

        let (root, store) = test_store("permissions");
        let _ = fs::remove_dir_all(&root);
        let persistent = PersistentApp {
            config: AppConfig::default(),
            state: AppState::default(),
            user_data: UserData::default(),
        };

        store.save(&persistent).expect("save persistence");

        for path in [
            store.config_path(),
            store.state_path(),
            store.user_data_path(),
        ] {
            let parent_mode = fs::metadata(path.parent().expect("parent"))
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777;
            let file_mode = fs::metadata(path)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(parent_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }
        let _ = fs::remove_dir_all(root);
    }
}
