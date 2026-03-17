//! Geographic routing — assign requests to the closest CDN edge node using
//! Haversine distance and estimate propagation latency.
//!
//! # Overview
//!
//! [`GeoRouter`] holds a registry of [`EdgeNodeGeo`] entries (each combining an
//! [`EdgeNodeId`] with a [`GeoLocation`]) and can:
//!
//! - Return the **closest** edge node for a client location
//!   ([`GeoRouter::assign_edge`]).
//! - Estimate the **latency** in milliseconds between any two locations
//!   ([`GeoRouter::latency_estimate_ms`]).
//! - List all nodes within a given distance ([`GeoRouter::nodes_within_km`]).
//!
//! # Latency model
//!
//! `latency_ms = distance_km / 200.0 * 1000.0 + 5.0`
//!
//! This approximates the speed of light in optical fibre (≈ 200 km/ms) plus a
//! fixed 5 ms base overhead.

use std::fmt;

// ─── Region ───────────────────────────────────────────────────────────────────

/// Broad geographic region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// North America (US, CA, MX, …).
    NorthAmerica,
    /// Europe (EU, UK, CH, NO, …).
    Europe,
    /// Asia-Pacific (JP, AU, SG, CN, IN, …).
    AsiaPacific,
    /// Latin America (BR, AR, CO, CL, …).
    LatinAmerica,
    /// Middle East and Africa (AE, SA, ZA, EG, …).
    MiddleEastAfrica,
    /// Could not be determined.
    Unknown,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::NorthAmerica => "north_america",
            Self::Europe => "europe",
            Self::AsiaPacific => "asia_pacific",
            Self::LatinAmerica => "latin_america",
            Self::MiddleEastAfrica => "middle_east_africa",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

impl Region {
    /// Derive a region from a two-letter ISO 3166-1 alpha-2 country code.
    ///
    /// Unknown or unrecognised codes map to [`Region::Unknown`].
    pub fn from_country_code(cc: &str) -> Self {
        match cc.to_uppercase().as_str() {
            // North America
            "US" | "CA" | "MX" | "GT" | "BZ" | "HN" | "SV" | "NI" | "CR" | "PA" | "CU" | "JM"
            | "HT" | "DO" | "PR" | "TT" | "BB" | "LC" | "VC" | "GD" | "AG" | "KN" | "DM" | "AI"
            | "KY" | "TC" | "VG" | "VI" | "AW" | "CW" | "SX" | "BQ" | "BS" | "TF" | "GU" | "MP"
            | "AS" | "UM" => Self::NorthAmerica,

            // Latin America (South America)
            "BR" | "AR" | "CL" | "CO" | "PE" | "VE" | "EC" | "BO" | "PY" | "UY" | "SR" | "GY"
            | "GF" | "FK" | "GS" => Self::LatinAmerica,

            // Europe
            "GB" | "DE" | "FR" | "IT" | "ES" | "PT" | "NL" | "BE" | "LU" | "CH" | "AT" | "SE"
            | "NO" | "DK" | "FI" | "IS" | "IE" | "PL" | "CZ" | "SK" | "HU" | "RO" | "BG" | "HR"
            | "SI" | "BA" | "RS" | "ME" | "MK" | "AL" | "GR" | "CY" | "MT" | "EE" | "LV" | "LT"
            | "BY" | "UA" | "MD" | "RU" | "TR" | "GE" | "AM" | "AZ" | "LI" | "MC" | "SM" | "VA"
            | "AD" | "FO" | "GI" | "JE" | "GG" | "IM" | "AX" | "SJ" | "PM" | "GL" | "XK" => {
                Self::Europe
            }

            // Middle East & Africa
            "AE" | "SA" | "QA" | "BH" | "KW" | "OM" | "YE" | "IQ" | "IR" | "SY" | "LB" | "JO"
            | "IL" | "PS" | "EG" | "LY" | "TN" | "DZ" | "MA" | "MR" | "ML" | "NE" | "TD" | "SD"
            | "SS" | "ER" | "ET" | "DJ" | "SO" | "KE" | "UG" | "RW" | "BI" | "TZ" | "MZ" | "ZM"
            | "MW" | "ZW" | "BW" | "NA" | "ZA" | "LS" | "SZ" | "AO" | "ZR" | "CD" | "CG" | "CM"
            | "CF" | "GQ" | "GA" | "ST" | "CV" | "GW" | "GN" | "SL" | "LR" | "CI" | "GH" | "TG"
            | "BJ" | "NG" | "SN" | "GM" | "BF" | "MG" | "KM" | "MU" | "SC" | "RE" | "YT" | "SH"
            | "EH" | "AF" | "PK" => Self::MiddleEastAfrica,

            // Asia-Pacific
            "JP" | "CN" | "KR" | "TW" | "HK" | "MO" | "MN" | "KP" | "VN" | "TH" | "MY" | "SG"
            | "ID" | "PH" | "MM" | "KH" | "LA" | "BN" | "TL" | "IN" | "BD" | "LK" | "NP" | "BT"
            | "MV" | "AU" | "NZ" | "FJ" | "PG" | "SB" | "VU" | "WS" | "TO" | "TV" | "NR" | "PW"
            | "FM" | "MH" | "KI" | "CK" | "NU" | "TK" | "WF" | "PF" | "NC" | "KZ" | "UZ" | "TM"
            | "TJ" | "KG" => Self::AsiaPacific,

            _ => Self::Unknown,
        }
    }
}

