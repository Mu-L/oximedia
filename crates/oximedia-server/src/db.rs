//! Database layer using SQLx.

use crate::error::ServerResult;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
    ConnectOptions, Row,
};
use std::str::FromStr;
use tracing::log::LevelFilter;

/// Database connection pool.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Creates a new database connection pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn new(url: &str) -> ServerResult<Self> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .log_statements(LevelFilter::Debug);

        let pool = SqlitePoolOptions::new()
            .max_connections(32)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// Returns a reference to the connection pool.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Runs database migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if migrations fail.
    pub async fn migrate(&self) -> ServerResult<()> {
        // Users table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_login INTEGER
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // API keys table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                key_hash TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER,
                last_used INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Media table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS media (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                duration REAL,
                width INTEGER,
                height INTEGER,
                codec_video TEXT,
                codec_audio TEXT,
                bitrate INTEGER,
                framerate REAL,
                sample_rate INTEGER,
                channels INTEGER,
                thumbnail_path TEXT,
                sprite_path TEXT,
                preview_path TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Media metadata table (flexible key-value store)
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS media_metadata (
                media_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY (media_id, key),
                FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Transcoding jobs table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS transcode_jobs (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                media_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'queued',
                progress REAL NOT NULL DEFAULT 0.0,
                output_format TEXT NOT NULL,
                output_codec_video TEXT,
                output_codec_audio TEXT,
                output_width INTEGER,
                output_height INTEGER,
                output_bitrate INTEGER,
                output_path TEXT,
                error_message TEXT,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Collections table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                thumbnail_path TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Collection items table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS collection_items (
                collection_id TEXT NOT NULL,
                media_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                added_at INTEGER NOT NULL,
                PRIMARY KEY (collection_id, media_id),
                FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Multipart uploads table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS multipart_uploads (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                uploaded_size INTEGER NOT NULL DEFAULT 0,
                chunk_size INTEGER NOT NULL,
                total_chunks INTEGER NOT NULL,
                completed_chunks INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'uploading',
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Upload chunks table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS upload_chunks (
                upload_id TEXT NOT NULL,
                chunk_number INTEGER NOT NULL,
                chunk_path TEXT NOT NULL,
                chunk_size INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL,
                PRIMARY KEY (upload_id, chunk_number),
                FOREIGN KEY (upload_id) REFERENCES multipart_uploads(id) ON DELETE CASCADE
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_user_id ON media(user_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_status ON media(status)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_created_at ON media(created_at)")
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_transcode_jobs_user_id ON transcode_jobs(user_id)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_transcode_jobs_media_id ON transcode_jobs(media_id)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_transcode_jobs_status ON transcode_jobs(status)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id)")
            .execute(&self.pool)
            .await?;

        // Create full-text search table
        sqlx::query(
            r"
            CREATE VIRTUAL TABLE IF NOT EXISTS media_fts USING fts5(
                media_id,
                filename,
                original_filename,
                metadata,
                content='media',
                content_rowid='rowid'
            )
            ",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Checks if the database is healthy.
    ///
    /// # Errors
    ///
    /// Returns an error if the health check fails.
    pub async fn health_check(&self) -> ServerResult<bool> {
        let result: i64 = sqlx::query_scalar("SELECT 1").fetch_one(&self.pool).await?;
        Ok(result == 1)
    }

    /// Gets storage statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn get_storage_stats(&self) -> ServerResult<StorageStats> {
        let row = sqlx::query(
            r"
            SELECT
                COUNT(*) as total_files,
                SUM(file_size) as total_size,
                AVG(file_size) as avg_size,
                MAX(file_size) as max_size,
                MIN(file_size) as min_size
            FROM media
            WHERE status = 'ready'
            ",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(StorageStats {
            total_files: row.get::<i64, _>("total_files") as u64,
            total_size: row.get::<Option<i64>, _>("total_size").unwrap_or(0) as u64,
            avg_size: row.get::<Option<f64>, _>("avg_size").unwrap_or(0.0),
            max_size: row.get::<Option<i64>, _>("max_size").unwrap_or(0) as u64,
            min_size: row.get::<Option<i64>, _>("min_size").unwrap_or(0) as u64,
        })
    }
}

/// Storage statistics.
#[derive(Debug, Clone)]
pub struct StorageStats {
    /// Total number of files
    pub total_files: u64,
    /// Total storage size in bytes
    pub total_size: u64,
    /// Average file size in bytes
    pub avg_size: f64,
    /// Largest file size in bytes
    pub max_size: u64,
    /// Smallest file size in bytes
    pub min_size: u64,
}
