//! Durable media identity, playback history, favorites, and manual playlists.
//!
//! Paths and embedded tags are mutable observations. The catalog assigns a
//! random [`TrackId`] once and keeps playlist references attached to that ID
//! while a scanner reconciles moves and metadata-only edits.

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use wavora_core::{Playlist, PlaylistId, Track, TrackId};
use wavora_media::{
    AcousticFingerprint, CachedAudioEvidence, FileIdentity, ScannedTrack,
    acoustic_fingerprints_match,
};

const FAVORITES_KIND: &str = "favorites";
const SCHEMA_VERSION: i64 = 2;
const EXACT_DURATION_TOLERANCE_MS: u64 = 1_500;
const MIN_FUZZY_DURATION_TOLERANCE_MS: u64 = 3_000;
const MAX_FUZZY_DURATION_TOLERANCE_MS: u64 = 10_000;
const MAX_FUZZY_CANDIDATES: usize = 32;
const MAX_ACOUSTIC_FINGERPRINT_ITEMS: usize = 2_048;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("catalog database failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("catalog contains an invalid identifier: {0}")]
    InvalidId(#[from] uuid::Error),
    #[error("playlist names cannot be empty")]
    EmptyPlaylistName,
    #[error("the requested playlist does not exist")]
    PlaylistNotFound,
    #[error("system playlists cannot be renamed or deleted")]
    SystemPlaylist,
    #[error("no library scan is active")]
    NoActiveScan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistEntry {
    pub id: i64,
    pub playlist_id: PlaylistId,
    pub track_id: TrackId,
    pub position: i64,
    pub added_at_ms: i64,
}

struct ScanSession {
    root: PathBuf,
    seen: HashSet<TrackId>,
}

pub struct Catalog {
    connection: Connection,
    path: PathBuf,
    active_scan: Option<ScanSession>,
}

impl Catalog {
    /// Opens or creates the catalog and applies schema migrations.
    ///
    /// # Errors
    ///
    /// Returns an I/O or `SQLite` error when the catalog cannot be prepared.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, CatalogError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            create_private_directory(parent)?;
        }
        prepare_private_file(&path)?;
        let connection = Connection::open(&path)?;
        Self::from_connection(connection, path)
    }

    #[cfg(test)]
    fn in_memory() -> Result<Self, CatalogError> {
        Self::from_connection(Connection::open_in_memory()?, PathBuf::from(":memory:"))
    }

    fn from_connection(connection: Connection, path: PathBuf) -> Result<Self, CatalogError> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        migrate(&connection)?;
        let mut catalog = Self {
            connection,
            path,
            active_scan: None,
        };
        catalog.ensure_favorites_playlist()?;
        Ok(catalog)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Begins a reconciliation boundary. A scanner emits one start/finish pair
    /// at a time, so only one session is active in the UI-owned catalog.
    pub fn begin_scan(&mut self, root: impl Into<PathBuf>) {
        self.active_scan = Some(ScanSession {
            root: root.into(),
            seen: HashSet::new(),
        });
    }

    pub fn abort_scan(&mut self) {
        self.active_scan = None;
    }

    /// Reconciles a filesystem observation into an immutable catalog ID.
    ///
    /// Exact tag-independent PCM evidence is preferred. A strict, versioned
    /// acoustic match can reconnect one missing candidate after exact matching
    /// fails. Path and inode remain locator hints rather than identities.
    ///
    /// # Errors
    ///
    /// Returns an error when catalog reconciliation or identifier parsing fails.
    pub fn reconcile(&mut self, scanned: &ScannedTrack) -> Result<Track, CatalogError> {
        let now = unix_time_ms();
        let transaction = self.connection.transaction()?;
        let identity = find_identity(&transaction, scanned)?;
        let id = identity.unwrap_or(scanned.track.id);

        if identity.is_none() {
            insert_track(&transaction, id, scanned, now)?;
        } else {
            update_track(&transaction, id, scanned, now)?;
        }
        transaction.commit()?;

        if let Some(scan) = self.active_scan.as_mut() {
            scan.seen.insert(id);
        }
        self.track(id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
    }

    /// Completes a scan and marks unseen records under its root unavailable.
    /// Records are retained so playlist items become repairable tombstones.
    ///
    /// # Errors
    ///
    /// Returns an error when no scan is active or the catalog cannot be updated.
    pub fn finish_scan(&mut self) -> Result<Vec<TrackId>, CatalogError> {
        let scan = self.active_scan.take().ok_or(CatalogError::NoActiveScan)?;
        let mut statement = self
            .connection
            .prepare("SELECT id, path FROM tracks WHERE available = 1")?;
        let rows = statement.query_map([], |row| {
            Ok((
                parse_track_id(row, 0)?,
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })?;
        let mut missing = Vec::new();
        for row in rows {
            let (id, path) = row?;
            let is_in_scan = if scan.root.is_file() {
                path == scan.root
            } else {
                path.starts_with(&scan.root)
            };
            if is_in_scan && !scan.seen.contains(&id) {
                missing.push(id);
            }
        }
        drop(statement);
        let transaction = self.connection.transaction()?;
        for id in &missing {
            transaction.execute(
                "UPDATE tracks SET available = 0, updated_at_ms = ?2 WHERE id = ?1",
                params![id.to_string(), unix_time_ms()],
            )?;
        }
        transaction.commit()?;
        Ok(missing)
    }

    /// Loads one track, including an unavailable tombstone.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn track(&self, id: TrackId) -> Result<Option<Track>, CatalogError> {
        track_query(&self.connection, "t.id = ?1", params![id.to_string()])
    }

    /// Loads the exact, tag-independent PCM signature for one track.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried or contains an
    /// invalid signature.
    pub fn audio_signature(&self, id: TrackId) -> Result<Option<[u8; 32]>, CatalogError> {
        self.connection
            .query_row(
                "SELECT audio_signature FROM tracks WHERE id = ?1",
                params![id.to_string()],
                |row| blob_32(row, 0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Loads the currently available track at a file URI.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn track_by_uri(&self, uri: &str) -> Result<Option<Track>, CatalogError> {
        track_query(
            &self.connection,
            "t.uri = ?1 AND t.available = 1",
            params![uri],
        )
    }

    /// Loads all available library tracks.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn available_tracks(&self) -> Result<Vec<Track>, CatalogError> {
        tracks_query(&self.connection, "t.available = 1", [])
    }

    /// Loads scanner hints that avoid recomputing unchanged audio evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog contains invalid audio evidence.
    pub fn audio_evidence_cache(&self) -> Result<Vec<CachedAudioEvidence>, CatalogError> {
        let mut statement = self.connection.prepare(
            "SELECT path, device, inode, size_bytes, modified_ns, audio_signature,
                    acoustic_fingerprint_algorithm, acoustic_fingerprint
             FROM tracks ORDER BY updated_at_ms DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(CachedAudioEvidence {
                path: PathBuf::from(row.get::<_, String>(0)?),
                file: FileIdentity {
                    device: row
                        .get::<_, Option<i64>>(1)?
                        .and_then(|value| u64::try_from(value).ok()),
                    inode: row
                        .get::<_, Option<i64>>(2)?
                        .and_then(|value| u64::try_from(value).ok()),
                    size_bytes: u64::try_from(row.get::<_, i64>(3)?).unwrap_or_default(),
                    modified_ns: row.get(4)?,
                },
                audio_signature: blob_32(row, 5)?,
                acoustic_fingerprint: optional_acoustic_fingerprint(row, 6, 7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Creates an empty manual playlist.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty name or a failed catalog transaction.
    pub fn create_playlist(&mut self, name: &str) -> Result<Playlist, CatalogError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CatalogError::EmptyPlaylistName);
        }
        let playlist = Playlist {
            id: PlaylistId::new(),
            name: name.to_owned(),
            system_kind: None,
            created_at_ms: unix_time_ms(),
            updated_at_ms: unix_time_ms(),
        };
        self.connection.execute(
            "INSERT INTO playlists (id, name, system_kind, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, NULL, ?3, ?4)",
            params![
                playlist.id.to_string(),
                playlist.name,
                playlist.created_at_ms,
                playlist.updated_at_ms
            ],
        )?;
        Ok(playlist)
    }

    /// Loads user-created playlists in display order.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn playlists(&self) -> Result<Vec<Playlist>, CatalogError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, system_kind, created_at_ms, updated_at_ms
             FROM playlists WHERE system_kind IS NULL
             ORDER BY name COLLATE NOCASE, created_at_ms",
        )?;
        let rows = statement.query_map([], playlist_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Renames a manual playlist.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty name, a system playlist, or a database failure.
    pub fn rename_playlist(
        &mut self,
        playlist_id: PlaylistId,
        name: &str,
    ) -> Result<(), CatalogError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CatalogError::EmptyPlaylistName);
        }
        ensure_manual_playlist(&self.connection, playlist_id)?;
        self.connection.execute(
            "UPDATE playlists SET name = ?2, updated_at_ms = ?3 WHERE id = ?1",
            params![playlist_id.to_string(), name, unix_time_ms()],
        )?;
        Ok(())
    }

    /// Deletes a manual playlist and its items.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/system playlist or a database failure.
    pub fn delete_playlist(&mut self, playlist_id: PlaylistId) -> Result<(), CatalogError> {
        ensure_manual_playlist(&self.connection, playlist_id)?;
        self.connection.execute(
            "DELETE FROM playlists WHERE id = ?1",
            params![playlist_id.to_string()],
        )?;
        Ok(())
    }

    /// Appends a track to a playlist. Duplicate tracks are allowed.
    ///
    /// # Errors
    ///
    /// Returns an error for missing records or a failed transaction.
    pub fn add_to_playlist(
        &mut self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<PlaylistEntry, CatalogError> {
        ensure_playlist_exists(&self.connection, playlist_id)?;
        let transaction = self.connection.transaction()?;
        let position = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_items WHERE playlist_id = ?1",
            params![playlist_id.to_string()],
            |row| row.get(0),
        )?;
        let added_at_ms = unix_time_ms();
        transaction.execute(
            "INSERT INTO playlist_items (playlist_id, track_id, position, added_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                playlist_id.to_string(),
                track_id.to_string(),
                position,
                added_at_ms
            ],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "UPDATE playlists SET updated_at_ms = ?2 WHERE id = ?1",
            params![playlist_id.to_string(), added_at_ms],
        )?;
        transaction.commit()?;
        Ok(PlaylistEntry {
            id,
            playlist_id,
            track_id,
            position,
            added_at_ms,
        })
    }

    /// Loads stable playlist-entry identities in playback order.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn playlist_entries(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Vec<PlaylistEntry>, CatalogError> {
        let mut statement = self.connection.prepare(
            "SELECT id, playlist_id, track_id, position, added_at_ms
             FROM playlist_items WHERE playlist_id = ?1
             ORDER BY position, id",
        )?;
        let rows = statement.query_map(params![playlist_id.to_string()], |row| {
            Ok(PlaylistEntry {
                id: row.get(0)?,
                playlist_id: parse_playlist_id(row, 1)?,
                track_id: parse_track_id(row, 2)?,
                position: row.get(3)?,
                added_at_ms: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns playlist tracks including unavailable tombstones.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing playlist or an invalid catalog record.
    pub fn playlist_tracks(&self, playlist_id: PlaylistId) -> Result<Vec<Track>, CatalogError> {
        ensure_playlist_exists(&self.connection, playlist_id)?;
        let mut statement = self.connection.prepare(
            "SELECT t.id, t.uri, t.title, t.artist, t.album, t.duration_ms, t.codec,
                    EXISTS (
                        SELECT 1 FROM playlist_items favorite
                        JOIN playlists fp ON fp.id = favorite.playlist_id
                        WHERE favorite.track_id = t.id AND fp.system_kind = 'favorites'
                    ), t.available
             FROM playlist_items item
             JOIN tracks t ON t.id = item.track_id
             WHERE item.playlist_id = ?1
             ORDER BY item.position, item.id",
        )?;
        let rows = statement.query_map(params![playlist_id.to_string()], track_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Removes one occurrence from a playlist and compacts its positions.
    ///
    /// # Errors
    ///
    /// Returns an error when the transaction cannot be committed.
    pub fn remove_playlist_entry(&mut self, entry_id: i64) -> Result<(), CatalogError> {
        let playlist_id: Option<String> = self
            .connection
            .query_row(
                "SELECT playlist_id FROM playlist_items WHERE id = ?1",
                params![entry_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(playlist_id) = playlist_id else {
            return Ok(());
        };
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM playlist_items WHERE id = ?1",
            params![entry_id],
        )?;
        normalize_positions(&transaction, &playlist_id)?;
        transaction.execute(
            "UPDATE playlists SET updated_at_ms = ?2 WHERE id = ?1",
            params![playlist_id, unix_time_ms()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Moves one playlist occurrence to a zero-based position.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing entry or a failed transaction.
    pub fn move_playlist_entry(
        &mut self,
        entry_id: i64,
        new_position: usize,
    ) -> Result<(), CatalogError> {
        let playlist_id: String = self
            .connection
            .query_row(
                "SELECT playlist_id FROM playlist_items WHERE id = ?1",
                params![entry_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(CatalogError::PlaylistNotFound)?;
        let transaction = self.connection.transaction()?;
        let mut ids = {
            let mut statement = transaction.prepare(
                "SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
            )?;
            statement
                .query_map(params![playlist_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let Some(old_position) = ids.iter().position(|candidate| *candidate == entry_id) else {
            return Ok(());
        };
        let id = ids.remove(old_position);
        let target = new_position.min(ids.len());
        ids.insert(target, id);
        for (position, id) in ids.into_iter().enumerate() {
            transaction.execute(
                "UPDATE playlist_items SET position = ?2 WHERE id = ?1",
                params![id, i64::try_from(position).unwrap_or(i64::MAX)],
            )?;
        }
        transaction.execute(
            "UPDATE playlists SET updated_at_ms = ?2 WHERE id = ?1",
            params![playlist_id, unix_time_ms()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Adds or removes a track from the built-in favorites playlist.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing track or a failed transaction.
    pub fn set_favorite(&mut self, track_id: TrackId, favorite: bool) -> Result<(), CatalogError> {
        let playlist_id = self.favorites_playlist_id()?;
        let existing: Option<i64> = self
            .connection
            .query_row(
                "SELECT item.id FROM playlist_items item
                 WHERE item.playlist_id = ?1 AND item.track_id = ?2 LIMIT 1",
                params![playlist_id.to_string(), track_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        match (favorite, existing) {
            (true, None) => {
                self.add_to_playlist(playlist_id, track_id)?;
            }
            (false, Some(id)) => self.remove_playlist_entry(id)?,
            _ => {}
        }
        Ok(())
    }

    /// Records a play and updates restart state atomically.
    ///
    /// # Errors
    ///
    /// Returns an error when the transaction cannot be committed.
    pub fn record_played(&mut self, track_id: TrackId) -> Result<(), CatalogError> {
        let now = unix_time_ms();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO playback_history (track_id, last_played_at_ms, play_count)
             VALUES (?1, ?2, 1)
             ON CONFLICT(track_id) DO UPDATE SET
                 last_played_at_ms = excluded.last_played_at_ms,
                 play_count = playback_history.play_count + 1",
            params![track_id.to_string(), now],
        )?;
        transaction.execute(
            "INSERT INTO catalog_state (key, value) VALUES ('last_track_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![track_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Imports restart state without incrementing play statistics.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be updated.
    pub fn set_last_track(&mut self, track_id: TrackId) -> Result<(), CatalogError> {
        self.connection.execute(
            "INSERT INTO catalog_state (key, value) VALUES ('last_track_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![track_id.to_string()],
        )?;
        Ok(())
    }

    /// Imports URI-based recent history once during the catalog migration.
    /// Existing catalog history always wins over legacy JSON state.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be updated.
    pub fn import_recent_track(
        &mut self,
        track_id: TrackId,
        legacy_rank: usize,
    ) -> Result<(), CatalogError> {
        let last_played_at_ms =
            unix_time_ms().saturating_sub(i64::try_from(legacy_rank).unwrap_or(i64::MAX));
        self.connection.execute(
            "INSERT INTO playback_history (track_id, last_played_at_ms, play_count)
             VALUES (?1, ?2, 0)
             ON CONFLICT(track_id) DO NOTHING",
            params![track_id.to_string(), last_played_at_ms],
        )?;
        Ok(())
    }

    /// Loads the restart track identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the catalog cannot be queried.
    pub fn last_track_id(&self) -> Result<Option<TrackId>, CatalogError> {
        self.connection
            .query_row(
                "SELECT value FROM catalog_state WHERE key = 'last_track_id'",
                [],
                |row| parse_track_id(row, 0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn ensure_favorites_playlist(&mut self) -> Result<(), CatalogError> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE system_kind = ?1)",
            params![FAVORITES_KIND],
            |row| row.get(0),
        )?;
        if !exists {
            let now = unix_time_ms();
            self.connection.execute(
                "INSERT INTO playlists (id, name, system_kind, created_at_ms, updated_at_ms)
                 VALUES (?1, 'Favorites', ?2, ?3, ?3)",
                params![PlaylistId::new().to_string(), FAVORITES_KIND, now],
            )?;
        }
        Ok(())
    }

    fn favorites_playlist_id(&self) -> Result<PlaylistId, CatalogError> {
        self.connection
            .query_row(
                "SELECT id FROM playlists WHERE system_kind = ?1",
                params![FAVORITES_KIND],
                |row| parse_playlist_id(row, 0),
            )
            .map_err(Into::into)
    }
}

fn migrate(connection: &Connection) -> Result<(), CatalogError> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery.into());
    }
    if version == 0 {
        connection.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE tracks (
                 id                 TEXT PRIMARY KEY NOT NULL,
                 uri                TEXT NOT NULL,
                 path               TEXT NOT NULL,
                 title              TEXT NOT NULL,
                 artist             TEXT NOT NULL,
                 album              TEXT NOT NULL,
                 duration_ms        INTEGER NOT NULL,
                 codec              TEXT NOT NULL,
                 device             INTEGER,
                 inode              INTEGER,
                 size_bytes         INTEGER NOT NULL,
                 modified_ns        INTEGER NOT NULL,
                 audio_signature    BLOB NOT NULL CHECK(length(audio_signature) = 32),
                 acoustic_fingerprint_algorithm INTEGER,
                 acoustic_fingerprint BLOB,
                 available          INTEGER NOT NULL CHECK(available IN (0, 1)),
                 created_at_ms      INTEGER NOT NULL,
                 updated_at_ms      INTEGER NOT NULL,
                 CHECK (
                     (acoustic_fingerprint_algorithm IS NULL
                      AND acoustic_fingerprint IS NULL)
                     OR
                     (acoustic_fingerprint_algorithm IS NOT NULL
                      AND acoustic_fingerprint IS NOT NULL
                      AND length(acoustic_fingerprint) % 4 = 0)
                 )
             );
             CREATE INDEX tracks_uri_available ON tracks(uri, available);
             CREATE INDEX tracks_file_identity ON tracks(device, inode, available);
             CREATE INDEX tracks_audio_signature ON tracks(audio_signature, duration_ms);
             CREATE INDEX tracks_fuzzy_candidates ON tracks(duration_ms, available)
                 WHERE acoustic_fingerprint IS NOT NULL;

             CREATE TABLE playlists (
                 id                 TEXT PRIMARY KEY NOT NULL,
                 name               TEXT NOT NULL CHECK(length(trim(name)) > 0),
                 system_kind        TEXT UNIQUE,
                 created_at_ms      INTEGER NOT NULL,
                 updated_at_ms      INTEGER NOT NULL
             );
             CREATE TABLE playlist_items (
                 id                 INTEGER PRIMARY KEY AUTOINCREMENT,
                 playlist_id        TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                 track_id           TEXT NOT NULL REFERENCES tracks(id) ON DELETE RESTRICT,
                 position           INTEGER NOT NULL,
                 added_at_ms        INTEGER NOT NULL
             );
             CREATE INDEX playlist_items_order ON playlist_items(playlist_id, position, id);
             CREATE INDEX playlist_items_track ON playlist_items(track_id);

             CREATE TABLE playback_history (
                 track_id           TEXT PRIMARY KEY NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                 last_played_at_ms   INTEGER NOT NULL,
                 play_count         INTEGER NOT NULL
             );
             CREATE TABLE catalog_state (
                 key                TEXT PRIMARY KEY NOT NULL,
                 value              TEXT NOT NULL
             );
             PRAGMA user_version = 2;
             COMMIT;",
        )?;
    }
    if version == 1 {
        connection.execute_batch(
            "BEGIN IMMEDIATE;
             ALTER TABLE tracks ADD COLUMN acoustic_fingerprint_algorithm INTEGER;
             ALTER TABLE tracks ADD COLUMN acoustic_fingerprint BLOB;
             CREATE INDEX tracks_fuzzy_candidates ON tracks(duration_ms, available)
                 WHERE acoustic_fingerprint IS NOT NULL;
             PRAGMA user_version = 2;
             COMMIT;",
        )?;
    }
    Ok(())
}

fn find_identity(
    transaction: &Transaction<'_>,
    scanned: &ScannedTrack,
) -> Result<Option<TrackId>, CatalogError> {
    let same_path: Option<(TrackId, Vec<u8>, u64)> = transaction
        .query_row(
            "SELECT id, audio_signature, duration_ms FROM tracks
             WHERE uri = ?1 AND available = 1
             ORDER BY updated_at_ms DESC LIMIT 1",
            params![scanned.track.uri],
            |row| {
                Ok((
                    parse_track_id(row, 0)?,
                    row.get(1)?,
                    u64::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                ))
            },
        )
        .optional()?;
    if let Some((id, signature, duration_ms)) = same_path {
        if signature == scanned.audio_signature
            && duration_ms.abs_diff(scanned.track.duration_ms) <= EXACT_DURATION_TOLERANCE_MS
        {
            return Ok(Some(id));
        }
        transaction.execute(
            "UPDATE tracks SET available = 0, updated_at_ms = ?2 WHERE id = ?1",
            params![id.to_string(), unix_time_ms()],
        )?;
    }

    if let (Some(device), Some(inode)) = (scanned.file.device, scanned.file.inode) {
        let device = i64::try_from(device).ok();
        let inode = i64::try_from(inode).ok();
        let minimum_duration = sqlite_u64(
            scanned
                .track
                .duration_ms
                .saturating_sub(EXACT_DURATION_TOLERANCE_MS),
        );
        let maximum_duration = sqlite_u64(
            scanned
                .track
                .duration_ms
                .saturating_add(EXACT_DURATION_TOLERANCE_MS),
        );
        let by_file_identity = transaction
            .query_row(
                "SELECT id FROM tracks
                 WHERE device = ?1 AND inode = ?2 AND audio_signature = ?3
                   AND duration_ms BETWEEN ?4 AND ?5
                 ORDER BY available DESC, updated_at_ms DESC LIMIT 1",
                params![
                    device,
                    inode,
                    scanned.audio_signature.as_slice(),
                    minimum_duration,
                    maximum_duration
                ],
                |row| parse_track_id(row, 0),
            )
            .optional()?;
        if by_file_identity.is_some() {
            return Ok(by_file_identity);
        }
    }

    let mut statement = transaction.prepare(
        "SELECT id, path, available FROM tracks
         WHERE audio_signature = ?1 AND duration_ms BETWEEN ?2 AND ?3
         ORDER BY updated_at_ms DESC",
    )?;
    let minimum_duration = sqlite_u64(
        scanned
            .track
            .duration_ms
            .saturating_sub(EXACT_DURATION_TOLERANCE_MS),
    );
    let maximum_duration = sqlite_u64(
        scanned
            .track
            .duration_ms
            .saturating_add(EXACT_DURATION_TOLERANCE_MS),
    );
    let rows = statement.query_map(
        params![
            scanned.audio_signature.as_slice(),
            minimum_duration,
            maximum_duration
        ],
        |row| {
            Ok((
                parse_track_id(row, 0)?,
                PathBuf::from(row.get::<_, String>(1)?),
                row.get::<_, bool>(2)?,
            ))
        },
    )?;
    let candidates = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, path, available)| !*available || !path.exists())
        .map(|(id, _, _)| id)
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return Ok(Some(candidates[0]));
    }
    find_fuzzy_identity(transaction, scanned)
}

fn find_fuzzy_identity(
    transaction: &Transaction<'_>,
    scanned: &ScannedTrack,
) -> Result<Option<TrackId>, CatalogError> {
    let tolerance = fuzzy_duration_tolerance_ms(scanned.track.duration_ms);
    let minimum_duration = sqlite_u64(scanned.track.duration_ms.saturating_sub(tolerance));
    let maximum_duration = sqlite_u64(scanned.track.duration_ms.saturating_add(tolerance));
    let mut statement = transaction.prepare(
        "SELECT id, path, available, acoustic_fingerprint_algorithm,
                acoustic_fingerprint
         FROM tracks
         WHERE duration_ms BETWEEN ?1 AND ?2
           AND acoustic_fingerprint_algorithm IS NOT NULL
           AND acoustic_fingerprint IS NOT NULL
         ORDER BY updated_at_ms DESC",
    )?;
    let rows = statement.query_map(params![minimum_duration, maximum_duration], |row| {
        Ok((
            parse_track_id(row, 0)?,
            PathBuf::from(row.get::<_, String>(1)?),
            row.get::<_, bool>(2)?,
            optional_acoustic_fingerprint(row, 3, 4)?,
        ))
    })?;
    let eligible = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, path, available, _)| !*available || !path.exists())
        .filter_map(|(id, _, _, fingerprint)| fingerprint.map(|fingerprint| (id, fingerprint)))
        .collect::<Vec<_>>();
    if eligible.len() > MAX_FUZZY_CANDIDATES {
        return Ok(None);
    }
    let candidates = eligible
        .into_iter()
        .filter(|(_, fingerprint)| {
            acoustic_fingerprints_match(&scanned.acoustic_fingerprint, fingerprint)
        })
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    Ok((candidates.len() == 1).then(|| candidates[0]))
}

fn fuzzy_duration_tolerance_ms(duration_ms: u64) -> u64 {
    (duration_ms / 100).clamp(
        MIN_FUZZY_DURATION_TOLERANCE_MS,
        MAX_FUZZY_DURATION_TOLERANCE_MS,
    )
}

fn insert_track(
    transaction: &Transaction<'_>,
    id: TrackId,
    scanned: &ScannedTrack,
    now: i64,
) -> Result<(), rusqlite::Error> {
    let acoustic_fingerprint = acoustic_fingerprint_blob(&scanned.acoustic_fingerprint);
    transaction.execute(
        "INSERT INTO tracks (
             id, uri, path, title, artist, album, duration_ms, codec,
             device, inode, size_bytes, modified_ns, audio_signature,
             acoustic_fingerprint_algorithm, acoustic_fingerprint,
             available, created_at_ms, updated_at_ms
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
             ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, ?16, ?16
         )",
        params![
            id.to_string(),
            scanned.track.uri,
            scanned.path.to_string_lossy(),
            scanned.track.title,
            scanned.track.artist,
            scanned.track.album,
            sqlite_u64(scanned.track.duration_ms),
            scanned.track.codec,
            scanned
                .file
                .device
                .and_then(|value| i64::try_from(value).ok()),
            scanned
                .file
                .inode
                .and_then(|value| i64::try_from(value).ok()),
            sqlite_u64(scanned.file.size_bytes),
            scanned.file.modified_ns,
            scanned.audio_signature.as_slice(),
            i64::from(scanned.acoustic_fingerprint.algorithm()),
            acoustic_fingerprint,
            now
        ],
    )?;
    Ok(())
}

fn update_track(
    transaction: &Transaction<'_>,
    id: TrackId,
    scanned: &ScannedTrack,
    now: i64,
) -> Result<(), rusqlite::Error> {
    let acoustic_fingerprint = acoustic_fingerprint_blob(&scanned.acoustic_fingerprint);
    transaction.execute(
        "UPDATE tracks SET
             uri = ?2, path = ?3, title = ?4, artist = ?5, album = ?6,
             duration_ms = ?7, codec = ?8, device = ?9, inode = ?10,
             size_bytes = ?11, modified_ns = ?12, audio_signature = ?13,
             acoustic_fingerprint_algorithm = ?14,
             acoustic_fingerprint = ?15, available = 1, updated_at_ms = ?16
         WHERE id = ?1",
        params![
            id.to_string(),
            scanned.track.uri,
            scanned.path.to_string_lossy(),
            scanned.track.title,
            scanned.track.artist,
            scanned.track.album,
            sqlite_u64(scanned.track.duration_ms),
            scanned.track.codec,
            scanned
                .file
                .device
                .and_then(|value| i64::try_from(value).ok()),
            scanned
                .file
                .inode
                .and_then(|value| i64::try_from(value).ok()),
            sqlite_u64(scanned.file.size_bytes),
            scanned.file.modified_ns,
            scanned.audio_signature.as_slice(),
            i64::from(scanned.acoustic_fingerprint.algorithm()),
            acoustic_fingerprint,
            now
        ],
    )?;
    Ok(())
}

fn track_query<P>(
    connection: &Connection,
    condition: &str,
    parameters: P,
) -> Result<Option<Track>, CatalogError>
where
    P: rusqlite::Params,
{
    let query = format!(
        "SELECT t.id, t.uri, t.title, t.artist, t.album, t.duration_ms, t.codec,
                EXISTS (
                    SELECT 1 FROM playlist_items favorite
                    JOIN playlists fp ON fp.id = favorite.playlist_id
                    WHERE favorite.track_id = t.id AND fp.system_kind = 'favorites'
                ), t.available
         FROM tracks t WHERE {condition} ORDER BY t.updated_at_ms DESC LIMIT 1"
    );
    connection
        .query_row(&query, parameters, track_from_row)
        .optional()
        .map_err(Into::into)
}

fn tracks_query<P>(
    connection: &Connection,
    condition: &str,
    parameters: P,
) -> Result<Vec<Track>, CatalogError>
where
    P: rusqlite::Params,
{
    let query = format!(
        "SELECT t.id, t.uri, t.title, t.artist, t.album, t.duration_ms, t.codec,
                EXISTS (
                    SELECT 1 FROM playlist_items favorite
                    JOIN playlists fp ON fp.id = favorite.playlist_id
                    WHERE favorite.track_id = t.id AND fp.system_kind = 'favorites'
                ), t.available
         FROM tracks t WHERE {condition}
         ORDER BY lower(t.artist), lower(t.title), t.id"
    );
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(parameters, track_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn track_from_row(row: &Row<'_>) -> Result<Track, rusqlite::Error> {
    Ok(Track {
        id: parse_track_id(row, 0)?,
        uri: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        duration_ms: u64::try_from(row.get::<_, i64>(5)?).unwrap_or_default(),
        codec: row.get(6)?,
        favorite: row.get(7)?,
        available: row.get(8)?,
    })
}

fn playlist_from_row(row: &Row<'_>) -> Result<Playlist, rusqlite::Error> {
    Ok(Playlist {
        id: parse_playlist_id(row, 0)?,
        name: row.get(1)?,
        system_kind: row.get(2)?,
        created_at_ms: row.get(3)?,
        updated_at_ms: row.get(4)?,
    })
}

fn parse_track_id(row: &Row<'_>, index: usize) -> Result<TrackId, rusqlite::Error> {
    parse_id(row, index)
}

fn parse_playlist_id(row: &Row<'_>, index: usize) -> Result<PlaylistId, rusqlite::Error> {
    parse_id(row, index)
}

fn parse_id<T>(row: &Row<'_>, index: usize) -> Result<T, rusqlite::Error>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let value = row.get::<_, String>(index)?;
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn blob_32(row: &Row<'_>, index: usize) -> Result<[u8; 32], rusqlite::Error> {
    let value = row.get::<_, Vec<u8>>(index)?;
    value.try_into().map_err(|value: Vec<u8>| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Blob,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "expected 32-byte audio signature, found {} bytes",
                    value.len()
                ),
            )
            .into(),
        )
    })
}

fn acoustic_fingerprint_blob(fingerprint: &AcousticFingerprint) -> Vec<u8> {
    fingerprint
        .items()
        .iter()
        .flat_map(|item| item.to_le_bytes())
        .collect()
}

fn optional_acoustic_fingerprint(
    row: &Row<'_>,
    algorithm_index: usize,
    fingerprint_index: usize,
) -> Result<Option<AcousticFingerprint>, rusqlite::Error> {
    let algorithm = row.get::<_, Option<i64>>(algorithm_index)?;
    let fingerprint = row.get::<_, Option<Vec<u8>>>(fingerprint_index)?;
    let (algorithm, fingerprint) = match (algorithm, fingerprint) {
        (None, None) => return Ok(None),
        (Some(algorithm), Some(fingerprint)) => (algorithm, fingerprint),
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                fingerprint_index,
                rusqlite::types::Type::Blob,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "acoustic fingerprint algorithm and data must both be present",
                )
                .into(),
            ));
        }
    };
    let algorithm = u32::try_from(algorithm).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            algorithm_index,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    if !fingerprint.len().is_multiple_of(4) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            fingerprint_index,
            rusqlite::types::Type::Blob,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "acoustic fingerprint length must be divisible by 4, found {} bytes",
                    fingerprint.len()
                ),
            )
            .into(),
        ));
    }
    if fingerprint.len() / 4 > MAX_ACOUSTIC_FINGERPRINT_ITEMS {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            fingerprint_index,
            rusqlite::types::Type::Blob,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("acoustic fingerprint exceeds {MAX_ACOUSTIC_FINGERPRINT_ITEMS} items"),
            )
            .into(),
        ));
    }
    let items = fingerprint
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
        .collect();
    Ok(Some(AcousticFingerprint::from_parts(algorithm, items)))
}

