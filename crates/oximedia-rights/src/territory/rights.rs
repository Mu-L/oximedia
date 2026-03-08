//! Territory rights management for content distribution

#![allow(dead_code)]

/// Represents a geographic territory with ISO code
#[derive(Debug, Clone, PartialEq)]
pub struct Territory {
    /// ISO 3166-1 alpha-2 territory code (e.g. "US", "GB")
    pub code: String,
    /// Human-readable name (e.g. "United States")
    pub name: String,
    /// Geographic / political region (e.g. "North America")
    pub region: String,
}

impl Territory {
    /// Create a new territory
    pub fn new(code: &str, name: &str, region: &str) -> Self {
        Self {
            code: code.to_string(),
            name: name.to_string(),
            region: region.to_string(),
        }
    }

    /// Return a full list of commonly used worldwide territories
    pub fn worldwide() -> Vec<Self> {
        let mut all = Self::north_america();
        all.extend(Self::europe());
        all.extend([
            Territory::new("JP", "Japan", "Asia"),
            Territory::new("CN", "China", "Asia"),
            Territory::new("IN", "India", "Asia"),
            Territory::new("KR", "South Korea", "Asia"),
            Territory::new("AU", "Australia", "Oceania"),
            Territory::new("NZ", "New Zealand", "Oceania"),
            Territory::new("BR", "Brazil", "South America"),
            Territory::new("AR", "Argentina", "South America"),
        ]);
        all
    }

    /// Return commonly used European territories
    pub fn europe() -> Vec<Self> {
        vec![
            Territory::new("GB", "United Kingdom", "Europe"),
            Territory::new("DE", "Germany", "Europe"),
            Territory::new("FR", "France", "Europe"),
            Territory::new("IT", "Italy", "Europe"),
            Territory::new("ES", "Spain", "Europe"),
            Territory::new("NL", "Netherlands", "Europe"),
            Territory::new("SE", "Sweden", "Europe"),
            Territory::new("NO", "Norway", "Europe"),
            Territory::new("DK", "Denmark", "Europe"),
            Territory::new("FI", "Finland", "Europe"),
            Territory::new("PL", "Poland", "Europe"),
            Territory::new("PT", "Portugal", "Europe"),
            Territory::new("CH", "Switzerland", "Europe"),
            Territory::new("AT", "Austria", "Europe"),
            Territory::new("BE", "Belgium", "Europe"),
        ]
    }

    /// Return commonly used North American territories
    pub fn north_america() -> Vec<Self> {
        vec![
            Territory::new("US", "United States", "North America"),
            Territory::new("CA", "Canada", "North America"),
            Territory::new("MX", "Mexico", "North America"),
        ]
    }
}

/// Rights configuration for a piece of content in specific territories
#[derive(Debug, Clone)]
pub struct TerritoryRights {
    /// Content identifier this rights record belongs to
    pub content_id: String,
    /// Territory codes explicitly allowed
    pub allowed: Vec<String>,
    /// Territory codes explicitly blocked (overrides allowed)
    pub blocked: Vec<String>,
    /// Optional Unix timestamp after which rights expire
    pub expires_at: Option<u64>,
}

impl TerritoryRights {
    /// Create a new territory rights record for the given content (deny-all by default)
    pub fn new(content_id: &str) -> Self {
        Self {
            content_id: content_id.to_string(),
            allowed: Vec::new(),
            blocked: Vec::new(),
            expires_at: None,
        }
    }

    /// Explicitly allow a territory by code
    pub fn allow(&mut self, territory_code: &str) {
        let code = territory_code.to_uppercase();
        if !self.allowed.contains(&code) {
            self.allowed.push(code);
        }
    }

    /// Explicitly block a territory by code (takes precedence over allowed)
    pub fn block(&mut self, territory_code: &str) {
        let code = territory_code.to_uppercase();
        if !self.blocked.contains(&code) {
            self.blocked.push(code);
        }
    }

    /// Allow all territories in the worldwide list
    pub fn allow_worldwide(&mut self) {
        for territory in Territory::worldwide() {
            self.allow(&territory.code);
        }
    }

    /// Check whether a territory is allowed (blocked list takes precedence)
    pub fn is_allowed(&self, territory_code: &str) -> bool {
        let code = territory_code.to_uppercase();
        if self.blocked.contains(&code) {
            return false;
        }
        self.allowed.contains(&code)
    }