// ─── GeoLocation ─────────────────────────────────────────────────────────────

/// A geographic position enriched with region and country metadata.
#[derive(Debug, Clone)]
pub struct GeoLocation {
    /// Latitude in decimal degrees (−90 … +90).
    pub latitude: f64,
    /// Longitude in decimal degrees (−180 … +180).
    pub longitude: f64,
    /// Broad geographic region.
    pub region: Region,
    /// ISO 3166-1 alpha-2 country code (e.g. `"US"`, `"DE"`).
    pub country_code: String,
}

impl GeoLocation {
    /// Create a new location from coordinates and a country code.
    ///
    /// The [`Region`] is inferred automatically via
    /// [`Region::from_country_code`].
    pub fn new(latitude: f64, longitude: f64, country_code: &str) -> Self {
        Self {
            latitude,
            longitude,
            region: Region::from_country_code(country_code),
            country_code: country_code.to_uppercase(),
        }
    }

    /// Create a location with an explicit region (overriding the inferred one).
    pub fn with_region(latitude: f64, longitude: f64, country_code: &str, region: Region) -> Self {
        Self {
            latitude,
            longitude,
            region,
            country_code: country_code.to_uppercase(),
        }
    }
}

// ─── Haversine distance ───────────────────────────────────────────────────────

/// Compute the great-circle distance between two points on Earth in kilometres.
///
/// Uses the Haversine formula with Earth radius R = 6 371.0 km.
///
/// # Arguments
/// All angles are in **decimal degrees**.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R_KM: f64 = 6_371.0;

    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let lat1_r = lat1.to_radians();
    let lat2_r = lat2.to_radians();

    let a = (dlat / 2.0).sin().powi(2) + lat1_r.cos() * lat2_r.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    R_KM * c
}

/// Estimate propagation latency in milliseconds given a distance in kilometres.
///
/// Model: `latency_ms = distance_km / 200.0 * 1000.0 + 5.0`
///
/// - 200 km/ms ≈ speed of light in optical fibre.
/// - +5 ms base overhead (switching, queuing).
pub fn latency_from_km(distance_km: f64) -> f64 {
    distance_km / 200.0 * 1000.0 + 5.0
}

// ─── EdgeNodeId ───────────────────────────────────────────────────────────────

/// Opaque identifier for a CDN edge node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeNodeId(pub String);

