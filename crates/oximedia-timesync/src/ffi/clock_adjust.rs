//! System clock adjustment via FFI.
//!
//! This module requires unsafe code to interface with system calls.

use crate::error::{TimeSyncError, TimeSyncResult};

/// Get current system time (nanoseconds since Unix epoch).
pub fn get_system_time() -> TimeSyncResult<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| TimeSyncError::ClockAdjust(format!("System time error: {e}")))?;

    Ok(duration.as_secs() * 1_000_000_000 + u64::from(duration.subsec_nanos()))
}

/// Adjust system clock (requires privileges).
///
/// Note: This is a placeholder implementation. On Linux, this would use:
/// - `clock_adjtime()` for fine-grained adjustments
/// - `settimeofday()` for step adjustments
/// - `adjtimex()` for frequency adjustments
///
/// On other platforms:
/// - macOS: `adjtime()`
/// - Windows: `SetSystemTime()`
///
/// For now, this returns success without actually adjusting the clock
/// to avoid requiring elevated privileges during testing.
pub fn adjust_system_clock(offset_ns: i64, _freq_adjust_ppb: f64) -> TimeSyncResult<()> {
    // In a full implementation, this would use FFI to call system functions:

    // On Linux:
    // ```
    // #[cfg(target_os = "linux")]
    // {
    //     use libc::{timex, adjtimex, MOD_OFFSET, MOD_FREQUENCY};
    //
    //     let mut tx: timex = unsafe { std::mem::zeroed() };
    //     tx.modes = MOD_OFFSET | MOD_FREQUENCY;
    //     tx.offset = offset_ns / 1000; // Convert to microseconds
    //     tx.freq = (freq_adjust_ppb * 65.536) as i64; // Convert to kernel units
    //
    //     let result = unsafe { adjtimex(&mut tx) };
    //     if result < 0 {
    //         return Err(TimeSyncError::ClockAdjust("adjtimex failed".to_string()));
    //     }
    // }
    // ```

    // For now, just validate the adjustment is reasonable
    if offset_ns.abs() > 1_000_000_000 {
        return Err(TimeSyncError::ClockAdjust(
            "Offset too large for safe adjustment".to_string(),
        ));
    }

    tracing::warn!(
        "Clock adjustment requested: offset={} ns, but not applied (placeholder implementation)",
        offset_ns
    );

    Ok(())
}

/// Get clock adjustment limits.
pub struct ClockLimits {
    /// Maximum offset adjustment (nanoseconds)
    pub max_offset_ns: i64,
    /// Maximum frequency adjustment (ppb)
    pub max_freq_ppb: f64,
    /// Whether clock adjustment is supported
    pub supported: bool,
}

impl Default for ClockLimits {
    fn default() -> Self {
        Self {
            max_offset_ns: 500_000_000, // 500ms
            max_freq_ppb: 500.0,        // 500 ppb = 0.5 ppm
            supported: cfg!(target_os = "linux") || cfg!(target_os = "macos"),
        }
    }
}

/// Get platform clock adjustment limits.
#[must_use]
pub fn get_clock_limits() -> ClockLimits {
    ClockLimits::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_time() {
        let time = get_system_time().expect("should succeed in test");
        assert!(time > 0);
    }

    #[test]
    fn test_adjust_clock_validation() {
        // Small adjustment should succeed (but not actually adjust)
        let result = adjust_system_clock(1000, 10.0);
        assert!(result.is_ok());

        // Large adjustment should fail
        let result = adjust_system_clock(2_000_000_000, 10.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_clock_limits() {
        let limits = get_clock_limits();
        assert!(limits.max_offset_ns > 0);
        assert!(limits.max_freq_ppb > 0.0);
    }
}
