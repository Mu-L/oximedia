//! NTP stratum hierarchy.

use std::fmt;

/// NTP stratum level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stratum(u8);

impl Stratum {
    /// Primary reference (e.g., GPS, atomic clock)
    pub const PRIMARY_REFERENCE: Self = Self(1);

    /// Secondary reference (synced via NTP) - minimum
    pub const SECONDARY_MIN: Self = Self(2);
    /// Secondary reference (synced via NTP) - maximum
    pub const SECONDARY_MAX: Self = Self(15);

    /// Unsynchronized
    pub const UNSYNCHRONIZED: Self = Self(16);

    /// Reserved
    pub const RESERVED: Self = Self(0);

    /// Create from u8.
    #[must_use]
    pub fn from_u8(value: u8) -> Self {
        Self(value)
    }

    /// Convert to u8.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self.0
    }

    /// Check if this is a primary reference.
    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.0 == 1
    }

    /// Check if this is a secondary reference.
    #[must_use]
    pub fn is_secondary(&self) -> bool {
        self.0 >= 2 && self.0 <= 15
    }

    /// Check if synchronized.
    #[must_use]
    pub fn is_synchronized(&self) -> bool {
        self.0 >= 1 && self.0 <= 15
    }

    /// Check if unsynchronized.
    #[must_use]
    pub fn is_unsynchronized(&self) -> bool {
        self.0 == 0 || self.0 == 16
    }

    /// Get description of this stratum.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self.0 {
            0 => "Reserved/Unspecified",
            1 => "Primary Reference (GPS, Atomic Clock, etc.)",
            2..=15 => "Secondary Reference (NTP)",
            16 => "Unsynchronized",
            _ => "Reserved",
        }
    }
}

impl fmt::Display for Stratum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stratum {} ({})", self.0, self.description())
    }
}

impl From<u8> for Stratum {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<Stratum> for u8 {
    fn from(stratum: Stratum) -> Self {
        stratum.to_u8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stratum_creation() {
        let s1 = Stratum::from_u8(1);
        assert!(s1.is_primary());
        assert!(s1.is_synchronized());

        let s2 = Stratum::from_u8(2);
        assert!(s2.is_secondary());
        assert!(s2.is_synchronized());

        let s16 = Stratum::from_u8(16);
        assert!(s16.is_unsynchronized());
        assert!(!s16.is_synchronized());
    }

    #[test]
    fn test_stratum_ordering() {
        let s1 = Stratum::from_u8(1);
        let s2 = Stratum::from_u8(2);
        let s3 = Stratum::from_u8(3);

        assert!(s1 < s2);
        assert!(s2 < s3);
        assert!(s1 < s3);
    }

    #[test]
    fn test_stratum_display() {
        let s1 = Stratum::PRIMARY_REFERENCE;
        let display = format!("{}", s1);
        assert!(display.contains("Primary Reference"));
    }
}
