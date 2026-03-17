//! Visible watermark application

#[cfg(not(target_arch = "wasm32"))]
use crate::database::RightsDatabase;
use crate::{watermark::WatermarkConfig, Result};
use uuid::Uuid;

/// Visible watermark applicator
pub struct VisibleWatermark {
    config: WatermarkConfig,
}

impl VisibleWatermark {
    /// Create a new visible watermark applicator
    pub fn new(config: WatermarkConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &WatermarkConfig {
        &self.config
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Save watermark configuration to database
    pub async fn save_config(&self, db: &RightsDatabase, asset_id: &str) -> Result<()> {
        let config_json = serde_json::to_string(&self.config)
            .map_err(|e| crate::RightsError::Serialization(e.to_string()))?;

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            r"
            INSERT INTO watermark_configs
            (id, asset_id, watermark_type, config_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(&id)
        .bind(asset_id)
        .bind("visible")
        .bind(&config_json)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(db.pool())
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_watermark() {
        let config = WatermarkConfig::visible_text("Test");
        let watermark = VisibleWatermark::new(config);
        assert!(watermark.config().text.is_some());
    }
}
