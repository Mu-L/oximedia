//! Royalty calculation

use crate::{database::RightsDatabase, usage::UsageLog, Result};
use chrono::{DateTime, Utc};

/// Royalty calculation method
#[derive(Debug, Clone)]
pub enum RoyaltyMethod {
    /// Fixed amount per use
    PerUse(f64),
    /// Percentage of revenue
    Percentage(f64),
    /// Flat fee
    FlatFee(f64),
}

/// Royalty calculator
pub struct RoyaltyCalculator {
    method: RoyaltyMethod,
}

impl RoyaltyCalculator {
    /// Create a new royalty calculator
    pub fn new(method: RoyaltyMethod) -> Self {
        Self { method }
    }

    /// Calculate royalties for a period
    pub async fn calculate(
        &self,
        _db: &RightsDatabase,
        _grant_id: &str,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<f64> {
        match &self.method {
            RoyaltyMethod::PerUse(amount) => Ok(*amount),
            RoyaltyMethod::Percentage(pct) => Ok(*pct),
            RoyaltyMethod::FlatFee(fee) => Ok(*fee),
        }
    }

    /// Calculate from usage logs
    pub fn calculate_from_usage(
        &self,
        usage_logs: &[UsageLog],
        revenue_per_use: Option<f64>,
    ) -> f64 {
        match &self.method {
            RoyaltyMethod::PerUse(amount) => usage_logs.len() as f64 * amount,
            RoyaltyMethod::Percentage(pct) => {
                if let Some(revenue) = revenue_per_use {
                    usage_logs.len() as f64 * revenue * (pct / 100.0)
                } else {
                    0.0
                }
            }
            RoyaltyMethod::FlatFee(fee) => *fee,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_use_calculation() {
        let calculator = RoyaltyCalculator::new(RoyaltyMethod::PerUse(10.0));
        let usage_logs = vec![
            UsageLog::new("asset1", crate::rights::UsageType::Commercial, Utc::now()),
            UsageLog::new("asset1", crate::rights::UsageType::Web, Utc::now()),
        ];

        let total = calculator.calculate_from_usage(&usage_logs, None);
        assert_eq!(total, 20.0);
    }

    #[test]
    fn test_percentage_calculation() {
        let calculator = RoyaltyCalculator::new(RoyaltyMethod::Percentage(10.0));
        let usage_logs = vec![UsageLog::new(
            "asset1",
            crate::rights::UsageType::Commercial,
            Utc::now(),
        )];

        let total = calculator.calculate_from_usage(&usage_logs, Some(100.0));
        assert_eq!(total, 10.0);
    }
}
