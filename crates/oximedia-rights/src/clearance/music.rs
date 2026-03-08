//! Music clearance tracking

use crate::{
    clearance::{ClearanceStatus, ClearanceType},
    database::RightsDatabase,
    Result,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Music clearance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicClearance {
    /// Unique identifier
    pub id: String,
    /// Asset ID
    pub asset_id: String,
    /// Track name
    pub track_name: String,
    /// Artist/composer
    pub artist: String,
    /// Publisher
    pub publisher: Option<String>,
    /// Status
    pub status: ClearanceStatus,
    /// Requested date
    pub requested_date: DateTime<Utc>,
    /// Approved date
    pub approved_date: Option<DateTime<Utc>>,
    /// Expiry date
    pub expiry_date: Option<DateTime<Utc>>,
    /// Notes
    pub notes: Option<String>,
}

impl MusicClearance {
    /// Create a new music clearance
    pub fn new(
        asset_id: impl Into<String>,
        track_name: impl Into<String>,
        artist: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            asset_id: asset_id.into(),
            track_name: track_name.into(),
            artist: artist.into(),
            publisher: None,
            status: ClearanceStatus::Requested,
            requested_date: Utc::now(),
            approved_date: None,
            expiry_date: None,
            notes: None,
        }
    }

    /// Approve clearance
    pub fn approve(&mut self) {
        self.status = ClearanceStatus::Cleared;
        self.approved_date = Some(Utc::now());
    }

    /// Save to database
    pub async fn save(&self, db: &RightsDatabase) -> Result<()> {
        let metadata = serde_json::json!({
            "track_name": self.track_name,
            "artist": self.artist,
            "publisher": self.publisher,
        });

        sqlx::query(
            r"
            INSERT INTO clearances
            (id, asset_id, clearance_type, status, requester, approver, requested_date,
             approved_date, expiry_date, notes, metadata_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                approved_date = excluded.approved_date,
                notes = excluded.notes,
                updated_at = excluded.updated_at
            ",
        )
        .bind(&self.id)
        .bind(&self.asset_id)
        .bind(ClearanceType::Music.as_str())
        .bind(self.status.as_str())
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind(self.requested_date.to_rfc3339())
        .bind(self.approved_date.map(|d| d.to_rfc3339()))
        .bind(self.expiry_date.map(|d| d.to_rfc3339()))
        .bind(&self.notes)
        .bind(metadata.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(db.pool())
        .await?;

        Ok(())
    }
}