impl EdgeNodeId {
    /// Create a new ID from any `Into<String>`.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for EdgeNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ─── EdgeNodeGeo ─────────────────────────────────────────────────────────────

/// Association between an edge node and its physical location.
#[derive(Debug, Clone)]
pub struct EdgeNodeGeo {
    /// The edge node identifier.
    pub id: EdgeNodeId,
    /// Geographic location of the node's PoP.
    pub location: GeoLocation,
    /// Whether this edge node is currently accepting traffic.
    pub active: bool,
}

impl EdgeNodeGeo {
    /// Create a new active edge node entry.
    pub fn new(id: impl Into<String>, location: GeoLocation) -> Self {
        Self {
            id: EdgeNodeId::new(id),
            location,
            active: true,
        }
    }
}

// ─── GeoRouter ───────────────────────────────────────────────────────────────

/// Routes client requests to the geographically closest (and active) edge node.
pub struct GeoRouter {
    nodes: Vec<EdgeNodeGeo>,
}

impl GeoRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Register an edge node.
    pub fn add_node(&mut self, node: EdgeNodeGeo) {
        self.nodes.push(node);
    }

    /// Remove a node by ID.  Returns `true` if a node was removed.
    pub fn remove_node(&mut self, id: &EdgeNodeId) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|n| &n.id != id);
        self.nodes.len() < before
    }

    /// Assign the closest **active** edge node to `location`.
    ///
    /// Returns `None` if no active nodes are registered.
    pub fn assign_edge(&self, location: &GeoLocation) -> Option<&EdgeNodeId> {
        self.nodes
            .iter()
            .filter(|n| n.active)
            .min_by(|a, b| {
                let da = haversine_km(
                    location.latitude,
                    location.longitude,
                    a.location.latitude,
                    a.location.longitude,
                );
                let db = haversine_km(
                    location.latitude,
                    location.longitude,
                    b.location.latitude,
                    b.location.longitude,
                );
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| &n.id)
    }

    /// Estimate the one-way propagation latency in milliseconds between
    /// `from` and `to`.
    ///
    /// Formula: `distance_km / 200.0 * 1000.0 + 5.0`
    pub fn latency_estimate_ms(&self, from: &GeoLocation, to: &GeoLocation) -> f64 {
        let dist = haversine_km(from.latitude, from.longitude, to.latitude, to.longitude);
        latency_from_km(dist)
    }

    /// Return references to all active nodes within `radius_km` of `location`.
    pub fn nodes_within_km(&self, location: &GeoLocation, radius_km: f64) -> Vec<&EdgeNodeGeo> {
        self.nodes
            .iter()
            .filter(|n| {
                n.active
                    && haversine_km(
                        location.latitude,
                        location.longitude,
                        n.location.latitude,
                        n.location.longitude,
                    ) <= radius_km
            })
            .collect()
    }

    /// Return the distance in km from `location` to the given node.
    ///
    /// Returns `None` if no node with that ID is registered.
    pub fn distance_to_node(&self, location: &GeoLocation, node_id: &EdgeNodeId) -> Option<f64> {
        self.nodes.iter().find(|n| &n.id == node_id).map(|n| {
            haversine_km(
                location.latitude,
                location.longitude,
                n.location.latitude,
                n.location.longitude,
            )
        })
    }

    /// All registered nodes (including inactive ones).
    pub fn nodes(&self) -> &[EdgeNodeGeo] {
        &self.nodes
    }

    /// Active node count.
    pub fn active_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.active).count()
    }

    /// Return the node with the lowest latency estimate from `location`,
    /// together with the latency value.
    pub fn best_with_latency(&self, location: &GeoLocation) -> Option<(&EdgeNodeId, f64)> {
        self.nodes
            .iter()
            .filter(|n| n.active)
            .map(|n| {
                let dist = haversine_km(
                    location.latitude,
                    location.longitude,
                    n.location.latitude,
                    n.location.longitude,
                );
                (&n.id, latency_from_km(dist))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }
}

impl Default for GeoRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers
    fn new_york() -> GeoLocation {
        GeoLocation::new(40.7128, -74.0060, "US")
    }

    fn london() -> GeoLocation {
        GeoLocation::new(51.5074, -0.1278, "GB")
    }

    fn tokyo() -> GeoLocation {
        GeoLocation::new(35.6762, 139.6503, "JP")
    }

    fn sydney() -> GeoLocation {
        GeoLocation::new(-33.8688, 151.2093, "AU")
    }

    fn sao_paulo() -> GeoLocation {
        GeoLocation::new(-23.5505, -46.6333, "BR")
    }

    // 1. Haversine: same point → 0
    #[test]
    fn test_haversine_same_point() {
        let d = haversine_km(40.7128, -74.0060, 40.7128, -74.0060);
        assert!(d < 1e-6, "d={d}");
    }

    // 2. Haversine: New York ↔ London ≈ 5 570 km
    #[test]
    fn test_haversine_ny_london() {
        let d = haversine_km(40.7128, -74.0060, 51.5074, -0.1278);
        assert!(d > 5_000.0 && d < 6_000.0, "d={d}");
    }

    // 3. Haversine: symmetry
    #[test]
    fn test_haversine_symmetry() {
        let d1 = haversine_km(40.7128, -74.0060, 51.5074, -0.1278);
        let d2 = haversine_km(51.5074, -0.1278, 40.7128, -74.0060);
        assert!((d1 - d2).abs() < 1e-6, "d1={d1} d2={d2}");
    }

    // 4. latency_from_km formula
    #[test]
    fn test_latency_from_km() {
        let lat = latency_from_km(0.0);
        assert!((lat - 5.0).abs() < 1e-9, "lat={lat}");
        let lat2 = latency_from_km(200.0);
        assert!((lat2 - 1005.0).abs() < 1e-9, "lat2={lat2}");
    }

    // 5. Region::from_country_code
    #[test]
    fn test_region_from_country_code() {
        assert_eq!(Region::from_country_code("US"), Region::NorthAmerica);
        assert_eq!(Region::from_country_code("DE"), Region::Europe);
        assert_eq!(Region::from_country_code("JP"), Region::AsiaPacific);
        assert_eq!(Region::from_country_code("BR"), Region::LatinAmerica);
        assert_eq!(Region::from_country_code("AE"), Region::MiddleEastAfrica);
        assert_eq!(Region::from_country_code("ZZ"), Region::Unknown);
    }

    // 6. GeoLocation::new derives region
    #[test]
    fn test_geo_location_new_derives_region() {
        let loc = GeoLocation::new(51.5074, -0.1278, "GB");
        assert_eq!(loc.region, Region::Europe);
        assert_eq!(loc.country_code, "GB");
    }

    // 7. GeoLocation::with_region overrides region
    #[test]
    fn test_geo_location_with_region() {
        let loc = GeoLocation::with_region(0.0, 0.0, "ZZ", Region::AsiaPacific);
        assert_eq!(loc.region, Region::AsiaPacific);
    }

    // 8. assign_edge returns closest node
    #[test]
    fn test_assign_edge_closest() {
        let mut router = GeoRouter::new();
        router.add_node(EdgeNodeGeo::new("london-pop", london()));
        router.add_node(EdgeNodeGeo::new("tokyo-pop", tokyo()));
        // Client in New York → London should be closer than Tokyo
        let id = router.assign_edge(&new_york()).expect("should find node");
        assert_eq!(id.0, "london-pop");
    }

    // 9. assign_edge returns None when no active nodes
    #[test]
    fn test_assign_edge_no_nodes() {
        let router = GeoRouter::new();
        assert!(router.assign_edge(&new_york()).is_none());
    }

    // 10. assign_edge skips inactive nodes
    #[test]
    fn test_assign_edge_skips_inactive() {
        let mut router = GeoRouter::new();
        let mut near = EdgeNodeGeo::new("near", GeoLocation::new(40.5, -74.0, "US"));
        near.active = false;
        router.add_node(near);
        router.add_node(EdgeNodeGeo::new("far", london()));
        let id = router.assign_edge(&new_york()).expect("fallback");
        assert_eq!(id.0, "far");
    }

    // 11. latency_estimate_ms between two locations
    #[test]
    fn test_latency_estimate_ms() {
        let router = GeoRouter::new();
        let lat = router.latency_estimate_ms(&new_york(), &london());
        // NY-London ≈ 5 570 km → 5570/200*1000 + 5 ≈ 27 855 ms
        assert!(lat > 5.0, "lat={lat}");
        assert!(
            lat > 1000.0,
            "should be over 1s for transatlantic: lat={lat}"
        );
    }

    // 12. nodes_within_km
    #[test]
    fn test_nodes_within_km() {
        let mut router = GeoRouter::new();
        // New York area node
        router.add_node(EdgeNodeGeo::new(
            "ny-pop",
            GeoLocation::new(40.7, -74.0, "US"),
        ));
        // Sydney far away
        router.add_node(EdgeNodeGeo::new("sydney-pop", sydney()));
        let close = router.nodes_within_km(&new_york(), 100.0);
        assert_eq!(close.len(), 1);
        assert_eq!(close[0].id.0, "ny-pop");
    }

    // 13. distance_to_node
    #[test]
    fn test_distance_to_node() {
        let mut router = GeoRouter::new();
        let node_id = EdgeNodeId::new("london-pop");
        router.add_node(EdgeNodeGeo::new("london-pop", london()));
        let dist = router
            .distance_to_node(&new_york(), &node_id)
            .expect("distance");
        assert!(dist > 5_000.0 && dist < 6_000.0, "dist={dist}");
    }

    // 14. remove_node
    #[test]
    fn test_remove_node() {
        let mut router = GeoRouter::new();
        router.add_node(EdgeNodeGeo::new("n1", new_york()));
        let id = EdgeNodeId::new("n1");
        assert!(router.remove_node(&id));
        assert_eq!(router.active_count(), 0);
        assert!(!router.remove_node(&id)); // already gone
    }

    // 15. best_with_latency
    #[test]
    fn test_best_with_latency() {
        let mut router = GeoRouter::new();
        router.add_node(EdgeNodeGeo::new(
            "ny-pop",
            GeoLocation::new(40.7, -74.0, "US"),
        ));
        router.add_node(EdgeNodeGeo::new("sp-pop", sao_paulo()));
        // Client in New York — ny-pop should be closest
        let (id, lat) = router.best_with_latency(&new_york()).expect("best");
        assert_eq!(id.0, "ny-pop");
        // Should be close to base latency (< 50 ms)
        assert!(lat < 100.0, "lat={lat}");
    }

    // 16. Region Display
    #[test]
    fn test_region_display() {
        assert_eq!(Region::NorthAmerica.to_string(), "north_america");
        assert_eq!(Region::Europe.to_string(), "europe");
        assert_eq!(Region::AsiaPacific.to_string(), "asia_pacific");
        assert_eq!(Region::LatinAmerica.to_string(), "latin_america");
        assert_eq!(Region::MiddleEastAfrica.to_string(), "middle_east_africa");
        assert_eq!(Region::Unknown.to_string(), "unknown");
    }

    // 17. Case-insensitive country codes
    #[test]
    fn test_region_case_insensitive() {
        assert_eq!(Region::from_country_code("us"), Region::NorthAmerica);
        assert_eq!(Region::from_country_code("De"), Region::Europe);
    }

    // 18. Tokyo → Sydney closer than Tokyo → London
    #[test]
    fn test_haversine_tokyo_sydney_closer_than_london() {
        let d_sy = haversine_km(35.6762, 139.6503, -33.8688, 151.2093);
        let d_lo = haversine_km(35.6762, 139.6503, 51.5074, -0.1278);
        assert!(
            d_sy < d_lo,
            "Sydney ({d_sy:.0} km) should be closer to Tokyo than London ({d_lo:.0} km)"
        );
    }

    // 19. active_count
    #[test]
    fn test_active_count() {
        let mut router = GeoRouter::new();
        let mut inactive = EdgeNodeGeo::new("off", new_york());
        inactive.active = false;
        router.add_node(inactive);
        router.add_node(EdgeNodeGeo::new("on", london()));
        assert_eq!(router.active_count(), 1);
    }

    // 20. EdgeNodeId Display
    #[test]
    fn test_edge_node_id_display() {
        let id = EdgeNodeId::new("pop-lax-1");
        assert_eq!(id.to_string(), "pop-lax-1");
    }
}