    /// Check whether the rights have expired relative to the given Unix timestamp
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at.is_some_and(|exp| now >= exp)
    }

    /// Set an expiry timestamp
    pub fn set_expires_at(&mut self, timestamp: u64) {
        self.expires_at = Some(timestamp);
    }

    /// Number of explicitly allowed territories
    pub fn allowed_count(&self) -> usize {
        self.allowed.len()
    }

    /// Number of explicitly blocked territories
    pub fn blocked_count(&self) -> usize {
        self.blocked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_territory_new_stores_fields() {
        let t = Territory::new("US", "United States", "North America");
        assert_eq!(t.code, "US");
        assert_eq!(t.name, "United States");
        assert_eq!(t.region, "North America");
    }

    #[test]
    fn test_north_america_returns_three_territories() {
        let na = Territory::north_america();
        assert_eq!(na.len(), 3);
        let codes: Vec<&str> = na.iter().map(|t| t.code.as_str()).collect();
        assert!(codes.contains(&"US"));
        assert!(codes.contains(&"CA"));
        assert!(codes.contains(&"MX"));
    }

    #[test]
    fn test_europe_returns_territories() {
        let eu = Territory::europe();
        assert!(!eu.is_empty());
        let codes: Vec<&str> = eu.iter().map(|t| t.code.as_str()).collect();
        assert!(codes.contains(&"GB"));
        assert!(codes.contains(&"DE"));
        assert!(codes.contains(&"FR"));
    }

    #[test]
    fn test_worldwide_includes_both_regions() {
        let ww = Territory::worldwide();
        let codes: Vec<&str> = ww.iter().map(|t| t.code.as_str()).collect();
        assert!(codes.contains(&"US"));
        assert!(codes.contains(&"GB"));
        assert!(codes.contains(&"JP"));
        assert!(codes.contains(&"AU"));
    }

    #[test]
    fn test_territory_rights_new_deny_all() {
        let rights = TerritoryRights::new("content-1");
        assert!(!rights.is_allowed("US"));
        assert!(!rights.is_allowed("GB"));
        assert_eq!(rights.allowed_count(), 0);
        assert_eq!(rights.blocked_count(), 0);
    }

    #[test]
    fn test_allow_single_territory() {
        let mut rights = TerritoryRights::new("content-2");
        rights.allow("US");
        assert!(rights.is_allowed("US"));
        assert!(!rights.is_allowed("GB"));
    }

    #[test]
    fn test_allow_is_case_insensitive() {
        let mut rights = TerritoryRights::new("content-3");
        rights.allow("us");
        assert!(rights.is_allowed("US"));
        assert!(rights.is_allowed("us"));
    }

    #[test]
    fn test_block_overrides_allow() {
        let mut rights = TerritoryRights::new("content-4");
        rights.allow("US");
        rights.block("US");
        assert!(!rights.is_allowed("US"));
    }

    #[test]
    fn test_allow_worldwide_grants_access() {
        let mut rights = TerritoryRights::new("content-5");
        rights.allow_worldwide();
        assert!(rights.is_allowed("US"));
        assert!(rights.is_allowed("GB"));
        assert!(rights.is_allowed("JP"));
        assert!(!rights.is_allowed("XX")); // Not in worldwide list
    }

    #[test]
    fn test_not_expired_when_no_expiry_set() {
        let rights = TerritoryRights::new("content-6");
        assert!(!rights.is_expired(9_999_999_999));
    }

    #[test]
    fn test_is_expired_after_timestamp() {
        let mut rights = TerritoryRights::new("content-7");
        rights.set_expires_at(1_000_000);
        assert!(rights.is_expired(1_000_001));
        assert!(rights.is_expired(1_000_000));
        assert!(!rights.is_expired(999_999));
    }

    #[test]
    fn test_allow_does_not_duplicate() {
        let mut rights = TerritoryRights::new("content-8");
        rights.allow("US");
        rights.allow("US");
        assert_eq!(rights.allowed_count(), 1);
    }

    #[test]
    fn test_block_does_not_duplicate() {
        let mut rights = TerritoryRights::new("content-9");
        rights.block("CN");
        rights.block("CN");
        assert_eq!(rights.blocked_count(), 1);
    }
}
