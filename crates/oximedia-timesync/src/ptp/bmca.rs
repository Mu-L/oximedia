//! Best Master Clock Algorithm (BMCA) implementation.
//!
//! Implements IEEE 1588-2019 BMCA for selecting the best master clock.

use super::dataset::DefaultDataSet;
use super::message::{AnnounceMessage, ClockQuality};
use super::{ClockIdentity, PortIdentity};
use std::cmp::Ordering;

/// Result of BMCA comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmcaResult {
    /// The announce message represents a better master
    BetterMaster,
    /// The announce message represents a worse master
    WorseMaster,
    /// The announce message is from the same master
    SameMaster,
}

/// Compare two announce messages using BMCA.
#[must_use]
pub fn compare_announce(local: &DefaultDataSet, announce: &AnnounceMessage) -> BmcaResult {
    // Compare using dataset comparison algorithm
    let ordering = compare_dataset_quality(
        local.priority1,
        &local.clock_quality,
        local.priority2,
        local.clock_identity,
        announce.grandmaster_priority1,
        &announce.grandmaster_clock_quality,
        announce.grandmaster_priority2,
        announce.grandmaster_identity,
    );

    match ordering {
        Ordering::Less => BmcaResult::WorseMaster,
        Ordering::Greater => BmcaResult::BetterMaster,
        Ordering::Equal => BmcaResult::SameMaster,
    }
}

/// Compare two clocks using the dataset comparison algorithm.
///
/// Returns `Ordering::Greater` if clock A is better than clock B.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn compare_dataset_quality(
    priority1_a: u8,
    quality_a: &ClockQuality,
    priority2_a: u8,
    identity_a: ClockIdentity,
    priority1_b: u8,
    quality_b: &ClockQuality,
    priority2_b: u8,
    identity_b: ClockIdentity,
) -> Ordering {
    // Step 1: Compare priority1 (lower is better)
    match priority1_a.cmp(&priority1_b) {
        Ordering::Less => return Ordering::Greater,
        Ordering::Greater => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Step 2: Compare clock class (lower is better)
    match quality_a.clock_class.cmp(&quality_b.clock_class) {
        Ordering::Less => return Ordering::Greater,
        Ordering::Greater => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Step 3: Compare clock accuracy (lower is better)
    match quality_a.clock_accuracy.cmp(&quality_b.clock_accuracy) {
        Ordering::Less => return Ordering::Greater,
        Ordering::Greater => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Step 4: Compare offset scaled log variance (lower is better)
    match quality_a
        .offset_scaled_log_variance
        .cmp(&quality_b.offset_scaled_log_variance)
    {
        Ordering::Less => return Ordering::Greater,
        Ordering::Greater => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Step 5: Compare priority2 (lower is better)
    match priority2_a.cmp(&priority2_b) {
        Ordering::Less => return Ordering::Greater,
        Ordering::Greater => return Ordering::Less,
        Ordering::Equal => {}
    }

    // Step 6: Compare clock identity (lower is better)
    match identity_a.cmp(&identity_b) {
        Ordering::Less => Ordering::Greater,
        Ordering::Greater => Ordering::Less,
        Ordering::Equal => Ordering::Equal,
    }
}

/// PTP port state based on BMCA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Initializing
    Initializing,
    /// Faulty
    Faulty,
    /// Disabled
    Disabled,
    /// Listening
    Listening,
    /// Pre-Master
    PreMaster,
    /// Master
    Master,
    /// Passive
    Passive,
    /// Uncalibrated
    Uncalibrated,
    /// Slave
    Slave,
}

/// Recommended state for a port based on BMCA result.
pub struct StateRecommendation {
    /// Recommended port state
    pub state: PortState,
    /// Best master port identity (if slave)
    pub best_master: Option<PortIdentity>,
}

/// Determine recommended state based on BMCA result.
#[must_use]
pub fn recommend_state(
    local: &DefaultDataSet,
    announce: Option<&AnnounceMessage>,
    current_state: PortState,
) -> StateRecommendation {
    match announce {
        None => {
            // No announce messages received, we should be master
            StateRecommendation {
                state: if current_state == PortState::Initializing {
                    PortState::Listening
                } else {
                    PortState::Master
                },
                best_master: None,
            }
        }
        Some(ann) => {
            let result = compare_announce(local, ann);
            match result {
                BmcaResult::BetterMaster => {
                    // External clock is better, become slave
                    StateRecommendation {
                        state: PortState::Slave,
                        best_master: Some(ann.header.source_port_identity),
                    }
                }
                BmcaResult::WorseMaster => {
                    // We are better, become master
                    StateRecommendation {
                        state: PortState::Master,
                        best_master: None,
                    }
                }
                BmcaResult::SameMaster => {
                    // Same master, maintain current state
                    StateRecommendation {
                        state: current_state,
                        best_master: Some(ann.header.source_port_identity),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority1_comparison() {
        let id_a = ClockIdentity([1, 2, 3, 4, 5, 6, 7, 8]);
        let id_b = ClockIdentity([2, 2, 3, 4, 5, 6, 7, 8]);

        let quality = ClockQuality {
            clock_class: 6,
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4000,
        };

        // Lower priority1 is better
        let result = compare_dataset_quality(100, &quality, 128, id_a, 200, &quality, 128, id_b);
        assert_eq!(result, Ordering::Greater);

        let result = compare_dataset_quality(200, &quality, 128, id_a, 100, &quality, 128, id_b);
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_clock_class_comparison() {
        let id_a = ClockIdentity([1, 2, 3, 4, 5, 6, 7, 8]);
        let id_b = ClockIdentity([2, 2, 3, 4, 5, 6, 7, 8]);

        let quality_a = ClockQuality {
            clock_class: 6,
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4000,
        };

        let quality_b = ClockQuality {
            clock_class: 7,
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4000,
        };

        // Same priority, lower clock class is better
        let result =
            compare_dataset_quality(128, &quality_a, 128, id_a, 128, &quality_b, 128, id_b);
        assert_eq!(result, Ordering::Greater);
    }

    #[test]
    fn test_clock_identity_tiebreaker() {
        let id_a = ClockIdentity([1, 2, 3, 4, 5, 6, 7, 8]);
        let id_b = ClockIdentity([2, 2, 3, 4, 5, 6, 7, 8]);

        let quality = ClockQuality {
            clock_class: 6,
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4000,
        };

        // Same everything, lower clock identity wins
        let result = compare_dataset_quality(128, &quality, 128, id_a, 128, &quality, 128, id_b);
        assert_eq!(result, Ordering::Greater);
    }
}
