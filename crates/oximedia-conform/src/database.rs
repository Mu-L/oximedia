//! `SQLite` database for media catalog and conform sessions.

use crate::error::ConformResult;
use crate::types::{FrameRate, MediaFile, Timecode};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Database schema version.
///
/// Version history:
/// - 1: initial schema
/// - 2: adds `file_mtime INTEGER` and `file_size INTEGER` to `media_files`
const SCHEMA_VERSION: i32 = 2;

/// Database manager for media catalog.
#[derive(Clone)]
pub struct Database {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl Database {
    /// Create or open a database at the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created or opened.
    pub fn open<P: AsRef<Path>>(path: P) -> ConformResult<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::new(manager)?;

        let db = Self {
            pool: Arc::new(pool),
        };

        db.initialize_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created.
    pub fn in_memory() -> ConformResult<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;

        let db = Self {
            pool: Arc::new(pool),
        };

        db.initialize_schema()?;
        Ok(db)
    }

    /// Initialize the database schema.
    fn initialize_schema(&self) -> ConformResult<()> {
        let conn = self.pool.get()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        let version: Option<i32> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();

        match version {
            None => {
                // Fresh database — create all tables at the current schema version.
                self.create_tables(&conn)?;
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![SCHEMA_VERSION],
                )?;
            }
            Some(v) if v < 2 => {
                // Migrate from schema version 1 → 2: add incremental-scan columns.
                self.migrate_v1_to_v2(&conn)?;
                conn.execute(
                    "UPDATE schema_version SET version = ?1",
                    params![SCHEMA_VERSION],
                )?;
            }
            _ => {
                // Already at the current version — nothing to do.
            }
        }

