//! Database schema migration utilities.

use crate::error::ClipResult;
use sqlx::{Row, SqlitePool};

/// Migrates the database to the latest schema version.
///
/// # Errors
///
/// Returns an error if the migration fails.
pub async fn migrate_database(pool: &SqlitePool) -> ClipResult<()> {
    // Create version table
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await?;

    // Get current version
    let current_version = get_current_version(pool).await?;

    // Apply migrations
    if current_version < 1 {
        apply_migration_v1(pool).await?;
    }

    if current_version < 2 {
        apply_migration_v2(pool).await?;
    }

    if current_version < 3 {
        apply_migration_v3(pool).await?;
    }

    Ok(())
}

async fn get_current_version(pool: &SqlitePool) -> ClipResult<i32> {
    let row = sqlx::query("SELECT MAX(version) as version FROM schema_version")
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        Ok(row.try_get("version").unwrap_or(0))
    } else {
        Ok(0)
    }
}

async fn apply_migration_v1(pool: &SqlitePool) -> ClipResult<()> {
    // Create the core clips table.  This must happen here (and not rely on
    // storage.rs) so that the migration path is self-contained when invoked
    // directly (e.g. in tests using an in-memory database).
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS clips (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            duration INTEGER,
            frame_rate_num INTEGER,
            frame_rate_den INTEGER,
            in_point INTEGER,
            out_point INTEGER,
            rating INTEGER NOT NULL DEFAULT 0,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            is_rejected INTEGER NOT NULL DEFAULT 0,
            keywords TEXT,
            created_at TEXT NOT NULL,
            modified_at TEXT NOT NULL,
            custom_metadata TEXT
        )
        ",
    )
    .execute(pool)
    .await?;

    // Record migration
    sqlx::query("INSERT INTO schema_version (version, applied_at) VALUES (1, datetime('now'))")
        .execute(pool)
        .await?;

    Ok(())
}

async fn apply_migration_v2(pool: &SqlitePool) -> ClipResult<()> {
    // Future migration placeholder

    // Create bins table
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS bins (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            modified_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await?;

    // Create clip_bins junction table
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS clip_bins (
            clip_id TEXT NOT NULL,
            bin_id TEXT NOT NULL,
            PRIMARY KEY (clip_id, bin_id),
            FOREIGN KEY (clip_id) REFERENCES clips(id) ON DELETE CASCADE,
            FOREIGN KEY (bin_id) REFERENCES bins(id) ON DELETE CASCADE
        )
        ",
    )
    .execute(pool)
    .await?;

    // Record migration
    sqlx::query("INSERT INTO schema_version (version, applied_at) VALUES (2, datetime('now'))")
        .execute(pool)
        .await?;

    Ok(())
}

/// Migration v3: Add indexes on `keywords` and `rating` columns for faster
/// filtered queries.
async fn apply_migration_v3(pool: &SqlitePool) -> ClipResult<()> {
    // Index on the `rating` column – used heavily by rating-based filters.
    sqlx::query(
        r"
        CREATE INDEX IF NOT EXISTS idx_clips_rating
            ON clips (rating)
        ",
    )
    .execute(pool)
    .await?;

    // Index on the `keywords` column – SQLite FTS5 would be ideal, but a
    // plain B-tree index on the JSON string still speeds up `LIKE '%keyword%'`
    // scans for moderate-sized libraries.
    sqlx::query(
        r"
        CREATE INDEX IF NOT EXISTS idx_clips_keywords
            ON clips (keywords)
        ",
    )
    .execute(pool)
    .await?;

    // Composite index on (is_favorite, rating) for common "show favorites
    // above a threshold" queries.
    sqlx::query(
        r"
        CREATE INDEX IF NOT EXISTS idx_clips_favorite_rating
            ON clips (is_favorite, rating)
        ",
    )
    .execute(pool)
    .await?;

    // Record migration
    sqlx::query("INSERT INTO schema_version (version, applied_at) VALUES (3, datetime('now'))")
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_migration() {
        let options = SqliteConnectOptions::from_str(":memory:")
            .expect("operation should succeed")
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("connect_with should succeed");

        migrate_database(&pool)
            .await
            .expect("operation should succeed");

        let version = get_current_version(&pool)
            .await
            .expect("get_current_version should succeed");
        assert!(version >= 1);
    }
}
