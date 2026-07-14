use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use wavora_i18n::LanguagePreference;

const CONFIG_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub library_roots: Vec<PathBuf>,
    pub recent_uris: Vec<String>,
    pub favorite_uris: Vec<String>,
    pub last_track_uri: Option<String>,
    pub volume: f32,
    pub visual_preset: usize,
    pub visual_intensity: f32,
    pub visual_motion: f32,
    pub visual_depth: f32,
    pub visual_glow: f32,
    pub language: LanguagePreference,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            library_roots: Vec::new(),
            recent_uris: Vec::new(),
            favorite_uris: Vec::new(),
            last_track_uri: None,
            volume: 0.72,
            visual_preset: 0,
            visual_intensity: 1.0,
            visual_motion: 1.0,
            visual_depth: 1.0,
            visual_glow: 0.9,
            language: LanguagePreference::System,
        }
    }
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
        self.library_roots.sort();
        self.library_roots.dedup();
        let mut seen_recent = HashSet::new();
        self.recent_uris
            .retain(|uri| seen_recent.insert(uri.clone()));
        self.recent_uris.truncate(40);
        self.favorite_uris.sort();
        self.favorite_uris.dedup();
    }

    pub fn remember_uri(&mut self, uri: &str) {
        self.recent_uris.retain(|item| item != uri);
        self.recent_uris.insert(0, uri.to_owned());
        self.recent_uris.truncate(40);
        self.last_track_uri = Some(uri.to_owned());
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("the operating system did not provide a configuration directory")]
    NoConfigDirectory,
    #[error("configuration I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("configuration is invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    /// Finds the platform-native configuration location.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform provides no usable config directory.
    pub fn discover() -> Result<Self, ConfigError> {
        let dirs =
            ProjectDirs::from("dev", "Wavora", "Wavora").ok_or(ConfigError::NoConfigDirectory)?;
        Ok(Self {
            path: dirs.config_dir().join("config.json"),
        })
    }

    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads and normalizes the configuration, or returns defaults when absent.
    ///
    /// # Errors
    ///
    /// Returns an I/O or JSON decoding error for an existing invalid file.
    pub fn load(&self) -> Result<AppConfig, ConfigError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let mut config: AppConfig = serde_json::from_slice(&bytes)?;
                config.normalize();
                Ok(config)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Loads the configuration, preserving an invalid JSON file beside a
    /// fresh default configuration so one damaged write cannot brick startup.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if an invalid configuration cannot be backed up.
    pub fn load_resilient(&self) -> Result<(AppConfig, Option<PathBuf>), ConfigError> {
        match self.load() {
            Ok(config) => Ok((config, None)),
            Err(ConfigError::Json(_)) => {
                let backup = self.available_invalid_backup();
                fs::rename(&self.path, &backup)?;
                Ok((AppConfig::default(), Some(backup)))
            }
            Err(error) => Err(error),
        }
    }

    fn available_invalid_backup(&self) -> PathBuf {
        let first = self.path.with_extension("json.invalid");
        if !first.exists() {
            return first;
        }
        for suffix in 1_u32.. {
            let candidate = self.path.with_extension(format!("json.invalid.{suffix}"));
            if !candidate.exists() {
                return candidate;
            }
        }
        unreachable!("the backup suffix space is finite but cannot be exhausted in practice")
    }

    /// Atomically writes the configuration beside its temporary file.
    ///
    /// # Errors
    ///
    /// Returns an I/O or JSON serialization error.
    pub fn save(&self, config: &AppConfig) -> Result<(), ConfigError> {
        let parent = self.path.parent().ok_or(ConfigError::NoConfigDirectory)?;
        fs::create_dir_all(parent)?;
        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(config)?;
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_configuration_atomically() {
        let unique = format!("wavora-config-test-{}", std::process::id());
        let path = std::env::temp_dir().join(unique).join("config.json");
        let store = ConfigStore::at(&path);
        let mut config = AppConfig {
            volume: 0.41,
            ..AppConfig::default()
        };
        config.remember_uri("file:///music/test.flac");
        store.save(&config).expect("save config");
        let loaded = store.load().expect("load config");
        assert!((loaded.volume - 0.41).abs() < f32::EPSILON);
        assert_eq!(
            loaded.last_track_uri.as_deref(),
            Some("file:///music/test.flac")
        );
        let _ = fs::remove_dir_all(path.parent().expect("test parent"));
    }

    #[test]
    fn preserves_invalid_configuration_and_recovers_defaults() {
        let unique = format!("wavora-invalid-config-test-{}", std::process::id());
        let path = std::env::temp_dir().join(unique).join("config.json");
        let store = ConfigStore::at(&path);
        fs::create_dir_all(path.parent().expect("test parent")).expect("create test directory");
        fs::write(&path, b"{ definitely not json").expect("write invalid config");

        let (config, backup) = store.load_resilient().expect("recover configuration");
        let backup = backup.expect("invalid config backup");
        assert!((config.volume - AppConfig::default().volume).abs() < f32::EPSILON);
        assert!(!path.exists());
        assert_eq!(
            fs::read(backup).expect("read backup"),
            b"{ definitely not json"
        );
        let _ = fs::remove_dir_all(path.parent().expect("test parent"));
    }
}
