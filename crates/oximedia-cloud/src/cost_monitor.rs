//! Cloud cost monitoring
//!
//! Provides in-memory cost tracking and budget alerting for cloud workloads:
//! - Categorised cost entries (Storage, Egress, Compute, API Calls, Transcoding)
//! - Monthly budget limits with configurable alert thresholds
//! - Aggregation by category and identification of top cost drivers

#![allow(dead_code)]

/// High-level category of a cloud cost entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CostCategory {
    /// Object storage charges (capacity × time)
    Storage,
    /// Data-transfer / egress charges
    Egress,
    /// Compute / instance charges
    Compute,
    /// Per-request API call charges
    ApiCalls,
    /// Media transcoding job charges
    Transcoding,
}

impl CostCategory {
    /// Returns `true` if this category's cost is variable (usage-dependent).
    ///
    /// `Compute` is treated as a fixed reservation cost in this model.
    #[must_use]
    pub fn is_variable(&self) -> bool {
        matches!(
            self,
            CostCategory::Storage
                | CostCategory::Egress
                | CostCategory::ApiCalls
                | CostCategory::Transcoding
        )
    }
}

/// A single recorded cost event.
#[derive(Debug, Clone)]
pub struct CostEntry {
    /// The cost category
    pub category: CostCategory,
    /// Unix epoch timestamp (seconds) when the cost was incurred
    pub timestamp_epoch: u64,
    /// Amount in USD
    pub amount_usd: f64,
    /// Human-readable description of the charge
    pub description: String,
}

impl CostEntry {
    /// Construct a new cost entry with an auto-generated empty description.
    #[must_use]
    pub fn new(cat: CostCategory, epoch: u64, amount: f64) -> Self {
        Self {
            category: cat,
            timestamp_epoch: epoch,
            amount_usd: amount,
            description: String::new(),
        }
    }
}

/// Monthly budget configuration.
#[derive(Debug, Clone)]
pub struct CostBudget {
    /// Maximum allowed spend per month in USD
    pub monthly_limit_usd: f64,
    /// Fraction of the budget (0.0–1.0) at which an alert is triggered
    pub alert_threshold: f64,
}

impl CostBudget {
    /// Returns `true` if `spent` has exceeded the monthly limit.
    #[must_use]
    pub fn is_over_budget(&self, spent: f64) -> bool {
        spent > self.monthly_limit_usd
    }

    /// Returns `true` if `spent` has reached or exceeded the alert threshold.
    #[must_use]
    pub fn should_alert(&self, spent: f64) -> bool {
        spent >= self.monthly_limit_usd * self.alert_threshold
    }
}

/// In-memory cloud cost monitor.
#[derive(Debug)]
pub struct CostMonitor {
    /// All recorded cost entries
    pub entries: Vec<CostEntry>,
    /// Budget configuration
    pub budget: CostBudget,
}

impl CostMonitor {
    /// Create a new cost monitor with the given budget.
    #[must_use]
    pub fn new(budget: CostBudget) -> Self {
        Self {
            entries: Vec::new(),
            budget,
        }
    }

    /// Record a new cost entry.
    pub fn record(&mut self, entry: CostEntry) {
        self.entries.push(entry);
    }