fn ensure_playlist_exists(
    connection: &Connection,
    playlist_id: PlaylistId,
) -> Result<(), CatalogError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
        params![playlist_id.to_string()],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(CatalogError::PlaylistNotFound)
    }
}

fn ensure_manual_playlist(
    connection: &Connection,
    playlist_id: PlaylistId,
) -> Result<(), CatalogError> {
    let kind: Option<Option<String>> = connection
        .query_row(
            "SELECT system_kind FROM playlists WHERE id = ?1",
            params![playlist_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    match kind {
        Some(None) => Ok(()),
        Some(Some(_)) => Err(CatalogError::SystemPlaylist),
        None => Err(CatalogError::PlaylistNotFound),
    }
}

fn normalize_positions(
    transaction: &Transaction<'_>,
    playlist_id: &str,
) -> Result<(), rusqlite::Error> {
    let ids = {
        let mut statement = transaction.prepare(
            "SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
        )?;
        statement
            .query_map(params![playlist_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (position, id) in ids.into_iter().enumerate() {
        transaction.execute(
            "UPDATE playlist_items SET position = ?2 WHERE id = ?1",
            params![id, i64::try_from(position).unwrap_or(i64::MAX)],
        )?;
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn prepare_private_file(path: &Path) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        options.open(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        options.open(path)?;
        Ok(())
    }
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

fn sqlite_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wavora_media::{AcousticFingerprint, FileIdentity};

    fn acoustic_fingerprint(seed: u32) -> AcousticFingerprint {
        let mut state = seed;
        let items = (0..800)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                state
            })
            .collect();
        AcousticFingerprint::from_parts(1, items)
    }

    fn scanned(path: &Path, title: &str, signature_byte: u8) -> ScannedTrack {
        let uri = format!("file://{}", path.display());
        ScannedTrack {
            track: Track {
                id: TrackId::new(),
                uri,
                title: title.to_owned(),
                artist: "Artist".to_owned(),
                album: "Album".to_owned(),
                duration_ms: 180_000,
                codec: "FLAC".to_owned(),
                favorite: false,
                available: true,
            },
            path: path.to_owned(),
            file: FileIdentity {
                device: Some(1),
                inode: Some(u64::from(signature_byte)),
                size_bytes: 1_024,
                modified_ns: 1,
            },
            audio_signature: [signature_byte; 32],
            acoustic_fingerprint: acoustic_fingerprint(u32::from(signature_byte)),
        }
    }

    #[test]
    fn move_and_metadata_change_preserve_playlist_identity() {
        let mut catalog = Catalog::in_memory().expect("catalog");
        let original = scanned(Path::new("/missing/old.flac"), "Old title", 7);
        catalog.begin_scan(Path::new("/missing"));
        let first = catalog.reconcile(&original).expect("initial track");
        catalog.finish_scan().expect("finish initial scan");
        let playlist = catalog.create_playlist("Road trip").expect("playlist");
        catalog
            .add_to_playlist(playlist.id, first.id)
            .expect("playlist item");
        catalog.set_favorite(first.id, true).expect("favorite");

        let mut moved = scanned(Path::new("/missing/new.flac"), "Edited title", 7);
        moved.file.inode = Some(99);
        catalog.begin_scan(Path::new("/missing"));
        let reconciled = catalog.reconcile(&moved).expect("moved track");
        catalog.finish_scan().expect("finish moved scan");

        assert_eq!(reconciled.id, first.id);
        assert_eq!(reconciled.title, "Edited title");
        assert!(reconciled.favorite);
        let tracks = catalog
            .playlist_tracks(playlist.id)
            .expect("playlist tracks");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, first.id);
        assert_eq!(tracks[0].title, "Edited title");
    }

    #[test]
    fn replacing_a_path_does_not_retarget_existing_playlist_items() {
        let mut catalog = Catalog::in_memory().expect("catalog");
        let original = scanned(Path::new("/music/song.flac"), "Original", 1);
        let first = catalog.reconcile(&original).expect("initial track");
        let playlist = catalog.create_playlist("Keep").expect("playlist");
        catalog
            .add_to_playlist(playlist.id, first.id)
            .expect("playlist item");

        let replacement = scanned(Path::new("/music/song.flac"), "Replacement", 2);
        let second = catalog.reconcile(&replacement).expect("replacement");

        assert_ne!(first.id, second.id);
        let tracks = catalog
            .playlist_tracks(playlist.id)
            .expect("playlist tracks");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, first.id);
        assert_eq!(tracks[0].title, "Original");
    }

    #[test]
    fn a_unique_missing_fuzzy_match_preserves_identity_after_reencoding() {
        let mut catalog = Catalog::in_memory().expect("catalog");
        let original = scanned(Path::new("/missing/original.flac"), "Original", 21);
        let original_fingerprint = original.acoustic_fingerprint.clone();
        let first = catalog.reconcile(&original).expect("initial track");
        let playlist = catalog.create_playlist("Keep").expect("playlist");
        catalog
            .add_to_playlist(playlist.id, first.id)
            .expect("playlist item");

        let mut reencoded = scanned(Path::new("/missing/reencoded.mp3"), "Reencoded", 22);
        reencoded.file.inode = Some(222);
        reencoded.track.duration_ms += 2_000;
        reencoded.acoustic_fingerprint = original_fingerprint;
        let reconciled = catalog.reconcile(&reencoded).expect("reencoded track");

        assert_eq!(reconciled.id, first.id);
        assert_eq!(reconciled.uri, reencoded.track.uri);
        let playlist_tracks = catalog
            .playlist_tracks(playlist.id)
            .expect("playlist tracks");
        assert_eq!(playlist_tracks[0].id, first.id);
        assert_eq!(playlist_tracks[0].title, "Reencoded");
    }

    #[test]
    fn ambiguous_missing_fuzzy_matches_do_not_merge() {
        let root = std::env::temp_dir().join(format!(
            "wavora-catalog-fuzzy-ambiguity-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let first_path = root.join("first.flac");
        let second_path = root.join("second.flac");
        std::fs::write(&first_path, b"first").expect("first file");
        std::fs::write(&second_path, b"second").expect("second file");

        let mut catalog = Catalog::in_memory().expect("catalog");
        let first = scanned(&first_path, "First", 31);
        let shared_fingerprint = first.acoustic_fingerprint.clone();
        let first = catalog.reconcile(&first).expect("first track");
        let mut second = scanned(&second_path, "Second", 32);
        second.acoustic_fingerprint = shared_fingerprint.clone();
        let second = catalog.reconcile(&second).expect("second track");
        assert_ne!(first.id, second.id);

        std::fs::remove_file(&first_path).expect("remove first");
        std::fs::remove_file(&second_path).expect("remove second");
        let mut restored = scanned(&root.join("restored.mp3"), "Restored", 33);
        restored.acoustic_fingerprint = shared_fingerprint;
        let restored = catalog.reconcile(&restored).expect("restored track");

        assert_ne!(restored.id, first.id);
        assert_ne!(restored.id, second.id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn playlists_allow_duplicates_and_reordering() {
        let mut catalog = Catalog::in_memory().expect("catalog");
        let track = catalog
            .reconcile(&scanned(Path::new("/music/song.flac"), "Song", 3))
            .expect("track");
        let playlist = catalog.create_playlist("Loop").expect("playlist");
        let first = catalog
            .add_to_playlist(playlist.id, track.id)
            .expect("first item");
        let second = catalog
            .add_to_playlist(playlist.id, track.id)
            .expect("second item");

        catalog
            .move_playlist_entry(second.id, 0)
            .expect("move item");
        let entries = catalog.playlist_entries(playlist.id).expect("entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, second.id);
        assert_eq!(entries[1].id, first.id);
    }

    #[test]
    fn an_existing_identical_copy_gets_its_own_track_identity() {
        let root =
            std::env::temp_dir().join(format!("wavora-catalog-copy-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let first_path = root.join("first.flac");
        let copy_path = root.join("copy.flac");
        std::fs::write(&first_path, b"same audio").expect("first file");
        std::fs::write(&copy_path, b"same audio").expect("copy file");

        let mut catalog = Catalog::in_memory().expect("catalog");
        let first = catalog
            .reconcile(&scanned(&first_path, "First", 8))
            .expect("first track");
        let mut copy = scanned(&copy_path, "Copy", 8);
        copy.file.inode = Some(9);
        let second = catalog.reconcile(&copy).expect("copied track");

        assert_ne!(first.id, second.id);
        assert_eq!(catalog.available_tracks().expect("tracks").len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_missing_file_remains_as_a_playlist_tombstone() {
        let root = Path::new("/missing-library");
        let mut catalog = Catalog::in_memory().expect("catalog");
        catalog.begin_scan(root);
        let track = catalog
            .reconcile(&scanned(&root.join("song.flac"), "Song", 10))
            .expect("track");
        catalog.finish_scan().expect("initial scan");
        let playlist = catalog.create_playlist("Archive").expect("playlist");
        catalog
            .add_to_playlist(playlist.id, track.id)
            .expect("playlist item");

        catalog.begin_scan(root);
        let missing = catalog.finish_scan().expect("missing scan");

        assert_eq!(missing, [track.id]);
        assert!(catalog.available_tracks().expect("tracks").is_empty());
        let playlist_tracks = catalog
            .playlist_tracks(playlist.id)
            .expect("playlist tracks");
        assert_eq!(playlist_tracks.len(), 1);
        assert!(!playlist_tracks[0].available);
    }

    #[test]
    fn catalog_and_playlist_ids_survive_reopening_the_database() {
        let root = std::env::temp_dir().join(format!(
            "wavora-catalog-persistence-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("library.sqlite3");
        let (track_id, playlist_id) = {
            let mut catalog = Catalog::open(&path).expect("open catalog");
            let track = catalog
                .reconcile(&scanned(Path::new("/music/song.flac"), "Song", 11))
                .expect("track");
            let playlist = catalog.create_playlist("Persistent").expect("playlist");
            catalog
                .add_to_playlist(playlist.id, track.id)
                .expect("playlist item");
            (track.id, playlist.id)
        };

        let catalog = Catalog::open(&path).expect("reopen catalog");
        assert_eq!(
            catalog.track(track_id).expect("track").unwrap().id,
            track_id
        );
        assert_eq!(
            catalog.playlist_tracks(playlist_id).expect("playlist")[0].id,
            track_id
        );
        let cached = catalog
            .audio_evidence_cache()
            .expect("audio evidence cache");
        assert_eq!(cached.len(), 1);
        assert_eq!(
            cached[0].acoustic_fingerprint,
            Some(acoustic_fingerprint(11))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)
                    .expect("database metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(catalog);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migrates_v1_catalogs_for_lazy_fingerprint_backfill() {
        let connection = Connection::open_in_memory().expect("connection");
        connection
            .execute_batch(
                "CREATE TABLE tracks (
                     id TEXT PRIMARY KEY NOT NULL,
                     duration_ms INTEGER NOT NULL,
                     available INTEGER NOT NULL
                 );
                 PRAGMA user_version = 1;",
            )
            .expect("v1 schema");

        migrate(&connection).expect("migration");

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);
        let mut statement = connection
            .prepare("PRAGMA table_info(tracks)")
            .expect("table info");
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("column names");
        assert!(
            columns
                .iter()
                .any(|column| column == "acoustic_fingerprint_algorithm")
        );
        assert!(
            columns
                .iter()
                .any(|column| column == "acoustic_fingerprint")
        );
        let fuzzy_index: bool = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'index' AND name = 'tracks_fuzzy_candidates'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("fuzzy candidate index");
        assert!(fuzzy_index);
    }
}