        Ok(())
    }

    /// Check whether `col_name` exists in `media_files` by querying PRAGMA.
    fn column_exists(conn: &rusqlite::Connection, col_name: &str) -> ConformResult<bool> {
        let mut stmt = conn.prepare("PRAGMA table_info(media_files)")?;
        let names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(names.iter().any(|n| n == col_name))
    }

    /// Migrate from schema version 1 to version 2.
    ///
    /// Adds `file_mtime INTEGER` and `file_size INTEGER` columns to the
    /// `media_files` table.  Both columns use `ALTER TABLE … ADD COLUMN`
    /// which is safe on SQLite — existing rows get `NULL` for the new columns.
    fn migrate_v1_to_v2(&self, conn: &rusqlite::Connection) -> ConformResult<()> {
        // `ALTER TABLE … ADD COLUMN` silently succeeds if the column already
        // exists on some SQLite builds, but errors on standard builds.  We
        // check first using `PRAGMA table_info` to be safe.
        if !Self::column_exists(conn, "file_mtime")? {
            conn.execute("ALTER TABLE media_files ADD COLUMN file_mtime INTEGER", [])?;
        }

        if !Self::column_exists(conn, "file_size")? {
            conn.execute("ALTER TABLE media_files ADD COLUMN file_size INTEGER", [])?;
        }

        Ok(())
    }

    /// Create database tables.
    fn create_tables(&self, conn: &rusqlite::Connection) -> ConformResult<()> {
        // Media files table — schema version 2 includes file_mtime / file_size.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS media_files (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                filename TEXT NOT NULL,
                duration REAL,
                timecode_start TEXT,
                width INTEGER,
                height INTEGER,
                fps REAL,
                size INTEGER,
                md5 TEXT,
                xxhash TEXT,
                metadata TEXT,
                cataloged_at TEXT NOT NULL,
                file_mtime INTEGER,
                file_size INTEGER
            )",
            [],
        )?;

        // Create indices
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_filename ON media_files(filename)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_path ON media_files(path)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_md5 ON media_files(md5)",
            [],
        )?;

        // Conform sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conform_sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                edl_path TEXT,
                status TEXT NOT NULL,
                config TEXT
            )",
            [],
        )?;

        // Clip matches table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clip_matches (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                clip_id TEXT NOT NULL,
                media_file_id TEXT NOT NULL,
                match_score REAL NOT NULL,
                match_method TEXT NOT NULL,
                details TEXT,
                FOREIGN KEY(session_id) REFERENCES conform_sessions(id),
                FOREIGN KEY(media_file_id) REFERENCES media_files(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clip_matches_session ON clip_matches(session_id)",
            [],
        )?;

        Ok(())
    }

    /// Add a media file to the catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the insertion fails.
    pub fn add_media_file(&self, media: &MediaFile) -> ConformResult<()> {
        let conn = self.pool.get()?;

        let timecode_start = media.timecode_start.map(|tc| tc.to_string());
        let fps = media.fps.map(super::types::FrameRate::as_f64);

        conn.execute(
            "INSERT OR REPLACE INTO media_files
             (id, path, filename, duration, timecode_start, width, height, fps, size, md5, xxhash, metadata, cataloged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                media.id.to_string(),
                media.path.to_string_lossy(),
                media.filename,
                media.duration,
                timecode_start,
                media.width,
                media.height,
                fps,
                media.size.map(|s| s as i64),
                media.md5,
                media.xxhash,
                media.metadata,
                media.cataloged_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Find media files by filename.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn find_by_filename(&self, filename: &str) -> ConformResult<Vec<MediaFile>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, duration, timecode_start, width, height, fps, size, md5, xxhash, metadata, cataloged_at
             FROM media_files WHERE filename = ?1",
        )?;

        let files = stmt
            .query_map(params![filename], Self::row_to_media_file)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Find media files by path pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn find_by_path_pattern(&self, pattern: &str) -> ConformResult<Vec<MediaFile>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, duration, timecode_start, width, height, fps, size, md5, xxhash, metadata, cataloged_at
             FROM media_files WHERE path LIKE ?1",
        )?;

        let files = stmt
            .query_map(params![pattern], Self::row_to_media_file)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Find media files by MD5 hash.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn find_by_md5(&self, md5: &str) -> ConformResult<Vec<MediaFile>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, duration, timecode_start, width, height, fps, size, md5, xxhash, metadata, cataloged_at
             FROM media_files WHERE md5 = ?1",
        )?;

        let files = stmt
            .query_map(params![md5], Self::row_to_media_file)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Get all media files.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_all_media_files(&self) -> ConformResult<Vec<MediaFile>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, duration, timecode_start, width, height, fps, size, md5, xxhash, metadata, cataloged_at
             FROM media_files ORDER BY cataloged_at DESC",
        )?;

        let files = stmt
            .query_map([], Self::row_to_media_file)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Get media file count.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_media_file_count(&self) -> ConformResult<usize> {
        let conn = self.pool.get()?;
        let count: usize = conn.query_row("SELECT COUNT(*) FROM media_files", [], |row| {
            row.get::<_, i64>(0).map(|v| v as usize)
        })?;
        Ok(count)
    }

    /// Remove a media file from the catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn remove_media_file(&self, id: &uuid::Uuid) -> ConformResult<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "DELETE FROM media_files WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Clear all media files from the catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn clear_media_files(&self) -> ConformResult<()> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM media_files", [])?;
        Ok(())
    }

    // ── Incremental scan helpers ───────────────────────────────────────────────

    /// Check whether the catalog already has an up-to-date entry for `path`.
    ///
    /// Returns `true` iff the catalog contains a row whose `path`, `file_size`,
    /// and `file_mtime` all match the supplied values — meaning the file has
    /// not changed since it was last ingested.
    ///
    /// A return value of `false` means either the file is absent from the
    /// catalog or its size / mtime differs from the stored values.
    ///
    /// `mtime` is expressed as Unix seconds (u64).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn has_unchanged(&self, path: &Path, size: u64, mtime: u64) -> ConformResult<bool> {
        let conn = self.pool.get()?;
        let path_str = path.to_string_lossy();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM media_files
             WHERE path = ?1
               AND file_size = ?2
               AND file_mtime = ?3",
            params![path_str.as_ref(), size as i64, mtime as i64],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Record or refresh the `file_mtime` and `file_size` for a path already
    /// present in the catalog.
    ///
    /// This is called after ingesting a media file so that subsequent calls to
    /// `has_unchanged` can skip re-processing it.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn update_mtime(&self, path: &Path, size: u64, mtime: u64) -> ConformResult<()> {
        let conn = self.pool.get()?;
        let path_str = path.to_string_lossy();
        conn.execute(
            "UPDATE media_files SET file_mtime = ?1, file_size = ?2 WHERE path = ?3",
            params![mtime as i64, size as i64, path_str.as_ref()],
        )?;
        Ok(())
    }

    /// Create a new conform session.
    ///
    /// # Errors
    ///
    /// Returns an error if the insertion fails.
    pub fn create_session(
        &self,
        id: uuid::Uuid,
        name: &str,
        edl_path: Option<&Path>,
        config: &str,
    ) -> ConformResult<()> {
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO conform_sessions (id, name, created_at, updated_at, edl_path, status, config)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                name,
                &now,
                &now,
                edl_path.map(|p| p.to_string_lossy().to_string()),
                "created",
                config,
            ],
        )?;

        Ok(())
    }

    /// Update session status.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn update_session_status(&self, id: &uuid::Uuid, status: &str) -> ConformResult<()> {
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE conform_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, id.to_string()],
        )?;

        Ok(())
    }

    /// Convert a database row to a `MediaFile`.
    fn row_to_media_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaFile> {
        let id_str: String = row.get(0)?;
        let path_str: String = row.get(1)?;
        let timecode_str: Option<String> = row.get(4)?;
        let fps_f64: Option<f64> = row.get(7)?;
        let cataloged_str: String = row.get(12)?;

        Ok(MediaFile {
            id: uuid::Uuid::parse_str(&id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            path: PathBuf::from(path_str),
            filename: row.get(2)?,
            duration: row.get(3)?,
            timecode_start: timecode_str.and_then(|s| Timecode::parse(&s).ok()),
            width: row.get(5)?,
            height: row.get(6)?,
            fps: fps_f64.map(FrameRate::Custom),
            size: row.get::<_, Option<i64>>(8)?.map(|s| s as u64),
            md5: row.get(9)?,
            xxhash: row.get(10)?,
            metadata: row.get(11)?,
            cataloged_at: DateTime::parse_from_rfc3339(&cataloged_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::in_memory().expect("db should be valid");
        assert_eq!(
            db.get_media_file_count()
                .expect("get_media_file_count should succeed"),
            0
        );
    }

    #[test]
    fn test_add_media_file() {
        let db = Database::in_memory().expect("db should be valid");
        let media = MediaFile::new(PathBuf::from("/test/file.mov"));
        db.add_media_file(&media)
            .expect("add_media_file should succeed");
        assert_eq!(
            db.get_media_file_count()
                .expect("get_media_file_count should succeed"),
            1
        );
    }

    #[test]
    fn test_find_by_filename() {
        let db = Database::in_memory().expect("db should be valid");
        let media = MediaFile::new(PathBuf::from("/test/file.mov"));
        db.add_media_file(&media)
            .expect("add_media_file should succeed");

        let found = db
            .find_by_filename("file.mov")
            .expect("found should be valid");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].filename, "file.mov");
    }

    #[test]
    fn test_clear_media_files() {
        let db = Database::in_memory().expect("db should be valid");
        let media1 = MediaFile::new(PathBuf::from("/test/file1.mov"));
        let media2 = MediaFile::new(PathBuf::from("/test/file2.mov"));
        db.add_media_file(&media1)
            .expect("add_media_file should succeed");
        db.add_media_file(&media2)
            .expect("add_media_file should succeed");
        assert_eq!(
            db.get_media_file_count()
                .expect("get_media_file_count should succeed"),
            2
        );

        db.clear_media_files()
            .expect("clear_media_files should succeed");
        assert_eq!(
            db.get_media_file_count()
                .expect("get_media_file_count should succeed"),
            0
        );
    }

    #[test]
    fn test_session_creation() {
        let db = Database::in_memory().expect("db should be valid");
        let session_id = uuid::Uuid::new_v4();
        db.create_session(session_id, "Test Session", None, "{}")
            .expect("test expectation failed");
        db.update_session_status(&session_id, "completed")
            .expect("update_session_status should succeed");
    }

    // ── Incremental scan tests ─────────────────────────────────────────────────

    /// An unchanged file (same path, size, mtime) must be recognised as
    /// unchanged so the scan can skip re-ingesting it.
    #[test]
    fn test_incremental_scan_skips_unchanged_file() {
        let db = Database::in_memory().expect("in-memory db must open");
        let path = PathBuf::from("/media/project/A001C001.mov");
        let size: u64 = 1_234_567;
        let mtime: u64 = 1_717_200_000; // arbitrary Unix timestamp

        // Before any insertion has_unchanged must return false
        assert!(
            !db.has_unchanged(&path, size, mtime)
                .expect("has_unchanged must not error"),
            "file not yet cataloged must return false"
        );

        // Ingest the file and record its mtime
        let mut media = MediaFile::new(path.clone());
        media.size = Some(size);
        db.add_media_file(&media)
            .expect("add_media_file must succeed");
        db.update_mtime(&path, size, mtime)
            .expect("update_mtime must succeed");

        // Now has_unchanged must return true
        assert!(
            db.has_unchanged(&path, size, mtime)
                .expect("has_unchanged must not error"),
            "unchanged file must be recognised as unchanged after ingest"
        );
    }

    /// When the mtime is bumped the catalog entry is stale and the file must
    /// be re-ingested (has_unchanged returns false for the new mtime).
    #[test]
    fn test_incremental_scan_reingests_on_mtime_bump() {
        let db = Database::in_memory().expect("in-memory db must open");
        let path = PathBuf::from("/media/project/B002C002.mxf");
        let size: u64 = 9_876_543;
        let mtime_old: u64 = 1_717_200_000;
        let mtime_new: u64 = mtime_old + 60; // file was modified 60 s later

        // Ingest with the original mtime
        let mut media = MediaFile::new(path.clone());
        media.size = Some(size);
        db.add_media_file(&media)
            .expect("add_media_file must succeed");
        db.update_mtime(&path, size, mtime_old)
            .expect("update_mtime must succeed");

        // Confirm the old mtime is recognised as unchanged
        assert!(
            db.has_unchanged(&path, size, mtime_old)
                .expect("has_unchanged must not error"),
            "old mtime must be recognised as unchanged"
        );

        // Bump the mtime — the catalog has the old mtime so has_unchanged must
        // return false, signalling that re-ingest is required.
        assert!(
            !db.has_unchanged(&path, size, mtime_new)
                .expect("has_unchanged must not error"),
            "bumped mtime must NOT be recognised as unchanged (re-ingest required)"
        );
    }
}