    /// Returns the sum of all recorded costs in USD.
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.entries.iter().map(|e| e.amount_usd).sum()
    }

    /// Returns the total cost for the given category.
    #[must_use]
    pub fn cost_by_category(&self, cat: &CostCategory) -> f64 {
        self.entries
            .iter()
            .filter(|e| &e.category == cat)
            .map(|e| e.amount_usd)
            .sum()
    }

    /// Returns `true` if the total cost is within the monthly budget.
    #[must_use]
    pub fn is_within_budget(&self) -> bool {
        !self.budget.is_over_budget(self.total_cost())
    }

    /// Returns the top `n` cost categories sorted by descending total cost.
    ///
    /// Each element is a reference to the category and its summed cost.
    #[must_use]
    pub fn top_cost_categories(&self, n: usize) -> Vec<(&CostCategory, f64)> {
        // Collect unique categories
        let mut categories: Vec<&CostCategory> = Vec::new();
        for entry in &self.entries {
            if !categories.contains(&&entry.category) {
                categories.push(&entry.category);
            }
        }

        let mut sums: Vec<(&CostCategory, f64)> = categories
            .into_iter()
            .map(|cat| (cat, self.cost_by_category(cat)))
            .collect();

        sums.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sums.truncate(n);
        sums
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_budget() -> CostBudget {
        CostBudget {
            monthly_limit_usd: 1000.0,
            alert_threshold: 0.8,
        }
    }

    fn make_monitor() -> CostMonitor {
        CostMonitor::new(default_budget())
    }

    #[test]
    fn test_cost_category_variable_storage() {
        assert!(CostCategory::Storage.is_variable());
    }

    #[test]
    fn test_cost_category_variable_egress() {
        assert!(CostCategory::Egress.is_variable());
    }

    #[test]
    fn test_cost_category_variable_transcoding() {
        assert!(CostCategory::Transcoding.is_variable());
    }

    #[test]
    fn test_cost_category_not_variable_compute() {
        assert!(!CostCategory::Compute.is_variable());
    }

    #[test]
    fn test_cost_entry_new_empty_description() {
        let entry = CostEntry::new(CostCategory::Storage, 1000, 9.99);
        assert!(entry.description.is_empty());
        assert_eq!(entry.amount_usd, 9.99);
    }

    #[test]
    fn test_budget_over_budget_false() {
        let budget = default_budget();
        assert!(!budget.is_over_budget(999.99));
    }

    #[test]
    fn test_budget_over_budget_true() {
        let budget = default_budget();
        assert!(budget.is_over_budget(1001.0));
    }

    #[test]
    fn test_budget_should_alert_true() {
        let budget = default_budget();
        // 80% of 1000 = 800
        assert!(budget.should_alert(800.0));
    }

    #[test]
    fn test_budget_should_alert_false() {
        let budget = default_budget();
        assert!(!budget.should_alert(799.99));
    }

    #[test]
    fn test_total_cost_empty() {
        let monitor = make_monitor();
        assert_eq!(monitor.total_cost(), 0.0);
    }

    #[test]
    fn test_total_cost_sum() {
        let mut monitor = make_monitor();
        monitor.record(CostEntry::new(CostCategory::Storage, 1, 50.0));
        monitor.record(CostEntry::new(CostCategory::Egress, 2, 30.0));
        assert_eq!(monitor.total_cost(), 80.0);
    }

    #[test]
    fn test_cost_by_category() {
        let mut monitor = make_monitor();
        monitor.record(CostEntry::new(CostCategory::Storage, 1, 10.0));
        monitor.record(CostEntry::new(CostCategory::Storage, 2, 20.0));
        monitor.record(CostEntry::new(CostCategory::Egress, 3, 5.0));
        assert_eq!(monitor.cost_by_category(&CostCategory::Storage), 30.0);
        assert_eq!(monitor.cost_by_category(&CostCategory::Egress), 5.0);
    }

    #[test]
    fn test_is_within_budget_true() {
        let monitor = make_monitor();
        assert!(monitor.is_within_budget());
    }

    #[test]
    fn test_is_within_budget_false() {
        let mut monitor = make_monitor();
        monitor.record(CostEntry::new(CostCategory::Transcoding, 1, 1500.0));
        assert!(!monitor.is_within_budget());
    }

    #[test]
    fn test_top_cost_categories() {
        let mut monitor = make_monitor();
        monitor.record(CostEntry::new(CostCategory::Storage, 1, 100.0));
        monitor.record(CostEntry::new(CostCategory::Egress, 2, 200.0));
        monitor.record(CostEntry::new(CostCategory::Compute, 3, 50.0));
        let top = monitor.top_cost_categories(2);
        assert_eq!(top.len(), 2);
        assert_eq!(*top[0].0, CostCategory::Egress);
        assert_eq!(*top[1].0, CostCategory::Storage);
    }

    #[test]
    fn test_top_cost_categories_n_larger_than_categories() {
        let mut monitor = make_monitor();
        monitor.record(CostEntry::new(CostCategory::Storage, 1, 50.0));
        let top = monitor.top_cost_categories(10);
        assert_eq!(top.len(), 1);
    }
}
